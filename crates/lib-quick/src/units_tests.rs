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
        answer("ile sekund ma doba").as_deref(),
        Some("1 dzień = **86 400 s**")
    );
    assert_eq!(answer("48 h w dniach").as_deref(), Some("48 h = **2 dni**"));
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
