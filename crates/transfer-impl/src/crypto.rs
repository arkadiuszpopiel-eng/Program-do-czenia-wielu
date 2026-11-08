//! Szyfrowanie całej paczki (docs/formats/alfa-package.md §5): nagłówek `ALFAENC1` + parametry
//! KDF + szyfrogram XChaCha20-Poly1305 w konstrukcji STREAM (fragmenty 64 KiB; nonce =
//! prefiks 19 B ‖ licznik u32 BE ‖ flaga ostatniego fragmentu — jak `aead::stream::StreamBE32`).
//! Nagłówek jest danymi uwierzytelnianymi (AAD) każdego fragmentu; obcięcie pliku wykrywa flaga
//! ostatniego fragmentu. Odczyt jest **losowy** ([`DecryptingReader`] implementuje `Seek`), więc
//! archiwum ZIP czyta się wprost z szyfrogramu — bez jawnej kopii na dysku.
//!
//! Klucz: Argon2id z hasła (sól 16 B) albo klucz maszyny z Credential Managera (snapshoty).

use std::io::{self, Read, Seek, SeekFrom, Write};

use accounts_hub_contract::SecretString;
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use transfer_contract::{EncryptionInfo, PackageKind, TransferError, hex};
use zeroize::Zeroizing;

/// Znacznik pliku zaszyfrowanego.
pub const MAGIC: &[u8; 8] = b"ALFAENC1";
/// Schemat w nagłówku i manifeście.
pub const SCHEME: &str = "xchacha20poly1305-stream-be32";
/// Rozmiar znacznika uwierzytelnienia.
const TAG: usize = 16;
/// Rozmiar fragmentu jawnego.
pub const CHUNK: usize = 64 * 1024;
const PREFIX: usize = 19;
const MAX_HEADER: u32 = 64 * 1024;

/// Parametry Argon2id. Domyślne mieszczą się w budżecie RAM modułu (≤ 50 MB): 46 MiB, 2 przebiegi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Pamięć (KiB).
    pub m_kib: u32,
    /// Liczba przebiegów.
    pub t: u32,
    /// Równoległość.
    pub p: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            m_kib: 46 * 1024,
            t: 2,
            p: 1,
        }
    }
}

impl KdfParams {
    /// Górne granice przy **odczycie** (obca paczka nie może wymusić dowolnej pamięci).
    fn within_caps(&self) -> bool {
        (8..=256 * 1024).contains(&self.m_kib)
            && (1..=16).contains(&self.t)
            && (1..=8).contains(&self.p)
    }
}

/// Wyprowadzenie klucza zapisane w nagłówku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "alg", rename_all = "kebab-case")]
pub enum Kdf {
    /// Klucz z hasła.
    Argon2id {
        /// Parametry.
        #[serde(flatten)]
        params: KdfParams,
        /// Sól (hex).
        salt: String,
    },
    /// Klucz maszyny z magazynu sekretów (nazwa sekretu).
    MachineKey {
        /// Nazwa sekretu.
        key: String,
    },
}

/// Nagłówek JSON po znaczniku `ALFAENC1` (poza szyfrogramem: tylko rodzaj i wersja schematu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncHeader {
    /// Wersja formatu nagłówka.
    pub format: u32,
    /// Rodzaj paczki.
    pub kind: PackageKind,
    /// Wersja schematu paczki.
    pub schema_version: String,
    /// Schemat szyfrowania.
    pub scheme: String,
    /// Rozmiar fragmentu jawnego.
    pub chunk: u32,
    /// Wyprowadzenie klucza.
    pub kdf: Kdf,
    /// Prefiks nonce (hex, 19 B).
    pub nonce_prefix: String,
}

fn random<const N: usize>() -> Result<[u8; N], TransferError> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| TransferError::io(format!("losowość: {e}")))?;
    Ok(buf)
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            text.get(i..i + 2)
                .and_then(|b| u8::from_str_radix(b, 16).ok())
        })
        .collect()
}

/// Klucz 32 B z hasła (Argon2id).
pub fn derive_key(
    password: &SecretString,
    salt: &[u8],
    params: KdfParams,
) -> Result<Zeroizing<[u8; 32]>, TransferError> {
    let params = Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|e| TransferError::invalid("kdf", e))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.expose_secret().as_bytes(), salt, key.as_mut())
        .map_err(|e| TransferError::invalid("kdf", e))?;
    Ok(key)
}

/// Klucz maszyny zapisany jako hex.
pub fn parse_machine_key(hex_key: &SecretString) -> Option<Zeroizing<[u8; 32]>> {
    let bytes = Zeroizing::new(unhex(hex_key.expose_secret())?);
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(Zeroizing::new(arr))
}

/// Nowy losowy klucz maszyny (hex do magazynu sekretów).
pub fn new_machine_key() -> Result<SecretString, TransferError> {
    let key = Zeroizing::new(random::<32>()?);
    Ok(SecretString::new(hex(key.as_ref())))
}

fn nonce(prefix: &[u8; PREFIX], index: u64, last: bool) -> Result<XNonce, TransferError> {
    let counter = u32::try_from(index).map_err(|_| {
        transfer_contract::limit("fragmenty szyfrogramu", index, u64::from(u32::MAX))
    })?;
    let mut n = [0u8; 24];
    n[..PREFIX].copy_from_slice(prefix);
    n[PREFIX..PREFIX + 4].copy_from_slice(&counter.to_be_bytes());
    n[23] = u8::from(last);
    Ok(XNonce::from(n))
}

/// Szyfrator paczki (parametry znane przed zapisem — trafiają też do manifestu).
pub struct Sealer {
    aad: Vec<u8>,
    cipher: XChaCha20Poly1305,
    prefix: [u8; PREFIX],
    info: EncryptionInfo,
}

impl Sealer {
    fn build(
        kind: PackageKind,
        kdf: Kdf,
        key: &[u8; 32],
        salt: Option<String>,
    ) -> Result<Self, TransferError> {
        let prefix = random::<PREFIX>()?;
        let header = EncHeader {
            format: 1,
            kind,
            schema_version: transfer_contract::SCHEMA_VERSION.to_owned(),
            scheme: SCHEME.to_owned(),
            chunk: CHUNK as u32,
            kdf: kdf.clone(),
            nonce_prefix: hex(&prefix),
        };
        let json =
            serde_json::to_vec(&header).map_err(|e| TransferError::invalid("nagłówek", e))?;
        let len = u32::try_from(json.len())
            .map_err(|_| TransferError::invalid("nagłówek", "za długi"))?;
        let mut aad = MAGIC.to_vec();
        aad.extend_from_slice(&len.to_le_bytes());
        aad.extend_from_slice(&json);
        let kdf_name = match kdf {
            Kdf::Argon2id { .. } => "argon2id",
            Kdf::MachineKey { .. } => "machine-key",
        };
        Ok(Self {
            aad,
            cipher: XChaCha20Poly1305::new(key.into()),
            prefix,
            info: EncryptionInfo {
                scheme: SCHEME.to_owned(),
                kdf: kdf_name.to_owned(),
                salt,
                nonce: hex(&prefix),
            },
        })
    }

    /// Szyfrowanie hasłem (Argon2id, nowa sól).
    pub fn with_password(
        kind: PackageKind,
        password: &SecretString,
        params: KdfParams,
    ) -> Result<Self, TransferError> {
        let salt = random::<16>()?;
        let key = derive_key(password, &salt, params)?;
        Self::build(
            kind,
            Kdf::Argon2id {
                params,
                salt: hex(&salt),
            },
            &key,
            Some(hex(&salt)),
        )
    }

    /// Szyfrowanie kluczem maszyny (snapshoty).
    pub fn with_machine_key(
        kind: PackageKind,
        name: &str,
        key: &[u8; 32],
    ) -> Result<Self, TransferError> {
        Self::build(
            kind,
            Kdf::MachineKey {
                key: name.to_owned(),
            },
            key,
            None,
        )
    }

    /// Parametry do manifestu.
    pub fn info(&self) -> &EncryptionInfo {
        &self.info
    }

    /// Szyfruje strumień `plain` do `out` (nagłówek + fragmenty); zwraca liczbę bajtów wyjścia.
    pub fn seal(&self, plain: &mut dyn Read, out: &mut dyn Write) -> Result<u64, TransferError> {
        out.write_all(&self.aad)?;
        let mut written = self.aad.len() as u64;
        let mut current = Zeroizing::new(vec![0u8; CHUNK]);
        let mut next = Zeroizing::new(vec![0u8; CHUNK]);
        let mut n = read_full(plain, &mut current)?;
        let mut index = 0u64;
        loop {
            let m = if n == CHUNK {
                read_full(plain, &mut next)?
            } else {
                0
            };
            let last = m == 0;
            let payload = Payload {
                msg: &current[..n],
                aad: &self.aad,
            };
            let sealed = self
                .cipher
                .encrypt(&nonce(&self.prefix, index, last)?, payload)
                .map_err(|_| TransferError::io("szyfrowanie nie powiodło się"))?;
            out.write_all(&sealed)?;
            written += sealed.len() as u64;
            if last {
                return Ok(written);
            }
            std::mem::swap(&mut current, &mut next);
            n = m;
            index += 1;
        }
    }
}

fn read_full(r: &mut dyn Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(k) => filled += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Czy plik zaczyna się znacznikiem `ALFAENC1` (odczyt nagłówka pozostawia pozycję na początku).
pub fn read_header<R: Read + Seek>(
    r: &mut R,
) -> Result<Option<(EncHeader, Vec<u8>)>, TransferError> {
    r.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 8];
    if read_full(r, &mut magic)? < 8 || &magic != MAGIC {
        r.seek(SeekFrom::Start(0))?;
        return Ok(None);
    }
    let mut len = [0u8; 4];
    if read_full(r, &mut len)? < 4 {
        return Err(TransferError::corrupt("obcięty nagłówek szyfrowania"));
    }
    let len = u32::from_le_bytes(len);
    if len == 0 || len > MAX_HEADER {
        return Err(TransferError::corrupt("zły rozmiar nagłówka szyfrowania"));
    }
    let mut json = vec![0u8; len as usize];
    if read_full(r, &mut json)? < json.len() {
        return Err(TransferError::corrupt("obcięty nagłówek szyfrowania"));
    }
    let header: EncHeader = serde_json::from_slice(&json)
        .map_err(|e| TransferError::corrupt(format!("nagłówek szyfrowania: {e}")))?;
    let mut aad = magic.to_vec();
    aad.extend_from_slice(&len.to_le_bytes());
    aad.extend_from_slice(&json);
    Ok(Some((header, aad)))
}

/// Klucz dla nagłówka: z hasła (Argon2id w granicach) albo z dostawcy klucza maszyny.
pub fn key_for(
    header: &EncHeader,
    password: Option<&SecretString>,
    machine: &dyn Fn(&str) -> Option<Zeroizing<[u8; 32]>>,
) -> Result<Zeroizing<[u8; 32]>, TransferError> {
    if header.format != 1 || header.scheme != SCHEME || header.chunk as usize != CHUNK {
        return Err(TransferError::NewerSchema {
            found: format!("{} (format {})", header.scheme, header.format),
            supported: SCHEME.to_owned(),
        });
    }
    match &header.kdf {
        Kdf::Argon2id { params, salt } => {
            let salt = unhex(salt)
                .filter(|s| (16..=64).contains(&s.len()))
                .ok_or_else(|| TransferError::corrupt("zła sól"))?;
            if !params.within_caps() {
                return Err(transfer_contract::limit(
                    "parametry Argon2id",
                    u64::from(params.m_kib),
                    256 * 1024,
                ));
            }
            let password = password.ok_or(TransferError::PasswordRequired)?;
            derive_key(password, &salt, *params)
        }
        Kdf::MachineKey { key } => machine(key).ok_or_else(|| TransferError::NotFound {
            what: format!("klucz maszyny `{key}` (snapshot z innej maszyny?)"),
        }),
    }
}

#[path = "crypto_reader.rs"]
mod reader;
pub use reader::DecryptingReader;
