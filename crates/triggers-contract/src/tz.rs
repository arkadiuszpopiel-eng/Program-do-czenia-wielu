//! Strefy czasowe bez bazy tzdata: UTC, stałe przesunięcie i strefy UE z regułą czasu letniego
//! obowiązującą od 1996 r. (początek: ostatnia niedziela marca 01:00 UTC, koniec: ostatnia
//! niedziela października 01:00 UTC). Domyślna strefa Alfy: `Europe/Warsaw` (CET/CEST).

use std::fmt;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strefy UE z regułą DST: (nazwa IANA, przesunięcie zimowe w minutach).
const EU_ZONES: [(&str, i32); 24] = [
    ("Europe/Warsaw", 60),
    ("Europe/Berlin", 60),
    ("Europe/Prague", 60),
    ("Europe/Vienna", 60),
    ("Europe/Bratislava", 60),
    ("Europe/Budapest", 60),
    ("Europe/Paris", 60),
    ("Europe/Brussels", 60),
    ("Europe/Amsterdam", 60),
    ("Europe/Madrid", 60),
    ("Europe/Rome", 60),
    ("Europe/Copenhagen", 60),
    ("Europe/Stockholm", 60),
    ("Europe/Oslo", 60),
    ("Europe/Vilnius", 120),
    ("Europe/Riga", 120),
    ("Europe/Tallinn", 120),
    ("Europe/Helsinki", 120),
    ("Europe/Kyiv", 120),
    ("Europe/Athens", 120),
    ("Europe/Bucharest", 120),
    ("Europe/London", 0),
    ("Europe/Dublin", 0),
    ("Europe/Lisbon", 0),
];

/// Strefa czasowa (serializowana jako nazwa: `Europe/Warsaw`, `UTC`, `+02:00`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Tz {
    /// UTC.
    Utc,
    /// Stałe przesunięcie (minuty, bez DST).
    Fixed(i32),
    /// Strefa UE z DST.
    Eu {
        /// Nazwa IANA.
        name: &'static str,
        /// Przesunięcie zimowe (minuty).
        std_offset_min: i32,
    },
}

/// Wynik zamiany czasu lokalnego na UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalTime {
    /// Jednoznaczny.
    Single(i64),
    /// Godzina powtórzona przy zmianie na czas zimowy: pierwsze (letnie) i drugie wystąpienie.
    Ambiguous {
        /// Pierwsze wystąpienie (ms UTC).
        first: i64,
        /// Drugie wystąpienie (ms UTC).
        second: i64,
    },
    /// Godzina nieistniejąca (przeskok na czas letni); `next_valid` = chwila przeskoku.
    Gap {
        /// Pierwsza istniejąca chwila po luce (ms UTC).
        next_valid: i64,
    },
}

/// Ostatnia niedziela miesiąca.
fn last_sunday(year: i32, month: u32) -> Option<NaiveDate> {
    let first_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    let last = first_next.pred_opt()?;
    let back = i64::from(last.weekday().num_days_from_sunday());
    last.checked_sub_signed(Duration::days(back))
}

/// Granice czasu letniego UE w roku (ms UTC): [początek, koniec).
pub fn eu_dst_bounds(year: i32) -> Option<(i64, i64)> {
    let at = |d: NaiveDate| {
        d.and_hms_opt(1, 0, 0)
            .map(|t| t.and_utc().timestamp_millis())
    };
    Some((at(last_sunday(year, 3)?)?, at(last_sunday(year, 10)?)?))
}

impl Tz {
    /// Europe/Warsaw (domyślna).
    pub fn warsaw() -> Self {
        Self::Eu {
            name: "Europe/Warsaw",
            std_offset_min: 60,
        }
    }

    /// Strefa z nazwy: nazwa IANA z tabeli UE, `UTC`/`Z`, albo `±HH:MM`.
    pub fn parse(name: &str) -> Option<Self> {
        let n = name.trim();
        if n.eq_ignore_ascii_case("utc") || n == "Z" || n == "Etc/UTC" {
            return Some(Self::Utc);
        }
        if let Some((zone, off)) = EU_ZONES.iter().find(|(z, _)| z.eq_ignore_ascii_case(n)) {
            return Some(Self::Eu {
                name: zone,
                std_offset_min: *off,
            });
        }
        let (sign, rest) = match n.as_bytes().first() {
            Some(b'+') => (1, &n[1..]),
            Some(b'-') => (-1, &n[1..]),
            _ => return None,
        };
        let (h, m) = rest.split_once(':')?;
        let (h, m): (i32, i32) = (h.parse().ok()?, m.parse().ok()?);
        ((0..=14).contains(&h) && (0..60).contains(&m)).then_some(Self::Fixed(sign * (h * 60 + m)))
    }

    /// Nazwa.
    pub fn name(&self) -> String {
        match self {
            Self::Utc => "UTC".into(),
            Self::Fixed(min) => {
                let sign = if *min < 0 { '-' } else { '+' };
                format!("{sign}{:02}:{:02}", min.abs() / 60, min.abs() % 60)
            }
            Self::Eu { name, .. } => (*name).to_owned(),
        }
    }

    /// Przesunięcie względem UTC (minuty) w chwili `utc_ms`.
    pub fn offset_min_at(&self, utc_ms: i64) -> i32 {
        match self {
            Self::Utc => 0,
            Self::Fixed(min) => *min,
            Self::Eu { std_offset_min, .. } => {
                let year = DateTime::from_timestamp_millis(utc_ms).map_or(1970, |t| t.year());
                match eu_dst_bounds(year) {
                    Some((start, end)) if (start..end).contains(&utc_ms) => std_offset_min + 60,
                    _ => *std_offset_min,
                }
            }
        }
    }

    /// Czas lokalny chwili `utc_ms`.
    pub fn to_local(&self, utc_ms: i64) -> Option<NaiveDateTime> {
        let shifted = utc_ms.checked_add(i64::from(self.offset_min_at(utc_ms)) * 60_000)?;
        DateTime::from_timestamp_millis(shifted).map(|t| t.naive_utc())
    }

    /// Chwila(e) UTC odpowiadające czasowi lokalnemu.
    pub fn from_local(&self, local: NaiveDateTime) -> LocalTime {
        let wall = local.and_utc().timestamp_millis();
        let candidate = |offset: i32| wall - i64::from(offset) * 60_000;
        let (std, dst) = match self {
            Self::Utc => return LocalTime::Single(wall),
            Self::Fixed(min) => return LocalTime::Single(candidate(*min)),
            Self::Eu { std_offset_min, .. } => (*std_offset_min, std_offset_min + 60),
        };
        let as_std = candidate(std);
        let as_dst = candidate(dst);
        let std_ok = self.offset_min_at(as_std) == std;
        let dst_ok = self.offset_min_at(as_dst) == dst;
        match (dst_ok, std_ok) {
            (true, true) => LocalTime::Ambiguous {
                first: as_dst,
                second: as_std,
            },
            (true, false) => LocalTime::Single(as_dst),
            (false, true) => LocalTime::Single(as_std),
            (false, false) => {
                let year = local.year();
                let next_valid = eu_dst_bounds(year).map_or(as_std, |(start, _)| start);
                LocalTime::Gap { next_valid }
            }
        }
    }

    /// Dzień tygodnia i minuta doby w czasie lokalnym (okna ciszy).
    pub fn weekday_minute(&self, utc_ms: i64) -> Option<(Weekday, u16)> {
        use chrono::Timelike;
        let local = self.to_local(utc_ms)?;
        let minute = u16::try_from(local.hour() * 60 + local.minute()).ok()?;
        Some((local.weekday(), minute))
    }
}

impl Default for Tz {
    fn default() -> Self {
        Self::warsaw()
    }
}

impl fmt::Display for Tz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

impl Serialize for Tz {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.name())
    }
}

impl<'de> Deserialize<'de> for Tz {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("nieznana strefa: {raw}")))
    }
}

impl JsonSchema for Tz {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Tz".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Strefa: nazwa IANA strefy UE (np. Europe/Warsaw), UTC albo ±HH:MM."
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> i64 {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
            .unwrap()
            .and_utc()
            .timestamp_millis()
    }

    fn local(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn warsaw_dst_2026() {
        // 2026: przeskok 29 marca, powrót 25 października.
        let (start, end) = eu_dst_bounds(2026).unwrap();
        assert_eq!(start, utc("2026-03-29 01:00"));
        assert_eq!(end, utc("2026-10-25 01:00"));
        let w = Tz::warsaw();
        assert_eq!(w.offset_min_at(utc("2026-01-15 12:00")), 60);
        assert_eq!(w.offset_min_at(utc("2026-07-15 12:00")), 120);
        assert_eq!(
            w.to_local(utc("2026-07-15 12:00")),
            Some(local("2026-07-15 14:00"))
        );
        assert_eq!(
            w.from_local(local("2026-03-29 02:30")),
            LocalTime::Gap {
                next_valid: utc("2026-03-29 01:00")
            }
        );
        assert_eq!(
            w.from_local(local("2026-10-25 02:30")),
            LocalTime::Ambiguous {
                first: utc("2026-10-25 00:30"),
                second: utc("2026-10-25 01:30")
            }
        );
        assert_eq!(
            w.from_local(local("2026-10-25 03:00")),
            LocalTime::Single(utc("2026-10-25 02:00"))
        );
    }

    #[test]
    fn parse_and_names() {
        assert_eq!(Tz::parse("europe/warsaw"), Some(Tz::warsaw()));
        assert_eq!(Tz::parse("UTC"), Some(Tz::Utc));
        assert_eq!(Tz::parse("+05:30"), Some(Tz::Fixed(330)));
        assert_eq!(Tz::parse("-03:00").map(|t| t.name()), Some("-03:00".into()));
        assert_eq!(Tz::parse("Mars/Olympus"), None);
        assert_eq!(Tz::parse("+25:00"), None);
        let json = serde_json::to_string(&Tz::warsaw()).unwrap();
        assert_eq!(json, "\"Europe/Warsaw\"");
        assert_eq!(serde_json::from_str::<Tz>(&json).unwrap(), Tz::warsaw());
        assert!(serde_json::from_str::<Tz>("\"x\"").is_err());
        let london = Tz::parse("Europe/London").unwrap();
        assert_eq!(london.offset_min_at(utc("2026-06-01 00:00")), 60);
        assert_eq!(
            Tz::Fixed(-90).from_local(local("2026-01-01 00:00")),
            LocalTime::Single(utc("2026-01-01 01:30"))
        );
    }
}
