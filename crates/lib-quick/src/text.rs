//! Normalizacja polecenia: małe litery bez polskich znaków, bez zwrotu do Alfy, grzecznościowych
//! wstępów i końcowej interpunkcji; odmiana liczebników.

use std::sync::OnceLock;

use regex::Regex;

/// Małe litery, polskie znaki zamienione na łacińskie (znak na znak — pozycje słów się zgadzają).
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        })
        .collect()
}

const PREFIX: &str = r"^(?:(?:hej|czesc|sluchaj)[ ,]+)?(?:alfa[ ,:]+)?(?:(?:prosze|powiedz(?: mi)?|sprawdz|a)[ ,]+)?";

/// Polecenie znormalizowane: `fold`, pojedyncze spacje, bez zwrotu („hej Alfa,”), wstępu
/// („proszę”, „powiedz mi”) i końcowej interpunkcji.
pub fn normalize(s: &str) -> String {
    let folded = fold(s);
    let joined = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = joined.trim_end_matches(['?', '!', '.', ' ', '…']);
    static PREFIX_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let rest = match re(&PREFIX_RE, PREFIX) {
        Some(prefix) => prefix.replace(trimmed, "").into_owned(),
        None => trimmed.to_owned(),
    };
    rest.trim_end_matches([',', ' '])
        .trim_end_matches(" prosze")
        .trim_end_matches([',', ' '])
        .to_owned()
}

/// Wyrażenie regularne ze stałego wzorca; błąd wzorca = brak dopasowania (każdy wzorzec
/// sprawdzają testy modułu, które bez niego nie dostałyby odpowiedzi).
pub fn re(cell: &'static OnceLock<Option<Regex>>, pattern: &str) -> Option<&'static Regex> {
    cell.get_or_init(|| Regex::new(pattern).ok()).as_ref()
}

/// Odmiana po liczbie: 1 litera, 2–4 litery, 5 liter (12–14 — „liter”).
pub fn plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        return one;
    }
    let (d, h) = (n % 10, n % 100);
    if (2..=4).contains(&d) && !(12..=14).contains(&h) {
        few
    } else {
        many
    }
}

/// Liczba po polsku: grupy tysięcy oddzielone spacją, przecinek dziesiętny, najwyżej 6 miejsc.
pub fn number(x: f64) -> String {
    let rounded = (x * 1e6).round() / 1e6;
    let negative = rounded < 0.0;
    let abs = rounded.abs();
    let whole = abs.trunc();
    let mut frac = format!("{:.6}", abs - whole);
    frac = frac
        .trim_start_matches('0')
        .trim_end_matches('0')
        .to_owned();
    let digits = format!("{whole:.0}");
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 && digits.len() > 4 {
            grouped.push(' ');
        }
        grouped.push(c);
    }
    let sign = if negative && (whole > 0.0 || frac.len() > 1) {
        "-"
    } else {
        ""
    };
    if frac.len() > 1 {
        format!("{sign}{grouped},{}", &frac[1..])
    } else {
        format!("{sign}{grouped}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_address_and_punctuation() {
        assert_eq!(normalize("Hej Alfa, która godzina?"), "ktora godzina");
        assert_eq!(normalize("Proszę, policz 2+2!"), "policz 2+2");
        assert_eq!(
            normalize("Ile to jest 7 razy 8, proszę?"),
            "ile to jest 7 razy 8"
        );
        assert_eq!(normalize("  Żółć  "), "zolc");
    }

    #[test]
    fn plural_and_numbers() {
        assert_eq!(plural(1, "litera", "litery", "liter"), "litera");
        assert_eq!(plural(3, "litera", "litery", "liter"), "litery");
        assert_eq!(plural(12, "litera", "litery", "liter"), "liter");
        assert_eq!(plural(22, "litera", "litery", "liter"), "litery");
        assert_eq!(number(56.0), "56");
        assert_eq!(number(1234.0), "1234");
        assert_eq!(number(12345.0), "12 345");
        assert_eq!(number(1_000_000.0), "1 000 000");
        assert_eq!(number(3.5), "3,5");
        assert_eq!(number(-0.25), "-0,25");
        assert_eq!(number(1.0 / 3.0), "0,333333");
    }
}
