//! Trwały zapis metadanych kont w pliku JSON (bez sekretów; zapis atomowy tmp + rename).

use std::path::{Path, PathBuf};

use accounts_hub_contract::{Account, AccountsError, AccountsRepository};
use serde::{Deserialize, Serialize};

/// Wersja formatu pliku metadanych.
pub const ACCOUNTS_FILE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct AccountsFile {
    version: u32,
    accounts: Vec<Account>,
}

/// Plik `accounts.json` (np. w `%APPDATA%\Alfa\config`). Klucze są wyłącznie w Credential Manager.
#[derive(Debug, Clone)]
pub struct JsonFileRepository {
    path: PathBuf,
}

impl JsonFileRepository {
    /// Repozytorium w podanym pliku.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Ścieżka pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn persist_err(e: impl std::fmt::Display) -> AccountsError {
    AccountsError::Persist(e.to_string())
}

impl AccountsRepository for JsonFileRepository {
    fn load(&self) -> Result<Vec<Account>, AccountsError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(persist_err(e)),
        };
        let file: AccountsFile = serde_json::from_str(&text).map_err(persist_err)?;
        if file.version != ACCOUNTS_FILE_VERSION {
            return Err(persist_err(format!(
                "nieobsługiwana wersja pliku kont {}",
                file.version
            )));
        }
        Ok(file.accounts)
    }

    fn save(&self, accounts: &[Account]) -> Result<(), AccountsError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(persist_err)?;
        }
        let file = AccountsFile {
            version: ACCOUNTS_FILE_VERSION,
            accounts: accounts.to_vec(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(persist_err)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(persist_err)?;
        std::fs::rename(&tmp, &self.path).map_err(persist_err)
    }
}
