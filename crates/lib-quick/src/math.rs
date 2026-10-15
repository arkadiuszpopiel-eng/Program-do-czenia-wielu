//! Rachunki: „ile to jest 7 razy 8”, „oblicz (2+3)*4”, „20% z 150”, „pierwiastek z 81”,
//! „dwa do potęgi dziesięć”. Każde słowo musi być liczbą, działaniem albo wypełniaczem
//! („ile”, „to”, „jest”, „oblicz”…) — inaczej pytanie idzie do modelu („ile kosztuje 5 jabłek”).
//! Liczenie na `f64` parserem zstępującym (bez `eval`), limity: wykładnik ≤ 32, wynik ≤ 10¹⁵.

use std::sync::OnceLock;

use regex::Regex;

use crate::text::{number, re};
use crate::words;

const LIMIT: f64 = 1e15;
const FILLER: &[&str] = &[
    "ile",
    "to",
    "jest",
    "bedzie",
    "wynosi",
    "oblicz",
    "policz",
    "wylicz",
    "rozwiaz",
    "mi",
    "nam",
    "wynik",
    "dzialania",
    "dzialanie",
    "rowna",
    "sie",
    "rowne",
    "a",
    "ze",
];
const ASK: &[&str] = &["ile", "oblicz", "policz", "wylicz", "rozwiaz"];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Op(char, &'static str),
    Open,
    Close,
    Sqrt,
    Percent,
}

/// Słowo albo fraza oznaczająca działanie (najdłuższe najpierw).
const PHRASES: &[(&str, Tok)] = &[
    ("pierwiastek kwadratowy z", Tok::Sqrt),
    ("pierwiastek z", Tok::Sqrt),
    ("podzielone przez", Tok::Op('/', ":")),
    ("podzielona przez", Tok::Op('/', ":")),
    ("podzielic przez", Tok::Op('/', ":")),
    ("pomnozone przez", Tok::Op('*', "×")),
    ("pomnozyc przez", Tok::Op('*', "×")),
    ("do potegi", Tok::Op('^', "^")),
    ("razy", Tok::Op('*', "×")),
    ("plus", Tok::Op('+', "+")),
    ("dodac", Tok::Op('+', "+")),
    ("minus", Tok::Op('-', "−")),
    ("odjac", Tok::Op('-', "−")),
    ("przez", Tok::Op('/', ":")),
    ("procent", Tok::Percent),
];

fn symbol(c: char) -> Option<Tok> {
    Some(match c {
        '+' => Tok::Op('+', "+"),
        '-' | '−' | '–' => Tok::Op('-', "−"),
        '*' | '×' | '·' | 'x' => Tok::Op('*', "×"),
        '/' | ':' | '÷' => Tok::Op('/', ":"),
        '^' => Tok::Op('^', "^"),
        '(' => Tok::Open,
        ')' => Tok::Close,
        '%' => Tok::Percent,
        _ => return None,
    })
}

/// Tokeny ze znormalizowanego tekstu; `None` — słowo spoza rachunku.
fn tokenize(text: &str) -> Option<(Vec<Tok>, bool)> {
    let spaced: String = text
        .chars()
        .flat_map(|c| match c {
            '+' | '-' | '−' | '–' | '*' | '×' | '·' | 'x' | '/' | ':' | '÷' | '^' | '(' | ')'
            | '%' | '=' => {
                vec![' ', c, ' ']
            }
            _ => vec![c],
        })
        .collect();
    let words: Vec<&str> = spaced.split_whitespace().collect();
    let mut out = Vec::new();
    let mut asked = false;
    let mut i = 0;
    'outer: while i < words.len() {
        let w = words[i];
        if let Some(value) = decimal(w) {
            if value > LIMIT {
                return None;
            }
            out.push(Tok::Num(value));
            i += 1;
            continue;
        }
        if let Some((value, used)) = words::parse(&words[i..]) {
            if value as f64 > LIMIT {
                return None;
            }
            out.push(Tok::Num(value as f64));
            i += used;
            continue;
        }
        if w == "do" && words.get(i + 1) == Some(&"kwadratu") {
            out.extend([Tok::Op('^', "^"), Tok::Num(2.0)]);
            i += 2;
            continue;
        }
        if w == "do" && words.get(i + 1) == Some(&"szescianu") {
            out.extend([Tok::Op('^', "^"), Tok::Num(3.0)]);
            i += 2;
            continue;
        }
        // „20% z 150” — „z” po procencie to mnożenie.
        if w == "z" && matches!(out.last(), Some(Tok::Percent)) {
            out.push(Tok::Op('*', "z"));
            i += 1;
            continue;
        }
        for (phrase, tok) in PHRASES {
            let n = phrase.split(' ').count();
            if words
                .get(i..i + n)
                .is_some_and(|ws| ws.join(" ") == *phrase)
            {
                out.push(tok.clone());
                i += n;
                continue 'outer;
            }
        }
        if w.chars().count() == 1 {
            if let Some(tok) = w.chars().next().and_then(symbol) {
                out.push(tok);
                i += 1;
                continue;
            }
            if w == "=" {
                i += 1;
                continue;
            }
        }
        if FILLER.contains(&w) {
            asked |= ASK.contains(&w);
            i += 1;
            continue;
        }
        return None;
    }
    Some((out, asked))
}

fn decimal(w: &str) -> Option<f64> {
    if !w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    if !w
        .chars()
        .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
    {
        return None;
    }
    if w.matches([',', '.']).count() > 1 {
        return None;
    }
    w.replace(',', ".").parse().ok()
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    /// Ostatni czynnik był procentem liczby („23%”) — w sumie to procent lewej strony.
    percent: bool,
}

type Calc = Result<f64, &'static str>;

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn sum(&mut self) -> Calc {
        let mut v = self.product()?;
        while let Some(Tok::Op(op @ ('+' | '-'), _)) = self.peek() {
            let op = *op;
            self.pos += 1;
            self.percent = false;
            let r = self.product()?;
            // „100 plus 23%” = 123, „200 minus 10%” = 180 (jak w kalkulatorze).
            let r = if self.percent { v * r } else { r };
            v = if op == '+' { v + r } else { v - r };
        }
        Ok(v)
    }

    fn product(&mut self) -> Calc {
        let mut v = self.unary()?;
        while let Some(Tok::Op(op @ ('*' | '/'), _)) = self.peek() {
            let op = *op;
            self.pos += 1;
            self.percent = false;
            let r = self.unary()?;
            if op == '/' && r == 0.0 {
                return Err("zero");
            }
            v = if op == '*' { v * r } else { v / r };
        }
        Ok(v)
    }

    fn unary(&mut self) -> Calc {
        if let Some(Tok::Op('-', _)) = self.peek() {
            self.pos += 1;
            return Ok(-self.unary()?);
        }
        self.power()
    }

    fn power(&mut self) -> Calc {
        let base = self.postfix()?;
        if let Some(Tok::Op('^', _)) = self.peek() {
            self.pos += 1;
            let exp = self.unary()?;
            if exp.abs() > 32.0 {
                return Err("big");
            }
            return Ok(base.powf(exp));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Calc {
        let mut v = self.primary()?;
        self.percent = false;
        while let Some(Tok::Percent) = self.peek() {
            self.pos += 1;
            v /= 100.0;
            self.percent = true;
        }
        Ok(v)
    }

    fn primary(&mut self) -> Calc {
        let tok = self.peek().cloned().ok_or("syntax")?;
        self.pos += 1;
        match tok {
            Tok::Num(v) => Ok(v),
            Tok::Sqrt => {
                let v = self.postfix()?;
                if v < 0.0 {
                    return Err("sqrt");
                }
                Ok(v.sqrt())
            }
            Tok::Open => {
                let v = self.sum()?;
                match self.peek() {
                    Some(Tok::Close) => {
                        self.pos += 1;
                        Ok(v)
                    }
                    _ => Err("syntax"),
                }
            }
            _ => Err("syntax"),
        }
    }
}

fn shown(toks: &[Tok]) -> String {
    let mut out = String::new();
    for t in toks {
        let piece = match t {
            Tok::Num(v) => number(*v),
            Tok::Op(_, s) => format!(" {s} "),
            Tok::Open => "(".into(),
            Tok::Close => ")".into(),
            Tok::Sqrt => "√".into(),
            Tok::Percent => "%".into(),
        };
        out.push_str(&piece);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Odpowiedź na rachunek albo `None` (to nie jest rachunek).
pub fn answer(text: &str) -> Option<String> {
    static DATE: OnceLock<Option<Regex>> = OnceLock::new();
    if re(&DATE, r"^\d{1,4}[-./]\d{1,2}[-./]\d{1,4}$")?.is_match(text) || text.len() > 160 {
        return None;
    }
    let (toks, asked) = tokenize(text)?;
    let numbers = toks.iter().filter(|t| matches!(t, Tok::Num(_))).count();
    let ops = toks
        .iter()
        .filter(|t| matches!(t, Tok::Op(..) | Tok::Sqrt | Tok::Percent))
        .count();
    let enough =
        ops > 0 && (numbers >= 2 || toks.contains(&Tok::Sqrt) || toks.contains(&Tok::Percent));
    if !enough || (numbers == 1 && !asked && !toks.contains(&Tok::Sqrt)) {
        return None;
    }
    let mut parser = Parser {
        toks: &toks,
        pos: 0,
        percent: false,
    };
    let result = parser.sum();
    if parser.pos != toks.len() {
        return None;
    }
    let expr = shown(&toks);
    Some(match result {
        Ok(v) if v.is_finite() && v.abs() <= LIMIT => {
            let mut out = format!("{expr} = **{}**", number(v));
            if let Some(rest) = remainder(&toks) {
                out.push_str(&format!("\n\nW szkole zapisuje się to jako {rest}."));
            }
            out
        }
        Err("zero") => "Przez zero nie da się dzielić — to działanie nie ma wyniku.".into(),
        Err("sqrt") => {
            "Pierwiastka kwadratowego z liczby ujemnej nie ma wśród liczb rzeczywistych.".into()
        }
        Err("syntax") => return None,
        Ok(v) if v.is_nan() => "To działanie nie ma wyniku wśród liczb rzeczywistych.".into(),
        _ => "Wynik jest za duży, żeby podać go tutaj dokładnie.".into(),
    })
}

/// „7 : 2” na liczbach całkowitych → „3 reszty 1”.
fn remainder(toks: &[Tok]) -> Option<String> {
    let [Tok::Num(a), Tok::Op('/', _), Tok::Num(b)] = toks else {
        return None;
    };
    let whole = |x: f64| x.fract() == 0.0 && x > 0.0 && x < LIMIT;
    if !whole(*a) || !whole(*b) {
        return None;
    }
    let (a, b) = (*a as u64, *b as u64);
    (a % b != 0).then(|| format!("{} reszty {}", a / b, a % b))
}

#[cfg(test)]
#[path = "math_tests.rs"]
mod tests;
