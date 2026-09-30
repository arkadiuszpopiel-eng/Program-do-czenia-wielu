//! Testy właściwości normalizatora i chunkera (proptest, deterministyczne ziarno).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use voice_persona_contract::{ChunkerCfg, Lexicon, SpeechChunker};
use voice_persona_impl::{SentenceChunker, builtin_lexicon, normalize};

const WORDS: &[&str] = &[
    "dzień",
    "dobry",
    "jak",
    "się",
    "masz",
    "zażółć",
    "gęślą",
    "jaźń",
    "kot",
    "pies",
    "idzie",
    "szybko",
    "deploy",
    "staging",
    "review",
    "agentka",
    "zrobiłam",
    "sprawdziłam",
    "Warszawa",
    "Kraków",
    "poniedziałek",
    "żółw",
    "ćma",
    "źdźbło",
    "Łódź",
    "rzeka",
    "góra",
    "mleko",
];
const PUNCT: &[&str] = &["", "", "", ",", ".", "!", "?", ";", ":", " -"];

fn plain_sentence() -> impl Strategy<Value = String> {
    prop::collection::vec((0..WORDS.len(), 0..PUNCT.len()), 1..20).prop_map(|items| {
        items
            .into_iter()
            .map(|(w, p)| format!("{}{}", WORDS[w], PUNCT[p]))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn noisy_text() -> impl Strategy<Value = String> {
    let atoms = prop::sample::select(vec![
        "12,50 zł",
        "$3",
        "14:05",
        "1 października 2026",
        "01.10.2026",
        "5 km",
        "15%",
        "np.",
        "itd.",
        "m.in.",
        "ok. 5",
        "https://github.com/x",
        "a@b.pl",
        "`code()`",
        "2026-10-01",
        "-7",
        "3.5",
        "1.2.3",
        "007",
        "+48 600 700 800",
        "5-10 minut",
        "°C",
        "tys.",
        " ",
        "\n",
        "x5",
        "GPT-4",
        "r.",
        "w",
        "o",
        "od",
    ]);
    prop::collection::vec(
        prop_oneof![
            atoms.prop_map(str::to_owned),
            "[0-9]{1,25}",
            "[a-ząćęłńóśźż]{1,8}",
            "[ .,:;!?%$€/-]{1,3}"
        ],
        0..30,
    )
    .prop_map(|parts| parts.join(" "))
}

fn run_chunker(text: &str, split_at: &[usize]) -> Vec<String> {
    let mut c = SentenceChunker::new(ChunkerCfg::default());
    let mut out = Vec::new();
    let mut last = 0;
    for &at in split_at {
        let at = (0..=at.min(text.len()))
            .rev()
            .find(|i| text.is_char_boundary(*i))
            .unwrap_or(0);
        if at > last {
            out.extend(c.push(&text[last..at]));
            last = at;
        }
    }
    out.extend(c.push(&text[last..]));
    out.extend(c.finish());
    out.into_iter().map(|c| c.text).collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, failure_persistence: None, rng_seed: proptest::test_runner::RngSeed::Fixed(0x5EED), ..ProptestConfig::default() })]

    /// Tekst bez liczb, skrótów, URL-i i kodu przechodzi bez zmian (identyczność).
    #[test]
    fn identity_on_plain_text(text in plain_sentence()) {
        prop_assert_eq!(normalize(&text, &builtin_lexicon()), text);
    }

    /// W wyniku nigdy nie ma cyfr ASCII (poza zakresem → cyfra po cyfrze).
    #[test]
    fn never_leaves_ascii_digits(text in noisy_text()) {
        let out = normalize(&text, &builtin_lexicon());
        prop_assert!(!out.chars().any(|c| c.is_ascii_digit()), "{} → {}", text, out);
    }

    /// Idempotencja: druga normalizacja nic nie zmienia.
    #[test]
    fn idempotent(text in noisy_text()) {
        let lx = builtin_lexicon();
        let once = normalize(&text, &lx);
        prop_assert_eq!(normalize(&once, &lx), once);
    }

    /// Dowolna liczba 0..=10^12-1 jest czytana bez cyfr i bez pustego wyniku.
    #[test]
    fn any_number_is_spoken(n in 0u64..1_000_000_000_000) {
        let out = normalize(&n.to_string(), &Lexicon::new());
        prop_assert!(!out.is_empty() && !out.chars().any(|c| c.is_ascii_digit()));
    }

    /// Chunker: treść zachowana, wynik niezależny od podziału strumienia, fragmenty niepuste.
    #[test]
    fn chunker_stream_invariant(text in prop_oneof![plain_sentence(), noisy_text()], cuts in prop::collection::vec(0usize..200, 0..8)) {
        let mut cuts = cuts;
        cuts.sort_unstable();
        let whole = run_chunker(&text, &[]);
        let streamed = run_chunker(&text, &cuts);
        prop_assert_eq!(&streamed, &whole);
        let joined = whole.join(" ");
        prop_assert_eq!(joined.split_whitespace().collect::<Vec<_>>(), text.split_whitespace().collect::<Vec<_>>());
        prop_assert!(whole.iter().all(|c| !c.trim().is_empty()));
        prop_assert!(whole.iter().all(|c| c.chars().count() <= ChunkerCfg::default().hard_max_chars + 1));
    }
}
