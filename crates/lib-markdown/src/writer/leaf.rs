//! Elementy writera z własną logiką: komórki tabel, linki, bloki kodu, bloki HTML, obrazy.

use pulldown_cmark::{Alignment, LinkType};

use super::{Writer, escape_into};
use crate::CodeBlockMeta;
use crate::url::{safe_image_src, safe_link_href};

impl Writer<'_> {
    pub(super) fn start_cell(&mut self) {
        let align = match self.table.aligns.get(self.table.cell) {
            Some(Alignment::Left) => " data-align=\"left\"",
            Some(Alignment::Center) => " data-align=\"center\"",
            Some(Alignment::Right) => " data-align=\"right\"",
            Some(Alignment::None) | None => "",
        };
        let tag = if self.table.in_head { "th" } else { "td" };
        self.push(&format!("<{tag}{align}>"));
    }

    pub(super) fn start_link(&mut self, link_type: LinkType, dest: &str, title: &str) {
        let href = if link_type == LinkType::Email {
            safe_link_href(&format!("mailto:{dest}"))
        } else {
            safe_link_href(dest)
        };
        // Link o niedozwolonym schemacie: zostaje sam tekst.
        let Some(href) = href.filter(|_| self.links.is_empty()) else {
            self.flush_text();
            self.links.push(false);
            return;
        };
        self.push("<a href=\"");
        escape_into(&mut self.out, &href);
        self.out.push('"');
        if !title.is_empty() {
            self.out.push_str(" title=\"");
            escape_into(&mut self.out, title);
            self.out.push('"');
        }
        self.out.push('>');
        self.links.push(true);
    }

    pub(super) fn finish_code(&mut self) {
        let Some((lang, text)) = self.code.take() else {
            return;
        };
        let index = self.code_blocks.len();
        let lines = text.lines().count();
        self.out.push_str(&format!(
            "<pre data-code=\"{index}\" data-lines=\"{lines}\"><code"
        ));
        if let Some(lang) = &lang {
            self.out.push_str(" data-lang=\"");
            escape_into(&mut self.out, lang);
            self.out.push('"');
        }
        self.out.push('>');
        escape_into(&mut self.out, &text);
        self.out.push_str("</code></pre>\n");
        self.code_blocks.push(CodeBlockMeta {
            index,
            lang,
            lines,
            text,
        });
    }

    /// Blok surowego HTML → akapit z escapowanym tekstem (linie zachowane przez `<br>`).
    pub(super) fn finish_html_block(&mut self) {
        let Some(raw) = self.html_block.take() else {
            return;
        };
        self.out.push_str("<p>");
        for (i, line) in raw.trim_end_matches(['\n', '\r']).lines().enumerate() {
            if i > 0 {
                self.out.push_str("<br>\n");
            }
            escape_into(&mut self.out, line);
        }
        self.out.push_str("</p>\n");
    }

    pub(super) fn finish_image(&mut self) {
        let Some(img) = self.image.take() else {
            return;
        };
        let label = if img.alt.trim().is_empty() {
            "obraz".to_owned()
        } else {
            format!("obraz: {}", img.alt)
        };
        if let Some(src) = safe_image_src(&img.dest, self.opts) {
            self.out.push_str("<img src=\"");
            escape_into(&mut self.out, &src);
            self.out.push_str("\" alt=\"");
            escape_into(&mut self.out, &img.alt);
            self.out.push('"');
            if !img.title.is_empty() {
                self.out.push_str(" title=\"");
                escape_into(&mut self.out, &img.title);
                self.out.push('"');
            }
            self.out.push('>');
        } else if let Some(href) = safe_link_href(&img.dest).filter(|_| self.links.is_empty()) {
            // Obraz niedozwolony do automatycznego ładowania: link do kliknięcia.
            self.out.push_str("<a href=\"");
            escape_into(&mut self.out, &href);
            self.out.push_str("\">");
            escape_into(&mut self.out, &label);
            self.out.push_str("</a>");
        } else {
            escape_into(&mut self.out, &label);
        }
    }
}
