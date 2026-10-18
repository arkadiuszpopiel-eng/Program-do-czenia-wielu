//! Stan interpretera treści: macierze, stan graficzny i tekstowy, operandy i ich odczyt
//! (tablice `TJ` z limitem elementów, słowniki operandów pomijane).

use std::borrow::Cow;
use std::rc::Rc;

use super::fonts::Font;
use super::lexer::{Lexer, Tok};

/// Najwięcej elementów tablicy operandu (`TJ`).
const MAX_ARRAY: usize = 16_384;

/// Macierz przekształcenia `[a b c d e f]` (konwencja PDF: wektor wierszowy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct M(pub(super) [f64; 6]);

impl M {
    /// Macierz jednostkowa.
    pub(super) const ID: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// Przesunięcie.
    pub(super) fn translate(x: f64, y: f64) -> Self {
        Self([1.0, 0.0, 0.0, 1.0, x, y])
    }

    /// `self × o`.
    pub(super) fn mul(self, o: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [oa, ob, oc, od, oe, of] = o.0;
        Self([
            a * oa + b * oc,
            a * ob + b * od,
            c * oa + d * oc,
            c * ob + d * od,
            e * oa + f * oc + oe,
            e * ob + f * od + of,
        ])
    }

    /// Punkt początku układu.
    pub(super) fn origin(self) -> (f64, f64) {
        (self.0[4], self.0[5])
    }
}

/// Operand.
pub(super) enum Arg<'a> {
    Num(f64),
    Name(Cow<'a, [u8]>),
    Str(Vec<u8>),
    Arr(Vec<Arg<'a>>),
    Other,
}

/// Stan graficzny istotny dla tekstu.
#[derive(Clone)]
pub(super) struct Gs {
    pub(super) ctm: M,
    pub(super) font: Option<Rc<Font>>,
    pub(super) size: f64,
    pub(super) tc: f64,
    pub(super) tw: f64,
    pub(super) th: f64,
    pub(super) tl: f64,
    pub(super) rise: f64,
}

impl Gs {
    /// Stan początkowy z macierzą `ctm`.
    pub(super) fn new(ctm: M) -> Self {
        Self {
            ctm,
            font: None,
            size: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            rise: 0.0,
        }
    }
}

/// Stan interpretera jednego strumienia treści.
pub(super) struct State {
    pub(super) gs: Gs,
    pub(super) saved: Vec<Gs>,
    pub(super) overflow: usize,
    pub(super) tm: M,
    pub(super) tlm: M,
}

impl State {
    /// Przesunięcie do następnego wiersza (`Td`).
    pub(super) fn td(&mut self, x: f64, y: f64) {
        self.tlm = M::translate(x, y).mul(self.tlm);
        self.tm = self.tlm;
    }

    /// Macierz renderowania tekstu (bez przesunięcia glifu).
    pub(super) fn trm(&self) -> M {
        let g = &self.gs;
        M([g.size * g.th, 0.0, 0.0, g.size, 0.0, g.rise])
            .mul(self.tm)
            .mul(g.ctm)
    }
}

/// Liczby z ostatnich `N` operandów.
pub(super) fn nums<const N: usize>(args: &[Arg<'_>]) -> Option<[f64; N]> {
    let tail = args.get(args.len().checked_sub(N)?..)?;
    let mut out = [0.0; N];
    for (slot, arg) in out.iter_mut().zip(tail) {
        let Arg::Num(n) = arg else { return None };
        *slot = *n;
    }
    Some(out)
}

/// Macierz z sześciu ostatnich operandów.
pub(super) fn matrix(args: &[Arg<'_>]) -> Option<M> {
    nums::<6>(args).map(M)
}

/// Ostatnia nazwa wśród operandów.
pub(super) fn last_name<'b>(args: &'b [Arg<'_>]) -> Option<&'b [u8]> {
    args.iter().rev().find_map(|a| match a {
        Arg::Name(n) => Some(n.as_ref()),
        _ => None,
    })
}

/// Napis, gdy jest ostatnim operandem.
pub(super) fn last_str<'b>(args: &'b [Arg<'_>]) -> Option<&'b [u8]> {
    match args.last()? {
        Arg::Str(s) => Some(s),
        _ => None,
    }
}

/// Tablica operandu (po `[`): liczby i napisy z pierwszego poziomu.
pub(super) fn read_array<'a>(lex: &mut Lexer<'a>) -> Vec<Arg<'a>> {
    let mut out = Vec::new();
    let mut depth = 1usize;
    while let Some(tok) = lex.next() {
        match tok {
            Tok::ArrOpen => depth += 1,
            Tok::ArrClose => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Tok::Num(n) if depth == 1 && out.len() < MAX_ARRAY => out.push(Arg::Num(n)),
            Tok::Str(s) if depth == 1 && out.len() < MAX_ARRAY => out.push(Arg::Str(s)),
            _ => {}
        }
    }
    out
}

/// Pomija słownik (po `<<`) razem z zagnieżdżonymi.
pub(super) fn skip_dict(lex: &mut Lexer<'_>) {
    let mut depth = 1usize;
    while let Some(tok) = lex.next() {
        match tok {
            Tok::DictOpen => depth += 1,
            Tok::DictClose => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            _ => {}
        }
    }
}
