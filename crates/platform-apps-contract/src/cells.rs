//! Adresy komórek A1, nazwy arkuszy, bezpieczny tekst komórek i lista dozwolonych funkcji formuł
//! (bez DDE, odwołań zewnętrznych, funkcji sieciowych i wykonujących kod).

use crate::office::{MAX_FORMULA_CHARS, OfficeError};

/// Funkcje dozwolone w formułach (nazwy angielskie — `Range.Formula`), rozdzielone spacjami.
const FORMULA_FUNCTIONS: &str = "ABS AND AVERAGE AVERAGEA AVERAGEIF AVERAGEIFS CEILING CHOOSE CODE \
COLUMN COLUMNS CONCAT CONCATENATE CORREL COUNT COUNTA COUNTBLANK COUNTIF COUNTIFS DATE DATEDIF \
DATEVALUE DAY DAYS EDATE EOMONTH EXACT EXP FALSE FILTER FIND FLOOR FV HLOOKUP HOUR IF IFERROR IFNA \
IFS INDEX INT IRR ISBLANK ISERROR ISEVEN ISNA ISNUMBER ISODD ISTEXT LARGE LEFT LEN LET LN LOG LOG10 \
LOWER MATCH MAX MAXIFS MEDIAN MID MIN MINIFS MINUTE MOD MODE MONTH NETWORKDAYS NOT NOW NPER NPV \
OFFSET OR PERCENTILE PI PMT POWER PRODUCT PROPER PV QUARTILE RANK RATE REPT RIGHT ROUND ROUNDDOWN \
ROUNDUP ROW ROWS SEARCH SECOND SEQUENCE SIGN SMALL SORT SQRT STDEV STDEV.P STDEV.S SUBSTITUTE SUM \
SUMIF SUMIFS SUMPRODUCT SWITCH TEXT TEXTJOIN TODAY TRANSPOSE TRIM TRUE TRUNC UNIQUE UPPER VALUE VAR \
VAR.P VAR.S VLOOKUP WEEKDAY WEEKNUM WORKDAY XLOOKUP XMATCH YEAR";

/// Czy funkcja jest na liście dozwolonej.
pub fn is_allowed_function(name: &str) -> bool {
    FORMULA_FUNCTIONS
        .split_ascii_whitespace()
        .any(|f| f.eq_ignore_ascii_case(name))
}

/// Nazwa arkusza wg reguł Excela (1–31 znaków, bez `\ / ? * [ ] :`, nie `History`).
pub fn check_sheet_name(name: &str) -> Result<(), OfficeError> {
    let n = name.chars().count();
    let bad = name.contains(['\\', '/', '?', '*', '[', ']', ':'])
        || name.starts_with('\'')
        || name.ends_with('\'')
        || name.eq_ignore_ascii_case("history")
        || name.chars().any(char::is_control);
    if n == 0 || n > 31 || bad {
        return Err(OfficeError::Policy(format!(
            "niepoprawna nazwa arkusza `{name}`"
        )));
    }
    Ok(())
}

/// Komórka A1 → (kolumna, wiersz) od 1; `$` dozwolone.
pub fn parse_cell(a1: &str) -> Option<(u32, u32)> {
    let s = a1.trim().replace('$', "").to_ascii_uppercase();
    let split = s.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = s.split_at(split);
    if letters.is_empty() || letters.len() > 3 || !letters.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let col = letters
        .bytes()
        .fold(0u32, |acc, b| acc * 26 + u32::from(b - b'A' + 1));
    let row: u32 = digits.parse().ok()?;
    (col <= 16_384 && (1..=1_048_576).contains(&row)).then_some((col, row))
}

/// Zakres `A1:C3` (albo jedna komórka) → narożniki uporządkowane.
pub fn parse_range(range: &str) -> Option<((u32, u32), (u32, u32))> {
    let (a, b) = range.split_once(':').unwrap_or((range, range));
    let (c1, r1) = parse_cell(a)?;
    let (c2, r2) = parse_cell(b)?;
    Some(((c1.min(c2), r1.min(r2)), (c1.max(c2), r1.max(r2))))
}

/// (kolumna, wiersz) → `A1`.
pub fn cell_name(col: u32, row: u32) -> String {
    let mut letters = Vec::new();
    let mut c = col.max(1);
    while c > 0 {
        let rem = (c - 1) % 26;
        letters.push(char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        c = (c - 1) / 26;
    }
    letters.iter().rev().collect::<String>() + &row.to_string()
}

/// Tekst do komórki: zaczynający się od znaku formuły dostaje apostrof (tekst, nie formuła —
/// obrona przed wstrzyknięciem formuł/DDE).
pub fn safe_cell_text(text: &str) -> String {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r', '\n']) {
        format!("'{text}")
    } else {
        text.to_owned()
    }
}

/// Formuła tylko z dozwolonych funkcji, bez DDE (`|`), odwołań zewnętrznych (`[`, `\`, URL)
/// i funkcji spoza listy (sieć, wykonanie kodu, informacje o systemie). Treść literałów
/// napisowych nie jest interpretowana.
pub fn check_formula(formula: &str) -> Result<(), String> {
    let f = formula.trim();
    if !f.starts_with('=') || f.chars().count() > MAX_FORMULA_CHARS || f.len() < 2 {
        return Err("formuła musi zaczynać się od `=` i mieć ≤ 2000 znaków".into());
    }
    let mut code = String::with_capacity(f.len());
    let mut in_string = false;
    for c in f.chars() {
        if c == '"' {
            in_string = !in_string;
            code.push(' ');
        } else if !in_string {
            code.push(c);
        }
    }
    if in_string {
        return Err("niezamknięty napis w formule".into());
    }
    if code.contains(['|', '[', ']', '\\', '{']) || code.to_lowercase().contains("://") {
        return Err("formuła z DDE, odwołaniem zewnętrznym albo adresem jest zablokowana".into());
    }
    let chars: Vec<char> = code.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '_' | '.'))
            {
                i += 1;
            }
            let name: String = chars[start..i]
                .iter()
                .collect::<String>()
                .to_ascii_uppercase();
            let name = name
                .trim_start_matches("_XLFN.")
                .trim_start_matches("_XLWS.");
            let call = chars[i..].iter().find(|c| !c.is_whitespace()) == Some(&'(');
            if call && !is_allowed_function(name) {
                return Err(format!("funkcja `{name}` nie jest dozwolona w formułach"));
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}
