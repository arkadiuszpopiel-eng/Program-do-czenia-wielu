//! Port rejestru Windows **tylko do odczytu** (PLAN §7.2, §8.5 — serwer MCP v1): gałęzie `HKCU`
//! i `HKLM`; deny-lista kluczy z sekretami (poświadczenia, LSA, SAM, hasła aplikacji, dane Alfy)
//! sprawdzana **w porcie** przed otwarciem klucza i dla każdego podklucza na liście; wartości
//! o nazwach sekretów są zawsze redagowane. Ścieżka sprawdzana = ścieżka używana: implementacja
//! otwiera dokładnie segmenty z [`RegKey::parse`].

use std::fmt;

use serde::{Deserialize, Serialize};

/// Najwięcej wpisów (podkluczy + wartości) na liście.
pub const MAX_REG_ENTRIES: usize = 500;
/// Najdłuższy zwracany napis wartości (znaki; dłuższy jest obcinany).
pub const MAX_REG_DATA_CHARS: usize = 4_096;
/// Najwięcej bajtów wartości binarnej pokazywanych szesnastkowo.
pub const MAX_REG_BINARY_PREVIEW: usize = 256;
/// Najgłębsza ścieżka klucza.
const MAX_DEPTH: usize = 64;

/// Segmenty kluczy z sekretami (porównanie bez wielkości liter, w dowolnym miejscu ścieżki —
/// obejmuje też `Wow6432Node` i `ControlSet00x`).
const SECRET_SEGMENTS: [&str; 34] = [
    "credentials",
    "protected storage",
    "protected storage system provider",
    "sam",
    "security",
    "secrets",
    "lsa",
    "intelliforms",
    "storage2",
    "winlogon",
    "identitycrl",
    "identity",
    "identities",
    "cryptography",
    "protect",
    "systemcertificates",
    "vault",
    "openssh",
    "simontatham",
    "9bis.com",
    "martin prikryl",
    "filezilla",
    "teamviewer",
    "realvnc",
    "tightvnc",
    "winvnc3",
    "winvnc4",
    "orl",
    "windows messaging subsystem",
    "outlook",
    "terminal server client",
    "authentication",
    "alfa",
    "alfa-broker",
];

/// Fragmenty nazw (segmentów i wartości) zawsze traktowane jak sekret.
const SECRET_FRAGMENTS: [&str; 13] = [
    "password",
    "passwd",
    "pwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "api-key",
    "credential",
    "privatekey",
    "private_key",
    "passphrase",
    "cookie",
];

/// Błąd portu rejestru.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// Klucz na deny-liście sekretów (odmowa przed jakimkolwiek odczytem).
    #[error("klucz rejestru z sekretami — odczyt zablokowany: {0}")]
    Denied(String),
    /// Niepoprawna ścieżka klucza albo nazwa wartości.
    #[error("niepoprawny klucz rejestru: {0}")]
    InvalidKey(String),
    /// Gałąź inna niż `HKCU`/`HKLM` (np. `HKU` z cudzymi profilami).
    #[error("nieobsługiwana gałąź rejestru: {0} (dozwolone HKCU i HKLM)")]
    UnsupportedHive(String),
    /// Klucz albo wartość nie istnieje.
    #[error("nie znaleziono w rejestrze: {0}")]
    NotFound(String),
    /// Brak uprawnień do klucza.
    #[error("brak uprawnień do klucza: {0}")]
    AccessDenied(String),
    /// Nieobsługiwane na tej platformie.
    #[error("nieobsługiwane: {0}")]
    Unsupported(String),
    /// Inny błąd systemu.
    #[error("błąd rejestru: {0}")]
    Io(String),
}

/// Gałąź rejestru.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegHive {
    /// `HKEY_CURRENT_USER`.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE`.
    LocalMachine,
}

impl RegHive {
    /// Skrót (`HKCU`, `HKLM`).
    pub fn short(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU",
            Self::LocalMachine => "HKLM",
        }
    }
}

/// Znormalizowana ścieżka klucza: gałąź + segmenty (bez pustych, bez `.`/`..`, bez znaków
/// sterujących). Postać tekstowa: `HKCU\Software\X`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegKey {
    hive: RegHive,
    segments: Vec<String>,
}

fn hive_of(first: &str) -> Option<RegHive> {
    match first.trim_end_matches(':').to_ascii_lowercase().as_str() {
        "hkcu" | "hkey_current_user" => Some(RegHive::CurrentUser),
        "hklm" | "hkey_local_machine" => Some(RegHive::LocalMachine),
        _ => None,
    }
}

impl RegKey {
    /// Parsuje `HKCU\Software\X`, `HKEY_LOCAL_MACHINE/SOFTWARE`, `HKLM:\...` albo
    /// `Registry::HKEY_CURRENT_USER\...`.
    pub fn parse(raw: &str) -> Result<Self, RegistryError> {
        let invalid = || RegistryError::InvalidKey(raw.chars().take(200).collect());
        if raw.len() > 2_048 || raw.chars().any(char::is_control) {
            return Err(invalid());
        }
        let mut text = raw.trim().replace('/', "\\");
        for prefix in ["microsoft.powershell.core\\registry::", "registry::"] {
            if text.to_ascii_lowercase().starts_with(prefix) {
                text = text[prefix.len()..].to_owned();
            }
        }
        let mut parts = text.split('\\').map(str::trim).filter(|s| !s.is_empty());
        let first = parts.next().ok_or_else(invalid)?;
        let hive = hive_of(first).ok_or_else(|| RegistryError::UnsupportedHive(first.into()))?;
        let segments: Vec<String> = parts.map(str::to_owned).collect();
        if segments.len() > MAX_DEPTH
            || segments
                .iter()
                .any(|s| s == "." || s == ".." || s.chars().count() > 255)
        {
            return Err(invalid());
        }
        Ok(Self { hive, segments })
    }

    /// Gałąź.
    pub fn hive(&self) -> RegHive {
        self.hive
    }

    /// Segmenty pod gałęzią.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Ścieżka pod gałęzią (`Software\X`; pusta dla korzenia gałęzi).
    pub fn subpath(&self) -> String {
        self.segments.join("\\")
    }

    /// Podklucz.
    pub fn child(&self, name: &str) -> Result<Self, RegistryError> {
        Self::parse(&format!("{self}\\{name}"))
    }

    /// Czy klucz (albo któryś przodek) jest na deny-liście sekretów.
    pub fn is_secret(&self) -> bool {
        self.segments.iter().any(|s| is_secret_segment(s))
    }
}

impl fmt::Display for RegKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.hive.short())?;
        for s in &self.segments {
            write!(f, "\\{s}")?;
        }
        Ok(())
    }
}

fn has_secret_fragment(lower: &str) -> bool {
    SECRET_FRAGMENTS.iter().any(|f| lower.contains(f))
}

/// Czy segment ścieżki oznacza klucz z sekretami.
pub fn is_secret_segment(segment: &str) -> bool {
    let lower = segment.trim().to_lowercase();
    SECRET_SEGMENTS.contains(&lower.as_str()) || has_secret_fragment(&lower)
}

/// Czy nazwa wartości wskazuje sekret (dane zawsze redagowane).
pub fn is_secret_value_name(name: &str) -> bool {
    has_secret_fragment(&name.to_lowercase())
}

/// Sprawdzenie przed odczytem: `Err(Denied)` dla klucza z sekretami.
pub fn check_key(key: &RegKey) -> Result<(), RegistryError> {
    if key.is_secret() {
        return Err(RegistryError::Denied(key.to_string()));
    }
    Ok(())
}

/// Dane wartości.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RegData {
    /// `REG_SZ`.
    String(String),
    /// `REG_EXPAND_SZ` (bez rozwijania zmiennych).
    ExpandString(String),
    /// `REG_MULTI_SZ`.
    MultiString(Vec<String>),
    /// `REG_DWORD`.
    Dword(u32),
    /// `REG_QWORD`.
    Qword(u64),
    /// `REG_BINARY` (podgląd szesnastkowy ≤ [`MAX_REG_BINARY_PREVIEW`] B).
    Binary {
        /// Rozmiar w bajtach.
        bytes: usize,
        /// Podgląd szesnastkowy.
        hex: String,
    },
    /// Inny typ (rozmiar bez treści).
    Other {
        /// Typ `REG_*`.
        kind: u32,
        /// Rozmiar w bajtach.
        bytes: usize,
    },
    /// Zredagowano (nazwa wartości wskazuje sekret).
    Redacted,
}

impl RegData {
    /// Binarne dane z podglądem.
    pub fn binary(data: &[u8]) -> Self {
        let hex = data
            .iter()
            .take(MAX_REG_BINARY_PREVIEW)
            .map(|b| format!("{b:02x}"))
            .collect();
        Self::Binary {
            bytes: data.len(),
            hex,
        }
    }
}

fn cut(text: String) -> String {
    if text.chars().count() > MAX_REG_DATA_CHARS {
        text.chars().take(MAX_REG_DATA_CHARS).collect()
    } else {
        text
    }
}

/// Wartość.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegValue {
    /// Nazwa (pusta = wartość domyślna klucza).
    pub name: String,
    /// Dane.
    pub data: RegData,
}

impl RegValue {
    /// Redakcja sekretów i obcięcie napisów (wołane przez każdą implementację).
    #[must_use]
    pub fn guarded(mut self) -> Self {
        if is_secret_value_name(&self.name) {
            self.data = RegData::Redacted;
            return self;
        }
        self.data = match self.data {
            RegData::String(s) => RegData::String(cut(s)),
            RegData::ExpandString(s) => RegData::ExpandString(cut(s)),
            RegData::MultiString(v) => RegData::MultiString(v.into_iter().map(cut).collect()),
            other => other,
        };
        self
    }
}

/// Zawartość klucza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegListing {
    /// Klucz (postać tekstowa).
    pub key: String,
    /// Podklucze (bez kluczy z sekretami).
    pub subkeys: Vec<String>,
    /// Wartości (sekrety zredagowane).
    pub values: Vec<RegValue>,
    /// Ile podkluczy ukryto (deny-lista).
    pub hidden_subkeys: u32,
    /// Obcięto limitem wpisów.
    pub truncated: bool,
}

/// Strażnik wyniku listy: ukrywa podklucze z sekretami, redaguje wartości, przycina do limitu.
pub fn guard_listing(key: &RegKey, listing: RegListing, max_entries: usize) -> RegListing {
    let max = max_entries.clamp(1, MAX_REG_ENTRIES);
    let before = listing.subkeys.len();
    let mut subkeys: Vec<String> = listing
        .subkeys
        .into_iter()
        .filter(|s| key.child(s).is_ok_and(|k| !k.is_secret()))
        .collect();
    let hidden = before.saturating_sub(subkeys.len());
    let mut values: Vec<RegValue> = listing.values.into_iter().map(RegValue::guarded).collect();
    let truncated = listing.truncated || subkeys.len() + values.len() > max;
    subkeys.truncate(max);
    values.truncate(max.saturating_sub(subkeys.len()));
    RegListing {
        key: key.to_string(),
        subkeys,
        values,
        hidden_subkeys: u32::try_from(hidden).unwrap_or(u32::MAX),
        truncated,
    }
}

/// Port rejestru tylko do odczytu. Każda implementacja: [`check_key`] przed otwarciem klucza,
/// [`guard_listing`] / [`RegValue::guarded`] przed zwróceniem wyniku. Brak metod zapisu.
pub trait RegistryPort: Send + Sync {
    /// Podklucze i wartości klucza.
    fn list(&self, key: &RegKey, max_entries: usize) -> Result<RegListing, RegistryError>;
    /// Jedna wartość (`name` puste = wartość domyślna).
    fn read_value(&self, key: &RegKey, name: &str) -> Result<RegValue, RegistryError>;
}
