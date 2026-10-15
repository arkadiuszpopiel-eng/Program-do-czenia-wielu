//! Przeliczanie jednostek bez modelu: „ile to 5 km w milach”, „zamień 30 stopni Celsjusza na
//! Fahrenheita”, „ile cm ma cal”. Długość, masa, objętość, temperatura, prędkość i czas;
//! jednostki z różnych wymiarów — brak odpowiedzi (wiadomość idzie do modelu).

use std::sync::OnceLock;

use regex::Regex;

use crate::text::number;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dim {
    Length,
    Mass,
    Volume,
    Temperature,
    Speed,
    Time,
}

/// Jednostka: nazwy (po `fold`), symbol, wymiar, mnożnik do jednostki bazowej.
struct Unit {
    names: &'static [&'static str],
    symbol: &'static str,
    dim: Dim,
    factor: f64,
}

const UNITS: &[Unit] = &[
    Unit {
        names: &["mm", "milimetr", "milimetry", "milimetrow", "milimetrach"],
        symbol: "mm",
        dim: Dim::Length,
        factor: 0.001,
    },
    Unit {
        names: &[
            "cm",
            "centymetr",
            "centymetry",
            "centymetrow",
            "centymetrach",
        ],
        symbol: "cm",
        dim: Dim::Length,
        factor: 0.01,
    },
    Unit {
        names: &["m", "metr", "metry", "metrow", "metrach"],
        symbol: "m",
        dim: Dim::Length,
        factor: 1.0,
    },
    Unit {
        names: &["km", "kilometr", "kilometry", "kilometrow", "kilometrach"],
        symbol: "km",
        dim: Dim::Length,
        factor: 1000.0,
    },
    Unit {
        names: &["cal", "cale", "cali", "calach"],
        symbol: "in",
        dim: Dim::Length,
        factor: 0.0254,
    },
    Unit {
        names: &["stopa", "stopy", "stop", "stopach"],
        symbol: "ft",
        dim: Dim::Length,
        factor: 0.3048,
    },
    Unit {
        names: &["jard", "jardy", "jardow", "jardach"],
        symbol: "yd",
        dim: Dim::Length,
        factor: 0.9144,
    },
    Unit {
        names: &["mila", "mile", "mil", "milach", "mi"],
        symbol: "mi",
        dim: Dim::Length,
        factor: 1609.344,
    },
    Unit {
        names: &[
            "mila morska",
            "mile morskie",
            "mil morskich",
            "milach morskich",
        ],
        symbol: "nmi",
        dim: Dim::Length,
        factor: 1852.0,
    },
    Unit {
        names: &["mg", "miligram", "miligramy", "miligramow"],
        symbol: "mg",
        dim: Dim::Mass,
        factor: 0.000_001,
    },
    Unit {
        names: &["g", "gram", "gramy", "gramow", "gramach"],
        symbol: "g",
        dim: Dim::Mass,
        factor: 0.001,
    },
    Unit {
        names: &["dag", "deko", "dekagram", "dekagramy", "dekagramow"],
        symbol: "dag",
        dim: Dim::Mass,
        factor: 0.01,
    },
    Unit {
        names: &[
            "kg",
            "kilogram",
            "kilogramy",
            "kilogramow",
            "kilogramach",
            "kilo",
        ],
        symbol: "kg",
        dim: Dim::Mass,
        factor: 1.0,
    },
    Unit {
        names: &["t", "tona", "tony", "ton", "tonach"],
        symbol: "t",
        dim: Dim::Mass,
        factor: 1000.0,
    },
    Unit {
        names: &["lb", "funt", "funty", "funtow", "funtach"],
        symbol: "lb",
        dim: Dim::Mass,
        factor: 0.453_592_37,
    },
    Unit {
        names: &["oz", "uncja", "uncje", "uncji", "uncjach"],
        symbol: "oz",
        dim: Dim::Mass,
        factor: 0.028_349_523_125,
    },
    Unit {
        names: &["ml", "mililitr", "mililitry", "mililitrow", "mililitrach"],
        symbol: "ml",
        dim: Dim::Volume,
        factor: 0.001,
    },
    Unit {
        names: &["l", "litr", "litry", "litrow", "litrach"],
        symbol: "l",
        dim: Dim::Volume,
        factor: 1.0,
    },
    Unit {
        names: &["galon", "galony", "galonow", "galonach", "gal"],
        symbol: "gal",
        dim: Dim::Volume,
        factor: 3.785_411_784,
    },
    Unit {
        names: &[
            "c",
            "°c",
            "stopni celsjusza",
            "stopien celsjusza",
            "stopnie celsjusza",
            "celsjusza",
            "celsjuszach",
        ],
        symbol: "°C",
        dim: Dim::Temperature,
        factor: 1.0,
    },
    Unit {
        names: &[
            "f",
            "°f",
            "stopni fahrenheita",
            "stopien fahrenheita",
            "stopnie fahrenheita",
            "fahrenheita",
            "fahrenheitach",
        ],
        symbol: "°F",
        dim: Dim::Temperature,
        factor: 1.0,
    },
    Unit {
        names: &["k", "kelwin", "kelwiny", "kelwinow", "kelwinach"],
        symbol: "K",
        dim: Dim::Temperature,
        factor: 1.0,
    },
    Unit {
        names: &["km/h", "kilometrow na godzine"],
        symbol: "km/h",
        dim: Dim::Speed,
        factor: 1.0 / 3.6,
    },
    Unit {
        names: &["m/s", "metrow na sekunde"],
        symbol: "m/s",
        dim: Dim::Speed,
        factor: 1.0,
    },
    Unit {
        names: &["mph", "mil na godzine"],
        symbol: "mph",
        dim: Dim::Speed,
        factor: 0.447_04,
    },
    Unit {
        names: &["wezel", "wezly", "wezlow", "wezlach"],
        symbol: "kn",
        dim: Dim::Speed,
        factor: 1852.0 / 3600.0,
    },
    Unit {
        names: &["s", "sekunda", "sekundy", "sekund", "sekundach"],
        symbol: "s",
        dim: Dim::Time,
        factor: 1.0,
    },
    Unit {
        names: &["min", "minuta", "minuty", "minut", "minutach"],
        symbol: "min",
        dim: Dim::Time,
        factor: 60.0,
    },
    Unit {
        names: &["h", "godz", "godzina", "godziny", "godzin", "godzinach"],
        symbol: "h",
        dim: Dim::Time,
        factor: 3600.0,
    },
    Unit {
        names: &["dzien", "dni", "dniach", "doba", "doby"],
        symbol: "dni",
        dim: Dim::Time,
        factor: 86_400.0,
    },
    Unit {
        names: &["tydzien", "tygodnie", "tygodni", "tygodniach"],
        symbol: "tyg.",
        dim: Dim::Time,
        factor: 604_800.0,
    },
];

fn unit(name: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|u| u.names.contains(&name))
}

/// Alternatywa nazw jednostek do wyrażenia (najdłuższe najpierw, znaki specjalne ucieczką).
fn alternation() -> String {
    let mut names: Vec<&str> = UNITS.iter().flat_map(|u| u.names.iter().copied()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    names
        .iter()
        .map(|n| regex::escape(n))
        .collect::<Vec<_>>()
        .join("|")
}

fn to_base(u: &Unit, x: f64) -> f64 {
    match (u.dim, u.symbol) {
        (Dim::Temperature, "°F") => (x - 32.0) * 5.0 / 9.0,
        (Dim::Temperature, "K") => x - 273.15,
        _ => x * u.factor,
    }
}

fn from_base(u: &Unit, x: f64) -> f64 {
    match (u.dim, u.symbol) {
        (Dim::Temperature, "°F") => x * 9.0 / 5.0 + 32.0,
        (Dim::Temperature, "K") => x + 273.15,
        _ => x / u.factor,
    }
}

/// Zaokrąglenie do 6 cyfr znaczących (wynik czytelny, bez ogona dzielenia).
fn significant(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let digits = 6 - (x.abs().log10().floor() as i32) - 1;
    let scale = 10f64.powi(digits.clamp(-15, 15));
    (x * scale).round() / scale
}

/// Wynik do wyświetlenia: wartości poniżej 0,001 z 6 cyframi znaczącymi (zaokrąglenie do
/// 6 miejsc po przecinku zrobiłoby z nich zero).
fn shown(x: f64) -> String {
    if x == 0.0 || x.abs() >= 1e-3 || !x.is_finite() {
        return number(x);
    }
    let digits =
        usize::try_from((6 - (x.abs().log10().floor() as i32) - 1).clamp(0, 15)).unwrap_or(6);
    let text = format!("{x:.digits$}");
    text.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// Dwa wzorce: „5 km w milach” i „ile cm ma cal” (budowane raz, z listy jednostek).
fn patterns() -> Option<&'static (Regex, Regex)> {
    static CELL: OnceLock<Option<(Regex, Regex)>> = OnceLock::new();
    CELL.get_or_init(|| {
        let units = alternation();
        let forward = format!(
            r"^(?:ile to (?:jest )?|ile jest |zamien |przelicz |przeloz |ile wynosi )?(-?\d+(?:[.,]\d+)?) ?({units}) (?:w|na|to ile|ile to|=) ({units})$"
        );
        let one =
            format!(r"^ile ({units}) (?:ma|to|jest w) (?:jeden |jedna |jedno |1 )?({units})$");
        Some((Regex::new(&forward).ok()?, Regex::new(&one).ok()?))
    })
    .as_ref()
}

/// Odpowiedź albo `None` (to nie jest przeliczenie jednostek).
pub fn answer(text: &str) -> Option<String> {
    let (forward, one) = patterns()?;
    let (value, from, to) = if let Some(c) = forward.captures(text) {
        let v: f64 = c.get(1)?.as_str().replace(',', ".").parse().ok()?;
        (v, unit(c.get(2)?.as_str())?, unit(c.get(3)?.as_str())?)
    } else {
        let c = one.captures(text)?;
        (1.0, unit(c.get(2)?.as_str())?, unit(c.get(1)?.as_str())?)
    };
    if from.dim != to.dim || value.abs() > 1e12 {
        return None;
    }
    let result = significant(from_base(to, to_base(from, value)));
    Some(format!(
        "{} {} = **{} {}**",
        number(value),
        from.symbol,
        shown(result),
        to.symbol
    ))
}

#[cfg(test)]
mod tests {
    use super::answer;

    #[test]
    fn converts_common_units() {
        assert_eq!(
            answer("ile to 5 km w milach").as_deref(),
            Some("5 km = **3,10686 mi**")
        );
        assert_eq!(
            answer("zamien 30 stopni celsjusza na fahrenheita").as_deref(),
            Some("30 °C = **86 °F**")
        );
        assert_eq!(
            answer("ile cm ma cal").as_deref(),
            Some("1 in = **2,54 cm**")
        );
        assert_eq!(
            answer("100 km/h na m/s").as_deref(),
            Some("100 km/h = **27,7778 m/s**")
        );
        assert_eq!(
            answer("2,5 kg w funtach").as_deref(),
            Some("2,5 kg = **5,51156 lb**")
        );
        assert_eq!(answer("0 k na c").as_deref(), Some("0 K = **-273,15 °C**"));
        assert_eq!(
            answer("ile to 1 cal w km").as_deref(),
            Some("1 in = **0,0000254 km**")
        );
        assert_eq!(
            answer("1 mg w tonach").as_deref(),
            Some("1 mg = **0,000000001 t**")
        );
        assert_eq!(
            answer("90 minut w godzinach").as_deref(),
            Some("90 min = **1,5 h**")
        );
    }

    #[test]
    fn mixed_dimensions_and_other_text_go_to_the_model() {
        assert_eq!(answer("5 km w kilogramach"), None);
        assert_eq!(answer("ile kosztuje 5 kg jablek"), None);
        assert_eq!(answer("km na mile"), None);
    }
}
