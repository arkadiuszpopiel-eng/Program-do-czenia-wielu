//! Test połączenia i wykrywanie modeli dla `accounts-hub` przez adaptery `providers-api-impl`
//! (`list_models` = test klucza, SPEC providers-api). Klucz nie opuszcza tego wywołania.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use accounts_hub_contract::{
    ConnectionReport, ConnectionRequest, ConnectionTester, ModelId, ModelInfo, ModelListError,
    ModelLister, ProbeDetail, TestOutcome,
};
use async_trait::async_trait;
use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
use providers_contract::{ApiKey, ModelProvider, ProviderError, ProviderErrorKind, StaticKey};

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
        let mut profile = AccountProfile::new(Arc::new(key));
        profile.base_url = request.base_url.map(str::to_owned);
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
        // ale to nie błąd połączenia — konto zostaje w dotychczasowym stanie.
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
                ConnectionReport {
                    outcome: TestOutcome::Unsupported {
                        message: "ten dostawca pokazuje listę modeli bez klucza — klucz sprawdzi                                   dopiero pierwsza rozmowa"
                            .to_owned(),
                    },
                    latency_ms,
                    detail: None,
                }
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
mod tests {
    use super::*;

    #[test]
    fn provider_errors_map_to_test_outcomes() {
        let e = |kind| ProviderError::new(kind, "x");
        assert_eq!(
            outcome(&e(ProviderErrorKind::Auth)),
            TestOutcome::InvalidKey
        );
        assert_eq!(
            outcome(&e(ProviderErrorKind::RateLimited {
                retry_after_ms: None
            })),
            TestOutcome::RateLimited
        );
        assert!(matches!(
            outcome(&e(ProviderErrorKind::Network)),
            TestOutcome::Network { .. }
        ));
    }

    fn error(status: u16, code: &str, message: &str) -> ProviderError {
        let kind = providers_contract::classify_http_status(status, None);
        let mut e = ProviderError::new(kind, message).with_status(status);
        if !code.is_empty() {
            e = e.with_provider_code(code);
        }
        e
    }

    /// Zgłoszenie właściciela (2026-10-09): każdy błąd 401 był „kluczem odrzuconym”. Klucz z
    /// ograniczonymi uprawnieniami, konto bez środków i odmowa z innego powodu to inne sytuacje.
    #[test]
    fn each_refusal_is_told_apart() {
        let scopes = error(
            401,
            "",
            "You have insufficient permissions for this operation. Missing scopes: api.model.read.",
        );
        assert!(matches!(outcome(&scopes), TestOutcome::Restricted { .. }));
        assert!(outcome(&scopes).key_works());
        let anthropic = error(403, "permission_error", "not allowed");
        assert!(matches!(
            outcome(&anthropic),
            TestOutcome::Restricted { .. }
        ));

        // Brak środków: kod OpenAI na 429 (także `billing_not_active`), HTTP 402 bez kodu
        // (DeepSeek, OpenRouter), sformułowanie Anthropic na 400.
        for quota in [
            ProviderError::new(ProviderErrorKind::Auth, "You exceeded your current quota")
                .with_provider_code("insufficient_quota")
                .with_status(429),
            error(
                429,
                "billing_not_active",
                "Your account is not active, please check your billing details.",
            ),
            error(402, "", "Insufficient Balance"),
            error(
                400,
                "invalid_request_error",
                "Your credit balance is too low to access the API.",
            ),
        ] {
            assert!(
                matches!(outcome(&quota), TestOutcome::NoCredit { .. }),
                "{quota:?}"
            );
            assert!(!outcome(&quota).key_works());
        }

        let wrong = error(401, "invalid_api_key", "Incorrect API key provided");
        assert_eq!(outcome(&wrong), TestOutcome::InvalidKey);
        assert_eq!(
            outcome(&error(401, "", "HTTP 401")),
            TestOutcome::InvalidKey
        );

        // Google i xAI zgłaszają zły klucz kodem 400 — to nadal „zły klucz”, nie „błąd połączenia”.
        for bad in [
            error(
                400,
                "INVALID_ARGUMENT",
                "API key not valid. Please pass a valid API key.",
            ),
            error(400, "", "Incorrect API key provided: [x]"),
            error(403, "API_KEY_INVALID", "denied"),
        ] {
            assert_eq!(outcome(&bad), TestOutcome::InvalidKey, "{bad:?}");
        }

        // 403 bez kodu „zły klucz”: kraj, wyłączone API, strona zapory — to nie wina klucza.
        for refused in [
            error(
                403,
                "unsupported_country_region_territory",
                "Country, region, or territory not supported",
            ),
            error(403, "", "HTTP 403"),
        ] {
            assert!(
                matches!(outcome(&refused), TestOutcome::Refused { .. }),
                "{refused:?}"
            );
            assert!(!outcome(&refused).key_works());
        }

        // Adres bez listy modeli nie robi z konta „błędu połączenia”.
        assert!(matches!(
            outcome(&error(404, "", "Not Found")),
            TestOutcome::Unsupported { .. }
        ));
        assert!(matches!(
            outcome(&error(500, "", "boom")),
            TestOutcome::Network { .. }
        ));
    }

    /// Dostawca odsyła klucz zamaskowany albo w całości — do interfejsu i na dysk nie trafia żaden.
    #[test]
    fn provider_messages_never_carry_key_fragments() {
        let echoed = error(
            401,
            "invalid_api_key",
            "Incorrect API key provided: sk-proj-****************************abcd. You can find your API key at https://platform.openai.com/account/api-keys.",
        );
        let shown = detail(&echoed);
        assert_eq!(
            (shown.status, shown.code.as_deref()),
            (Some(401), Some("invalid_api_key"))
        );
        assert!(
            shown
                .message
                .starts_with("Incorrect API key provided: [•••]"),
            "{}",
            shown.message
        );
        assert!(!shown.message.contains("abcd") && !shown.message.contains("sk-proj"));
        assert!(
            shown.message.contains("https://platform.openai.com"),
            "adres zostaje"
        );
        // Krótki zamaskowany fragment (DeepSeek: cztery ostatnie znaki) też znika.
        let short = error(
            401,
            "",
            "Authentication Fails, Your api key: ****abcd is invalid",
        );
        assert_eq!(
            detail(&short).message,
            "Authentication Fails, Your api key: [•••] is invalid"
        );
        let code = error(401, &format!("x\u{202E}{}", "y".repeat(200)), "m");
        assert_eq!(detail(&code).code.map(|c| c.len()), Some(CODE_CHARS));
        let bidi = error(401, "", "bad\u{202E}key\u{0007} xai-0000000000000000");
        assert_eq!(detail(&bidi).message, "badkey [•••]");
        let long = error(401, "", &"x".repeat(5000));
        assert_eq!(detail(&long).message.chars().count(), DETAIL_CHARS);
    }
}
