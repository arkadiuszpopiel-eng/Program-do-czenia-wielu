//! Zapis profilu Dziecko w prywatnym katalogu Sędziego (ścieżka Jądra — agentki nie mają do niego
//! dostępu): włączenie, skrót PIN-u rodzica (Argon2id z solą z CSPRNG systemu, bez samego PIN-u),
//! licznik błędnych prób i blokada. Uszkodzony plik — kłódka zamknięta i bez zmian (fail-closed;
//! reset: administrator komputera usuwa plik).

use std::path::PathBuf;

use argon2::{Algorithm, Argon2, Params, Version};
use safety_broker_contract::{PIN_BLOCK_MS, PIN_MAX_FAILURES, Pin, hex};
use serde::{Deserialize, Serialize};

/// Długość skrótu i soli (bajty).
const HASH_LEN: usize = 32;
const SALT_LEN: usize = 16;
/// Najdłuższa blokada: 15 min × 2^6 = 16 h.
const MAX_BLOCK_SHIFT: u32 = 6;

/// Koszt Argon2id skrótu PIN-u.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinParams {
    /// Pamięć (KiB).
    pub memory_kib: u32,
    /// Liczba przejść.
    pub iterations: u32,
}

impl Default for PinParams {
    /// Zalecenie OWASP dla Argon2id: 19 MiB, 2 przejścia, 1 wątek.
    fn default() -> Self {
        Self {
            memory_kib: 19_456,
            iterations: 2,
        }
    }
}

impl PinParams {
    /// Najtańszy koszt — WYŁĄCZNIE testy (Argon2 w kompilacji debug jest wolny).
    pub const FAST_FOR_TESTS: Self = Self {
        memory_kib: 8,
        iterations: 1,
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PinHash {
    salt: String,
    hash: String,
    #[serde(flatten)]
    params: PinParams,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    #[serde(default)]
    pin: Option<PinHash>,
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) failures: u32,
    #[serde(default)]
    pub(crate) blocked_until_ms: u64,
    #[serde(default)]
    blocks: u32,
}

/// Wynik sprawdzenia PIN-u.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Check {
    /// PIN poprawny.
    Ok,
    /// Błędny PIN (licznik zwiększony; może zacząć blokadę).
    Wrong,
    /// Blokada po serii błędnych prób — PIN nie był sprawdzany.
    Blocked(u64),
    /// Brak PIN-u albo uszkodzony zapis.
    Unavailable,
}

/// Plik profilu Dziecko z bieżącym stanem.
pub(crate) struct ChildStore {
    path: Option<PathBuf>,
    params: PinParams,
    pub(crate) record: Record,
    /// Plik istnieje, ale nie da się go odczytać — kłódka zamknięta, zmiany odrzucane.
    pub(crate) corrupt: bool,
    /// Katalog niedostępny dla konta dziecka (usługa na osobnym koncie).
    pub(crate) tamper_resistant: bool,
}

impl ChildStore {
    /// Wczytuje zapis (`None` — bez pliku, np. Sędzia w procesie testów).
    pub(crate) fn load(path: Option<PathBuf>, params: PinParams) -> Self {
        let (record, corrupt) = match path.as_ref().map(std::fs::read_to_string) {
            None => (Record::default(), false),
            Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => (Record::default(), false),
            Some(Err(_)) => (Record::default(), true),
            Some(Ok(text)) => match serde_json::from_str::<Record>(&text) {
                Ok(r) if r.pin.as_ref().is_none_or(PinHash::well_formed) => (r, false),
                _ => (Record::default(), true),
            },
        };
        Self {
            path,
            params,
            record,
            corrupt,
            tamper_resistant: false,
        }
    }

    /// Czy kłódka obowiązuje (profil włączony albo zapis uszkodzony).
    pub(crate) fn locks(&self) -> bool {
        self.record.enabled || self.corrupt
    }

    pub(crate) fn pin_set(&self) -> bool {
        self.record.pin.is_some()
    }

    /// Sprawdza PIN z limitem prób; błędna próba i blokada są zapisywane od razu (restart
    /// Sędziego nie zeruje licznika).
    pub(crate) fn check(&mut self, pin: &Pin, now: u64) -> std::io::Result<Check> {
        if self.corrupt {
            return Ok(Check::Unavailable);
        }
        if now < self.record.blocked_until_ms {
            return Ok(Check::Blocked(self.record.blocked_until_ms));
        }
        let Some(stored) = self.record.pin.clone() else {
            return Ok(Check::Unavailable);
        };
        let good = pin.is_valid() && stored.matches(pin);
        if good {
            if self.record.failures > 0 || self.record.blocks > 0 {
                self.record.failures = 0;
                self.record.blocks = 0;
                self.save()?;
            }
            return Ok(Check::Ok);
        }
        self.record.failures = self.record.failures.saturating_add(1);
        if self.record.failures >= PIN_MAX_FAILURES {
            self.record.failures = 0;
            let shift = self.record.blocks.min(MAX_BLOCK_SHIFT);
            self.record.blocks = self.record.blocks.saturating_add(1);
            self.record.blocked_until_ms = now.saturating_add(PIN_BLOCK_MS << shift);
        }
        self.save()?;
        Ok(Check::Wrong)
    }

    /// Pozostałe próby przed blokadą.
    pub(crate) fn attempts_left(&self) -> u32 {
        PIN_MAX_FAILURES.saturating_sub(self.record.failures)
    }

    /// Ustawia nowy PIN (skrót z nową solą).
    pub(crate) fn set_pin(&mut self, pin: &Pin) -> std::io::Result<()> {
        let mut salt = [0u8; SALT_LEN];
        getrandom::fill(&mut salt).map_err(|e| std::io::Error::other(e.to_string()))?;
        let hash = derive(pin, &salt, self.params).map_err(std::io::Error::other)?;
        let before = self.record.pin.replace(PinHash {
            salt: hex::encode(&salt),
            hash: hex::encode(&hash),
            params: self.params,
        });
        self.save().inspect_err(|_| self.record.pin = before)
    }

    pub(crate) fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let text = serde_json::to_string(&self.record).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("tmp");
        // Na dysk przed podmianą — przerwa zasilania nie zostawi pustego pliku.
        let mut file = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut file, text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    }
}

impl PinHash {
    fn well_formed(&self) -> bool {
        hex::decode(&self.salt).is_some_and(|s| s.len() == SALT_LEN)
            && hex::decode(&self.hash).is_some_and(|h| h.len() == HASH_LEN)
    }

    fn matches(&self, pin: &Pin) -> bool {
        let (Some(salt), Some(want)) = (hex::decode(&self.salt), hex::decode(&self.hash)) else {
            return false;
        };
        derive(pin, &salt, self.params).is_ok_and(|got| {
            // Porównanie w stałym czasie.
            got.len() == want.len()
                && got.iter().zip(&want).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
        })
    }
}

fn derive(pin: &Pin, salt: &[u8], p: PinParams) -> Result<[u8; HASH_LEN], String> {
    let params =
        Params::new(p.memory_kib, p.iterations, 1, Some(HASH_LEN)).map_err(|e| e.to_string())?;
    let mut out = [0u8; HASH_LEN];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(pin.expose().as_bytes(), salt, &mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(s: &str) -> Pin {
        Pin::new(s).expect("PIN")
    }

    #[test]
    fn hash_only_and_wrong_attempts_block_with_growing_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("child-lock.json");
        let mut s = ChildStore::load(Some(path.clone()), PinParams::FAST_FOR_TESTS);
        assert_eq!(s.check(&pin("1234"), 0).expect("check"), Check::Unavailable);
        s.set_pin(&pin("2468")).expect("PIN");
        let text = std::fs::read_to_string(&path).expect("plik");
        assert!(!text.contains("2468"), "w pliku tylko skrót: {text}");
        assert_eq!(s.check(&pin("2468"), 0).expect("check"), Check::Ok);
        for _ in 0..PIN_MAX_FAILURES {
            assert_eq!(s.check(&pin("1111"), 10).expect("check"), Check::Wrong);
        }
        let first = 10 + PIN_BLOCK_MS;
        assert_eq!(
            s.check(&pin("2468"), 20).expect("check"),
            Check::Blocked(first)
        );
        // Restart Sędziego nie zdejmuje blokady.
        let mut s = ChildStore::load(Some(path), PinParams::FAST_FOR_TESTS);
        assert_eq!(
            s.check(&pin("2468"), 20).expect("check"),
            Check::Blocked(first)
        );
        for _ in 0..PIN_MAX_FAILURES {
            assert_eq!(s.check(&pin("0000"), first).expect("check"), Check::Wrong);
        }
        assert_eq!(
            s.record.blocked_until_ms,
            first + 2 * PIN_BLOCK_MS,
            "dwa razy dłużej"
        );
        assert_eq!(
            s.check(&pin("2468"), first + 2 * PIN_BLOCK_MS)
                .expect("check"),
            Check::Ok
        );
        assert_eq!(s.attempts_left(), PIN_MAX_FAILURES);
    }

    #[test]
    fn damaged_file_keeps_the_lock_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("child-lock.json");
        std::fs::write(&path, "{zepsute").expect("plik");
        let mut s = ChildStore::load(Some(path.clone()), PinParams::FAST_FOR_TESTS);
        assert!(s.corrupt && s.locks());
        assert_eq!(s.check(&pin("1234"), 0).expect("check"), Check::Unavailable);
        std::fs::write(
            &path,
            r#"{"pin":{"salt":"zz","hash":"00","memory_kib":8,"iterations":1}}"#,
        )
        .expect("plik");
        assert!(ChildStore::load(Some(path), PinParams::FAST_FOR_TESTS).corrupt);
    }
}
