//! Leksykon składni PDF (treść stron, formularze, mapy CMap): liczby, nazwy, napisy, tablice,
//! słowniki i operatory — strumieniowo, bez budowania drzewa. Każde wywołanie `next` przesuwa
//! pozycję o co najmniej jeden bajt (zawsze się kończy), napisy są ucinane do [`MAX_STRING`].

use std::borrow::Cow;

/// Najdłuższy napis (bajty); dłuższy jest ucinany (reszta pomijana).
const MAX_STRING: usize = 64 * 1024;

/// Token.
#[derive(Debug, PartialEq)]
pub(super) enum Tok<'a> {
    /// Liczba (skończona).
    Num(f64),
    /// Nazwa bez ukośnika (z rozwiniętymi `#xx`).
    Name(Cow<'a, [u8]>),
    /// Napis dosłowny `( )` albo szesnastkowy `< >` jako bajty.
    Str(Vec<u8>),
    /// `[`
    ArrOpen,
    /// `]`
    ArrClose,
    /// `<<`
    DictOpen,
    /// `>>`
    DictClose,
    /// Operator albo słowo kluczowe (także `true`, `{`, `}`).
    Op(&'a [u8]),
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn push(out: &mut Vec<u8>, b: u8) {
    if out.len() < MAX_STRING {
        out.push(b);
    }
}

/// Leksykon nad bajtami.
pub(super) struct Lexer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Leksykon od początku `data`.
    pub(super) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn skip_space(&mut self) {
        while let Some(b) = self.peek() {
            if is_ws(b) {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(c) = self.peek() {
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// Bajty od `start` do końca słowa (do białego znaku albo ogranicznika).
    fn word(&mut self, start: usize) -> &'a [u8] {
        while let Some(b) = self.peek() {
            if is_ws(b) || is_delim(b) {
                break;
            }
            self.pos += 1;
        }
        self.data.get(start..self.pos).unwrap_or_default()
    }

    /// Następny token albo `None` na końcu danych.
    pub(super) fn next(&mut self) -> Option<Tok<'a>> {
        self.skip_space();
        let start = self.pos;
        let b = self.peek()?;
        let tok = match b {
            b'(' => {
                self.pos += 1;
                Tok::Str(self.literal())
            }
            b'<' if self.data.get(start + 1) == Some(&b'<') => {
                self.pos += 2;
                Tok::DictOpen
            }
            b'<' => {
                self.pos += 1;
                Tok::Str(self.hex())
            }
            b'>' if self.data.get(start + 1) == Some(&b'>') => {
                self.pos += 2;
                Tok::DictClose
            }
            b'[' => {
                self.pos += 1;
                Tok::ArrOpen
            }
            b']' => {
                self.pos += 1;
                Tok::ArrClose
            }
            b'/' => {
                self.pos += 1;
                Tok::Name(decode_name(self.word(start + 1)))
            }
            // Pojedynczy ogranicznik bez znaczenia składniowego: `{`, `}`, `)`, `>`.
            _ if is_delim(b) => {
                self.pos += 1;
                Tok::Op(self.data.get(start..self.pos).unwrap_or_default())
            }
            _ => {
                let word = self.word(start);
                number(word).map_or(Tok::Op(word), Tok::Num)
            }
        };
        Some(tok)
    }

    /// Napis dosłowny (po otwierającym nawiasie): nawiasy zagnieżdżone, sekwencje `\`.
    fn literal(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut depth = 1usize;
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                b'\\' => {
                    let Some(e) = self.peek() else { break };
                    self.pos += 1;
                    let value = match e {
                        b'n' => Some(b'\n'),
                        b'r' => Some(b'\r'),
                        b't' => Some(b'\t'),
                        b'b' => Some(8),
                        b'f' => Some(12),
                        b'0'..=b'7' => Some(self.octal(e)),
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                            None
                        }
                        b'\n' => None,
                        other => Some(other),
                    };
                    if let Some(v) = value {
                        push(&mut out, v);
                    }
                }
                b'(' => {
                    depth += 1;
                    push(&mut out, c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    push(&mut out, c);
                }
                _ => push(&mut out, c),
            }
        }
        out
    }

    /// Kod ósemkowy `\ddd` (do trzech cyfr; przepełnienie starszych bitów pomijane).
    fn octal(&mut self, first: u8) -> u8 {
        let mut value = u32::from(first - b'0');
        for _ in 0..2 {
            match self.peek() {
                Some(d @ b'0'..=b'7') => {
                    value = value * 8 + u32::from(d - b'0');
                    self.pos += 1;
                }
                _ => break,
            }
        }
        (value & 0xFF) as u8
    }

    /// Napis szesnastkowy (po `<`): białe znaki pomijane, nieparzysta cyfra uzupełniana zerem.
    fn hex(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        while let Some(c) = self.peek() {
            self.pos += 1;
            if c == b'>' {
                break;
            }
            let Some(digit) = char::from(c).to_digit(16) else {
                continue;
            };
            let digit = digit as u8;
            match high.take() {
                Some(h) => push(&mut out, h * 16 + digit),
                None => high = Some(digit),
            }
        }
        if let Some(h) = high {
            push(&mut out, h * 16);
        }
        out
    }

    /// Po operatorze `ID`: pomija dane obrazu osadzonego do `EI` otoczonego białymi znakami.
    pub(super) fn skip_inline_image(&mut self) {
        let mut i = self.pos + 1;
        while i + 1 < self.data.len() {
            let at_ei = self.data.get(i..i + 2) == Some(b"EI".as_slice());
            let before = i == 0 || self.data.get(i - 1).is_some_and(|&b| is_ws(b));
            let after = self
                .data
                .get(i + 2)
                .is_none_or(|&b| is_ws(b) || is_delim(b));
            if at_ei && before && after {
                self.pos = i + 2;
                return;
            }
            i += 1;
        }
        self.pos = self.data.len();
    }
}

/// Liczba PDF (`12`, `-3.5`, `.25`, `4.`); `None`, gdy słowo nie jest skończoną liczbą.
fn number(word: &[u8]) -> Option<f64> {
    let first = *word.first()?;
    if !(first.is_ascii_digit() || matches!(first, b'+' | b'-' | b'.')) {
        return None;
    }
    std::str::from_utf8(word)
        .ok()?
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}

/// Nazwa z rozwiniętymi sekwencjami `#xx`.
fn decode_name(raw: &[u8]) -> Cow<'_, [u8]> {
    if !raw.contains(&b'#') {
        return Cow::Borrowed(raw);
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while let Some(&b) = raw.get(i) {
        let hex = raw
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b, hex) {
            (b'#', Some(v)) => {
                out.push(v);
                i += 3;
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    Cow::Owned(out)
}
