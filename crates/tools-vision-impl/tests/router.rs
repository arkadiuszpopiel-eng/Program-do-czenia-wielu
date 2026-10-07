//! `RouterDescriber` na atrapach dostawców: wybór Routera wg prywatności (prywatna sesja nigdy
//! nie trafia do Routera hybrydowego), postać żądania (obraz base64, prompt systemowy, tag
//! prywatności, sesja), składanie strumienia, błędy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use providers_contract::{
    CancellationToken, ContentBlock, ImageSource, ModelProvider, PrivacyTag, ProviderError,
    ProviderErrorKind,
};
use providers_fake::{FakeProvider, Script};
use tools_vision_contract::{DescribeError, DescribePort, DescribeRequest, VisionPrivacy};
use tools_vision_impl::RouterDescriber;

fn request(privacy: VisionPrivacy) -> DescribeRequest {
    DescribeRequest {
        image: vec![0x89, b'P', b'N', b'G'],
        media_type: "image/png".into(),
        question: Some("Co jest na ekranie?".into()),
        privacy,
        session: "s1".into(),
        max_tokens: 300,
    }
}

fn providers() -> (Arc<FakeProvider>, Arc<FakeProvider>) {
    let normal = Arc::new(FakeProvider::new("router"));
    normal.push_script(Script::text("anthropic:claude", &["Okno ", "Notatnika."]));
    let local = Arc::new(FakeProvider::new("router-local"));
    local.push_script(Script::text("local:qwen-vl", &["Lokalny ", "opis."]));
    (normal, local)
}

fn describer(
    normal: Option<&Arc<FakeProvider>>,
    local: Option<&Arc<FakeProvider>>,
) -> RouterDescriber {
    RouterDescriber::new(
        normal.map(|p| p.clone() as Arc<dyn ModelProvider>),
        local.map(|p| p.clone() as Arc<dyn ModelProvider>),
        "auto",
    )
}

#[tokio::test]
async fn normal_session_goes_through_hybrid_router() {
    let (normal, local) = providers();
    let d = describer(Some(&normal), Some(&local));
    let out = d
        .describe(request(VisionPrivacy::Normal), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(out.text, "Okno Notatnika.");
    assert_eq!(out.model, "anthropic:claude");
    assert!(!out.local);
    let sent = normal.requests();
    assert_eq!(sent.len(), 1);
    assert!(local.requests().is_empty());
    let req = &sent[0];
    assert_eq!(req.model, "auto");
    assert_eq!(req.meta.privacy.tag, PrivacyTag::Normal);
    assert_eq!(req.meta.session.as_deref(), Some("s1"));
    assert_eq!(req.params.max_tokens, Some(300));
    assert!(req.system.as_deref().unwrap().contains("nigdy"));
    match &req.messages[0].content[0] {
        ContentBlock::Image {
            source: ImageSource::Base64 { media_type, data },
        } => {
            assert_eq!(media_type, "image/png");
            assert_eq!(data, "iVBORw==");
        }
        other => panic!("oczekiwano obrazu: {other:?}"),
    }
    assert!(format!("{:?}", req.messages[0].content[1]).contains("Co jest na ekranie?"));
}

#[tokio::test]
async fn private_session_never_reaches_the_hybrid_router() {
    let (normal, local) = providers();
    let d = describer(Some(&normal), Some(&local));
    let out = d
        .describe(request(VisionPrivacy::LocalOnly), CancellationToken::new())
        .await
        .unwrap();
    assert!(out.local);
    assert_eq!(out.text, "Lokalny opis.");
    assert_eq!(local.requests()[0].meta.privacy.tag, PrivacyTag::Private);
    assert!(normal.requests().is_empty());
    let no_local = describer(Some(&normal), None);
    assert_eq!(
        no_local
            .describe(request(VisionPrivacy::LocalOnly), CancellationToken::new())
            .await,
        Err(DescribeError::PrivateNoLocal)
    );
    assert!(normal.requests().is_empty(), "obraz nie wyszedł do chmury");
    let nothing = describer(None, None);
    assert!(matches!(
        nothing
            .describe(request(VisionPrivacy::Normal), CancellationToken::new())
            .await,
        Err(DescribeError::NoVisionModel(_))
    ));
    let only_local = describer(None, Some(&local));
    let fallback = only_local
        .describe(request(VisionPrivacy::Normal), CancellationToken::new())
        .await;
    assert!(fallback.is_err() || fallback.unwrap().local);
}

#[tokio::test]
async fn provider_errors_and_empty_answers_are_errors() {
    let normal = Arc::new(FakeProvider::new("router"));
    normal.fail_next(ProviderError::new(
        ProviderErrorKind::Unsupported,
        "model bez wizji",
    ));
    let d = describer(Some(&normal), None);
    assert!(matches!(
        d.describe(request(VisionPrivacy::Normal), CancellationToken::new())
            .await,
        Err(DescribeError::NoVisionModel(_))
    ));
    normal.fail_next(ProviderError::new(ProviderErrorKind::Network, "reset"));
    assert!(matches!(
        d.describe(request(VisionPrivacy::Normal), CancellationToken::new())
            .await,
        Err(DescribeError::Provider(_))
    ));
    normal.push_script(Script::text("m", &["   "]));
    assert!(matches!(
        d.describe(request(VisionPrivacy::Normal), CancellationToken::new())
            .await,
        Err(DescribeError::Provider(_))
    ));
    let cancel = CancellationToken::new();
    cancel.cancel();
    normal.push_script(Script::text("m", &["x"]));
    assert!(
        d.describe(request(VisionPrivacy::Normal), cancel)
            .await
            .is_err()
    );
}
