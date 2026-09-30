//! Podział na bloki najwyższego poziomu i renderowanie przyrostowe strumienia (ADR 0009 pkt 5).
//!
//! Blok najwyższego poziomu jest **zamknięty**, gdy w buforze zaczął się następny blok — w CommonMark
//! dopisanie tekstu na końcu nie może już zmienić jego zawartości. Każdy blok renderujemy
//! niezależnie z jego własnego fragmentu źródła, dlatego `render` całości i złożenie bloków ze
//! strumienia dają identyczny HTML. Konsekwencja (świadoma): definicje odnośników `[x]: url`
//! działają tylko w obrębie jednego bloku najwyższego poziomu.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser};
use serde::{Deserialize, Serialize};

use crate::sanitize::Sanitizer;
use crate::writer::write_events;
use crate::{CodeBlockMeta, RenderOptions};

/// Rozszerzenia GFM włączone w parserze (autolinki „gołych” adresów robi writer).
pub(crate) fn parser_options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Wyrenderowany blok najwyższego poziomu (bezpieczny HTML gotowy do wstawienia w DOM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Stabilny identyfikator w obrębie dokumentu/strumienia (0, 1, 2…).
    pub id: u64,
    /// Sanitizowany HTML bloku.
    pub html: String,
    /// Metadane bloków kodu w tym bloku (akcje „kopiuj / zapisz / uruchom”).
    pub code: Vec<CodeBlockMeta>,
}

/// Wynik jednego kroku strumienia.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamUpdate {
    /// Bloki zamknięte w tym kroku (już się nie zmienią).
    pub closed: Vec<Block>,
    /// Bieżący otwarty blok (renderowany od nowa przy każdej delcie); `None`, gdy go nie ma.
    pub open: Option<Block>,
}

/// Zakresy bajtowe bloków najwyższego poziomu, rozszerzone do początku linii.
pub(crate) fn top_level_ranges(src: &str) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut depth = 0usize;
    for (ev, range) in Parser::new_ext(src, parser_options()).into_offset_iter() {
        let starts_block = match ev {
            Event::Start(_) => {
                depth += 1;
                depth == 1
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                false
            }
            _ => depth == 0,
        };
        if starts_block {
            let prev_end = out.last().map_or(0, |r| r.end);
            let line_start = src[..range.start].rfind('\n').map_or(0, |i| i + 1);
            let start = line_start.max(prev_end).min(range.start);
            out.push(start..range.end.max(start));
        }
    }
    out
}

/// Renderuje jeden fragment źródła do bloku (writer + sanitizer).
pub(crate) fn render_slice(id: u64, slice: &str, opts: &RenderOptions, san: &Sanitizer) -> Block {
    let (raw, code) = write_events(Parser::new_ext(slice, parser_options()), opts);
    Block {
        id,
        html: san.clean(&raw),
        code,
    }
}

/// Renderuje cały dokument do jednego HTML: bloki pisane niezależnie (jak w strumieniu),
/// sanitizowane jednym przebiegiem `ammonia` (fragmenty są zbalansowane, więc wynik jest
/// identyczny z połączeniem bloków — sprawdza to test własności strumienia).
pub(crate) fn render_document(md: &str, opts: RenderOptions) -> String {
    let raw: String = top_level_ranges(md)
        .into_iter()
        .map(|range| write_events(Parser::new_ext(&md[range], parser_options()), &opts).0)
        .collect();
    Sanitizer::shared(opts).clean(&raw)
}

/// Renderuje cały dokument jako listę bloków najwyższego poziomu (id od 0).
pub fn render_blocks(md: &str, opts: RenderOptions) -> Vec<Block> {
    let san = Sanitizer::shared(opts);
    top_level_ranges(md)
        .into_iter()
        .zip(0u64..)
        .map(|(range, id)| render_slice(id, &md[range], &opts, san))
        .collect()
}

/// Renderer przyrostowy strumienia Markdown z LLM.
///
/// `push(delta)` zwraca bloki zamknięte w tym kroku oraz bieżący blok otwarty. Parsowany jest
/// tylko „ogon” od początku otwartego bloku, więc koszt kroku zależy od otwartego bloku, a nie od
/// długości całej odpowiedzi.
pub struct IncrementalRenderer {
    opts: RenderOptions,
    sanitizer: &'static Sanitizer,
    source: String,
    open_from: usize,
    next_id: u64,
    closed: Vec<Block>,
}

impl IncrementalRenderer {
    /// Nowy renderer z podanymi opcjami.
    pub fn new(opts: RenderOptions) -> Self {
        Self {
            opts,
            sanitizer: Sanitizer::shared(opts),
            source: String::new(),
            open_from: 0,
            next_id: 0,
            closed: Vec::new(),
        }
    }

    /// Dopisuje deltę tekstu i zwraca zmiany.
    ///
    /// Granice bloków ustalamy tylko na pełnych liniach: niedokończona ostatnia linia może jeszcze
    /// zmienić znaczenie (np. `*` = pusty punkt listy, a `**gruby**` = kontynuacja akapitu).
    pub fn push(&mut self, delta: &str) -> StreamUpdate {
        self.source.push_str(delta);
        let base = self.open_from;
        let complete_len = self.source[base..].rfind('\n').map_or(0, |i| i + 1);
        let ranges = top_level_ranges(&self.source[base..base + complete_len]);
        let mut closed = Vec::new();
        if let Some((last, done)) = ranges.split_last() {
            closed = self.close(done.iter().map(|r| base + r.start..base + r.end));
            self.open_from = base + last.start;
        }
        StreamUpdate {
            closed,
            open: self.render_open(),
        }
    }

    /// Renderuje otwarty obszar (1–2 bloki wg bieżącego bufora) jako jeden blok tymczasowy.
    fn render_open(&self) -> Option<Block> {
        let region = &self.source[self.open_from..];
        let ranges = top_level_ranges(region);
        if ranges.is_empty() {
            return None;
        }
        let mut open = Block {
            id: self.next_id,
            html: String::new(),
            code: Vec::new(),
        };
        for range in ranges {
            let part = render_slice(self.next_id, &region[range], &self.opts, self.sanitizer);
            open.html.push_str(&part.html);
            open.code.extend(part.code);
        }
        Some(open)
    }

    /// Kończy strumień: zamyka wszystkie pozostałe bloki.
    pub fn finish(mut self) -> StreamUpdate {
        let base = self.open_from;
        let ranges = top_level_ranges(&self.source[base..]);
        let closed = self.close(ranges.into_iter().map(|r| base + r.start..base + r.end));
        StreamUpdate { closed, open: None }
    }

    /// Wszystkie dotąd zamknięte bloki.
    pub fn closed_blocks(&self) -> &[Block] {
        &self.closed
    }

    /// Całe dotąd otrzymane źródło.
    pub fn source(&self) -> &str {
        &self.source
    }

    fn close(&mut self, ranges: impl Iterator<Item = Range<usize>>) -> Vec<Block> {
        let mut fresh = Vec::new();
        for range in ranges {
            let block = render_slice(
                self.next_id,
                &self.source[range],
                &self.opts,
                self.sanitizer,
            );
            self.next_id += 1;
            self.closed.push(block.clone());
            fresh.push(block);
        }
        fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Usuwa `data-code="N"` (indeks bloku kodu jest liczony w obrębie bloku najwyższego poziomu).
    fn without_code_index(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(pos) = rest.find(" data-code=\"") {
            out.push_str(&rest[..pos]);
            let after = &rest[pos + " data-code=\"".len()..];
            rest = after.find('"').map_or("", |q| &after[q + 1..]);
        }
        out.push_str(rest);
        out
    }

    /// Niesanitizowany HTML całości z jednego przebiegu parsera (bez podziału na bloki).
    fn whole_raw(md: &str) -> String {
        let raw = write_events(
            Parser::new_ext(md, parser_options()),
            &RenderOptions::default(),
        )
        .0;
        without_code_index(&raw)
    }

    /// Niesanitizowany HTML złożony z niezależnie renderowanych bloków.
    fn sliced_raw(md: &str) -> String {
        top_level_ranges(md)
            .into_iter()
            .map(|r| {
                let opts = RenderOptions::default();
                without_code_index(
                    &write_events(Parser::new_ext(&md[r], parser_options()), &opts).0,
                )
            })
            .collect()
    }

    #[test]
    fn ranges_cover_blocks() {
        let md = "# T\n\npara\nciąg\n\n- a\n- b\n\n```rs\nx\n```\n\n---\n| a |\n|---|\n| 1 |\n";
        let blocks: Vec<&str> = top_level_ranges(md).into_iter().map(|r| &md[r]).collect();
        assert_eq!(blocks.len(), 6, "{blocks:?}");
        assert!(blocks[4].starts_with("---"));
    }

    fn fragment() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "# Nagłówek\n",
            "tekst ",
            "zażółć\n",
            "\n",
            "\n\n",
            "- punkt\n",
            "  - zagn\n",
            "1. raz\n",
            "> cytat\n",
            "```\n",
            "```rust\n",
            "    wcięty\n",
            "| a | b |\n",
            "|---|:-:|\n",
            "**gr** ",
            "_k_ ",
            "~~s~~ ",
            "`k` ",
            "[l](https://a.pl) ",
            "https://b.pl ",
            "<div>\n",
            "</div>\n",
            "---\n",
            "===\n",
            "- [ ] z\n",
            "\t",
            "\r\n",
            "<!-- k\n",
            "-->\n",
            "![o](x.png) ",
            "\\",
            "*",
            "_",
            "|",
            "`",
        ])
        .prop_map(str::to_owned)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]
        /// Renderowanie po blokach = jeden przebieg parsera po całości (bez definicji odnośników).
        #[test]
        fn slicing_preserves_output(parts in prop::collection::vec(fragment(), 0..40)) {
            let md: String = parts.concat();
            prop_assert_eq!(sliced_raw(&md), whole_raw(&md));
        }
    }
}
