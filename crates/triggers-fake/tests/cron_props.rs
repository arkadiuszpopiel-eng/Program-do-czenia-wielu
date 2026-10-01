//! Właściwości crona w strefach z DST (ACC-F5-triggers-02): następne wystąpienie jest ściśle
//! późniejsze, zgodne z polami w czasie lokalnym (albo jest chwilą przeskoku), minimalne (żadna
//! wcześniejsza minuta lokalna nie została pominięta), a stała godzina dzienna w Europe/Warsaw
//! wypada dokładnie raz na każdy dzień kalendarzowy — także w dniach zmiany czasu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use proptest::prelude::*;
use triggers_contract::{CronExpr, LocalTime, Tz, eu_dst_bounds};

#[derive(Debug, Clone)]
struct Field {
    text: String,
    set: BTreeSet<u32>,
    star: bool,
}

fn field(lo: u32, hi: u32) -> impl Strategy<Value = Field> {
    prop_oneof![
        Just(Field {
            text: "*".into(),
            set: (lo..=hi).collect(),
            star: true
        }),
        (1u32..=6).prop_map(move |k| Field {
            text: format!("*/{k}"),
            set: (lo..=hi).step_by(k as usize).collect(),
            star: true
        }),
        proptest::collection::btree_set(lo..=hi, 1..4).prop_map(|s| Field {
            text: s
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            set: s,
            star: false
        }),
    ]
}

#[derive(Debug, Clone)]
struct Expr {
    text: String,
    minute: Field,
    hour: Field,
    dom: Field,
    month: Field,
    dow: Field,
}

fn expr() -> impl Strategy<Value = Expr> {
    (
        field(0, 59),
        field(0, 23),
        field(1, 31),
        field(1, 12),
        field(0, 6),
    )
        .prop_map(|(minute, hour, dom, month, dow)| Expr {
            text: format!(
                "{} {} {} {} {}",
                minute.text, hour.text, dom.text, month.text, dow.text
            ),
            minute,
            hour,
            dom,
            month,
            dow,
        })
}

/// Wzorcowe dopasowanie (reguła Vixie dla dnia miesiąca/tygodnia).
fn matches(e: &Expr, t: NaiveDateTime) -> bool {
    let dom = e.dom.set.contains(&t.day());
    let dow = e.dow.set.contains(&t.weekday().num_days_from_sunday());
    let day = if e.dom.star || e.dow.star {
        dom && dow
    } else {
        dom || dow
    };
    day && e.month.set.contains(&t.month())
        && e.hour.set.contains(&t.hour())
        && e.minute.set.contains(&t.minute())
}

fn tz() -> impl Strategy<Value = Tz> {
    prop_oneof![
        Just(Tz::warsaw()),
        Just(Tz::Utc),
        Just(Tz::parse("Europe/London").unwrap()),
        Just(Tz::parse("Europe/Helsinki").unwrap()),
        Just(Tz::parse("+05:30").unwrap()),
    ]
}

fn is_dst_start(t: i64) -> bool {
    let year = chrono::DateTime::from_timestamp_millis(t).map_or(0, |d| d.year());
    eu_dst_bounds(year).is_some_and(|(start, _)| start == t)
}

/// Czy chwila `t` (wyrównana do minuty) to pierwsze wystąpienie swojego czasu lokalnego.
fn first_occurrence(tz: &Tz, t: i64) -> Option<NaiveDateTime> {
    let local = tz.to_local(t)?;
    match tz.from_local(local) {
        LocalTime::Single(u) | LocalTime::Ambiguous { first: u, .. } if u == t => Some(local),
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 400, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn next_is_later_matching_and_minimal(e in expr(), z in tz(), offset in 0u64..(6 * 366 * 86_400_000)) {
        let cron = CronExpr::parse(&e.text).unwrap();
        let after = 1_704_067_200_000u64 + offset; // od 2024-01-01
        let Some(next) = cron.next_after(after, &z) else {
            // Brak w 8 lat — dozwolone tylko dla wyrażeń „niemożliwych” (np. 31 lutego).
            prop_assert!(!e.dom.star && e.dow.star);
            return Ok(());
        };
        prop_assert!(next > after);
        let n = i64::try_from(next).unwrap();
        let local = z.to_local(n).unwrap();
        prop_assert!(matches(&e, local) || is_dst_start(n), "{} @ {local}", e.text);
        // Minimalność: sprawdzamy do 3000 minut wstecz od wyniku.
        let start = (i64::try_from(after).unwrap() / 60_000 + 1) * 60_000;
        let from = start.max(n - 3000 * 60_000);
        let mut t = from;
        while t < n {
            if let Some(l) = first_occurrence(&z, t) {
                prop_assert!(!matches(&e, l), "pominięto {l} przed {local} dla `{}`", e.text);
            }
            t += 60_000;
        }
        // Kolejne wystąpienie jest ściśle późniejsze (bez duplikatów).
        if let Some(next2) = cron.next_after(next, &z) {
            prop_assert!(next2 > next);
        }
    }

    #[test]
    fn fixed_daily_time_fires_once_per_local_day(h in 0u32..24, m in 0u32..60, year in 2025i32..2031) {
        let cron = CronExpr::parse(&format!("{m} {h} * * *")).unwrap();
        let z = Tz::warsaw();
        let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let mut t = u64::try_from(start.and_utc().timestamp_millis()).unwrap();
        let mut days = BTreeSet::new();
        for _ in 0..366 {
            t = cron.next_after(t, &z).unwrap();
            let day = z.to_local(i64::try_from(t).unwrap()).unwrap().date();
            prop_assert!(days.insert(day), "dwa razy w dniu {day}");
        }
        // 366 kolejnych wystąpień = 366 kolejnych dni kalendarzowych.
        let first = *days.iter().next().unwrap();
        let last = *days.iter().next_back().unwrap();
        prop_assert_eq!((last - first).num_days(), 365);
    }
}

#[test]
fn errors_and_names() {
    for bad in [
        "60 * * * *",
        "* * *",
        "@reboot",
        "*/0 * * * *",
        "1-0 * * * *",
        "* * * * x",
    ] {
        assert!(CronExpr::parse(bad).is_err(), "{bad}");
    }
    assert!(
        CronExpr::parse("0 0 30 2 *")
            .unwrap()
            .next_after(0, &Tz::Utc)
            .is_none()
    );
    let pl = CronExpr::parse("0 9 * sty-mar pn,śr,pt").unwrap();
    let en = CronExpr::parse("0 9 * jan-mar mon,wed,fri").unwrap();
    let after = 1_767_225_600_000; // 2026-01-01 00:00 UTC
    assert_eq!(
        pl.next_after(after, &Tz::Utc),
        en.next_after(after, &Tz::Utc)
    );
    assert_eq!(
        CronExpr::parse("@daily")
            .unwrap()
            .next_after(after, &Tz::Utc),
        Some(after + 86_400_000)
    );
    let sunday7 = CronExpr::parse("0 0 * * 7").unwrap();
    let sunday0 = CronExpr::parse("0 0 * * 0").unwrap();
    assert_eq!(
        sunday7.next_after(after, &Tz::Utc),
        sunday0.next_after(after, &Tz::Utc)
    );
    // Vixie: 13. dnia ALBO piątek.
    let vixie = CronExpr::parse("0 0 13 * fri").unwrap();
    assert_eq!(vixie.next_after(after, &Tz::Utc), Some(after + 86_400_000)); // pt 2.01.2026
    assert_eq!(serde_json::to_string(&vixie).unwrap(), "\"0 0 13 * fri\"");
}
