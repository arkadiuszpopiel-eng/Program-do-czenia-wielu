//! Klasyfikator intencji przerwania: tabela przykładów PL per klasa (dokładność per klasa).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_dialog_contract::{InterruptClassifier, InterruptContext, InterruptIntent};
use voice_dialog_impl::HeuristicClassifier;

const CASES: &[(InterruptIntent, &[&str])] = &[
    (
        InterruptIntent::Correction,
        &[
            "nie, chodziło mi o wersję z zeszłego tygodnia",
            "nie to, tamten plik",
            "chodziło mi o wtorek",
            "miałem na myśli Kraków",
            "źle, ma być dwadzieścia",
            "pomyłka, to było wczoraj",
            "raczej w PDF",
            "to nie ten dokument",
            "nie Beta tylko Gama",
            "poprawka: jutro o dziesiątej",
            "no nie, inaczej",
            "nie o to mi chodziło",
        ],
    ),
    (
        InterruptIntent::Addition,
        &[
            "i jeszcze dodaj załącznik",
            "a jeszcze wyślij to Markowi",
            "dodaj też podpis",
            "poza tym sprawdź pocztę",
            "oprócz tego zrób kopię",
            "dorzuć wykres",
            "i też zapisz notatkę",
            "a także przypomnij mi jutro",
            "jeszcze jedno: zamknij okno",
            "aha, i dodaj datę",
        ],
    ),
    (
        InterruptIntent::Clarify,
        &[
            "a to będzie w PDF?",
            "czyli jutro?",
            "czy to działa na laptopie?",
            "ile to kosztuje?",
            "kiedy to wyślesz?",
            "jak to zrobisz?",
            "to znaczy wszystkie pliki?",
            "który folder?",
            "dlaczego tak długo?",
            "a gdzie to zapiszesz?",
            "w sensie teraz?",
        ],
    ),
    (
        InterruptIntent::TopicChange,
        &[
            "słuchaj, a co z pocztą?",
            "zostaw, powiedz mi o pogodzie",
            "a co z kalendarzem",
            "zmieńmy temat",
            "inna sprawa: faktury",
            "przy okazji, jaki mam plan dnia",
            "powiedz mi o spotkaniu",
            "a teraz zajmij się kodem",
            "wracając do raportu",
            "tak w ogóle, która godzina",
        ],
    ),
    (
        InterruptIntent::StopCancel,
        &[
            "stop",
            "zostaw to",
            "wystarczy",
            "dość",
            "przestań",
            "anuluj",
            "nieważne",
            "daj spokój",
            "nie teraz",
            "koniec",
            "never mind",
        ],
    ),
    (
        InterruptIntent::Continue,
        &[
            "dalej",
            "mów dalej",
            "kontynuuj",
            "mhm, mów dalej",
            "no i co dalej",
            "dobra, dalej",
            "tak, dalej",
            "wznów",
            "sorry, kontynuuj",
            "go on",
        ],
    ),
    (
        InterruptIntent::Backchannel,
        &[
            "mhm",
            "tak",
            "aha",
            "okej",
            "nie no, dobrze",
            "jasne",
            "rozumiem",
        ],
    ),
];

#[test]
fn per_class_accuracy() {
    let c = HeuristicClassifier::default();
    let mut failures = Vec::new();
    for (intent, examples) in CASES {
        let mut ok = 0;
        for e in *examples {
            let r = c.classify(&InterruptContext {
                heard_prefix: "Jutro będzie",
                unsaid: "słonecznie.",
                utterance: e,
            });
            if r.intent == *intent {
                ok += 1;
            } else {
                failures.push(format!("{e} → {:?} (oczekiwano {intent:?})", r.intent));
            }
            assert!((0.0..=1.0).contains(&r.confidence));
        }
        let acc = ok as f64 / examples.len() as f64;
        eprintln!("{intent:?}: {ok}/{} ({:.0} %)", examples.len(), acc * 100.0);
        assert!(acc >= 0.9, "{intent:?} poniżej 90 %: {failures:#?}");
    }
    assert_eq!(
        c.classify(&InterruptContext {
            heard_prefix: "",
            unsaid: "",
            utterance: "zrób mi kawę"
        })
        .intent,
        InterruptIntent::Correction
    );
    assert_eq!(
        c.classify(&InterruptContext {
            heard_prefix: "",
            unsaid: "",
            utterance: "  "
        })
        .intent,
        InterruptIntent::Backchannel
    );
}
