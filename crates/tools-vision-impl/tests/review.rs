//! Przegląd poprawności fali 3 (`docs/reviews/2026-10-wave3-review.md`, W3-06): współrzędne linii
//! OCR muszą trafić do modelu. Do modelu idzie wyłącznie `ToolOutcome::text`
//! (`agent-runtime-impl::prompt::tool_result`), a `data` (z `lines`) zostaje w aplikacji — przed
//! poprawką tekst zawierał same napisy i zdanie „Współrzędne linii w `lines`…”, więc agentka nie
//! mogła kliknąć znalezionego tekstu (SPEC: „linie ze współrzędnymi ekranu — do kliknięcia przez
//! `tools-input`”).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{ctx, env};
use serde_json::json;

#[tokio::test]
async fn ocr_text_for_the_model_carries_screen_coordinates() {
    let e = env();
    let out = e
        .tool("vision_ocr")
        .call(json!({"source": "screen", "target": "monitor"}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    let lines = out.data["lines"].as_array().unwrap();
    assert!(!lines.is_empty());
    for l in lines {
        let (x, y, w, h) = (&l["x"], &l["y"], &l["width"], &l["height"]);
        let shown = format!("[x={x} y={y} w={w} h={h}]");
        assert!(
            out.text.contains(&shown),
            "brak współrzędnych {shown} w tekście dla modelu:\n{}",
            out.text
        );
    }
    // Dane bez zmian: tekst linii osobno, sekrety zredagowane także w wersji ze współrzędnymi.
    assert_eq!(lines[0]["text"], "Plik Edycja Widok");
    assert!(!out.text.contains("sk-ant-api03"), "{}", out.text);
}
