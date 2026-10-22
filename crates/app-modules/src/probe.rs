//! Test połączenia i wykrywanie modeli dla `accounts-hub` przez adaptery `providers-api-impl`
//! (`list_models` = test klucza, SPEC providers-api). Klucz nie opuszcza tego wywołania.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use accounts_hub_contract::{
    ConnectionReport, ConnectionRequest, ConnectionTester, EnterpriseSettings, ModelId, ModelInfo,
    ModelListError, ModelLister, ProbeDetail, TestOutcome,
};
use async_trait::async_trait;
use providers_api_impl::{AccountProfile, AzureOptions, CatalogEntry, build_provider};
use providers_contract::{
    ApiKey, ModelProvider, ProviderError, ProviderErrorKind, SecretSource, StaticKey,
};

/// Profil adaptera dla konta: klucz, adres i ustawienia konta firmowego — nagłówki organizacji
/// i projektu OpenAI, wersja API i wdrożenia Azure OpenAI (model Alfy → wdrożenie).
pub(crate) fn account_profile(
    key: Arc<dyn SecretSource>,
    base_url: Option<&str>,
    enterprise: &EnterpriseSettings,
) -> AccountProfile {
    let mut profile = AccountProfile::new(key);
    profile.base_url = base_url.map(str::to_owned);
    profile.organization.clone_from(&enterprise.organization);
    profile.project.clone_from(&enterprise.project);
    if !enterprise.deployments.is_empty() {
        profile.azure = Some(AzureOptions {
            api_version: enterprise.azure_api_version().to_owned(),
            deployments: (enterprise.deployments.iter())
                .map(|d| (d.alias().to_owned(), d.name.clone()))
                .collect(),
        });
    }
    profile
}

/// Sonda dostawców na wpisach katalogu.
pub struct ProviderProbe {
    catalog: Arc<BTreeMap<String, CatalogEntry>>,
}

impl ProviderProbe {
    /// Sonda na katalogu adapterów.
    pub fn new(catalog: Arc<BTreeMap<String, CatalogEntry>>) -> Self {
        Self { catalog }
    }

    fn provider(&self, request: &ConnectionRequest<'_>) -> Result<Arc<dyn ModelProvider>, String> {
        let id = request.provider.id.as_str();
        let entry = self
            .catalog
            .get(id)
            .ok_or_else(|| format!("brak adaptera dla dostawcy `{id}`"))?;
        let key = StaticKey(request.secret.map(|s| ApiKey::new(s.expose_secret())));
        let profile = account_profile(Arc::new(key), request.base_url, request.enterprise);
        build_provider(entry, profile).map_err(|e| e.to_string())
    }
}

/// Kody błędów dostawców, które znaczą „klucz jest poprawny, ale konto nie ma środków”.
const NO_CREDIT_CODES: [&str; 5] = [
    "insufficient_quota",
    "billing_hard_limit_reached",
    "billing_not_active",
    "billing_error",
    "insufficient_balance",
];

/// Kody, które znaczą „ten klucz jest zły” niezależnie od kodu HTTP.
const INVALID_KEY_CODES: [&str; 4] = [
    "invalid_api_key",
    "authentication_error",
    "invalid_key",
    "api_key_invalid",
];

/// Dostawcy, którzy oddają listę modeli każdemu — udany odczyt nie mówi nic o kluczu.
const PUBLIC_MODEL_LISTS: [&str; 1] = ["openrouter"];

/// Najdłuższy kod błędu dostawcy, jaki przekazujemy dalej.
const CODE_CHARS: usize = 64;

/// Dostawca wprost mówi, że klucz jest zły — także na HTTP 400 (Google, xAI).
fn names_bad_key(code: &str, text: &str) -> bool {
    INVALID_KEY_CODES.contains(&code)
        || text.contains("api key not valid")
        || text.contains("incorrect api key")
        || text.contains("invalid api key")
}

/// Najdłuższy komunikat dostawcy, jaki przekazujemy dalej.
const DETAIL_CHARS: usize = 400;

/// Czy dostawca odmówił z powodu uprawnień klucza, a nie samego klucza. OpenAI odpowiada wtedy
/// 401 „You have insufficient permissions for this operation. Missing scopes: …”, Anthropic —
/// `permission_error`. Taki klucz jest ważny; brakuje mu tylko prawa do listy modeli.
fn lacks_permission(code: &str, text: &str) -> bool {
    matches!(code, "insufficient_permissions" | "permission_error")
        || text.contains("insufficient permissions")
        || text.contains("missing scopes")
}

/// Brak środków: kod dostawcy, HTTP 402 albo znane sformułowanie (Anthropic zgłasza to jako
/// zwykły błąd żądania, DeepSeek i OpenRouter — kodem 402).
fn no_credit(code: &str, status: Option<u16>, text: &str) -> bool {
    NO_CREDIT_CODES.contains(&code)
        || status == Some(402)
        || text.contains("credit balance")
        || text.contains("insufficient balance")
        || text.contains("insufficient credits")
}

fn outcome(error: &ProviderError) -> TestOutcome {
    let code = error
        .provider_code
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let code = code.as_str();
    let text = error.message.to_lowercase();
    let message = || scrub(&error.message);
    if no_credit(code, error.status, &text) {
        return TestOutcome::NoCredit { message: message() };
    }
    if names_bad_key(code, &text) {
        return TestOutcome::InvalidKey;
    }
    match &error.kind {
        ProviderErrorKind::Auth if lacks_permission(code, &text) => {
            TestOutcome::Restricted { message: message() }
        }
        // „Zły klucz” to 401 albo jednoznaczny kod. Inne odmowy (403: kraj, wyłączone API, zapora)
        // nie mówią nic o kluczu — osobny wynik, żeby nie kazać wymieniać dobrego klucza.
        ProviderErrorKind::Auth if error.status.is_none_or(|s| s == 401) => TestOutcome::InvalidKey,
        ProviderErrorKind::Auth => TestOutcome::Refused { message: message() },
        ProviderErrorKind::RateLimited { .. } => TestOutcome::RateLimited,
        ProviderErrorKind::Timeout { .. } => TestOutcome::Timeout,
        ProviderErrorKind::Unsupported => TestOutcome::Unsupported { message: message() },
        // Adres bez listy modeli (własny serwer zgodny z OpenAI): nie ma jak sprawdzić klucza,
        // ale to nie błąd połączenia — test zostawia konto w dotychczasowym stanie (wymiana na nowy
        // klucz liczy stan od nowa, od „działa” — `try_rotate_now` w `accounts-hub-impl`).
        ProviderErrorKind::InvalidRequest if matches!(error.status, Some(404 | 405)) => {
            TestOutcome::Unsupported {
                message:
                    "ten adres nie udostępnia listy modeli — klucza nie da się sprawdzić z góry"
                        .to_owned(),
            }
        }
        _ => TestOutcome::Network {
            message: scrub(&error.to_string()),
        },
    }
}

/// Komunikat dostawcy do pokazania i zapisania: bez fragmentów kluczy (dostawcy odsyłają klucz
/// zamaskowany, np. `sk-proj-****abcd` — tego nie złapie porównanie z pełnym kluczem), bez znaków
/// sterujących i kierunkowych, skrócony.
fn scrub(message: &str) -> String {
    const KEY_STARTS: [&str; 8] = [
        "sk-", "sk_", "xai-", "aiza", "key-", "gsk_", "pk-", "bearer",
    ];
    let clean: String = message
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
        .collect();
    let words: Vec<String> = clean
        .split(' ')
        .map(|word| {
            let bare =
                word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            let lower = bare.to_ascii_lowercase();
            // Zamaskowany fragment maskujemy bez względu na długość: `****abcd` to cztery
            // ostatnie znaki prawdziwego klucza.
            let masked = (word.contains("**") || word.contains("…") || word.contains("..."))
                && word.chars().any(|c| c.is_ascii_alphanumeric());
            let key_like =
                KEY_STARTS.iter().any(|start| lower.starts_with(start)) && bare.len() >= 8;
            if key_like || masked {
                "[•••]".to_owned()
            } else {
                word.to_owned()
            }
        })
        .collect();
    words.join(" ").chars().take(DETAIL_CHARS).collect()
}

/// To, co odpowiedział dostawca — użytkownik widzi powód odmowy, a nie tylko „odrzucony”.
fn detail(error: &ProviderError) -> ProbeDetail {
    ProbeDetail {
        status: error.status,
        // Kod kontroluje serwer (także cudzy endpoint): tylko krótki ciąg drukowalnych znaków ASCII.
        code: error
            .provider_code
            .as_deref()
            .map(|code| {
                code.chars()
                    .filter(char::is_ascii_graphic)
                    .take(CODE_CHARS)
                    .collect::<String>()
            })
            .filter(|code| !code.is_empty()),
        message: scrub(&error.message),
    }
}

/// Lista modeli jest publiczna — o kluczu mówi dopiero osobne sprawdzenie (`check_key`).
async fn key_report(provider: &dyn ModelProvider, started: Instant) -> ConnectionReport {
    let result = provider.check_key().await;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).ok();
    match result {
        Ok(note) => ConnectionReport {
            outcome: TestOutcome::Ok,
            latency_ms,
            detail: note.map(|message| ProbeDetail {
                status: None,
                code: None,
                message: scrub(&message),
            }),
        },
        Err(e) if matches!(e.kind, ProviderErrorKind::Unsupported) => ConnectionReport {
            outcome: TestOutcome::Unsupported {
                message: "ten dostawca pokazuje listę modeli bez klucza — klucz sprawdzi dopiero pierwsza rozmowa".to_owned(),
            },
            latency_ms,
            detail: None,
        },
        Err(e) => ConnectionReport {
            outcome: outcome(&e),
            latency_ms,
            detail: Some(detail(&e)),
        },
    }
}

#[async_trait]
impl ConnectionTester for ProviderProbe {
    async fn test(&self, request: ConnectionRequest<'_>) -> ConnectionReport {
        let provider = match self.provider(&request) {
            Ok(p) => p,
            Err(message) => {
                return ConnectionReport {
                    outcome: TestOutcome::Unsupported { message },
                    latency_ms: None,
                    detail: None,
                };
            }
        };
        let started = Instant::now();
        let result = provider.list_models().await;
        let latency_ms = u64::try_from(started.elapsed().as_millis()).ok();
        match result {
            Ok(_) if PUBLIC_MODEL_LISTS.contains(&request.provider.id.as_str()) => {
                key_report(provider.as_ref(), started).await
            }
            Ok(_) => ConnectionReport {
                outcome: TestOutcome::Ok,
                latency_ms,
                detail: None,
            },
            Err(e) => ConnectionReport {
                outcome: outcome(&e),
                latency_ms,
                detail: Some(detail(&e)),
            },
        }
    }
}

#[async_trait]
impl ModelLister for ProviderProbe {
    async fn list_models(
        &self,
        request: ConnectionRequest<'_>,
    ) -> Result<Vec<ModelInfo>, ModelListError> {
        let provider = self
            .provider(&request)
            .map_err(|_| ModelListError::Unsupported)?;
        let models = provider.list_models().await.map_err(|e| match e.kind {
            ProviderErrorKind::Auth => ModelListError::InvalidKey,
            ProviderErrorKind::Timeout { .. } => ModelListError::Timeout,
            ProviderErrorKind::Unsupported => ModelListError::Unsupported,
            _ => ModelListError::Network {
                message: e.to_string(),
            },
        })?;
        Ok(models
            .into_iter()
            .filter_map(|m| {
                Some(ModelInfo {
                    id: ModelId::new(m.id).ok()?,
                    display_name: m.display_name,
                    capabilities: None,
                    context_window: m.capabilities.and_then(|c| c.context_window),
                })
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
