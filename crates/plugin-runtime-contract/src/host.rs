//! Operacje hosta (jedyny dostęp wtyczki do świata, funkcja WIT `host.call`): ścisłe
//! parsowanie, zdolność wymagana dla Brokera, port wykonawczy [`PluginHost`] (kompozycja:
//! `tools-fs`/`undo-journal`, klient HTTP z egress-allowlistą) i atrapa [`MemHost`].

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use compliance_contract::PathEnv;
use safety_broker_contract::{CapToken, Capability, Holder, HostPattern, PathScope};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Nazwy operacji hosta (WIT `host.call`, parametr `op`).
pub const HOST_OPS: [&str; 4] = ["log", "fs.read-text", "fs.write-text", "net.get"];
/// Najdłuższy wpis `log` (znaki).
pub const MAX_LOG_CHARS: usize = 512;

/// Operacja hosta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HostOp {
    /// Wpis dziennika wywołania (bez zdolności; obcięty, tylko do raportu i Diagnosty).
    Log {
        /// Treść.
        message: String,
    },
    /// Odczyt pliku tekstowego (`fs.read(ścieżka)`).
    FsReadText {
        /// Ścieżka bezwzględna.
        path: String,
    },
    /// Zapis pliku tekstowego (`fs.write(ścieżka)`).
    FsWriteText {
        /// Ścieżka bezwzględna.
        path: String,
        /// Treść.
        content: String,
    },
    /// Pobranie zasobu HTTPS (`net.egress(host)`).
    NetGet {
        /// Adres `https://…`.
        url: String,
    },
}

/// Błąd operacji hosta (tekst trafia do wtyczki jako `err`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "host_error", content = "detail", rename_all = "snake_case")]
pub enum HostError {
    /// Nieznana operacja.
    #[error("nieznana operacja hosta `{0}`")]
    UnknownOp(String),
    /// Argumenty niezgodne z operacją (pola nieznane, typy, JSON).
    #[error("niepoprawne argumenty operacji: {0}")]
    BadArgs(String),
    /// Zdolność spoza manifestu wtyczki (odmowa przed Brokerem).
    #[error("operacja wymaga zdolności `{0}`, której wtyczka nie zadeklarowała")]
    NotDeclared(String),
    /// Argumenty albo wynik za duże.
    #[error("dane operacji hosta przekraczają limit ({0} B)")]
    TooLarge(usize),
    /// Odmowa Brokera (Jądro, właściciel, wygaśnięcie, token).
    #[error("odmowa Brokera: {0}")]
    Denied(String),
    /// Błąd wykonania (we/wy, sieć).
    #[error("błąd operacji: {0}")]
    Failed(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogArgs {
    message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteArgs {
    path: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UrlArgs {
    url: String,
}

fn args<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, HostError> {
    serde_json::from_str(raw).map_err(|e| HostError::BadArgs(e.to_string()))
}

fn exact(path: &str) -> Result<PathScope, HostError> {
    PathScope::exact(path, &PathEnv::new()).map_err(|e| HostError::BadArgs(e.to_string()))
}

impl HostOp {
    /// Ścisłe parsowanie (`op` z [`HOST_OPS`], `args` = obiekt JSON bez pól nieznanych).
    pub fn parse(op: &str, raw: &str) -> Result<Self, HostError> {
        match op {
            "log" => args::<LogArgs>(raw).map(|a| Self::Log { message: a.message }),
            "fs.read-text" => args::<PathArgs>(raw).map(|a| Self::FsReadText { path: a.path }),
            "fs.write-text" => args::<WriteArgs>(raw).map(|a| Self::FsWriteText {
                path: a.path,
                content: a.content,
            }),
            "net.get" => args::<UrlArgs>(raw).map(|a| Self::NetGet { url: a.url }),
            other => Err(HostError::UnknownOp(other.chars().take(64).collect())),
        }
    }

    /// Nazwa operacji.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Log { .. } => "log",
            Self::FsReadText { .. } => "fs.read-text",
            Self::FsWriteText { .. } => "fs.write-text",
            Self::NetGet { .. } => "net.get",
        }
    }

    /// Zdolność potrzebna do wykonania (`None` = bez Brokera: tylko `log`). Ścieżki muszą
    /// być bezwzględne; adres musi być `https://` z hostem.
    pub fn capability(&self) -> Result<Option<Capability>, HostError> {
        match self {
            Self::Log { .. } => Ok(None),
            Self::FsReadText { path } => Ok(Some(Capability::FsRead(exact(path)?))),
            Self::FsWriteText { path, .. } => Ok(Some(Capability::FsWrite(exact(path)?))),
            Self::NetGet { url } => {
                let rest = url
                    .strip_prefix("https://")
                    .ok_or_else(|| HostError::BadArgs("dozwolone tylko adresy https://".into()))?;
                let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
                if host.is_empty() || host.contains(['@', ':', '*']) {
                    return Err(HostError::BadArgs("adres bez jawnego hosta".into()));
                }
                HostPattern::parse(host)
                    .map(|h| Some(Capability::NetEgress(h)))
                    .map_err(|e| HostError::BadArgs(e.to_string()))
            }
        }
    }

    /// Czy operacja zmienia świat.
    pub fn mutating(&self) -> bool {
        matches!(self, Self::FsWriteText { .. } | Self::NetGet { .. })
    }
}

/// Czy potrzebna zdolność mieści się w którejś zadeklarowanej w manifeście.
pub fn declared(needed: &Capability, manifest_caps: &[Capability]) -> bool {
    manifest_caps.iter().any(|d| needed.is_subset_of(d))
}

/// Port wykonujący operacje hosta po zgodzie Brokera. Implementacja **musi** sama wywołać
/// `Broker::verify(token, potrzebna zdolność, holder)` (obrona w głąb) i nie może wykonać
/// operacji bez tokenu (poza `log`).
#[async_trait]
pub trait PluginHost: Send + Sync {
    /// Wykonuje operację (token: `None` tylko dla `log`).
    async fn execute(
        &self,
        op: &HostOp,
        token: Option<&CapToken>,
        holder: &Holder,
    ) -> Result<Value, HostError>;
}

/// Wywołanie zarejestrowane przez [`MemHost`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCall {
    /// Operacja.
    pub op: String,
    /// Czy był token.
    pub token: bool,
    /// Zdolność tokenu (postać tekstowa).
    pub capability: Option<String>,
}

/// Deterministyczny host w pamięci (testy, atrapa): pliki po ścieżce kanonicznej, odpowiedzi
/// sieci po adresie. Sprawdza, że token jest i obejmuje potrzebną zdolność.
#[derive(Debug, Default)]
pub struct MemHost {
    files: Mutex<BTreeMap<String, String>>,
    web: Mutex<BTreeMap<String, String>>,
    calls: Mutex<Vec<HostCall>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn key(path: &str) -> Result<String, HostError> {
    Ok(exact(path)?.canonical().to_owned())
}

impl MemHost {
    /// Host z plikami (ścieżka → treść).
    pub fn with_files(files: &[(&str, &str)]) -> Self {
        let host = Self::default();
        for (p, c) in files {
            if let Ok(k) = key(p) {
                lock(&host.files).insert(k, (*c).to_owned());
            }
        }
        host
    }

    /// Dodaje odpowiedź sieci.
    pub fn serve(&self, url: &str, body: &str) {
        lock(&self.web).insert(url.to_owned(), body.to_owned());
    }

    /// Treść pliku.
    pub fn file(&self, path: &str) -> Option<String> {
        key(path)
            .ok()
            .and_then(|k| lock(&self.files).get(&k).cloned())
    }

    /// Zarejestrowane wywołania.
    pub fn calls(&self) -> Vec<HostCall> {
        lock(&self.calls).clone()
    }
}

#[async_trait]
impl PluginHost for MemHost {
    async fn execute(
        &self,
        op: &HostOp,
        token: Option<&CapToken>,
        _holder: &Holder,
    ) -> Result<Value, HostError> {
        lock(&self.calls).push(HostCall {
            op: op.name().to_owned(),
            token: token.is_some(),
            capability: token.map(|t| t.cap.to_string()),
        });
        let needed = op.capability()?;
        if let Some(needed) = &needed {
            let ok = token.is_some_and(|t| needed.is_subset_of(&t.cap));
            if !ok {
                return Err(HostError::Denied(
                    "brak tokenu obejmującego operację".into(),
                ));
            }
        }
        match op {
            HostOp::Log { .. } => Ok(Value::Null),
            HostOp::FsReadText { path } => lock(&self.files)
                .get(&key(path)?)
                .map(|c| serde_json::json!({ "content": c }))
                .ok_or_else(|| HostError::Failed("nie znaleziono pliku".into())),
            HostOp::FsWriteText { path, content } => {
                lock(&self.files).insert(key(path)?, content.clone());
                Ok(serde_json::json!({ "written": content.len() }))
            }
            HostOp::NetGet { url } => lock(&self.web)
                .get(url)
                .map(|b| serde_json::json!({ "body": b }))
                .ok_or_else(|| HostError::Failed("brak odpowiedzi".into())),
        }
    }
}
