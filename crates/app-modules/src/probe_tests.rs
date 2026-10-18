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

/// Konto firmowe (C2.12): organizacja i projekt trafiają do profilu adaptera, wdrożenia Azure —
/// jako mapa „model Alfy → wdrożenie” z wersją API; zwykłe konto nie ma żadnego z nich.
#[test]
fn enterprise_settings_reach_the_adapter_profile() {
    use accounts_hub_contract::AzureDeployment;
    let key: Arc<dyn providers_contract::SecretSource> = Arc::new(StaticKey::new("k"));
    let plain = account_profile(Arc::clone(&key), None, &EnterpriseSettings::default());
    assert!(plain.organization.is_none() && plain.project.is_none() && plain.azure.is_none());

    let openai = EnterpriseSettings {
        organization: Some("org-Acme".into()),
        project: Some("proj_Budget".into()),
        ..EnterpriseSettings::default()
    };
    let p = account_profile(Arc::clone(&key), None, &openai);
    assert_eq!(p.organization.as_deref(), Some("org-Acme"));
    assert_eq!(p.project.as_deref(), Some("proj_Budget"));

    let azure = EnterpriseSettings {
        deployments: vec![
            AzureDeployment {
                name: "prod-4o".into(),
                model: Some("gpt-4o".into()),
            },
            AzureDeployment {
                name: "o3-mini".into(),
                model: None,
            },
        ],
        ..EnterpriseSettings::default()
    };
    let p = account_profile(key, Some("https://contoso.openai.azure.com"), &azure);
    assert_eq!(
        p.base_url.as_deref(),
        Some("https://contoso.openai.azure.com")
    );
    let options = p.azure.unwrap();
    assert_eq!(
        options.api_version,
        accounts_hub_contract::AZURE_DEFAULT_API_VERSION
    );
    assert_eq!(options.deployment("gpt-4o"), "prod-4o");
    assert_eq!(options.deployment("o3-mini"), "o3-mini");
}

/// Wbudowany katalog zna Azure OpenAI w obu widokach; bez adresu zasobu adaptera nie ma.
#[test]
fn builtin_catalog_has_azure_openai() {
    let catalog = crate::catalog::ProviderCatalog::builtin();
    let hub = catalog
        .hub
        .iter()
        .find(|e| e.id.as_str() == "azure-openai")
        .unwrap();
    assert!(hub.base_url.is_none(), "adres podaje konto");
    assert!(
        hub.env_vars.is_empty(),
        "import ze zmiennych nie zna wdrożeń"
    );
    let probe = ProviderProbe::new(Arc::new(catalog.api));
    let enterprise = EnterpriseSettings::default();
    let request = ConnectionRequest {
        provider: hub,
        base_url: None,
        secret: None,
        enterprise: &enterprise,
    };
    let error = probe.provider(&request).err().unwrap();
    assert!(error.contains("base_url"), "{error}");
}
