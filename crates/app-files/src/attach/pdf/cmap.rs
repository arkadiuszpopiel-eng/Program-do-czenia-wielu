//! Mapy CMap: `ToUnicode` (kod znaku → tekst) i przestrzeń kodów (długość kodu w bajtach) —
//! własny, ograniczony parser zamiast `lopdf` (tam zakres `<00000000> <FFFFFFFF>` rozwija się
//! element po elemencie, a pusty cel `<>` kończy się paniką). Zakresy trzymamy nierozwinięte.

use std::collections::HashMap;

use super::lexer::{Lexer, Tok};

/// Najwięcej wpisów (pojedynczych i zakresów) w jednej mapie; dalsze są pomijane.
const MAX_ENTRIES: usize = 100_000;
/// Najwięcej elementów tablicy celu w `bfrange`.
const MAX_LIST: usize = 65_536;

/// Zakres przestrzeni kodów: długość i granice każdego bajtu.
#[derive(Debug, Clone, Copy)]
struct Space {
    len: usize,
    lo: [u8; 4],
    hi: [u8; 4],
}

#[derive(Debug)]
enum Target {
    /// Cel rosnący: ostatnia jednostka UTF-16 zwiększana o przesunięcie kodu.
    Inc(Vec<u16>),
    /// Lista tekstów dla kolejnych kodów.
    List(Vec<String>),
}

#[derive(Debug)]
struct Range {
    len: usize,
    lo: u32,
    hi: u32,
    target: Target,
}

/// Mapa CMap.
#[derive(Debug, Default)]
pub(super) struct CMap {
    spaces: Vec<Space>,
    chars: HashMap<(usize, u32), String>,
    ranges: Vec<Range>,
}

/// Kod big-endian z 1–4 bajtów.
pub(super) fn code_of(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .take(4)
        .fold(0u32, |acc, &b| (acc << 8) | u32::from(b))
}

/// Tekst z bajtów UTF-16BE (nieparzysty ostatni bajt — jako osobna jednostka).
fn utf16(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks(2)
        .map(|c| match c {
            [hi, lo] => u16::from_be_bytes([*hi, *lo]),
            [one] => u16::from(*one),
            _ => 0,
        })
        .collect()
}

/// Operand sekcji mapy.
enum Item {
    Str(Vec<u8>),
    List(Vec<Vec<u8>>),
    Other,
}

/// Operandy do słowa `end` (tablice jako listy napisów).
fn section(lex: &mut Lexer<'_>, end: &[u8]) -> Vec<Item> {
    let mut items = Vec::new();
    while let Some(tok) = lex.next() {
        let item = match tok {
            Tok::Op(op) if op == end => break,
            Tok::Str(s) => Item::Str(s),
            Tok::ArrOpen => {
                let mut list = Vec::new();
                while let Some(t) = lex.next() {
                    match t {
                        Tok::ArrClose => break,
                        Tok::Str(s) if list.len() < MAX_LIST => list.push(s),
                        Tok::Op(op) if op == end => return items,
                        _ => {}
                    }
                }
                Item::List(list)
            }
            _ => Item::Other,
        };
        if items.len() < MAX_ENTRIES * 3 {
            items.push(item);
        }
    }
    items
}

impl CMap {
    /// Parsuje mapę; błędy składni pomijają tylko wadliwy wpis.
    pub(super) fn parse(data: &[u8]) -> Self {
        let mut map = Self::default();
        let mut lex = Lexer::new(data);
        while let Some(tok) = lex.next() {
            if map.chars.len() + map.ranges.len() >= MAX_ENTRIES {
                break;
            }
            match tok {
                Tok::Op(b"begincodespacerange") => {
                    let items = section(&mut lex, b"endcodespacerange");
                    map.spaces_from(&items);
                }
                Tok::Op(b"beginbfchar") => {
                    let items = section(&mut lex, b"endbfchar");
                    for pair in items.chunks(2) {
                        if let [Item::Str(src), Item::Str(dst)] = pair
                            && (1..=4).contains(&src.len())
                        {
                            let text = String::from_utf16_lossy(&utf16(dst));
                            map.chars.insert((src.len(), code_of(src)), text);
                        }
                    }
                }
                Tok::Op(b"beginbfrange") => {
                    let items = section(&mut lex, b"endbfrange");
                    map.ranges_from(items);
                }
                _ => {}
            }
        }
        map.spaces.sort_by_key(|s| s.len);
        map.ranges.sort_by_key(|r| (r.len, r.lo));
        map
    }

    fn spaces_from(&mut self, items: &[Item]) {
        for pair in items.chunks(2) {
            if let [Item::Str(lo), Item::Str(hi)] = pair
                && lo.len() == hi.len()
                && (1..=4).contains(&lo.len())
            {
                let mut space = Space {
                    len: lo.len(),
                    lo: [0; 4],
                    hi: [0; 4],
                };
                space.lo[..lo.len()].copy_from_slice(lo);
                space.hi[..hi.len()].copy_from_slice(hi);
                self.spaces.push(space);
            }
        }
    }

    fn ranges_from(&mut self, items: Vec<Item>) {
        let mut items = items.into_iter();
        while let (Some(lo), Some(hi), Some(dst)) = (items.next(), items.next(), items.next()) {
            let (Item::Str(lo), Item::Str(hi)) = (lo, hi) else {
                continue;
            };
            let (a, b) = (code_of(&lo), code_of(&hi));
            if lo.len() != hi.len() || !(1..=4).contains(&lo.len()) || a > b {
                continue;
            }
            let target = match dst {
                Item::Str(s) if !s.is_empty() => Target::Inc(utf16(&s)),
                Item::List(list) => Target::List(
                    list.iter()
                        .map(|s| String::from_utf16_lossy(&utf16(s)))
                        .collect(),
                ),
                _ => continue,
            };
            self.ranges.push(Range {
                len: lo.len(),
                lo: a,
                hi: b,
                target,
            });
        }
    }

    /// Czy mapa ma zakresy przestrzeni kodów.
    pub(super) fn has_spaces(&self) -> bool {
        !self.spaces.is_empty()
    }

    /// Długość kodu na początku `bytes` (pierwszy pasujący zakres; bez dopasowania — najkrótszy).
    pub(super) fn code_len(&self, bytes: &[u8]) -> Option<usize> {
        let shortest = self.spaces.first()?.len;
        let fits = |s: &Space| {
            bytes.get(..s.len).is_some_and(|chunk| {
                chunk
                    .iter()
                    .zip(s.lo.iter().zip(s.hi.iter()))
                    .all(|(b, (lo, hi))| lo <= b && b <= hi)
            })
        };
        Some(
            self.spaces
                .iter()
                .find(|s| fits(s))
                .map_or(shortest, |s| s.len),
        )
    }

    /// Tekst kodu o długości `len` bajtów.
    pub(super) fn lookup(&self, len: usize, code: u32) -> Option<String> {
        if let Some(text) = self.chars.get(&(len, code)) {
            return Some(text.clone());
        }
        let at = self
            .ranges
            .partition_point(|r| (r.len, r.lo) <= (len, code));
        let range = self.ranges.get(at.checked_sub(1)?)?;
        if range.len != len || code > range.hi {
            return None;
        }
        let offset = code - range.lo;
        match &range.target {
            Target::Inc(units) => {
                let mut units = units.clone();
                if let Some(last) = units.last_mut() {
                    *last = last.wrapping_add((offset & 0xFFFF) as u16);
                }
                Some(String::from_utf16_lossy(&units))
            }
            Target::List(list) => list.get(offset as usize).cloned(),
        }
    }
}
