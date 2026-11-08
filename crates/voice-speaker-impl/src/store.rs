//! Profil zaszyfrowany lokalnie: plik `ALFASPK1 ‖ nonce 24 B ‖ XChaCha20-Poly1305(JSON)` (AAD =
//! nagłówek), klucz 32 B z sejfu kluczy (`sessions-contract::KeyVault` — w aplikacji Windows
//! Credential Manager), zapis atomowy (plik tymczasowy + zmiana nazwy). Usunięcie kasuje plik
//! **i klucz** (crypto-shredding: kopia pliku z kopii zapasowej jest bezużyteczna). Bufory
//! z jawnym embeddingiem są zerowane. Profil nie leży w katalogu sesji ani w eksporcie `.alfa`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sessions_contract::{KeyVault, load_or_create_key};
use voice_speaker_contract::{Embedding, Profile, ProfileStore, SpeakerError};
use zeroize::{Zeroize, Zeroizing};

/// Nagłówek pliku (wersja formatu).
pub const MAGIC: &[u8; 8] = b"ALFASPK1";
/// Nazwa klucza w sejfie.
pub const KEY_NAME: &str = "alfa/voice/speaker";
const NONCE: usize = 24;

#[derive(Serialize, Deserialize)]
struct Plain {
    model: String,
    utterances: u32,
    embedding: Vec<f32>,
}

impl Drop for Plain {
    fn drop(&mut self) {
        self.embedding.zeroize();
    }
}

/// Magazyn profilu w zaszyfrowanym pliku.
pub struct EncryptedFileStore {
    path: PathBuf,
    vault: Arc<dyn KeyVault>,
}

impl std::fmt::Debug for EncryptedFileStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedFileStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn storage(e: impl std::fmt::Display) -> SpeakerError {
    SpeakerError::Storage(e.to_string())
}

fn crypto(e: impl std::fmt::Display) -> SpeakerError {
    SpeakerError::Crypto(e.to_string())
}

impl EncryptedFileStore {
    /// Magazyn w pliku `path` (np. `%LOCALAPPDATA%\Alfa\voice\speaker.bin`).
    pub fn new(path: impl Into<PathBuf>, vault: Arc<dyn KeyVault>) -> Self {
        Self {
            path: path.into(),
            vault,
        }
    }

    /// Ścieżka pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn cipher(&self, create: bool) -> Result<Option<XChaCha20Poly1305>, SpeakerError> {
        let key = if create {
            Some(load_or_create_key(self.vault.as_ref(), KEY_NAME).map_err(crypto)?)
        } else {
            self.vault.load(KEY_NAME).map_err(crypto)?
        };
        Ok(key.map(|k| XChaCha20Poly1305::new(k.as_bytes().into())))
    }
}

impl ProfileStore for EncryptedFileStore {
    fn load(&self) -> Result<Option<Profile>, SpeakerError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(storage(format!("{}: {e}", self.path.display()))),
        };
        let Some(cipher) = self.cipher(false)? else {
            // Plik bez klucza (po crypto-shreddingu) — profil nie istnieje.
            return Ok(None);
        };
        if bytes.len() < MAGIC.len() + NONCE || &bytes[..MAGIC.len()] != MAGIC {
            return Err(crypto("nieznany format pliku profilu"));
        }
        let (head, body) = bytes.split_at(MAGIC.len() + NONCE);
        let nonce = XNonce::from_slice(&head[MAGIC.len()..]);
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    nonce,
                    Payload {
                        msg: body,
                        aad: MAGIC,
                    },
                )
                .map_err(|_| crypto("profil uszkodzony albo zły klucz (uwierzytelnienie)"))?,
        );
        let p: Plain = serde_json::from_slice(&plain).map_err(crypto)?;
        Ok(Some(Profile {
            model: p.model.clone(),
            utterances: p.utterances,
            embedding: Embedding::new(p.embedding.clone())?,
        }))
    }

    fn save(&self, profile: &Profile) -> Result<(), SpeakerError> {
        let cipher = self.cipher(true)?.ok_or_else(|| crypto("brak klucza"))?;
        let plain = Plain {
            model: profile.model.clone(),
            utterances: profile.utterances,
            embedding: profile.embedding.as_slice().to_vec(),
        };
        let json = Zeroizing::new(serde_json::to_vec(&plain).map_err(crypto)?);
        let mut nonce = [0u8; NONCE];
        getrandom::fill(&mut nonce).map_err(crypto)?;
        let sealed = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &json,
                    aad: MAGIC,
                },
            )
            .map_err(|_| crypto("szyfrowanie nieudane"))?;
        let mut out = Vec::with_capacity(MAGIC.len() + NONCE + sealed.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(storage)?;
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &out).map_err(storage)?;
        std::fs::rename(&tmp, &self.path).map_err(storage)
    }

    fn delete(&self) -> Result<bool, SpeakerError> {
        let existed = match std::fs::remove_file(&self.path) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(storage(e)),
        };
        let key = self.vault.delete(KEY_NAME).map_err(crypto)?;
        Ok(existed || key)
    }
}
