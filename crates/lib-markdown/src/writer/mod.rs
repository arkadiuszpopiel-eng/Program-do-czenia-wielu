//! Własny writer zdarzeń `pulldown-cmark` → HTML (przed sanitizacją `ammonia`).
//!
//! Zasady: surowy HTML z Markdownu jest escapowany (widoczny jako tekst, nigdy renderowany);
//! linki tylko o bezpiecznym schemacie; obrazy wg `RenderOptions` (domyślnie zamiast `<img>` —
//! link „obraz: …”, żeby nic nie ładowało się samo); bloki kodu z escapowaną treścią i metadanymi.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Tag, TagEnd};

use crate::url::find_autolinks;
use crate::{CodeBlockMeta, RenderOptions};

mod leaf;

/// Maksymalna długość nazwy języka bloku kodu.
const MAX_LANG_LEN: usize = 32;

/// Escapuje tekst do HTML (treść i wartości atrybutów).
pub(crate) fn escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
}

/// Język bloku kodu z info-stringu: pierwsze słowo, tylko `[A-Za-z0-9_+#.-]`, ≤ 32 znaki.
pub(crate) fn code_lang(info: &str) -> Option<String> {
    let word = info.split_whitespace().next()?;
    let ok = word.len() <= MAX_LANG_LEN
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '#' | '.' | '-'));
    ok.then(|| word.to_ascii_lowercase())
}

struct ImageAcc {
    dest: String,
    title: String,
    alt: String,
    depth: usize,
}

#[derive(Default)]
struct TableState {
    aligns: Vec<Alignment>,
    cell: usize,
    in_head: bool,
    body_open: bool,
}

/// Stan writera jednego fragmentu (bloku najwyższego poziomu albo całego dokumentu).
pub(crate) struct Writer<'o> {
    opts: &'o RenderOptions,
    out: String,
    code_blocks: Vec<CodeBlockMeta>,
    text: String,
    links: Vec<bool>,
    code: Option<(Option<String>, String)>,
    image: Option<ImageAcc>,
    html_block: Option<String>,
    table: TableState,
    metadata: bool,
}

impl<'o> Writer<'o> {
    fn new(opts: &'o RenderOptions) -> Self {
        Self {
            opts,
            out: String::new(),
            code_blocks: Vec::new(),
            text: String::new(),
            links: Vec::new(),
            code: None,
            image: None,
            html_block: None,
            table: TableState::default(),
            metadata: false,
        }
    }

    fn push(&mut self, s: &str) {
        self.flush_text();
        self.out.push_str(s);
    }

    fn escaped(&mut self, s: &str) {
        self.flush_text();
        escape_into(&mut self.out, s);
    }

    /// Wypisuje zbuforowany tekst; poza linkami zamienia „gołe” adresy na linki.
    fn flush_text(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.text);
        if !self.links.is_empty() {
            escape_into(&mut self.out, &text);
            return;
        }
        let mut last = 0;
        for (range, href) in find_autolinks(&text) {
            escape_into(&mut self.out, &text[last..range.start]);
            self.out.push_str("<a href=\"");
            escape_into(&mut self.out, &href);
            self.out.push_str("\">");
            escape_into(&mut self.out, &text[range.clone()]);
            self.out.push_str("</a>");
            last = range.end;
        }
        escape_into(&mut self.out, &text[last..]);
    }

    fn event(&mut self, ev: Event<'_>) {
        if self.capture(&ev) {
            return;
        }
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.text.push_str(&t),
            Event::Code(t) | Event::InlineMath(t) | Event::DisplayMath(t) => {
                self.push("<code>");
                self.escaped(&t);
                self.push("</code>");
            }
            // Surowy HTML z modelu: escapowany, nigdy renderowany (ADR 0009).
            Event::Html(t) | Event::InlineHtml(t) => self.escaped(&t),
            Event::FootnoteReference(t) => {
                self.escaped("[^");
                self.escaped(&t);
                self.escaped("]");
            }
            Event::SoftBreak => self.text.push('\n'),
            Event::HardBreak => self.push("<br>\n"),
            Event::Rule => self.push("<hr>\n"),
            Event::TaskListMarker(checked) => self.push(if checked {
                "<input type=\"checkbox\" disabled=\"\" checked=\"\"> "
            } else {
                "<input type=\"checkbox\" disabled=\"\"> "
            }),
        }
    }

    /// Zbieranie treści bloku kodu, bloku HTML, tekstu alternatywnego obrazu i metadanych.
    /// Zwraca `true`, gdy zdarzenie zostało pochłonięte.
    fn capture(&mut self, ev: &Event<'_>) -> bool {
        if let Some((_, buf)) = self.code.as_mut() {
            match ev {
                Event::Text(t) => buf.push_str(t),
                Event::End(TagEnd::CodeBlock) => self.finish_code(),
                _ => {}
            }
            return true;
        }
        if let Some(buf) = self.html_block.as_mut() {
            match ev {
                Event::Html(t) | Event::Text(t) => buf.push_str(t),
                Event::End(TagEnd::HtmlBlock) => self.finish_html_block(),
                _ => {}
            }
            return true;
        }
        if let Some(img) = self.image.as_mut() {
            match ev {
                Event::Text(t) | Event::Code(t) => img.alt.push_str(t),
                Event::Start(Tag::Image { .. }) => img.depth += 1,
                Event::End(TagEnd::Image) if img.depth > 0 => img.depth -= 1,
                Event::End(TagEnd::Image) => self.finish_image(),
                _ => {}
            }
            return true;
        }
        if self.metadata {
            if matches!(ev, Event::End(TagEnd::MetadataBlock(_))) {
                self.metadata = false;
            }
            return true;
        }
        false
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.push("<p>"),
            Tag::Heading { level, .. } => self.push(&format!("<h{}>", level as usize)),
            Tag::BlockQuote(_) => self.push("<blockquote>\n"),
            Tag::CodeBlock(kind) => {
                self.flush_text();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => code_lang(&info),
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((lang, String::new()));
            }
            Tag::HtmlBlock => {
                self.flush_text();
                self.html_block = Some(String::new());
            }
            Tag::List(Some(1)) => self.push("<ol>\n"),
            Tag::List(Some(start)) => self.push(&format!("<ol start=\"{start}\">\n")),
            Tag::List(None) => self.push("<ul>\n"),
            Tag::Item => self.push("<li>"),
            Tag::Table(aligns) => {
                self.table = TableState {
                    aligns,
                    ..TableState::default()
                };
                self.push("<table>\n");
            }
            Tag::TableHead => {
                self.table.in_head = true;
                self.table.cell = 0;
                self.push("<thead><tr>");
            }
            Tag::TableRow => {
                self.table.cell = 0;
                if !self.table.body_open {
                    self.table.body_open = true;
                    self.push("<tbody>\n");
                }
                self.push("<tr>");
            }
            Tag::TableCell => self.start_cell(),
            Tag::Emphasis => self.push("<em>"),
            Tag::Strong => self.push("<strong>"),
            Tag::Strikethrough => self.push("<del>"),
            Tag::Superscript => self.push("<sup>"),
            Tag::Subscript => self.push("<sub>"),
            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            } => self.start_link(link_type, &dest_url, &title),
            Tag::Image {
                dest_url, title, ..
            } => {
                self.flush_text();
                self.image = Some(ImageAcc {
                    dest: dest_url.to_string(),
                    title: title.to_string(),
                    alt: String::new(),
                    depth: 0,
                });
            }
            Tag::MetadataBlock(_) => self.metadata = true,
            // Nieużywane rozszerzenia (wyłączone w opcjach parsera) — tylko treść.
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition => self.flush_text(),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.push("</p>\n"),
            TagEnd::Heading(level) => self.push(&format!("</h{}>\n", level as usize)),
            TagEnd::BlockQuote(_) => self.push("</blockquote>\n"),
            TagEnd::List(true) => self.push("</ol>\n"),
            TagEnd::List(false) => self.push("</ul>\n"),
            TagEnd::Item => self.push("</li>\n"),
            TagEnd::Table => {
                if self.table.body_open {
                    self.push("</tbody>\n");
                }
                self.push("</table>\n");
            }
            TagEnd::TableHead => {
                self.table.in_head = false;
                self.push("</tr></thead>\n");
            }
            TagEnd::TableRow => self.push("</tr>\n"),
            TagEnd::TableCell => {
                self.push(if self.table.in_head { "</th>" } else { "</td>" });
                self.table.cell += 1;
            }
            TagEnd::Emphasis => self.push("</em>"),
            TagEnd::Strong => self.push("</strong>"),
            TagEnd::Strikethrough => self.push("</del>"),
            TagEnd::Superscript => self.push("</sup>"),
            TagEnd::Subscript => self.push("</sub>"),
            TagEnd::Link => {
                if self.links.pop() == Some(true) {
                    self.push("</a>");
                } else {
                    self.flush_text();
                }
            }
            TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::Image
            | TagEnd::MetadataBlock(_)
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition => self.flush_text(),
        }
    }

    fn finish(mut self) -> (String, Vec<CodeBlockMeta>) {
        self.finish_code();
        self.finish_html_block();
        self.finish_image();
        self.flush_text();
        (self.out, self.code_blocks)
    }
}

/// Zamienia zdarzenia parsera na (niesanitizowany) HTML i metadane bloków kodu.
pub(crate) fn write_events<'a>(
    events: impl Iterator<Item = Event<'a>>,
    opts: &RenderOptions,
) -> (String, Vec<CodeBlockMeta>) {
    let mut writer = Writer::new(opts);
    for ev in events {
        writer.event(ev);
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_is_sanitized() {
        assert_eq!(code_lang("Rust ignore").as_deref(), Some("rust"));
        assert_eq!(code_lang("c++").as_deref(), Some("c++"));
        assert_eq!(code_lang("\"><script>"), None);
        assert_eq!(code_lang(""), None);
        assert_eq!(code_lang(&"x".repeat(40)), None);
    }

    #[test]
    fn escape_all_specials() {
        let mut s = String::new();
        escape_into(&mut s, "<a href='x'>&\"");
        assert_eq!(s, "&lt;a href=&#39;x&#39;&gt;&amp;&quot;");
    }
}
