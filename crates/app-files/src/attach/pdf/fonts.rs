//! Czcionki strony: kody znaków → tekst i szerokości glifów. Kolejność dekodowania: `ToUnicode`
//! (własny parser), potem kodowanie czcionki prostej (`encoding`: tabela bazowa + `Differences`
//! z nazwami glifów, np. `aogonek`), dla Type0 bez `ToUnicode` — tylko kodowania UCS-2. Szerokości (`Widths`, `W`) służą do wykrywania odstępów między słowami i wierszy.

use lopdf::{Dictionary, Document, Object};

use super::cmap::{CMap, code_of};
use super::encoding::simple_table;
use super::load::{get, get_dict, get_name, get_num, num};

/// Najwięcej wpisów szerokości czcionki złożonej (`W`).
const MAX_WIDTHS: usize = 65_536;

/// Szerokości glifów (w jednostkach glifu, zwykle 1/1000 em).
#[derive(Debug)]
enum Widths {
    Simple {
        first: u32,
        list: Vec<f64>,
        missing: f64,
    },
    Cid {
        default: f64,
        ranges: Vec<(u32, u32, f64)>,
    },
}

/// Glif z napisu: tekst, szerokość (w jednostkach tekstu na 1 pt rozmiaru) i czy to spacja
/// jednobajtowa (do niej stosuje się odstęp słów `Tw`).
pub(super) struct Glyph {
    pub(super) text: Option<String>,
    pub(super) width: f64,
    pub(super) space: bool,
}

/// Czcionka gotowa do dekodowania napisów.
#[derive(Debug)]
pub(super) struct Font {
    composite: bool,
    encoding_cmap: Option<CMap>,
    to_unicode: Option<CMap>,
    table: Option<Vec<String>>,
    ucs2: bool,
    widths: Widths,
    scale: f64,
}

/// Dane strumienia pod kluczem (rozpakowane w ramach budżetu `budget`, najwyżej `cap` bajtów).
fn stream_data(
    doc: &Document,
    dict: &Dictionary,
    key: &[u8],
    budget: &mut usize,
    cap: usize,
) -> Option<Vec<u8>> {
    let stream = get(doc, dict, key)?.as_stream().ok()?;
    let data = stream
        .get_plain_content_with_limit((*budget).min(cap))
        .ok()?;
    *budget = budget.saturating_sub(data.len());
    Some(data)
}

fn simple_widths(doc: &Document, dict: &Dictionary) -> Widths {
    let first = get_num(doc, dict, b"FirstChar")
        .unwrap_or(0.0)
        .clamp(0.0, 255.0) as u32;
    let list: Vec<f64> = get(doc, dict, b"Widths")
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            a.iter()
                .take(256)
                .map(|o| num(doc, o).unwrap_or(0.0))
                .collect()
        })
        .unwrap_or_default();
    let declared = get_dict(doc, dict, b"FontDescriptor")
        .and_then(|d| get_num(doc, d, b"MissingWidth"))
        .filter(|w| *w > 0.0);
    let positive: Vec<f64> = list.iter().copied().filter(|w| *w > 0.0).collect();
    let average =
        (!positive.is_empty()).then(|| positive.iter().sum::<f64>() / positive.len() as f64);
    let mono =
        get_name(doc, dict, b"BaseFont").is_some_and(|n| n.windows(7).any(|w| w == b"Courier"));
    let missing = declared
        .or(average)
        .unwrap_or(if mono { 600.0 } else { 500.0 });
    Widths::Simple {
        first,
        list,
        missing,
    }
}

fn cid_widths(doc: &Document, descendant: Option<&Dictionary>) -> Widths {
    let Some(desc) = descendant else {
        return Widths::Cid {
            default: 1000.0,
            ranges: Vec::new(),
        };
    };
    let default = get_num(doc, desc, b"DW").unwrap_or(1000.0);
    let mut ranges = Vec::new();
    if let Some(w) = get(doc, desc, b"W").and_then(|o| o.as_array().ok()) {
        let mut i = 0;
        while ranges.len() < MAX_WIDTHS {
            let Some(c) = w.get(i).and_then(|o| num(doc, o)) else {
                break;
            };
            let c = c.clamp(0.0, f64::from(u32::MAX)) as u32;
            match w
                .get(i + 1)
                .and_then(|o| super::load::resolve(doc, o))
                .map(|(_, o)| o)
            {
                Some(Object::Array(list)) => {
                    for (k, o) in list.iter().take(MAX_WIDTHS).enumerate() {
                        let code = c.saturating_add(k as u32);
                        ranges.push((code, code, num(doc, o).unwrap_or(default)));
                    }
                    i += 2;
                }
                Some(hi) => {
                    let hi = num(doc, hi).unwrap_or(0.0).clamp(0.0, f64::from(u32::MAX)) as u32;
                    let width = w.get(i + 2).and_then(|o| num(doc, o)).unwrap_or(default);
                    ranges.push((c, hi, width));
                    i += 3;
                }
                None => break,
            }
        }
        ranges.sort_by_key(|r| r.0);
    }
    Widths::Cid { default, ranges }
}

impl Font {
    /// Czcionka ze słownika (strumienie rozpakowywane w ramach `budget`, każdy ≤ `cap`).
    pub(super) fn build(doc: &Document, dict: &Dictionary, budget: &mut usize, cap: usize) -> Self {
        let subtype = get_name(doc, dict, b"Subtype").unwrap_or_default();
        let to_unicode = stream_data(doc, dict, b"ToUnicode", budget, cap).map(|d| CMap::parse(&d));
        if subtype == b"Type0" {
            let (encoding_cmap, ucs2) = match get(doc, dict, b"Encoding") {
                Some(Object::Name(n)) => (
                    None,
                    n.windows(4).any(|w| w == b"UCS2") || n.windows(5).any(|w| w == b"UTF16"),
                ),
                Some(Object::Stream(_)) => (
                    stream_data(doc, dict, b"Encoding", budget, cap).map(|d| CMap::parse(&d)),
                    false,
                ),
                _ => (None, false),
            };
            let descendant = get(doc, dict, b"DescendantFonts")
                .and_then(|o| o.as_array().ok())
                .and_then(|a| a.first())
                .and_then(|o| super::load::resolve(doc, o))
                .and_then(|(_, o)| o.as_dict().ok());
            return Self {
                composite: true,
                encoding_cmap,
                to_unicode,
                table: None,
                ucs2,
                widths: cid_widths(doc, descendant),
                scale: 0.001,
            };
        }
        let scale = if subtype == b"Type3" {
            get(doc, dict, b"FontMatrix")
                .and_then(|o| o.as_array().ok())
                .and_then(|m| m.first())
                .and_then(|o| num(doc, o))
                .filter(|s| s.abs() > 0.0)
                .unwrap_or(0.001)
        } else {
            0.001
        };
        Self {
            composite: false,
            encoding_cmap: None,
            to_unicode,
            table: Some(simple_table(doc, dict)),
            ucs2: false,
            widths: simple_widths(doc, dict),
            scale,
        }
    }

    /// Długość kodu złożonego na początku `bytes`.
    fn code_len(&self, bytes: &[u8]) -> usize {
        let from = |m: &Option<CMap>| {
            m.as_ref()
                .filter(|m| m.has_spaces())
                .and_then(|m| m.code_len(bytes))
        };
        from(&self.encoding_cmap)
            .or_else(|| from(&self.to_unicode))
            .unwrap_or(2)
            .clamp(1, 4)
    }

    fn unicode(&self, code: u32, len: usize) -> Option<String> {
        if let Some(text) = self.to_unicode.as_ref().and_then(|m| m.lookup(len, code)) {
            return Some(text);
        }
        if let Some(table) = &self.table {
            return table.get(code as usize).filter(|t| !t.is_empty()).cloned();
        }
        if self.ucs2 {
            return char::from_u32(code).map(String::from);
        }
        None
    }

    fn width(&self, code: u32) -> f64 {
        match &self.widths {
            Widths::Simple {
                first,
                list,
                missing,
            } => code
                .checked_sub(*first)
                .and_then(|i| list.get(i as usize))
                .copied()
                .filter(|w| *w > 0.0)
                .unwrap_or(*missing),
            Widths::Cid { default, ranges } => {
                let at = ranges.partition_point(|r| r.0 <= code);
                at.checked_sub(1)
                    .and_then(|i| ranges.get(i))
                    .filter(|r| code <= r.1)
                    .map_or(*default, |r| r.2)
            }
        }
    }

    /// Glify napisu po kolei.
    pub(super) fn glyphs(&self, bytes: &[u8], mut each: impl FnMut(Glyph)) {
        let mut rest = bytes;
        while !rest.is_empty() {
            let len = if self.composite {
                self.code_len(rest).min(rest.len())
            } else {
                1
            };
            let (chunk, tail) = rest.split_at(len);
            rest = tail;
            let code = code_of(chunk);
            each(Glyph {
                text: self.unicode(code, len),
                width: self.width(code) * self.scale,
                space: !self.composite && code == 32,
            });
        }
    }
}
