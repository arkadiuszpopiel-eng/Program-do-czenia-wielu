//! `tools-vision` — OCR na wirtualnym pulpicie i atrapach: kontrakt, maskowanie przed OCR (bajt
//! w bajt ten sam zamaskowany obraz), współrzędne ekranu, taint, okna chronione, pliki (deny-lista
//! przed Brokerem, bomba dekompresyjna, nie-obraz, limit), błędy OCR, odmowa Brokera, zdarzenia bez treści.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{config, ctx, env, env_with};
use platform_contract::{MASK_COLOR, encode_png};
use platform_fake::PASSWORD_COLOR;
use safety_broker_contract::{Broker, TaintSource};
use safety_broker_fake::ScriptedDecision;
use serde_json::json;
use tools_common_contract::{DenialReason, ToolErrorKind, ToolStatus, Toolset};
use tools_vision_contract::{OcrError, VisionToolsConfig};
use tools_vision_fake::FakeDescriber;

#[tokio::test]
async fn contract_suite() {
    let e = env();
    tools_vision_contract::contract_tests::run_all(&e.tools.tools()).await;
}

#[tokio::test]
async fn ocr_sees_only_the_masked_capture_and_maps_to_screen() {
    let e = env();
    let out = e
        .tool("vision_ocr")
        .call(json!({"source": "screen", "target": "monitor"}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.untrusted, Some(TaintSource::Screen));
    let masked = e.desktop.last_capture().unwrap();
    assert!(
        masked.pixels.chunks_exact(4).all(|p| p != PASSWORD_COLOR),
        "pole hasła zamaskowane"
    );
    assert_eq!(
        masked.pixel(1500, 300),
        Some(MASK_COLOR),
        "okno Alfy zamaskowane"
    );
    let seen = e.ocr.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].image,
        encode_png(&masked),
        "OCR dostał dokładnie zamaskowany obraz"
    );
    let scale = out.data["scale"].as_f64().unwrap();
    assert!(scale > 0.0);
    let first = &out.data["lines"][0];
    assert_eq!(first["text"], "Plik Edycja Widok");
    assert_eq!(first["x"].as_i64().unwrap(), (10.0 * scale).round() as i64);
    assert!(
        !out.text.contains("sk-ant-api03"),
        "sekrety zredagowane: {}",
        out.text
    );
    assert!(out.text.contains("niezaufane"));
    assert!(e.broker.session_security(&"s1".into()).tainted);
    let events = e.events();
    assert!(events.contains("\"lines\":2"), "{events}");
    assert!(
        !events.contains("Plik Edycja") && !events.contains("iVBOR"),
        "bez treści w zdarzeniach"
    );
}

#[tokio::test]
async fn protected_window_is_refused_before_capture() {
    let e = env();
    for name in ["vision_ocr", "vision_describe"] {
        let out = e
            .tool(name)
            .call(
                json!({"source": "screen", "target": "window", "window": e.alfa.0}),
                &ctx(),
            )
            .await;
        assert!(
            matches!(
                out.status,
                ToolStatus::Denied {
                    reason: DenialReason::KernelBlock { .. }
                }
            ),
            "{name}: {out:?}"
        );
    }
    assert!(e.ocr.requests().is_empty() && e.describer.requests().is_empty());
    let ok = e
        .tool("vision_ocr")
        .call(json!({"source": "screen", "target": "window", "window": e.notepad.0, "language": "en-US"}), &ctx())
        .await;
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(e.ocr.requests()[0].language.as_deref(), Some("en-US"));
}

#[tokio::test]
async fn files_are_checked_before_decoding() {
    let e = env();
    let ocr = e.tool("vision_ocr");
    let ok = ocr
        .call(json!({"source": "file", "path": "zrzut.png"}), &ctx())
        .await;
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(ok.untrusted, Some(TaintSource::File));
    assert_eq!(
        (ok.data["width"].as_u64(), ok.data["scale"].as_f64()),
        (Some(800), Some(1.0))
    );
    let denied = ocr
        .call(
            json!({"source": "file", "path": "/Users/ala/.ssh/klucz.png"}),
            &ctx(),
        )
        .await;
    assert_eq!(
        denied.status,
        ToolStatus::Denied {
            reason: DenialReason::DenyList
        }
    );
    for (path, kind) in [
        ("bomba.png", ToolErrorKind::InvalidArgs),
        ("notatka.txt", ToolErrorKind::InvalidArgs),
        ("film.mp4", ToolErrorKind::InvalidArgs),
        ("brak.png", ToolErrorKind::NotFound),
    ] {
        let out = ocr
            .call(json!({"source": "file", "path": path}), &ctx())
            .await;
        assert_eq!(
            out.status,
            ToolStatus::Failed { error: kind },
            "{path}: {out:?}"
        );
    }
    assert_eq!(
        e.ocr.requests().len(),
        1,
        "tylko poprawny obraz trafił do OCR"
    );
    let small = env_with(
        VisionToolsConfig {
            max_file_bytes: 20,
            ..config()
        },
        FakeDescriber::new(true, true),
    );
    let big = small
        .tool("vision_ocr")
        .call(json!({"source": "file", "path": "zrzut.png"}), &ctx())
        .await;
    assert_eq!(
        big.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
}

#[tokio::test]
async fn ocr_errors_are_readable() {
    let e = env();
    e.ocr.push(Err(OcrError::Unsupported("Linux".into())));
    let out = e
        .tool("vision_ocr")
        .call(json!({"source": "file", "path": "zrzut.png"}), &ctx())
        .await;
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        }
    );
    let lang = e
        .tool("vision_ocr")
        .call(
            json!({"source": "file", "path": "zrzut.png", "language": "de"}),
            &ctx(),
        )
        .await;
    assert!(lang.text.contains("pl, en-US"), "{}", lang.text);
}

#[tokio::test]
async fn broker_denial_stops_before_capture() {
    let e = env();
    e.broker
        .script("tools-vision.ocr", ScriptedDecision::NeedsApproval);
    let out = e
        .tool("vision_ocr")
        .call(json!({"source": "screen"}), &ctx())
        .await;
    assert!(matches!(out.status, ToolStatus::Denied { .. }), "{out:?}");
    assert!(e.desktop.last_capture().is_none() && e.ocr.requests().is_empty());
}
