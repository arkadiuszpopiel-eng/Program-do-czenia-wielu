//! Konta firmowe (C2.12): organizacja i projekt OpenAI (nagłówki `OpenAI-Organization`,
//! `OpenAI-Project`) oraz zasób Azure OpenAI (wersja API, wdrożenia modeli).
//!
//! Identyfikatory organizacji i projektu nie są sekretami, ale nie trafiają w całości do logów:
//! `Debug` pokazuje tylko początek i cztery ostatnie znaki ([`mask_id`]).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::validate_base_url;

/// Identyfikator dostawcy OpenAI w katalogu (organizacja i projekt dotyczą tylko jego kont).
pub const OPENAI: &str = "openai";
/// Identyfikator dostawcy Azure OpenAI w katalogu (`providers-catalog/azure-openai.toml`).
pub const AZURE_OPENAI: &str = "azure-openai";
/// Wersja API Azure OpenAI używana, gdy użytkownik jej nie poda (ostatnia wersja ogólnie dostępna).
pub const AZURE_DEFAULT_API_VERSION: &str = "2024-10-21";
/// Najwięcej wdrożeń w jednym koncie Azure OpenAI.
pub const MAX_DEPLOYMENTS: usize = 32;
/// Domeny zasobów Azure OpenAI (nowe zasoby Azure AI Foundry mają `cognitiveservices`).
pub const AZURE_HOST_SUFFIXES: [&str; 2] = [".openai.azure.com", ".cognitiveservices.azure.com"];

const MAX_ID: usize = 64;

/// Wdrożenie modelu w zasobie Azure OpenAI: Alfa wybiera model (`model`), a żądanie idzie na
/// adres wdrożenia (`name`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AzureDeployment {
    /// Nazwa wdrożenia w zasobie (część adresu `/openai/deployments/{name}/…`).
    pub name: String,
    /// Model, który wdrożenie udostępnia (np. `gpt-4o`); brak — model nazywa się jak wdrożenie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl AzureDeployment {
    /// Identyfikator modelu w Alfie (model albo — bez niego — nazwa wdrożenia).
    pub fn alias(&self) -> &str {
        self.model.as_deref().unwrap_or(&self.name)
    }
}

/// Ustawienia konta firmowego. Puste = zwykłe konto (pole pomijane w zapisie).
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnterpriseSettings {
    /// OpenAI: identyfikator organizacji (`org-…`) — nagłówek `OpenAI-Organization`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// OpenAI: identyfikator projektu (`proj_…`) — nagłówek `OpenAI-Project`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Azure OpenAI: wersja API (`api-version`, np. `2024-10-21`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    /// Azure OpenAI: wdrożenia modeli w zasobie.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployments: Vec<AzureDeployment>,
}

impl fmt::Debug for EnterpriseSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnterpriseSettings")
            .field("organization", &self.organization.as_deref().map(mask_id))
            .field("project", &self.project.as_deref().map(mask_id))
            .field("api_version", &self.api_version)
            .field("deployments", &self.deployments)
            .finish()
    }
}

/// Identyfikator skrócony do logów: przedrostek, wielokropek i cztery ostatnie znaki
/// (`org-…wxyz`); krótki identyfikator — sam przedrostek.
pub fn mask_id(id: &str) -> String {
    let prefix: String = id
        .char_indices()
        .find(|(_, c)| matches!(c, '-' | '_'))
        .map_or_else(String::new, |(i, c)| format!("{}{c}", &id[..i]));
    let count = id.chars().count();
    if count <= prefix.chars().count() + 8 {
        return format!("{prefix}…");
    }
    let tail: String = id.chars().skip(count - 4).collect();
    format!("{prefix}…{tail}")
}

fn plain(id: &str, extra: &[char]) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_') || extra.contains(&c))
}

/// Czy tekst wygląda na identyfikator organizacji OpenAI (`org-` + litery, cyfry, `-`, `_`).
pub fn is_openai_organization(id: &str) -> bool {
    id.strip_prefix("org-").is_some_and(|rest| !rest.is_empty()) && plain(id, &[])
}

/// Czy tekst wygląda na identyfikator projektu OpenAI (`proj_…`, dopuszczalnie `proj-…`).
pub fn is_openai_project(id: &str) -> bool {
    ["proj_", "proj-"]
        .iter()
        .any(|p| id.strip_prefix(p).is_some_and(|rest| !rest.is_empty()))
        && plain(id, &[])
}

/// Czy wersja API Azure ma postać `RRRR-MM-DD` albo `RRRR-MM-DD-preview`.
pub fn is_azure_api_version(version: &str) -> bool {
    // Tylko ASCII: dzielenie po bajcie 10 nie może wypaść w środku znaku.
    if !version.is_ascii() {
        return false;
    }
    let (date, suffix) = version.split_at(version.len().min(10));
    let shape = date.len() == 10
        && date.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    shape && matches!(suffix, "" | "-preview")
}

/// Czy nazwa wdrożenia jest bezpieczna w adresie (litery, cyfry, `-`, `_`, `.`; nie od kropki).
pub fn is_deployment_name(name: &str) -> bool {
    plain(name, &['.']) && !name.starts_with('.')
}

/// Adres zasobu Azure OpenAI: `https://<zasób>.openai.azure.com` (także
/// `*.cognitiveservices.azure.com`), bez ścieżki; dla testów i lokalnych bramek — host lokalny.
pub fn validate_azure_endpoint(url: &str) -> Result<(), String> {
    validate_base_url(url)?;
    let lower = url.trim().to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or_default();
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = authority.split(':').next().unwrap_or_default();
    let local = matches!(host, "localhost" | "127.0.0.1") || authority.starts_with("[::1]");
    let azure = AZURE_HOST_SUFFIXES.iter().any(|suffix| {
        host.strip_suffix(suffix).is_some_and(|resource| {
            !resource.is_empty()
                && resource
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    }) && !authority.contains(':');
    if (azure || local) && path.is_empty() && !rest.contains(['?', '#']) {
        return Ok(());
    }
    Err(format!(
        "adres zasobu Azure OpenAI ma postać https://<zasób>.openai.azure.com (bez dalszej ścieżki), a jest `{url}`"
    ))
}

impl EnterpriseSettings {
    /// Czy nic nie ustawiono (zwykłe konto).
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Projekt OpenAI, na który idą koszty (brak = projekt domyślny klucza). Ten sam klucz
    /// w dwóch projektach to dwa osobne konta (`OpenAI-Project` rozdziela budżety).
    pub fn billing_project(&self) -> Option<&str> {
        self.project
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
    }

    /// Wersja API Azure (podana albo domyślna).
    pub fn azure_api_version(&self) -> &str {
        self.api_version
            .as_deref()
            .unwrap_or(AZURE_DEFAULT_API_VERSION)
    }

    /// Ustawienia po oczyszczeniu (przycięte odstępy, puste pola usunięte) i sprawdzeniu dla
    /// dostawcy `provider` z endpointem `base_url`. Konto Azure dostaje domyślną wersję API.
    pub fn checked(self, provider: &str, base_url: Option<&str>) -> Result<Self, String> {
        let text = |v: Option<String>| v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        let mut s = Self {
            organization: text(self.organization),
            project: text(self.project),
            api_version: text(self.api_version),
            deployments: self
                .deployments
                .into_iter()
                .map(|d| AzureDeployment {
                    name: d.name.trim().to_owned(),
                    model: text(d.model),
                })
                .collect(),
        };
        if (s.organization.is_some() || s.project.is_some()) && provider != OPENAI {
            return Err("organizacja i projekt dotyczą tylko kont OpenAI".into());
        }
        if let Some(org) = s
            .organization
            .as_deref()
            .filter(|o| !is_openai_organization(o))
        {
            return Err(format!(
                "identyfikator organizacji zaczyna się od `org-`, a jest `{}`",
                mask_id(org)
            ));
        }
        if let Some(proj) = s.project.as_deref().filter(|p| !is_openai_project(p)) {
            return Err(format!(
                "identyfikator projektu zaczyna się od `proj_`, a jest `{}`",
                mask_id(proj)
            ));
        }
        if provider != AZURE_OPENAI {
            if s.api_version.is_some() || !s.deployments.is_empty() {
                return Err("wdrożenia i wersja API dotyczą tylko kont Azure OpenAI".into());
            }
            return Ok(s);
        }
        validate_azure_endpoint(base_url.unwrap_or_default())?;
        let version = s
            .api_version
            .get_or_insert_with(|| AZURE_DEFAULT_API_VERSION.to_owned());
        if !is_azure_api_version(version) {
            return Err(format!(
                "wersja API Azure ma postać RRRR-MM-DD (np. {AZURE_DEFAULT_API_VERSION}), a jest `{version}`"
            ));
        }
        check_deployments(&s.deployments)?;
        s.deployments.sort_by(|a, b| a.alias().cmp(b.alias()));
        Ok(s)
    }
}

fn check_deployments(deployments: &[AzureDeployment]) -> Result<(), String> {
    if deployments.is_empty() {
        return Err("podaj co najmniej jedno wdrożenie modelu z zasobu Azure OpenAI".into());
    }
    if deployments.len() > MAX_DEPLOYMENTS {
        return Err(format!(
            "najwyżej {MAX_DEPLOYMENTS} wdrożeń w jednym koncie"
        ));
    }
    for (i, d) in deployments.iter().enumerate() {
        if !is_deployment_name(&d.name) {
            return Err(format!(
                "nazwa wdrożenia `{}` może mieć tylko litery, cyfry, `-`, `_` i `.`",
                d.name
            ));
        }
        if d.model.as_deref().is_some_and(|m| !plain(m, &['.', ':'])) {
            return Err(format!(
                "nazwa modelu wdrożenia `{}` może mieć tylko litery, cyfry, `-`, `_`, `.` i `:`",
                d.name
            ));
        }
        if deployments[..i].iter().any(|o| o.alias() == d.alias()) {
            return Err(format!(
                "model `{}` ma dwa wdrożenia — zostaw jedno",
                d.alias()
            ));
        }
    }
    Ok(())
}
