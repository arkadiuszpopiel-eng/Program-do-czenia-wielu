//! Współdzielone testy kontraktowe (feature `contract-tests`), uruchamiane na `-impl` i `-fake`.

use std::future::Future;

use chrono::NaiveDate;

use crate::{
    ChangeOrigin, Compliance, ComplianceError, DecisionReason, DenyLists, KernelAuthority,
    PrivacyTag, ProviderApiStatus, ProviderPolicyInput, Registry, RouteId, RouteStatus, RouteTags,
    SessionTag,
};

/// Rejestr-fixture: trasy zielona, szara, zabroniona i „CN”; dostawcy EU, CN i nieznany.
pub const FIXTURE_REGISTRY: &str = r#"{
  "schema_version": 1, "max_age_days": 30, "generated_at": "2026-09-01", "notes": "fixture",
  "routes": [
    {"id": "green-cli", "provider": "acme", "mode": "cli-p", "status": "green",
     "verified_at": "2026-09-01", "sources": [{"url": "https://example.com/tos", "quote": "ok",
     "retrieved_at": "2026-09-01", "confidence": "V"}], "privacy_tags": ["eu"],
     "allowed": ["wszystko"], "forbidden": [], "cli_pinned_version": null, "enabled_by_default": true},
    {"id": "grey-cli", "provider": "acme", "mode": "sdk", "status": "gray",
     "verified_at": "2026-09-01", "sources": [{"url": "TODO", "quote": "?",
     "retrieved_at": "2026-09-01", "confidence": "?"}], "privacy_tags": ["eu"],
     "allowed": [], "forbidden": [], "cli_pinned_version": null, "enabled_by_default": false},
    {"id": "banned-cli", "provider": "acme", "mode": "cli-p", "status": "forbidden",
     "verified_at": "2026-09-01", "sources": [{"url": "TODO", "quote": "nie",
     "retrieved_at": "2026-09-01", "confidence": "W"}], "privacy_tags": ["eu"],
     "allowed": [], "forbidden": ["wszystko"], "cli_pinned_version": null, "enabled_by_default": false},
    {"id": "cn-cli", "provider": "dragon", "mode": "cli-p", "status": "green",
     "verified_at": "2026-09-01", "sources": [{"url": "TODO", "quote": "ok",
     "retrieved_at": "2026-09-01", "confidence": "W"}], "privacy_tags": ["cn-may-train"],
     "allowed": [], "forbidden": [], "cli_pinned_version": null, "enabled_by_default": true}
  ],
  "providers": [
    {"id": "acme", "jurisdiction": "EU", "privacy_tags": ["eu"], "confidence": "V", "notes": ""},
    {"id": "dragon", "jurisdiction": "CN", "privacy_tags": ["cn-may-train"], "confidence": "W", "notes": ""},
    {"id": "mystery", "jurisdiction": "unknown", "privacy_tags": ["unknown"], "confidence": "?", "notes": ""}
  ]
}"#;

/// Dzień, w którym fixture jest świeży.
pub fn fresh_day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default()
}

/// Dzień, w którym fixture jest nieświeży (> 30 dni).
pub fn stale_day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 1).unwrap_or_default()
}

/// Rejestr-fixture sparsowany.
pub fn fixture_registry() -> Registry {
    Registry::from_json(FIXTURE_REGISTRY).unwrap_or_else(|e| panic!("fixture: {e}"))
}

/// Wpisy katalogu-fixture: `acme` (zielony) i `mystery` (niezweryfikowany, tagi nieznane).
pub fn fixture_catalog() -> Vec<ProviderPolicyInput> {
    let tags = |t: PrivacyTag, j: &str| RouteTags {
        privacy: [t].into(),
        jurisdiction: j.parse().unwrap_or_default(),
    };
    vec![
        ProviderPolicyInput {
            provider: "acme".into(),
            tags: tags(PrivacyTag::Eu, "EU"),
            api_status: ProviderApiStatus::Green,
        },
        ProviderPolicyInput {
            provider: "mystery".into(),
            tags: tags(PrivacyTag::Unknown, "unknown"),
            api_status: ProviderApiStatus::Unverified,
        },
    ]
}

fn id(s: &str) -> RouteId {
    RouteId::new(s).unwrap_or_else(|| panic!("id {s}"))
}

/// Zielona, świeża trasa jest dozwolona; widoki obejmują rejestr i trasy API z katalogu.
pub async fn green_route_allowed<C: Compliance>(c: &C) {
    let d = c.route_allowed(&id("green-cli"), SessionTag::Standard);
    assert!(d.allowed, "{d:?}");
    assert_eq!(d.reason, DecisionReason::Allowed);
    assert!(
        c.route_allowed(&id("green-cli"), SessionTag::Private)
            .allowed
    );
    assert_eq!(c.views().len(), 6);
    assert!(c.route(&id("green-cli")).is_some());
    assert!(c.route(&id("acme.api")).is_none());
    assert!(c.view(&id("acme.api")).is_some());
}

/// Nieświeży rejestr degraduje zieloną trasę do szarej i ją wyłącza; jawne włączenie → ostrzeżenie.
pub async fn stale_route_degrades<C: Compliance>(c: &C) {
    let g = id("green-cli");
    let eff = c
        .effective_status(&g)
        .unwrap_or_else(|| panic!("brak trasy"));
    assert_eq!(eff.status, RouteStatus::Grey);
    assert!(eff.stale);
    let d = c.route_allowed(&g, SessionTag::Standard);
    assert_eq!(d.reason, DecisionReason::Disabled { stale: true });
    c.set_enabled(&g, true, ChangeOrigin::User)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let d = c.route_allowed(&g, SessionTag::Standard);
    assert!(d.allowed);
    assert!(matches!(
        d.reason,
        DecisionReason::AllowedWithWarning { stale: true, .. }
    ));
}

/// Szara trasa jest domyślnie wyłączona; po włączeniu działa z ostrzeżeniem.
pub async fn grey_route_needs_enable<C: Compliance>(c: &C) {
    let g = id("grey-cli");
    assert_eq!(
        c.route_allowed(&g, SessionTag::Standard).reason,
        DecisionReason::Disabled { stale: false }
    );
    let view = c
        .set_enabled(&g, true, ChangeOrigin::Broker)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(view.enabled);
    let d = c.route_allowed(&g, SessionTag::Standard);
    assert!(d.allowed && matches!(d.reason, DecisionReason::AllowedWithWarning { .. }));
}

/// Zabronionej trasy nie da się włączyć (ani użytkownik, ani Broker).
pub async fn forbidden_route_never_enabled<C: Compliance>(c: &C) {
    let b = id("banned-cli");
    for origin in [ChangeOrigin::User, ChangeOrigin::Broker] {
        assert_eq!(
            c.set_enabled(&b, true, origin).await,
            Err(ComplianceError::ForbiddenRoute(b.clone()))
        );
    }
    assert_eq!(
        c.route_allowed(&b, SessionTag::Standard).reason,
        DecisionReason::Forbidden
    );
}

/// Kryterium F1-13: wyłączona trasa — 0 zgód w 100 próbach.
pub async fn disabled_route_zero_calls<C: Compliance>(c: &C) {
    let g = id("green-cli");
    c.set_enabled(&g, false, ChangeOrigin::User)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let allowed = (0..100)
        .filter(|i| {
            let tag = if i % 2 == 0 {
                SessionTag::Standard
            } else {
                SessionTag::Private
            };
            c.route_allowed(&g, tag).allowed
        })
        .count();
    assert_eq!(allowed, 0);
}

/// Agentka, Ulepszacz i inne moduły nie przestawiają wyłączników.
pub async fn agents_cannot_toggle<C: Compliance>(c: &C) {
    let g = id("green-cli");
    for origin in [
        ChangeOrigin::Agent("alfa".into()),
        ChangeOrigin::Improver,
        ChangeOrigin::Module("router".into()),
    ] {
        assert!(matches!(
            c.set_enabled(&g, false, origin).await,
            Err(ComplianceError::NotPermitted(_))
        ));
    }
    assert!(c.route_allowed(&g, SessionTag::Standard).allowed);
    assert!(matches!(
        c.set_enabled(&id("nope"), true, ChangeOrigin::User).await,
        Err(ComplianceError::UnknownRoute(_))
    ));
}

/// Sesja prywatna: bez CN, bez „może trenować”, bez nieznanych tagów.
pub async fn private_session_policy<C: Compliance>(c: &C) {
    let cn = id("cn-cli");
    assert!(c.route_allowed(&cn, SessionTag::Standard).allowed);
    assert_eq!(
        c.route_allowed(&cn, SessionTag::Private).reason,
        DecisionReason::PrivateJurisdiction {
            jurisdiction: "CN".into()
        }
    );
    let mystery = id("mystery.api");
    let d = c.route_allowed(&mystery, SessionTag::Standard);
    assert!(d.allowed);
    assert_eq!(
        d.reason,
        DecisionReason::AllowedWithWarning {
            stale: false,
            unverified: true
        }
    );
    assert_eq!(
        c.route_allowed(&mystery, SessionTag::Private).reason,
        DecisionReason::PrivateUnknownPrivacy
    );
    assert!(
        c.route_allowed(&id("acme.api"), SessionTag::Private)
            .allowed
    );
    assert_eq!(
        c.route_allowed(&id("nope"), SessionTag::Standard).reason,
        DecisionReason::UnknownRoute
    );
}

/// Deny-listy: ścieżki poświadczeń i domeny webowych UI.
pub async fn deny_lists_basics<C: Compliance>(c: &C) {
    for p in [
        r"%USERPROFILE%\.codex\auth.json",
        r"C:\Users\Someone\.CLAUDE\credentials.json",
        r"\\?\C:\Users\x\AppData\Local\Google\Chrome\User Data\Default\Cookies",
        "D:/backup/AppData/Roaming/Mozilla/Firefox/Profiles/abc/logins.json",
    ] {
        assert!(c.is_denied_path(p), "{p}");
    }
    for p in [
        r"C:\projects\claude-notes.md",
        r"C:\Users\x\Documents\codex.txt",
    ] {
        assert!(!c.is_denied_path(p), "{p}");
    }
    assert!(c.is_denied_domain("https://www.claude.ai/chat/1"));
    assert!(c.is_denied_domain("ChatGPT.com."));
    assert!(!c.is_denied_domain("api.anthropic.com"));
}

/// Podmiana deny-list wymaga dowodu Brokera i nie może usunąć segmentów obowiązkowych.
pub async fn deny_list_update_rules<C: Compliance>(c: &C) {
    let authority = KernelAuthority::__broker_only();
    let mut broken = c.deny_lists();
    broken.path_segments.retain(|s| s != ".codex");
    assert!(matches!(
        c.replace_deny_lists(&authority, broken).await,
        Err(ComplianceError::InvalidDenyList(_))
    ));
    assert!(c.is_denied_path(r"C:\Users\u\.codex"));
    let mut next = DenyLists::baseline();
    next.version = 2;
    next.domains.push("chat.example.org".into());
    c.replace_deny_lists(&authority, next)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(c.deny_lists().version, 2);
    assert!(c.is_denied_domain("https://chat.example.org/x"));
}

/// Uruchamia cały zestaw; `factory(rejestr, katalog, dziś)` daje świeżą instancję.
pub async fn run_all<C, F, Fut>(factory: F)
where
    C: Compliance,
    F: Fn(Registry, Vec<ProviderPolicyInput>, NaiveDate) -> Fut,
    Fut: Future<Output = C>,
{
    let fresh = || factory(fixture_registry(), fixture_catalog(), fresh_day());
    green_route_allowed(&fresh().await).await;
    stale_route_degrades(&factory(fixture_registry(), fixture_catalog(), stale_day()).await).await;
    grey_route_needs_enable(&fresh().await).await;
    forbidden_route_never_enabled(&fresh().await).await;
    disabled_route_zero_calls(&fresh().await).await;
    agents_cannot_toggle(&fresh().await).await;
    private_session_policy(&fresh().await).await;
    deny_lists_basics(&fresh().await).await;
    deny_list_update_rules(&fresh().await).await;
}
