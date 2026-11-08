//! Wyrażenia cron (5 pól: minuta godzina dzień-miesiąca miesiąc dzień-tygodnia) w strefie
//! wyzwalacza. Składnia: `*`, liczby, zakresy `a-b`, kroki `*/n`, `a-b/n`, `a/n`, listy `,`,
//! nazwy angielskie (`mon`, `jan`) i polskie (`pn`, `sty`), makra `@hourly`, `@daily`,
//! `@weekly`, `@monthly`, `@yearly`. Dzień miesiąca i tygodnia — reguła Vixie (gdy oba
//! ograniczone, wystarczy jeden). DST: godzina nieistniejąca → pierwsza chwila po przeskoku;
//! godzina powtórzona → tylko pierwsze wystąpienie (każda minuta lokalna najwyżej raz).

use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, Duration, NaiveDate, Timelike};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::tz::{LocalTime, Tz};

/// Najdalszy horyzont szukania następnego wystąpienia (lata — 29 lutego wypada co ≤ 8 lat).
const SEARCH_YEARS: i64 = 8;

/// Błąd wyrażenia cron.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("niepoprawne wyrażenie cron `{expr}`: {reason}")]
pub struct CronError {
    /// Wyrażenie.
    pub expr: String,
    /// Powód.
    pub reason: String,
}

/// Wyrażenie cron (serializowane jako tekst źródłowy).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CronExpr {
    src: String,
    minutes: u64,
    hours: u32,
    dom: u32,
    months: u16,
    dow: u8,
    dom_star: bool,
    dow_star: bool,
}

const DOW_EN: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const DOW_PL: [&str; 7] = ["nd", "pn", "wt", "sr", "cz", "pt", "sb"];
const MON_EN: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const MON_PL: [&str; 12] = [
    "sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paz", "lis", "gru",
];

fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'ś' => 's',
            'ź' | 'ż' => 'z',
            'ó' => 'o',
            'ą' => 'a',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ć' => 'c',
            other => other,
        })
        .collect()
}

fn value(token: &str, names: &[&[&str]], name_base: u32) -> Option<u32> {
    if let Ok(n) = token.parse::<u32>() {
        return Some(n);
    }
    let t = fold(token);
    names
        .iter()
        .find_map(|list| list.iter().position(|n| *n == t))
        .and_then(|i| u32::try_from(i).ok())
        .map(|i| i + name_base)
}

/// Pole jako maska bitowa wartości `lo..=hi`. Zwraca (maska, czy `*`).
fn field(
    src: &str,
    lo: u32,
    hi: u32,
    names: &[&[&str]],
    name_base: u32,
) -> Result<(u64, bool), String> {
    let mut mask = 0u64;
    // Reguła Vixie: pole zaczynające się od `*` (także `*/n`) liczy się jako „dowolne”.
    let star = src.starts_with('*');
    for part in src.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => {
                let s: u32 = s.parse().map_err(|_| format!("krok `{s}`"))?;
                if s == 0 {
                    return Err("krok 0".into());
                }
                (r, s)
            }
            None => (part, 1),
        };
        let (a, b) = if range == "*" {
            (lo, hi)
        } else if let Some((a, b)) = range.split_once('-') {
            let a = value(a, names, name_base).ok_or(format!("wartość `{a}`"))?;
            let b = value(b, names, name_base).ok_or(format!("wartość `{b}`"))?;
            (a, b)
        } else {
            let a = value(range, names, name_base).ok_or(format!("wartość `{range}`"))?;
            (a, if part.contains('/') { hi } else { a })
        };
        if a < lo || b > hi || a > b {
            return Err(format!("zakres {a}-{b} poza {lo}-{hi}"));
        }
        let mut v = a;
        while v <= b {
            mask |= 1 << v;
            v += step;
        }
    }
    Ok((mask, star))
}

impl CronExpr {
    /// Parsuje wyrażenie.
    pub fn parse(expr: &str) -> Result<Self, CronError> {
        let err = |reason: String| CronError {
            expr: expr.to_owned(),
            reason,
        };
        let src = expr.trim();
        let expanded = match src {
            "@hourly" => "0 * * * *",
            "@daily" | "@midnight" => "0 0 * * *",
            "@weekly" => "0 0 * * 0",
            "@monthly" => "0 0 1 * *",
            "@yearly" | "@annually" => "0 0 1 1 *",
            other if other.starts_with('@') => return Err(err("nieznane makro".into())),
            other => other,
        };
        let parts: Vec<&str> = expanded.split_whitespace().collect();
        let [mi, ho, dm, mo, dw] = parts[..] else {
            return Err(err(format!("oczekiwano 5 pól, jest {}", parts.len())));
        };
        let (minutes, _) = field(mi, 0, 59, &[], 0).map_err(err)?;
        let (hours, _) = field(ho, 0, 23, &[], 0).map_err(err)?;
        let (dom, dom_star) = field(dm, 1, 31, &[], 1).map_err(err)?;
        let (months, _) = field(mo, 1, 12, &[&MON_EN, &MON_PL], 1).map_err(err)?;
        let (dow, dow_star) = field(dw, 0, 7, &[&DOW_EN, &DOW_PL], 0).map_err(err)?;
        let dow = (dow | (dow >> 7)) & 0x7f; // 7 = niedziela
        Ok(Self {
            src: src.to_owned(),
            minutes,
            hours: u32::try_from(hours).map_err(|_| err("godziny".into()))?,
            dom: u32::try_from(dom).map_err(|_| err("dni".into()))?,
            months: u16::try_from(months).map_err(|_| err("miesiące".into()))?,
            dow: u8::try_from(dow).map_err(|_| err("dni tygodnia".into()))?,
            dom_star,
            dow_star,
        })
    }

    /// Tekst źródłowy.
    pub fn as_str(&self) -> &str {
        &self.src
    }

    fn date_matches(&self, d: NaiveDate) -> bool {
        if self.months & (1 << d.month()) == 0 {
            return false;
        }
        let dom = self.dom & (1 << d.day()) != 0;
        let dow = self.dow & (1 << d.weekday().num_days_from_sunday()) != 0;
        // Vixie: gdy któreś pole zaczyna się od `*`, oba muszą pasować; inaczej wystarczy jedno.
        if self.dom_star || self.dow_star {
            dom && dow
        } else {
            dom || dow
        }
    }

    /// Pierwsza chwila (ms UTC) ściśle po `after_ms` zgodna z wyrażeniem w strefie `tz`;
    /// `None`, gdy nie wystąpi w ciągu 8 lat.
    pub fn next_after(&self, after_ms: u64, tz: &Tz) -> Option<u64> {
        let after = i64::try_from(after_ms).ok()?;
        let start = tz.to_local(after)?.with_second(0)?.with_nanosecond(0)?;
        let start = start.checked_add_signed(Duration::minutes(1))?;
        for day in 0..=(SEARCH_YEARS * 366) {
            let date = start.date().checked_add_signed(Duration::days(day))?;
            if !self.date_matches(date) {
                continue;
            }
            let first_day = day == 0;
            for hour in 0..24u32 {
                if self.hours & (1 << hour) == 0 || (first_day && hour < start.hour()) {
                    continue;
                }
                for minute in 0..60u32 {
                    if self.minutes & (1 << minute) == 0
                        || (first_day && hour == start.hour() && minute < start.minute())
                    {
                        continue;
                    }
                    let local = date.and_hms_opt(hour, minute, 0)?;
                    let at = match tz.from_local(local) {
                        LocalTime::Single(t) | LocalTime::Ambiguous { first: t, .. } => t,
                        LocalTime::Gap { next_valid } => next_valid,
                    };
                    if at > after {
                        return u64::try_from(at).ok();
                    }
                }
            }
        }
        None
    }
}

impl FromStr for CronExpr {
    type Err = CronError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for CronExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.src)
    }
}

impl Serialize for CronExpr {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.src)
    }
}

impl<'de> Deserialize<'de> for CronExpr {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for CronExpr {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CronExpr".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Wyrażenie cron (5 pól) albo makro @hourly/@daily/@weekly/@monthly/@yearly."
        })
    }
}
