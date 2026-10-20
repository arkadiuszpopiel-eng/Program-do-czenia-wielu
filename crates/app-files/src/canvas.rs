//! Kanwa artefaktów (C2.8, ADR 0009): podgląd artefaktu z rozmowy obok czatu.
//!
//! Markdown → bezpieczny HTML jak w wiadomościach (`lib_markdown::render`, UI wstawia go jako
//! zwykły DOM). HTML i SVG → pełny dokument po sanitizacji (`lib_markdown::artifact_document`),
//! który UI wstawia wyłącznie do ramki `sandbox` bez skryptów i bez `allow-same-origin`.

use app_api::dto::CanvasKind;
use app_api::error::AppError;

pub use lib_markdown::ARTIFACT_MAX_BYTES;

/// `canvas_render` ⟶ HTML do podglądu w kanwie; źródło ponad limit albo puste — błąd z powodem.
/// `lang` — język interfejsu: dokument ramki go dostaje, gdy artefakt nie ma `<html lang>`.
pub fn render_canvas(kind: CanvasKind, source: &str, lang: &str) -> Result<String, AppError> {
    if source.trim().is_empty() {
        return Err(AppError::invalid("Artefakt jest pusty."));
    }
    if source.len() > ARTIFACT_MAX_BYTES {
        return Err(AppError::invalid(format!(
            "Artefakt jest za duży do podglądu w kanwie (limit {} KB).",
            ARTIFACT_MAX_BYTES / 1024
        )));
    }
    Ok(match kind {
        CanvasKind::Markdown => lib_markdown::render(source),
        CanvasKind::Html | CanvasKind::Svg => lib_markdown::artifact_document(source, lang),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_rendered_like_messages() {
        let html = render_canvas(CanvasKind::Markdown, "# Plan\n\n<b>x</b>", "pl").unwrap();
        assert_eq!(html, "<h1>Plan</h1>\n<p>&lt;b&gt;x&lt;/b&gt;</p>\n");
    }

    #[test]
    fn html_and_svg_become_sanitized_documents() {
        let html = render_canvas(
            CanvasKind::Html,
            "<h1 onclick=\"x\">A</h1><script>1</script>",
            "en",
        )
        .unwrap();
        assert!(
            html.starts_with("<!doctype html><html lang=\"en\">"),
            "{html}"
        );
        assert!(html.contains("<body><h1>A</h1></body>"), "{html}");
        let svg = render_canvas(
            CanvasKind::Svg,
            "<svg viewBox=\"0 0 1 1\"><rect/></svg>",
            "pl",
        )
        .unwrap();
        assert!(
            svg.contains("<svg viewBox=\"0 0 1 1\"><rect></rect></svg>"),
            "{svg}"
        );
    }

    #[test]
    fn empty_and_oversized_sources_are_refused() {
        assert!(render_canvas(CanvasKind::Html, "  \n", "pl").is_err());
        let big = "a".repeat(ARTIFACT_MAX_BYTES + 1);
        let err = render_canvas(CanvasKind::Markdown, &big, "pl").unwrap_err();
        assert!(err.message.contains("512 KB"), "{}", err.message);
        assert!(render_canvas(CanvasKind::Markdown, &big[1..], "pl").is_ok());
    }
}
