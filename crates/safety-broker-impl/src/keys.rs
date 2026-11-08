//! Klucze Brokera: HMAC-SHA256 tokenów, rotacja z oknem łaski, czyszczenie przy kill-switchu,
//! nonce wyzwań. Klucze żyją wyłącznie w pamięci procesu Brokera (nowe przy każdym starcie →
//! token nie przechodzi między uruchomieniami).

use hmac::{Hmac, Mac};
use safety_broker_contract::{BootId, MAC_LEN, Nonce};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Źródło losowości kluczy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// CSPRNG systemu (`getrandom`) — produkcja.
    Random,
    /// Deterministyczne z ziarna — WYŁĄCZNIE atrapa i testy (bez sekretu!).
    Deterministic(u64),
}

#[derive(Clone)]
struct OldKey {
    epoch: u32,
    key: [u8; 32],
    valid_until_ms: u64,
}

/// Pierścień kluczy.
pub(crate) struct KeyRing {
    mode: KeyMode,
    counter: u64,
    boot: BootId,
    epoch: u32,
    current: [u8; 32],
    previous: Option<OldKey>,
}

impl KeyRing {
    pub(crate) fn new(mode: KeyMode) -> Result<Self, String> {
        let mut ring = Self {
            mode,
            counter: 0,
            boot: BootId::default(),
            epoch: 1,
            current: [0; 32],
            previous: None,
        };
        ring.boot = BootId(ring.fresh::<16>()?);
        ring.current = ring.fresh::<32>()?;
        Ok(ring)
    }

    fn fresh<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut out = [0u8; N];
        match self.mode {
            KeyMode::Random => getrandom::fill(&mut out).map_err(|e| format!("CSPRNG: {e}"))?,
            KeyMode::Deterministic(seed) => {
                self.counter += 1;
                let digest = Sha256::new()
                    .chain_update(b"alfa-broker-deterministic")
                    .chain_update(seed.to_be_bytes())
                    .chain_update(self.counter.to_be_bytes())
                    .finalize();
                for (o, d) in out.iter_mut().zip(digest.iter().cycle()) {
                    *o = *d;
                }
            }
        }
        Ok(out)
    }

    pub(crate) fn boot(&self) -> BootId {
        self.boot
    }

    pub(crate) fn epoch(&self) -> u32 {
        self.epoch
    }

    fn key_for(&self, epoch: u32, now_ms: u64) -> Option<&[u8; 32]> {
        if epoch == self.epoch {
            return Some(&self.current);
        }
        self.previous
            .as_ref()
            .filter(|old| old.epoch == epoch && now_ms < old.valid_until_ms)
            .map(|old| &old.key)
    }

    /// MAC ciała kluczem bieżącej epoki.
    pub(crate) fn sign(&self, body: &[u8]) -> [u8; MAC_LEN] {
        mac(&self.current, body)
    }

    /// Weryfikacja w czasie stałym (`verify_slice`).
    pub(crate) fn verify(&self, epoch: u32, body: &[u8], tag: &[u8], now_ms: u64) -> bool {
        let Some(key) = self.key_for(epoch, now_ms) else {
            return false;
        };
        let Ok(mut m) = <HmacSha256 as Mac>::new_from_slice(key) else {
            return false;
        };
        m.update(body);
        m.verify_slice(tag).is_ok()
    }

    /// Rotacja: nowy klucz; stary weryfikuje do `grace_until_ms` (maksymalny TTL tokenu).
    pub(crate) fn rotate(&mut self, grace_until_ms: u64) -> Result<(), String> {
        let next = self.fresh::<32>()?;
        self.previous = Some(OldKey {
            epoch: self.epoch,
            key: self.current,
            valid_until_ms: grace_until_ms,
        });
        self.current = next;
        self.epoch = self.epoch.wrapping_add(1);
        Ok(())
    }

    /// Kill-switch: nowy klucz bez okna łaski — wszystkie wydane tokeny tracą ważność.
    pub(crate) fn wipe(&mut self) -> Result<(), String> {
        let next = self.fresh::<32>()?;
        if let Some(old) = self.previous.as_mut() {
            old.key = [0; 32];
        }
        self.previous = None;
        self.current = next;
        self.epoch = self.epoch.wrapping_add(1);
        Ok(())
    }

    /// Jednorazowy nonce wyzwania.
    pub(crate) fn nonce(&mut self) -> Result<Nonce, String> {
        self.fresh::<16>().map(Nonce)
    }
}

/// HMAC-SHA256.
pub(crate) fn mac(key: &[u8; 32], body: &[u8]) -> [u8; MAC_LEN] {
    match <HmacSha256 as Mac>::new_from_slice(key) {
        Ok(mut m) => {
            m.update(body);
            m.finalize().into_bytes().into()
        }
        // HMAC przyjmuje klucz dowolnej długości — gałąź nieosiągalna; zwracamy MAC zerowy,
        // który nigdy nie przejdzie `verify_slice` z innym kluczem.
        Err(_) => [0; MAC_LEN],
    }
}

/// Porównanie w czasie stałym (nonce wyzwań).
pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4231_case_2() {
        // RFC 4231, przypadek 2 — klucz "Jefe" dopełniony zerami do 32 B jest RÓŻNY od "Jefe",
        // więc sprawdzamy wektor bezpośrednio na `Hmac`.
        let mut m = <HmacSha256 as Mac>::new_from_slice(b"Jefe").unwrap();
        m.update(b"what do ya want for nothing?");
        let out: [u8; 32] = m.finalize().into_bytes().into();
        assert_eq!(
            safety_broker_contract::hex::encode(&out),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn rotation_grace_and_wipe() {
        let mut k = KeyRing::new(KeyMode::Deterministic(7)).unwrap();
        let body = b"cialo";
        let e1 = k.epoch();
        let t1 = k.sign(body);
        assert!(k.verify(e1, body, &t1, 0));
        assert!(!k.verify(e1, b"inne", &t1, 0));
        k.rotate(100).unwrap();
        assert!(k.verify(e1, body, &t1, 99));
        assert!(!k.verify(e1, body, &t1, 100));
        let e2 = k.epoch();
        let t2 = k.sign(body);
        assert!(k.verify(e2, body, &t2, 0));
        k.wipe().unwrap();
        assert!(!k.verify(e2, body, &t2, 0));
        assert!(!k.verify(e1, body, &t1, 0));
        assert!(ct_eq(b"ab", b"ab") && !ct_eq(b"ab", b"ac") && !ct_eq(b"a", b"ab"));
    }

    #[test]
    fn random_keys_differ_between_instances() {
        let a = KeyRing::new(KeyMode::Random).unwrap();
        let b = KeyRing::new(KeyMode::Random).unwrap();
        assert_ne!(a.boot(), b.boot());
        assert_ne!(a.sign(b"x"), b.sign(b"x"));
        let mut c = KeyRing::new(KeyMode::Random).unwrap();
        assert_ne!(c.nonce().unwrap(), c.nonce().unwrap());
    }
}
