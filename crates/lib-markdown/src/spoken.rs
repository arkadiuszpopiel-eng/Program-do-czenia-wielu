//! Kanał mówiony (PLAN §6.7): czysty tekst bez Markdownu do syntezy mowy.
//!
//! Kod i tabele zostają na ekranie — głosem mówimy tylko krótką frazę. Linki czytamy jako sam
//! tekst, obrazy jako tekst alternatywny, surowy HTML bez znaczników.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::blocks::parser_options;

/// Fraza zastępująca blok kodu.
pub const SPOKEN_CODE: &str = "(kod na ekranie)";
/// Fraza zastępująca tabelę.
pub const SPOKEN_TABLE: &str = "(tabela na ekranie)";
/// Najdłuższy kod w linii czytany dosłownie; dłuższy zastępuje `SPOKEN_CODE`.
pub const MAX_SPOKEN_INLINE_CODE: usize = 40;

#[derive(Default)]
struct Spoken {
    out: String,
    skip_depth: usize,
}

impl Spoken {
    fn text(&mut self, t: &str) {
        if self.skip_depth == 0 {
            self.out.push_str(t);
        }
    }

    fn line_break(&mut self) {
        if self.skip_depth == 0 {
            self.out.push('\n');
        }
    }

    /// Kończy nagłówek kropką, żeby synteza zrobiła pauzę.
    fn end_heading(&mut self) {
        let trimmed = self.out.trim_end();
        if trimmed
            .chars()
            .next_back()
            .is_some_and(|c| !matches!(c, '.' | '!' | '?' | ':' | '\n'))
        {
            let len = trimmed.len();
            self.out.truncate(len);
            self.out.push('.');
        }
        self.out.push('\n');
    }

    fn start(&mut self, tag: &Tag<'_>) {
        match tag {
            Tag::CodeBlock(_) => {
                self.line_break();
                self.text(SPOKEN_CODE);
                self.line_break();
                self.skip_depth += 1;
            }
            Tag::Table(_) => {
                self.line_break();
                self.text(SPOKEN_TABLE);
                self.line_break();
                self.skip_depth += 1;
            }
            Tag::MetadataBlock(_) => self.skip_depth += 1,
            Tag::Paragraph | Tag::Heading { .. } | Tag::Item | Tag::HtmlBlock => self.line_break(),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::CodeBlock | TagEnd::Table | TagEnd::MetadataBlock(_) => {
                self.skip_depth = self.skip_depth.saturating_sub(1);
            }
            TagEnd::Heading(_) if self.skip_depth == 0 => self.end_heading(),
            TagEnd::Paragraph | TagEnd::Item | TagEnd::HtmlBlock | TagEnd::BlockQuote(_) => {
                self.line_break();
            }
            _ => {}
        }
    }

    fn event(&mut self, ev: Event<'_>) {
        match ev {
            Event::Start(tag) => self.start(&tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.text(&t),
            Event::Code(t) if t.chars().count() <= MAX_SPOKEN_INLINE_CODE => self.text(&t),
            Event::Code(_) | Event::DisplayMath(_) | Event::InlineMath(_) => {
                self.text(SPOKEN_CODE);
            }
            Event::Html(t) | Event::InlineHtml(t) => self.text(&strip_tags(&t)),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak | Event::Rule => self.line_break(),
            Event::FootnoteReference(_) | Event::TaskListMarker(_) => {}
        }
    }
}

/// Usuwa znaczniki `<…>` z surowego HTML (zostaje tekst).
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Zamienia Markdown na czysty tekst do mówienia: bez znaczników, kod → „(kod na ekranie)”,
/// tabele → „(tabela na ekranie)”, linki → sam tekst. Linie niepuste, spacje znormalizowane.
pub fn to_spoken_text(md: &str) -> String {
    let mut spoken = Spoken::default();
    for ev in Parser::new_ext(md, parser_options()) {
        spoken.event(ev);
    }
    spoken
        .out
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markup() {
        let md = "# Plan dnia\n\nZrobiłam **trzy** rzeczy, [szczegóły](https://a.pl).\n\n\
                  ```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- `ls` i ![wykres](x.png)\n";
        assert_eq!(
            to_spoken_text(md),
            "Plan dnia.\nZrobiłam trzy rzeczy, szczegóły.\n(kod na ekranie)\n(tabela na ekranie)\nls i wykres"
        );
    }

    #[test]
    fn long_inline_code_and_html() {
        let long = format!("`{}`", "x".repeat(MAX_SPOKEN_INLINE_CODE + 1));
        assert_eq!(to_spoken_text(&long), SPOKEN_CODE);
        assert_eq!(to_spoken_text("a <b>ważne</b> b"), "a ważne b");
        assert_eq!(to_spoken_text("<div>\nblok\n</div>"), "blok");
        assert_eq!(to_spoken_text(""), "");
    }
}
