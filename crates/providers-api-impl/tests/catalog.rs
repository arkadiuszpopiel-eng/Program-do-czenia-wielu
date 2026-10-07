//! Adapter generyczny z prawdziwych wpisów `providers-catalog/*.toml` (bez zmian w kodzie).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Arc;

use futures_util::StreamExt;
use providers_api_impl::{
    AccountProfile, CatalogCompat, CatalogEntry, ConfigError, build_provider,
};
use providers_contract::{
    CancellationToken, ChatRequest, InterruptionRendering, Message, ModelProvider, PrivacyTag,
    ProviderErrorKind, ProviderEvent, StaticKey, StopReason,
};
use support::{FixtureServer, Reply, anthropic_sse, openai_sse};

fn catalog_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers-catalog")
}

fn entry(id: &str) -> CatalogEntry {
    let text = std::fs::read_to_string(catalog_dir().join(format!("{id}.toml"))).unwrap();
    CatalogEntry::parse_toml(&text).unwrap()
}

fn account(server: Option<&FixtureServer>, model: &str) -> AccountProfile {
    let mut a = AccountProfile::new(Arc::new(StaticKey::new("key-123")));
    a.base_url = server.map(FixtureServer::url);
    a.default_model = Some(model.into());
    a
}

#[test]
fn every_catalog_entry_parses() {
    let mut n = 0;
    for f in std::fs::read_dir(catalog_dir()).unwrap() {
        let path = f.unwrap().path();
        if path.extension().is_some_and(|e| e == "toml") {
            let e = CatalogEntry::parse_toml(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            assert_eq!(
                Some(e.id.as_str()),
                path.file_stem().and_then(|s| s.to_str())
            );
            assert!(e.pricing.is_empty(), "cennik nie w katalogu: {}", e.id);
            n += 1;
        }
    }
    assert!(n >= 17, "{n}");
}

#[test]
fn builder_rules() {
    // Katalog bez adresu (wpis-szkic) → konto musi podać adres.
    let deepseek = entry("deepseek");
    assert!(matches!(
        build_provider(&deepseek, account(None, "deepseek-chat")),
        Err(ConfigError::MissingBaseUrl(_))
    ));
    // Usługi głosowe nie są czatem.
    assert!(matches!(
        build_provider(&entry("elevenlabs"), account(None, "x")),
        Err(ConfigError::Unsupported(..))
    ));
    // Natywne z domyślnym endpointem.
    let anthropic = build_provider(&entry("anthropic"), account(None, "claude-opus-5-5")).unwrap();
    assert_eq!(anthropic.id().as_str(), "anthropic");
    assert_eq!(
        anthropic.capabilities().interruption,
        InterruptionRendering::AppendNote
    );
    let openai = build_provider(&entry("openai"), account(None, "gpt-6-sol")).unwrap();
    assert_eq!(openai.id().as_str(), "openai");
    // xAI: adres z katalogu, prywatność z katalogu.
    let xai_entry = entry("xai");
    let xai = build_provider(&xai_entry, account(None, "grok-5")).unwrap();
    let caps = xai.capabilities();
    assert_eq!(caps.privacy.tag, xai_entry.privacy_tag);
    assert_eq!(
        caps.models.get("grok-5"),
        Some(&xai_entry.default_capabilities()),
        "możliwości z katalogu dla modelu domyślnego"
    );
    let mut forbidden = entry("xai");
    forbidden.compliance_status = "forbidden".into();
    assert!(matches!(
        build_provider(&forbidden, account(None, "grok-5")),
        Err(ConfigError::Forbidden(_))
    ));
    assert_eq!(
        entry("custom-anthropic-compatible").compat,
        CatalogCompat::Anthropic
    );
}

async fn run(p: &Arc<dyn ModelProvider>, req: ChatRequest) -> Vec<ProviderEvent> {
    p.stream(req, CancellationToken::new()).collect().await
}

#[tokio::test]
async fn openai_compatible_entry_streams_via_generic_adapter() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(openai_sse::chat_text(
        "deepseek-chat",
        &["Cześć"],
        "stop",
    )));
    let p = build_provider(&entry("deepseek"), account(Some(&server), "deepseek-chat")).unwrap();
    let events = run(
        &p,
        ChatRequest::new("deepseek-chat", vec![Message::user_text("hej")]),
    )
    .await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.path, "/chat/completions");
    assert_eq!(rec.header("authorization"), Some("Bearer key-123"));
    assert!(rec.json().get("max_tokens").is_some());

    // DeepSeek = CN / „może trenować": sesja prywatna nie wychodzi do sieci.
    let mut private = ChatRequest::new("deepseek-chat", vec![Message::user_text("tajne")]);
    private.meta.privacy.tag = PrivacyTag::Private;
    let events = run(&p, private).await;
    assert!(
        matches!(events.as_slice(), [ProviderEvent::Error(e)] if e.kind == ProviderErrorKind::PrivacyBlocked)
    );
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn anthropic_compatible_entry_uses_messages_api_without_betas() {
    let server = FixtureServer::start().await;
    server.reset(Reply::sse(anthropic_sse::text("glm-5", &["ok"])));
    let mut e = entry("custom-anthropic-compatible");
    e.privacy_tag = "sg".into();
    e.jurisdiction = "SG".into();
    let p = build_provider(&e, account(Some(&server), "glm-5")).unwrap();
    let events = run(
        &p,
        ChatRequest::new("glm-5", vec![Message::user_text("hej")]),
    )
    .await;
    assert_eq!(
        events.last(),
        Some(&ProviderEvent::stop(StopReason::EndTurn))
    );
    let rec = server.requests().pop().unwrap();
    assert_eq!(rec.path, "/v1/messages");
    assert_eq!(rec.header("x-api-key"), Some("key-123"));
    assert!(rec.header("anthropic-beta").is_none());
}
