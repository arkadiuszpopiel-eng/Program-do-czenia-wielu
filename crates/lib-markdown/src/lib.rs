//! `lib-markdown` — Markdown z LLM → bezpieczny HTML renderowany w Rust (ADR 0009, THREAT_MODEL S12).
//!
//! Potok: `pulldown-cmark` (CommonMark + GFM: tabele, listy zadań, przekreślenia; autolinki
//! „gołych” adresów) → własny writer (surowy HTML escapowany, linki tylko `http(s)`/`mailto`,
//! obrazy wyłączone domyślnie) → `ammonia` z białą listą. UI tylko wstawia wynik do DOM.
//!
//! - [`render`] / [`render_with`] / [`render_blocks`] — cały dokument,
//! - [`IncrementalRenderer`] — strumień: zamknięte bloki (stabilne) + bieżący otwarty,
//! - [`to_spoken_text`] — kanał mówiony (PLAN §6.7).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod blocks;
mod sanitize;
mod spoken;
mod url;
mod writer;

use serde::{Deserialize, Serialize};

pub use blocks::{Block, IncrementalRenderer, StreamUpdate, render_blocks};
pub use sanitize::{ALLOWED_TAGS, LINK_REL};
pub use spoken::{MAX_SPOKEN_INLINE_CODE, SPOKEN_CODE, SPOKEN_TABLE, to_spoken_text};
pub use url::{safe_image_src, safe_link_href};

/// Opcje renderowania. Domyślnie najbezpieczniej: żadnych obrazów ładowanych automatycznie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderOptions {
    /// Zezwala na obrazy `data:image/(png|jpeg|webp);base64,…` (bez SVG). Domyślnie `false`.
    #[serde(default)]
    pub allow_data_images: bool,
    /// Zezwala na zdalne obrazy `https://…`. Domyślnie `false` — adres obrazka może wynieść
    /// dane (prompt injection), więc zamiast `<img>` pokazujemy link „obraz: …” do kliknięcia.
    #[serde(default)]
    pub allow_remote_images: bool,
}

/// Metadane bloku kodu dla akcji „kopiuj / zapisz jako plik / uruchom w terminalu” (PLAN §14.8).
/// Podświetlanie składni robi UI w Web Workerze po zamknięciu bloku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBlockMeta {
    /// Indeks bloku kodu w obrębie bloku najwyższego poziomu (atrybut `data-code` na `<pre>`).
    pub index: usize,
    /// Język z info-stringu (znormalizowany, `[a-z0-9_+#.-]`, ≤ 32 znaki), jeśli podany.
    pub lang: Option<String>,
    /// Liczba linii kodu.
    pub lines: usize,
    /// Surowa treść kodu (nieescapowana) — do schowka / zapisu.
    pub text: String,
}

/// Renderuje Markdown do bezpiecznego HTML z opcjami domyślnymi.
pub fn render(md: &str) -> String {
    render_with(md, RenderOptions::default())
}

/// Renderuje Markdown do bezpiecznego HTML; wynik = złożenie bloków z [`render_blocks`].
pub fn render_with(md: &str, opts: RenderOptions) -> String {
    blocks::render_document(md, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gfm_features() {
        let html = render(
            "# Tytuł\n\n~~stare~~ **nowe** _k_\n\n- [x] zrobione\n- [ ] do zrobienia\n\n\
             | L | Ś | P |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n\nzobacz https://example.com.\n",
        );
        assert!(html.contains("<h1>Tytuł</h1>"), "{html}");
        assert!(html.contains("<del>stare</del> <strong>nowe</strong> <em>k</em>"));
        assert!(
            html.contains("<input type=\"checkbox\" disabled=\"\" checked=\"\">"),
            "{html}"
        );
        assert!(html.contains("<th data-align=\"left\">L</th><th data-align=\"center\">Ś</th>"));
        assert!(html.contains(
            "<a href=\"https://example.com\" data-external=\"\" rel=\"noopener noreferrer nofollow\">https://example.com</a>."
        ));
    }

    #[test]
    fn raw_html_is_escaped_not_rendered() {
        assert_eq!(render("a <b>x</b>"), "<p>a &lt;b&gt;x&lt;/b&gt;</p>\n");
        assert_eq!(
            render("<div onclick=\"x\">\nhej\n</div>"),
            "<p>&lt;div onclick=\"x\"&gt;<br>\nhej<br>\n&lt;/div&gt;</p>\n"
        );
    }

    #[test]
    fn code_block_metadata() {
        let blocks = render_blocks(
            "```Python\nprint(1)\nprint(2)\n```\n",
            RenderOptions::default(),
        );
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].html,
            "<pre data-code=\"0\" data-lines=\"2\"><code data-lang=\"python\">print(1)\nprint(2)\n</code></pre>\n"
        );
        let meta = &blocks[0].code[0];
        assert_eq!(meta.lang.as_deref(), Some("python"));
        assert_eq!(
            (meta.lines, meta.text.as_str()),
            (2, "print(1)\nprint(2)\n")
        );
    }

    #[test]
    fn images_default_to_links() {
        assert_eq!(
            render("![wykres](https://a.pl/w.png)"),
            "<p><a href=\"https://a.pl/w.png\" data-external=\"\" rel=\"noopener noreferrer nofollow\">obraz: wykres</a></p>\n"
        );
        let png = "![x](data:image/png;base64,iVBORw0KGgo=)";
        assert_eq!(render(png), "<p>obraz: x</p>\n");
        let allowed = RenderOptions {
            allow_data_images: true,
            ..RenderOptions::default()
        };
        assert_eq!(
            render_with(png, allowed),
            "<p><img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"x\"></p>\n"
        );
    }
}
