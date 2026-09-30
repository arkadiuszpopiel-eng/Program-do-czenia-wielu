//! Tabele normalizatora: waluty, jednostki, miesiące, przyimki rządzące przypadkiem, rodzaj.

use crate::numbers::{Gender, NumCase, Plural, plural};

pub(crate) use super::nouns::noun_gender;

/// Rzeczownik po liczebniku: formy (1, 2–4, 5+, dopełniacz l. poj.) + rodzaj.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Noun {
    pub one: &'static str,
    pub few: &'static str,
    pub many: &'static str,
    pub gen_sg: &'static str,
    /// Forma po ułamku dziesiętnym, gdy inna niż dopełniacz l. poj. (np. „procent”).
    pub frac: Option<&'static str>,
    pub gender: Gender,
}

const fn noun(
    one: &'static str,
    few: &'static str,
    many: &'static str,
    gen_sg: &'static str,
    gender: Gender,
) -> Noun {
    Noun {
        one,
        few,
        many,
        gen_sg,
        frac: None,
        gender,
    }
}

impl Noun {
    /// Forma rzeczownika dla liczby `n` w przypadku `case` (ułamek → dopełniacz l. poj.).
    pub(crate) fn form(&self, n: Option<u64>, has_frac: bool, case: NumCase) -> &'static str {
        if has_frac {
            return self.frac.unwrap_or(self.gen_sg);
        }
        let Some(n) = n else { return self.many };
        match (case, plural(n)) {
            (NumCase::Gen, Plural::One) => self.gen_sg,
            (NumCase::Gen, _) | (NumCase::Nom, Plural::Many) => self.many,
            (NumCase::Nom, Plural::One) => self.one,
            (NumCase::Nom, Plural::Few) => self.few,
        }
    }
}

const M: Gender = Gender::Masc;
const F: Gender = Gender::Fem;

/// Waluta: rzeczownik główny + podjednostka.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Currency {
    pub main: Noun,
    pub sub: Noun,
}

const ZLOTY: Currency = Currency {
    main: noun("złoty", "złote", "złotych", "złotego", M),
    sub: noun("grosz", "grosze", "groszy", "grosza", M),
};
const DOLLAR: Currency = Currency {
    main: noun("dolar", "dolary", "dolarów", "dolara", M),
    sub: noun("cent", "centy", "centów", "centa", M),
};
const EURO: Currency = Currency {
    main: noun("euro", "euro", "euro", "euro", Gender::Neut),
    sub: noun("cent", "centy", "centów", "centa", M),
};
const POUND: Currency = Currency {
    main: noun("funt", "funty", "funtów", "funta", M),
    sub: noun("pens", "pensy", "pensów", "pensa", M),
};

/// Waluta dla zapisu słownego (`zł`, `PLN`, `USD`…).
pub(crate) fn currency(word: &str) -> Option<Currency> {
    match word {
        "zł" | "PLN" | "pln" => Some(ZLOTY),
        "USD" | "usd" => Some(DOLLAR),
        "EUR" | "eur" => Some(EURO),
        "GBP" | "gbp" => Some(POUND),
        _ => None,
    }
}

/// Waluta dla symbolu (`$`, `€`, `£`).
pub(crate) fn currency_symbol(c: char) -> Option<Currency> {
    match c {
        '$' => Some(DOLLAR),
        '€' => Some(EURO),
        '£' => Some(POUND),
        _ => None,
    }
}

/// Mnożnik zapisany skrótem (`tys.`, `mln`, `mld`).
pub(crate) fn multiplier(word: &str) -> Option<Noun> {
    match word {
        "tys" => Some(noun("tysiąc", "tysiące", "tysięcy", "tysiąca", M)),
        "mln" => Some(noun("milion", "miliony", "milionów", "miliona", M)),
        "mld" => Some(noun("miliard", "miliardy", "miliardów", "miliarda", M)),
        _ => None,
    }
}

/// Procent (po ułamku: „dwa przecinek pięć procent”).
pub(crate) const PERCENT: Noun = Noun {
    one: "procent",
    few: "procent",
    many: "procent",
    gen_sg: "procenta",
    frac: Some("procent"),
    gender: M,
};

/// Jednostka zapisana jednym słowem (wielkość liter ma znaczenie: `GB`, `km`).
pub(crate) fn unit(word: &str) -> Option<Noun> {
    let n = match word {
        "km" => noun("kilometr", "kilometry", "kilometrów", "kilometra", M),
        "m" => noun("metr", "metry", "metrów", "metra", M),
        "cm" => noun("centymetr", "centymetry", "centymetrów", "centymetra", M),
        "mm" => noun("milimetr", "milimetry", "milimetrów", "milimetra", M),
        "kg" => noun("kilogram", "kilogramy", "kilogramów", "kilograma", M),
        "g" => noun("gram", "gramy", "gramów", "grama", M),
        "l" => noun("litr", "litry", "litrów", "litra", M),
        "GB" => noun("gigabajt", "gigabajty", "gigabajtów", "gigabajta", M),
        "MB" => noun("megabajt", "megabajty", "megabajtów", "megabajta", M),
        "TB" => noun("terabajt", "terabajty", "terabajtów", "terabajta", M),
        "kB" | "KB" => noun("kilobajt", "kilobajty", "kilobajtów", "kilobajta", M),
        "GHz" => noun("gigaherc", "gigaherce", "gigaherców", "gigaherca", M),
        "MHz" => noun("megaherc", "megaherce", "megaherców", "megaherca", M),
        "W" => noun("wat", "waty", "watów", "wata", M),
        "kW" => noun("kilowat", "kilowaty", "kilowatów", "kilowata", M),
        "ms" => noun("milisekunda", "milisekundy", "milisekund", "milisekundy", F),
        "s" => noun("sekunda", "sekundy", "sekund", "sekundy", F),
        "min" => noun("minuta", "minuty", "minut", "minuty", F),
        "h" => noun("godzina", "godziny", "godzin", "godziny", F),
        "px" => noun("piksel", "piksele", "pikseli", "piksela", M),
        "pkt" => noun("punkt", "punkty", "punktów", "punktu", M),
        _ => return None,
    };
    Some(n)
}

/// Kilometry na godzinę (`km/h`).
pub(crate) const KMH: Noun = noun(
    "kilometr na godzinę",
    "kilometry na godzinę",
    "kilometrów na godzinę",
    "kilometra na godzinę",
    M,
);

/// Stopnie Celsjusza (`°C`).
pub(crate) const CELSIUS: Noun = noun(
    "stopień Celsjusza",
    "stopnie Celsjusza",
    "stopni Celsjusza",
    "stopnia Celsjusza",
    M,
);

/// Nazwy miesięcy w dopełniaczu (indeks 0 = styczeń).
pub(crate) const MONTHS_GEN: [&str; 12] = [
    "stycznia",
    "lutego",
    "marca",
    "kwietnia",
    "maja",
    "czerwca",
    "lipca",
    "sierpnia",
    "września",
    "października",
    "listopada",
    "grudnia",
];

/// Numer miesiąca (1–12) dla nazwy w dopełniaczu.
pub(crate) fn month_index(word: &str) -> Option<usize> {
    let w = word.to_lowercase();
    MONTHS_GEN.iter().position(|m| *m == w).map(|i| i + 1)
}

/// Przyimki (i słowa) rządzące dopełniaczem liczebnika („od pięciu”, „około dwóch”).
pub(crate) fn gen_trigger(word: &str) -> bool {
    matches!(
        word,
        "od" | "do"
            | "około"
            | "koło"
            | "powyżej"
            | "poniżej"
            | "bez"
            | "dla"
            | "blisko"
            | "ciągu"
            | "spośród"
            | "wśród"
            | "u"
            | "oprócz"
            | "zamiast"
            | "sprzed"
            | "spod"
            | "wokół"
            | "z"
            | "ze"
    )
}

/// Case liczebnika głównego na podstawie poprzedniego słowa.
pub(crate) fn num_case(prev: Option<&str>) -> NumCase {
    match prev {
        Some(w) if gen_trigger(w) => NumCase::Gen,
        _ => NumCase::Nom,
    }
}

/// Słowa poprzedzające numer telefonu.
pub(crate) fn phone_trigger(word: &str) -> bool {
    matches!(
        word,
        "tel"
            | "telefon"
            | "telefonu"
            | "telefonem"
            | "numer"
            | "numerem"
            | "numeru"
            | "nr"
            | "komórka"
            | "komórki"
            | "zadzwoń"
            | "dzwoń"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noun_forms() {
        let km = unit("km").unwrap();
        assert_eq!(km.form(Some(1), false, NumCase::Nom), "kilometr");
        assert_eq!(km.form(Some(3), false, NumCase::Nom), "kilometry");
        assert_eq!(km.form(Some(5), false, NumCase::Nom), "kilometrów");
        assert_eq!(km.form(Some(2), true, NumCase::Nom), "kilometra");
        assert_eq!(km.form(Some(1), false, NumCase::Gen), "kilometra");
        assert_eq!(km.form(Some(3), false, NumCase::Gen), "kilometrów");
        assert_eq!(km.form(None, false, NumCase::Nom), "kilometrów");
        assert_eq!(PERCENT.form(Some(2), true, NumCase::Nom), "procent");
        assert_eq!(month_index("Października"), Some(10));
        assert_eq!(noun_gender("Minuty"), Gender::Fem);
        assert_eq!(noun_gender("zadanie"), Gender::Neut);
        assert_eq!(noun_gender("kot"), Gender::Masc);
        assert!(currency("zł").is_some() && currency_symbol('$').is_some());
        assert!(multiplier("mln").is_some() && multiplier("xyz").is_none());
        assert!(phone_trigger("tel") && !phone_trigger("kot"));
        assert_eq!(num_case(Some("od")), NumCase::Gen);
        assert_eq!(num_case(None), NumCase::Nom);
    }
}
