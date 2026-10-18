//! Testy kart licencji: kompletność dla katalogu i głosów, oznaczanie spraw otwartych.

use std::collections::BTreeSet;

use app_api::dto::{CommercialUse, LicenseCard, ModelItemKind};

use super::*;

const TO_CONFIRM_PL: &str = "Do potwierdzenia przez człowieka";
const TO_CONFIRM_EN: &str = "To be confirmed by a human";

fn get(id: &str) -> LicenseCard {
    let Some(c) = card(id) else {
        panic!("brak karty licencji: {id}");
    };
    c
}

#[test]
fn every_catalog_item_has_a_complete_license_card() {
    let items = crate::builtin();
    assert!(items.len() > 10);
    for item in &items {
        let c = get(&item.id);
        let id = &item.id;
        assert!(!c.id.is_empty() && !c.name.is_empty(), "{id}: licencja");
        assert!(
            c.url.starts_with("https://"),
            "{id}: adres licencji {}",
            c.url
        );
        assert!(!c.author.pl.is_empty() && !c.author.en.is_empty(), "{id}");
        assert!(
            c.notes.pl.len() > 40 && c.notes.en.len() > 40,
            "{id}: puste albo zbyt krótkie uwagi"
        );
        // Licencja nie jest „potwierdzona”, gdy sama pozycja czeka na człowieka.
        assert!(
            !c.confirmed || item.confirmed,
            "{id}: potwierdzona bez pozycji"
        );
        if !c.confirmed {
            assert!(
                c.notes.pl.contains(TO_CONFIRM_PL) && c.notes.en.contains(TO_CONFIRM_EN),
                "{id}: sprawa otwarta bez „do potwierdzenia przez człowieka”"
            );
        }
        let open_question = ["potwierdzenia", "sprawdzić"]
            .iter()
            .any(|w| item.license.contains(w));
        assert!(!(open_question && c.confirmed), "{id}: {}", item.license);
        assert!(
            c.commercial != CommercialUse::Unknown || !c.confirmed,
            "{id}: nieustalone użycie komercyjne nie może być potwierdzone"
        );
    }
    for id in all().keys() {
        assert!(
            items.iter().any(|i| &i.id == id),
            "karta licencji bez pozycji katalogu: {id}"
        );
    }
}

#[test]
fn every_voice_has_its_own_license_entry() {
    let voices = |id: &str| -> BTreeSet<String> {
        let c = get(id);
        for v in &c.voices {
            assert!(!v.license.is_empty(), "{id}/{}: licencja głosu", v.id);
            assert!(
                [&v.name, &v.origin, &v.notes]
                    .iter()
                    .all(|t| t.pl.len() > 5 && t.en.len() > 5),
                "{id}/{}: opis głosu",
                v.id
            );
        }
        c.voices.iter().map(|v| v.id.clone()).collect()
    };
    let expected =
        |list: &[&str]| -> BTreeSet<String> { list.iter().map(|s| (*s).to_owned()).collect() };
    assert_eq!(
        voices("supertonic-3"),
        expected(&voice_tts_contract::SUPERTONIC_ALL)
    );
    assert_eq!(
        voices("piper-pl_PL-gosia-medium"),
        expected(&[voice_tts_contract::PIPER_BASE])
    );
    assert_eq!(
        voices("sidecar-pocket-tts"),
        expected(&voice_tts_contract::POCKET_BASE)
    );
    // Barwy Supertonic to pliki stylu z katalogu — każda pobierana barwa ma kartę.
    let styles: BTreeSet<String> = crate::supertonic::supertonic()
        .files
        .iter()
        .filter_map(|f| f.name.strip_suffix(".json"))
        .filter(|n| n.len() == 2 && (n.starts_with('F') || n.starts_with('M')))
        .map(str::to_owned)
        .collect();
    assert_eq!(styles, voices("supertonic-3"));
}

/// Mowa syntetyczna: znak wodny to decyzja właściciela — karta mówi, że czeka (C5.7).
#[test]
fn speech_cards_say_the_watermark_decision_is_pending() {
    for item in crate::builtin() {
        let speaks = item.kind == ModelItemKind::Tts
            || ["sidecar-pocket-tts", "sidecar-piper"].contains(&item.id.as_str());
        let c = get(&item.id);
        let notes = (c.notes.pl.as_str(), c.notes.en.as_str());
        let mentions = notes.0.contains("Znak wodny") && notes.1.contains("watermark");
        assert_eq!(speaks, mentions, "{}: znak wodny w uwagach", item.id);
        if speaks {
            assert!(
                notes.0.contains("decyzja właściciela w toku"),
                "{}",
                item.id
            );
        }
    }
}

#[test]
fn known_open_questions_are_flagged() {
    let dml = get("directml");
    assert_eq!(dml.commercial, CommercialUse::Conditional);
    assert!(!dml.confirmed && dml.notes.pl.contains("diagnostycznych"));
    assert!(dml.notes.en.contains("diagnostic data"));
    let st = get("supertonic-3");
    assert_eq!(st.commercial, CommercialUse::Conditional);
    assert!(st.id.contains("OpenRAIL-M") && st.attribution);
    assert!(st.notes.pl.contains("porad medycznych"));
    assert!(st.notes.en.contains("medical advice"));
    for id in ["sidecar-llama-cuda", "sidecar-whisper-cuda"] {
        let c = get(id);
        assert!(c.id.contains("NVIDIA") && !c.confirmed, "{id}");
    }
    assert!(get("sidecar-piper").id.contains("GPL-3.0"));
    assert_eq!(get("silero-vad").commercial, CommercialUse::Yes);
}
