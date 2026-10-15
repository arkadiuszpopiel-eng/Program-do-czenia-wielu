//! Pytania o procenty bez modelu: „ile procent to 25 z 200”, „jakim procentem liczby 120 jest 30”,
//! „o ile procent wzrosło z 80 do 100”, „zmiana procentowa z 100 do 80”. „20% z 150” liczą
//! rachunki ([`crate::math`]).

use std::sync::OnceLock;

use regex::Regex;

use crate::text::number;

const NUM: &str = r"(-?\d+(?:[.,]\d+)?)";

fn patterns() -> Option<&'static [Regex; 3]> {
    static CELL: OnceLock<Option<[Regex; 3]>> = OnceLock::new();
    CELL.get_or_init(|| {
        let share = format!(
            r"^(?:ile (?:to )?procent|jaki (?:to )?procent|jakim procentem|ile %) (?:to |stanowi |jest )?{NUM} (?:z|ze|liczby|od) {NUM}$"
        );
        let share_rev = format!(
            r"^(?:ile (?:to )?procent|jaki (?:to )?procent|jakim procentem) (?:z |ze |liczby )?{NUM} (?:to|stanowi|jest) {NUM}$"
        );
        let change = format!(
            r"^(?:o ile procent|o ile %|jaka (?:jest )?zmiana procentowa|zmiana procentowa|jaki (?:jest )?wzrost procentowy|jaki (?:jest )?spadek procentowy)(?: (?:wzroslo|wzrosla|wzrosl|spadlo|spadla|spadl|sie zmienilo|sie zmienila|zmienilo sie|zmienila sie|zmienilo|zmienila|rosnie|spada))? (?:z|ze|od) {NUM} (?:do|na) {NUM}$"
        );
        Some([
            Regex::new(&share).ok()?,
            Regex::new(&share_rev).ok()?,
            Regex::new(&change).ok()?,
        ])
    })
    .as_ref()
}

fn parse(m: Option<regex::Match<'_>>) -> Option<f64> {
    let v: f64 = m?.as_str().replace(',', ".").parse().ok()?;
    (v.is_finite() && v.abs() <= 1e12).then_some(v)
}

/// Procent z dokładnością do 2 miejsc (bez „-0”).
fn pct(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    number(if r == 0.0 { 0.0 } else { r })
}

/// Odpowiedź albo `None` (`text` po `normalize`).
pub fn answer(text: &str) -> Option<String> {
    let [share, share_rev, change] = patterns()?;
    if let Some(c) = share.captures(text).or_else(|| share_rev.captures(text)) {
        let (part, whole) = if share.is_match(text) {
            (parse(c.get(1))?, parse(c.get(2))?)
        } else {
            (parse(c.get(2))?, parse(c.get(1))?)
        };
        if whole == 0.0 {
            return Some("Nie da się policzyć procentu z zera.".to_owned());
        }
        return Some(format!(
            "{} z {} to **{}%**",
            number(part),
            number(whole),
            pct(part / whole * 100.0)
        ));
    }
    let c = change.captures(text)?;
    let (from, to) = (parse(c.get(1))?, parse(c.get(2))?);
    if from == 0.0 {
        return Some("Zmiany procentowej od zera nie da się policzyć.".to_owned());
    }
    let delta = (to - from) / from.abs() * 100.0;
    let word = if delta > 0.0 {
        "wzrost"
    } else if delta < 0.0 {
        "spadek"
    } else {
        "bez zmiany"
    };
    let sign = if delta > 0.0 { "+" } else { "" };
    Some(format!(
        "Z {} do {}: **{sign}{}%** ({word})",
        number(from),
        number(to),
        pct(delta)
    ))
}

#[cfg(test)]
mod tests {
    use super::answer;

    #[test]
    fn share_and_change() {
        assert_eq!(
            answer("ile procent to 25 z 200").as_deref(),
            Some("25 z 200 to **12,5%**")
        );
        assert_eq!(
            answer("jakim procentem liczby 120 jest 30").as_deref(),
            Some("30 z 120 to **25%**")
        );
        assert_eq!(
            answer("jaki procent z 120 stanowi 30").as_deref(),
            Some("30 z 120 to **25%**")
        );
        assert_eq!(
            answer("o ile procent wzroslo z 80 do 100").as_deref(),
            Some("Z 80 do 100: **+25%** (wzrost)")
        );
        assert_eq!(
            answer("zmiana procentowa z 100 do 80").as_deref(),
            Some("Z 100 do 80: **-20%** (spadek)")
        );
        assert_eq!(
            answer("o ile procent spadla z 3,5 do 3,5").as_deref(),
            Some("Z 3,5 do 3,5: **0%** (bez zmiany)")
        );
        assert_eq!(
            answer("ile procent to 5 z 0").as_deref(),
            Some("Nie da się policzyć procentu z zera.")
        );
        assert_eq!(answer("ile procent ludzi pije kawe"), None);
    }
}
