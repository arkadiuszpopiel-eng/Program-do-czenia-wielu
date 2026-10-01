//! Zakresy tokenów zdolności: ścieżki (normalizacja Windows z `compliance-contract`), hosty,
//! aplikacje, sekrety, operacje administracyjne. Każdy zakres ma relację „podzbiór”
//! (atenuacja: potomek ≤ rodzic) liczoną na postaci kanonicznej.

use std::fmt;

use compliance_contract::deny::{NormPath, PathEnv, Root, host_matches, normalize, normalize_host};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Błąd budowy zakresu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    /// Ścieżka względna albo pusta — zakres musi być bezwzględny.
    #[error("zakres ścieżki musi być bezwzględny: `{0}`")]
    RelativePath(String),
    /// Postać niekanoniczna przy deserializacji.
    #[error("zakres ścieżki w postaci niekanonicznej: `{0}`")]
    NotCanonical(String),
    /// Host niepoprawny albo zbyt szeroki (`*`, `*.com`).
    #[error("niepoprawny wzorzec hosta: `{0}`")]
    InvalidHost(String),
    /// Aplikacja niepoprawna albo wieloznaczna.
    #[error("niepoprawny selektor aplikacji: `{0}`")]
    InvalidApp(String),
    /// Identyfikator sekretu niepoprawny.
    #[error("niepoprawny identyfikator sekretu: `{0}`")]
    InvalidSecret(String),
}

/// Kanoniczny zapis znormalizowanej ścieżki (`c:\users\u`, `\\srv\udział\x`).
pub fn render_path(p: &NormPath) -> Option<String> {
    let mut out = match &p.root {
        Root::Drive(d) => format!("{d}:"),
        Root::Unc(server, share) => format!("\\\\{server}\\{share}"),
        Root::Relative => return None,
    };
    if p.comps.is_empty() {
        out.push('\\');
    }
    for c in &p.comps {
        out.push('\\');
        out.push_str(c);
    }
    Some(out)
}

/// Postać surowa do deserializacji (walidowana do kanonicznej).
#[derive(Deserialize, JsonSchema)]
pub struct PathScopeRaw {
    /// Ścieżka.
    pub path: String,
    /// Poddrzewo.
    pub subtree: bool,
}

/// Zakres ścieżki: jeden plik/katalog (`subtree = false`) albo całe poddrzewo.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "PathScopeRaw")]
pub struct PathScope {
    /// Ścieżka kanoniczna (małe litery, `\`, bez `.`/`..`, bez strumieni ADS).
    path: String,
    /// Czy obejmuje poddrzewo.
    subtree: bool,
}

impl TryFrom<PathScopeRaw> for PathScope {
    type Error = ScopeError;

    fn try_from(raw: PathScopeRaw) -> Result<Self, Self::Error> {
        let scope = PathScope::new(&raw.path, raw.subtree, &PathEnv::new())?;
        if scope.path != raw.path {
            return Err(ScopeError::NotCanonical(raw.path));
        }
        Ok(scope)
    }
}

impl PathScope {
    /// Normalizuje ścieżkę w semantyce Windows (zmienne z `env`, `\\?\`, `..`, wielkość liter).
    pub fn new(raw: &str, subtree: bool, env: &PathEnv) -> Result<Self, ScopeError> {
        let norm = normalize(raw, env);
        let path = render_path(&norm).ok_or_else(|| ScopeError::RelativePath(raw.to_owned()))?;
        Ok(Self { path, subtree })
    }

    /// Całe poddrzewo.
    pub fn tree(raw: &str, env: &PathEnv) -> Result<Self, ScopeError> {
        Self::new(raw, true, env)
    }

    /// Dokładnie jedna ścieżka.
    pub fn exact(raw: &str, env: &PathEnv) -> Result<Self, ScopeError> {
        Self::new(raw, false, env)
    }

    /// Postać kanoniczna.
    pub fn canonical(&self) -> &str {
        &self.path
    }

    /// Czy obejmuje poddrzewo.
    pub fn subtree(&self) -> bool {
        self.subtree
    }

    /// Postać znormalizowana (do deny-list).
    pub fn norm(&self) -> NormPath {
        normalize(&self.path, &PathEnv::new())
    }

    /// Czy konkretna ścieżka mieści się w zakresie.
    pub fn contains(&self, p: &NormPath) -> bool {
        let me = self.norm();
        me.root == p.root
            && me.comps.len() <= p.comps.len()
            && me.comps.iter().zip(&p.comps).all(|(a, b)| a == b)
            && (self.subtree || me.comps.len() == p.comps.len())
    }

    /// Atenuacja: czy ten zakres jest podzbiorem `parent`.
    pub fn is_subset_of(&self, parent: &PathScope) -> bool {
        parent.contains(&self.norm()) && (parent.subtree || !self.subtree)
    }

    /// Czy zakresy mają część wspólną (jeden zawiera drugi).
    pub fn overlaps(&self, other: &PathScope) -> bool {
        self.contains(&other.norm()) || other.contains(&self.norm())
    }
}

impl fmt::Display for PathScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.subtree {
            write!(f, "{}\\**", self.path.trim_end_matches('\\'))
        } else {
            f.write_str(&self.path)
        }
    }
}

/// Wzorzec hosta `net.egress`: dokładny host albo domena z subdomenami (`*.example.com`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct HostPattern {
    host: String,
    subdomains: bool,
}

impl HostPattern {
    /// Parsuje `example.com`, `https://example.com/x` albo `*.example.com`. Odrzuca `*`,
    /// pojedynczą etykietę z gwiazdką (`*.com`) i hosty z białymi znakami.
    pub fn parse(input: &str) -> Result<Self, ScopeError> {
        let invalid = || ScopeError::InvalidHost(input.to_owned());
        let trimmed = input.trim();
        let (subdomains, rest) = match trimmed.strip_prefix("*.") {
            Some(rest) => (true, rest),
            None => (false, trimmed),
        };
        if rest.contains('*') {
            return Err(invalid());
        }
        let host = normalize_host(rest).ok_or_else(invalid)?;
        if subdomains && !host.contains('.') {
            return Err(invalid());
        }
        Ok(Self { host, subdomains })
    }

    /// Host bazowy.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Czy obejmuje subdomeny.
    pub fn subdomains(&self) -> bool {
        self.subdomains
    }

    /// Czy host (dowolna postać: domena, URL) mieści się we wzorcu.
    pub fn matches(&self, input: &str) -> bool {
        normalize_host(input)
            .is_some_and(|h| h == self.host || (self.subdomains && host_matches(&h, &self.host)))
    }

    /// Atenuacja: czy ten wzorzec jest podzbiorem `parent`.
    pub fn is_subset_of(&self, parent: &HostPattern) -> bool {
        if self.subdomains && !parent.subdomains {
            return false;
        }
        parent.matches(&self.host)
    }
}

impl TryFrom<String> for HostPattern {
    type Error = ScopeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<HostPattern> for String {
    fn from(value: HostPattern) -> Self {
        value.to_string()
    }
}

impl fmt::Display for HostPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.subdomains {
            write!(f, "*.{}", self.host)
        } else {
            f.write_str(&self.host)
        }
    }
}

/// Selektor aplikacji `gui.control`: nazwa pliku wykonywalnego (bez ścieżki, małe litery,
/// z `.exe`). Wieloznaczność (`*`, `?`) jest odrzucana — objęłaby procesy Jądra.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct AppSelector(String);

impl AppSelector {
    /// Parsuje `C:\Program Files\X\Word.EXE`, `word.exe` albo `word`.
    pub fn parse(input: &str) -> Result<Self, ScopeError> {
        let invalid = || ScopeError::InvalidApp(input.to_owned());
        let name = input
            .trim()
            .trim_matches('"')
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .trim()
            .trim_end_matches(['.', ' '])
            .to_lowercase();
        if name.is_empty() || name.contains(['*', '?', ':', '<', '>', '|', '"']) {
            return Err(invalid());
        }
        let exe = if name.ends_with(".exe") {
            name
        } else {
            format!("{name}.exe")
        };
        if exe == ".exe" {
            return Err(invalid());
        }
        Ok(Self(exe))
    }

    /// Nazwa pliku wykonywalnego.
    pub fn exe(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AppSelector {
    type Error = ScopeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<AppSelector> for String {
    fn from(value: AppSelector) -> Self {
        value.0
    }
}

impl fmt::Display for AppSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identyfikator sekretu (konto w `accounts-hub`), dopasowanie dokładne.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct SecretId(String);

impl SecretId {
    /// `[a-z0-9._:-]{1,128}`.
    pub fn parse(input: &str) -> Result<Self, ScopeError> {
        let ok = !input.is_empty()
            && input.len() <= 128
            && input.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | ':' | '-')
            });
        if ok {
            Ok(Self(input.to_owned()))
        } else {
            Err(ScopeError::InvalidSecret(input.to_owned()))
        }
    }

    /// Tekst identyfikatora.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SecretId {
    type Error = ScopeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<SecretId> for String {
    fn from(value: SecretId) -> Self {
        value.0
    }
}

/// Czynność na usłudze Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    /// Start.
    Start,
    /// Stop.
    Stop,
    /// Restart.
    Restart,
    /// Wyłączenie autostartu.
    Disable,
    /// Usunięcie.
    Delete,
    /// Zmiana konfiguracji.
    Configure,
}

/// Operacja `system.admin` (UAC na żądanie, PLAN §8.4). Dopasowanie dokładne.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AdminOp {
    /// Instalacja programu (pakiet/instalator).
    Install {
        /// Pakiet.
        package: String,
    },
    /// Sterowanie usługą.
    ServiceControl {
        /// Nazwa usługi.
        service: String,
        /// Czynność.
        action: ServiceAction,
    },
    /// Reguła zapory.
    Firewall {
        /// Reguła.
        rule: String,
    },
    /// Zapis w `HKLM`.
    RegistryMachine {
        /// Klucz.
        key: String,
    },
    /// Wyłączenie audytu (zawsze twarda blokada).
    DisableAudit,
    /// Formatowanie dysku.
    FormatDisk {
        /// Litera dysku.
        drive: char,
    },
    /// Modyfikacja bootloadera (zawsze twarda blokada).
    Bootloader,
    /// Zmiana polityk Jądra z pominięciem Broker-UI (zawsze twarda blokada).
    ChangeKernelPolicy,
    /// Inne polecenie elewowane (sprawdzane regułami powłoki).
    Other {
        /// Polecenie.
        command: String,
    },
}
