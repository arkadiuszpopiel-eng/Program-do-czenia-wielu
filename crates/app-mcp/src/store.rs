//! Trwały zapis serwerów MCP (`mcp-servers.json`) i zgód na narzędzia (`mcp-pins.json`) w katalogu
//! konfiguracji. W plikach nie ma sekretów: wartości zmiennych tajnych leżą w Menedżerze
//! poświadczeń Windows (`mcp/<serwer>/<ZMIENNA>`), plik zna tylko ich nazwy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use mcp_contract::client::DEFAULT_REQUEST_TIMEOUT_MS;
use mcp_contract::{PinStore, ToolFingerprint, ToolPin, TrustLevel};
use serde::{Deserialize, Serialize};

/// Plik serwerów w katalogu konfiguracji.
pub const SERVERS_FILE: &str = "mcp-servers.json";
/// Plik zgód na narzędzia.
pub const PINS_FILE: &str = "mcp-pins.json";
/// Najwięcej serwerów (każdy to osobny proces).
pub const MAX_SERVERS: usize = 32;

/// Wpis serwera w pliku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerEntry {
    /// Identyfikator (kebab-case).
    pub id: String,
    /// Nazwa do pokazania.
    pub name: String,
    /// Polecenie tak, jak wpisał je użytkownik.
    pub command: String,
    /// Argumenty.
    #[serde(default)]
    pub args: Vec<String>,
    /// Katalog roboczy.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Zmienne jawne.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Nazwy zmiennych tajnych (wartości w magazynie sekretów).
    #[serde(default)]
    pub secret_env: Vec<String>,
    /// Poziom zaufania.
    #[serde(default)]
    pub trust: TrustLevel,
    /// Włączony.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Limit czasu żądania (ms).
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn yes() -> bool {
    true
}

fn default_timeout() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_MS
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ServersDoc {
    #[serde(default)]
    servers: Vec<ServerEntry>,
}

/// Zapis JSON przez plik tymczasowy i zamianę nazwy.
fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Odczyt listy serwerów; brak pliku — pusta lista, zły format — pusta lista z wpisem w dzienniku
/// (plik zostaje nietknięty do pierwszego zapisu).
pub fn load_servers(path: &Path) -> Vec<ServerEntry> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<ServersDoc>(&text) {
        Ok(doc) => doc.servers,
        Err(e) => {
            tracing::warn!(plik = %path.display(), "lista serwerów MCP nieczytelna: {e}");
            Vec::new()
        }
    }
}

/// Zapis listy serwerów.
pub fn save_servers(path: &Path, servers: &[ServerEntry]) -> std::io::Result<()> {
    write_json(
        path,
        &ServersDoc {
            servers: servers.to_vec(),
        },
    )
}

type PinMap = BTreeMap<(String, String), ToolFingerprint>;

/// Zgody na narzędzia zapisywane od razu do pliku (`PinStore` dla klienta MCP).
pub struct FilePinStore {
    path: PathBuf,
    pins: Mutex<PinMap>,
}

impl FilePinStore {
    /// Magazyn wczytany z pliku (brak albo zły format — pusty; zgody trzeba wtedy udzielić na nowo).
    pub fn open(path: PathBuf) -> Self {
        let pins = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Vec<ToolPin>>(&t).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|p| ((p.server, p.tool), p.fingerprint))
            .collect();
        Self {
            path,
            pins: Mutex::new(pins),
        }
    }

    fn lock(&self) -> MutexGuard<'_, PinMap> {
        self.pins.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn persist(&self, pins: &PinMap) {
        let list: Vec<ToolPin> = pins
            .iter()
            .map(|((server, tool), fp)| ToolPin {
                server: server.clone(),
                tool: tool.clone(),
                fingerprint: fp.clone(),
            })
            .collect();
        if let Err(e) = write_json(&self.path, &list) {
            tracing::warn!(plik = %self.path.display(), "nie zapisano zgód MCP: {e}");
        }
    }

    /// Cofa zgodę na narzędzie; `true`, gdy była.
    pub fn revoke(&self, server: &str, tool: &str) -> bool {
        let mut pins = self.lock();
        let had = pins.remove(&(server.to_owned(), tool.to_owned())).is_some();
        if had {
            self.persist(&pins);
        }
        had
    }

    /// Usuwa wszystkie zgody serwera (usunięcie serwera albo zmiana polecenia).
    pub fn forget_server(&self, server: &str) {
        let mut pins = self.lock();
        let before = pins.len();
        pins.retain(|(s, _), _| s != server);
        if pins.len() != before {
            self.persist(&pins);
        }
    }
}

impl PinStore for FilePinStore {
    fn get(&self, server: &str, tool: &str) -> Option<ToolFingerprint> {
        self.lock()
            .get(&(server.to_owned(), tool.to_owned()))
            .cloned()
    }

    fn put(&self, pin: ToolPin) {
        let mut pins = self.lock();
        pins.insert((pin.server, pin.tool), pin.fingerprint);
        self.persist(&pins);
    }

    fn all(&self) -> Vec<ToolPin> {
        self.lock()
            .iter()
            .map(|((server, tool), fp)| ToolPin {
                server: server.clone(),
                tool: tool.clone(),
                fingerprint: fp.clone(),
            })
            .collect()
    }
}

/// Czy nazwa zmiennej jest poprawna (`[A-Za-z_][A-Za-z0-9_]*`, ≤ 64 znaki).
pub fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 64
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Nazwa wygląda na tajną (klucz, token, hasło) — wartość nigdy nie trafia do pliku.
pub fn looks_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "KEY",
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "PWD",
        "AUTH",
        "CREDENTIAL",
        "COOKIE",
    ]
    .iter()
    .any(|w| upper.contains(w))
}

/// Identyfikator z nazwy: małe litery i cyfry, reszta jako `-` (kebab-case, ≤ 48 znaków).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        };
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(48).collect();
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "serwer".to_owned()
    } else {
        out
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
