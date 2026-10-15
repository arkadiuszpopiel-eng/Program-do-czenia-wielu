use super::{answer, from_roman, roman, spelled};

#[test]
fn numbers_in_words() {
    assert_eq!(spelled(0), "zero");
    assert_eq!(spelled(7), "siedem");
    assert_eq!(spelled(15), "piętnaście");
    assert_eq!(spelled(40), "czterdzieści");
    assert_eq!(spelled(101), "sto jeden");
    assert_eq!(spelled(1000), "tysiąc");
    assert_eq!(spelled(1234), "tysiąc dwieście trzydzieści cztery");
    assert_eq!(spelled(2005), "dwa tysiące pięć");
    assert_eq!(spelled(12_000), "dwanaście tysięcy");
    assert_eq!(spelled(22_000), "dwadzieścia dwa tysiące");
    assert_eq!(spelled(101_000), "sto jeden tysięcy");
    assert_eq!(spelled(2_500_000), "dwa miliony pięćset tysięcy");
    assert_eq!(
        spelled(999_999_999_999),
        "dziewięćset dziewięćdziesiąt dziewięć miliardów dziewięćset dziewięćdziesiąt dziewięć milionów dziewięćset dziewięćdziesiąt dziewięć tysięcy dziewięćset dziewięćdziesiąt dziewięć"
    );
}

#[test]
fn roman_both_ways() {
    assert_eq!(roman(2026).as_deref(), Some("MMXXVI"));
    assert_eq!(roman(1994).as_deref(), Some("MCMXCIV"));
    assert_eq!(roman(3999).as_deref(), Some("MMMCMXCIX"));
    assert_eq!(roman(0), None);
    assert_eq!(roman(4000), None);
    assert_eq!(from_roman("mcmxc"), Some(1990));
    assert_eq!(from_roman("IV"), Some(4));
    assert_eq!(from_roman("XLII"), Some(42));
    for bad in ["IIX", "VX", "MMMM", "IC", "ABC", ""] {
        assert_eq!(from_roman(bad), None, "{bad}");
    }
}

#[test]
fn answers() {
    let a = |t: &str| answer(t);
    assert_eq!(
        a("1234 slownie").as_deref(),
        Some("1234 = **tysiąc dwieście trzydzieści cztery**")
    );
    assert_eq!(
        a("zapisz 1999,99 zl slownie").as_deref(),
        Some("1999,99 zł = **tysiąc dziewięćset dziewięćdziesiąt dziewięć złotych 99/100**")
    );
    assert_eq!(
        a("slownie 22 zl").as_deref(),
        Some("22 zł = **dwadzieścia dwa złote 00/100**")
    );
    assert_eq!(
        a("12 500 slownie").as_deref(),
        Some("12 500 = **dwanaście tysięcy pięćset**")
    );
    assert_eq!(
        a("3,5 slownie").as_deref(),
        Some("3,50 = **trzy i 50/100**")
    );
    assert_eq!(a("2026 rzymskimi").as_deref(), Some("2026 = **MMXXVI**"));
    assert_eq!(
        a("zamien 1994 na rzymskie").as_deref(),
        Some("1994 = **MCMXCIV**")
    );
    assert_eq!(a("rzymskie mcmxc").as_deref(), Some("MCMXC = **1990**"));
    assert_eq!(a("xlii po arabsku").as_deref(), Some("XLII = **42**"));
    for other in [
        "ile to mix",
        "5000 rzymskimi",
        "rzymskie iix",
        "napisz wypracowanie slownie",
        "1234567890123 slownie",
    ] {
        assert_eq!(a(other), None, "{other}");
    }
}
