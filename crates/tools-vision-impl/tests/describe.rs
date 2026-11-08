//! `tools-vision` — opis obrazu: model dostaje zamaskowany zrzut, prywatność sesji (prywatna
//! i nieznana — tylko lokalnie, bez modelu lokalnego odmowa), formaty dla modelu, anulowanie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{config, ctx, env, env_with};
use platform_contract::encode_png;
use safety_broker_contract::{Holder, TaintSource};
use serde_json::json;
use tools_common_contract::{DenialReason, ToolErrorKind, ToolStatus};
use tools_vision_contract::VisionPrivacy;
use tools_vision_fake::FakeDescriber;

#[tokio::test]
async fn describe_sends_masked_image_and_respects_privacy() {
    let e = env();
    let d = e.tool("vision_describe");
    let out = d
        .call(
            json!({"source": "screen", "question": "Jaki przycisk jest aktywny?"}),
            &ctx(),
        )
        .await;
    assert!(out.is_ok(), "{out:?}");
    let sent = e.describer.requests();
    assert_eq!(
        sent[0].image,
        encode_png(&e.desktop.last_capture().unwrap())
    );
    assert_eq!(sent[0].privacy, VisionPrivacy::Normal);
    assert_eq!(
        sent[0].question.as_deref(),
        Some("Jaki przycisk jest aktywny?")
    );
    assert_eq!(out.untrusted, Some(TaintSource::Screen));
    assert_eq!(out.data["local"], false);
    // Sesja prywatna: tylko lokalnie.
    e.privacy.set("s1", VisionPrivacy::LocalOnly);
    let local = d
        .call(json!({"source": "file", "path": "zrzut.png"}), &ctx())
        .await;
    assert!(local.is_ok() && local.data["local"] == true, "{local:?}");
    assert_eq!(e.describer.requests()[1].privacy, VisionPrivacy::LocalOnly);
    // Bez modelu lokalnego — odmowa, obraz nie wychodzi.
    let cloud_only = env_with(config(), FakeDescriber::new(false, true));
    cloud_only.privacy.set("s1", VisionPrivacy::LocalOnly);
    let refused = cloud_only
        .tool("vision_describe")
        .call(json!({"source": "screen"}), &ctx())
        .await;
    assert_eq!(
        refused.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        },
        "{refused:?}"
    );
    assert!(cloud_only.describer.requests().is_empty());
    // Nieznana sesja = fail-closed (tylko lokalnie).
    let mut other = ctx();
    other.holder = Holder::agent("s-nieznana", "delta");
    let unknown = cloud_only
        .tool("vision_describe")
        .call(json!({"source": "screen"}), &other)
        .await;
    assert!(matches!(unknown.status, ToolStatus::Denied { .. }));
    // BMP nie trafia do modelu.
    let bmp = d
        .call(json!({"source": "file", "path": "stary.bmp"}), &ctx())
        .await;
    assert_eq!(
        bmp.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    assert!(!e.events().contains("Atrapa"), "opis nie trafia do zdarzeń");
}

#[tokio::test]
async fn cancelled_context_does_nothing() {
    let e = env();
    let c = ctx();
    c.cancel.cancel();
    let out = e
        .tool("vision_describe")
        .call(json!({"source": "screen"}), &c)
        .await;
    assert_eq!(out.status, ToolStatus::Cancelled);
    assert!(e.desktop.last_capture().is_none());
}
