//! Uwagi o jakości pakietów 1–6. Normy są przywołane jako **zalecenie albo metoda pomiaru** — bez
//! deklaracji certyfikacji i bez wymyślonych wyników. Liczby pochodzą z pomiarów repozytorium
//! (`docs/STATUS.md` pkt 12 — próba generalna 2026-10-06, 4 vCPU), z manifestu modeli
//! (`providers-local-impl/models.toml`), z planu rozmieszczenia na karcie
//! (`docs/modules/providers-local/SPEC.md`) i z progów Alfy (`docs/ACCEPTANCE.md`, `docs/PLAN.md` §6.4).

use app_api::dto::{LocalizedText, QualityNote};

fn note(aspect: (&str, &str), standard: &str, text: (&str, &str)) -> QualityNote {
    QualityNote {
        aspect: LocalizedText::new(aspect.0, aspect.1),
        standard: standard.into(),
        text: LocalizedText::new(text.0, text.1),
    }
}

const ISO_25010: &str = "ISO/IEC 25010:2023";
const ISO_25059: &str = "ISO/IEC 25059:2023";
const ASPECT_LLM: (&str, &str) = ("Model rozmowy", "Chat model");
const ASPECT_PERF: (&str, &str) = ("Wydajność", "Performance efficiency");
const ASPECT_STT: (&str, &str) = ("Rozpoznawanie mowy", "Speech recognition");

fn llm_big() -> QualityNote {
    note(
        ASPECT_LLM,
        ISO_25059,
        (
            "Bielik 4.5B v3.0 (Q8_0 — 8 bitów, jakość bliska pełnej precyzji) obsługuje narzędzia agentek. Zalecenie według modelu jakości systemów AI (ISO/IEC 25059: poprawność funkcjonalna, odporność): poprawność sprawdza zamrożony zestaw ewaluacyjny Alfy — to metoda pomiaru, nie certyfikat.",
            "Bielik 4.5B v3.0 (Q8_0 — 8-bit, quality close to full precision) supports the agents' tools. Recommendation per the AI system quality model (ISO/IEC 25059: functional correctness, robustness): correctness is checked with Alfa's frozen evaluation set — a measurement method, not a certification.",
        ),
    )
}

fn llm_small() -> QualityNote {
    note(
        ASPECT_LLM,
        ISO_25059,
        (
            "Bielik 1.5B v3.0 (Q8_0) prowadzi rozmowę, ale nie wywołuje narzędzi agentek (robi to zbyt zawodnie). Według ISO/IEC 25059 to ograniczenie poprawności funkcjonalnej: do zadań agentek potrzebny pakiet 5–6 albo model w chmurze.",
            "Bielik 1.5B v3.0 (Q8_0) holds a conversation but does not call the agents' tools (it does so too unreliably). Per ISO/IEC 25059 this limits functional correctness: agent tasks need bundle 5–6 or a cloud model.",
        ),
    )
}

fn perf_small() -> QualityNote {
    note(
        ASPECT_PERF,
        ISO_25010,
        (
            "Wydajność (zachowanie w czasie, wykorzystanie zasobów): Bielik 1.5B Q8_0 — 21 tok/s na samym procesorze (4 vCPU, próba generalna 2026-10-06); według manifestu ok. 2,4 GB RAM na procesorze albo ok. 2,2 GB na karcie (mieści się w całości).",
            "Performance efficiency (time behaviour, resource utilisation): Bielik 1.5B Q8_0 — 21 tok/s on the CPU alone (4 vCPU, dress rehearsal 2026-10-06); per the manifest about 2.4 GB of RAM on the CPU or about 2.2 GB on a GPU (fits entirely).",
        ),
    )
}

fn stt_turbo() -> QualityNote {
    note(
        ASPECT_STT,
        "WER (NIST SCLITE)",
        (
            "Whisper large-v3-turbo (q5_0). Jakość mierzy wskaźnik WER (odsetek błędnie rozpoznanych słów, metodyka NIST SCLITE); próg Alfy: WER po polsku ≤ 12 % na zamrożonym zestawie testowym (ACCEPTANCE F2-03) — do zmierzenia na tej maszynie.",
            "Whisper large-v3-turbo (q5_0). Quality is measured by WER (word error rate, NIST SCLITE methodology); Alfa's threshold: Polish WER ≤ 12% on the frozen test set (ACCEPTANCE F2-03) — to be measured on this machine.",
        ),
    )
}

fn stt_small() -> QualityNote {
    note(
        ASPECT_STT,
        "WER (NIST SCLITE)",
        (
            "Whisper small (q5_1) — mniejszy i mniej dokładny od turbo. Miara: WER (metodyka NIST SCLITE), próg Alfy ≤ 12 % (ACCEPTANCE F2-03) — do zmierzenia. W próbie generalnej na 4 vCPU bez karty 2,1 s mowy rozpoznawało się 53 s: bez karty to raczej dyktowanie niż płynna rozmowa.",
            "Whisper small (q5_1) — smaller and less accurate than turbo. Metric: WER (NIST SCLITE methodology), Alfa's threshold ≤ 12% (ACCEPTANCE F2-03) — to be measured. In the dress rehearsal on 4 vCPU without a GPU, 2.1 s of speech took 53 s to recognise: without a GPU this is dictation rather than fluent conversation.",
        ),
    )
}

fn tts() -> QualityNote {
    note(
        ("Głos", "Voice"),
        "ITU-T P.800 / P.808",
        (
            "Piper pl_PL gosia (medium). Naturalność mowy ocenia się średnią ocen słuchaczy MOS w skali 1–5 (ITU-T P.800; wariant z oceną przez internautów — P.808); wynik dla tego głosu — do zmierzenia w Voice Lab. Zmierzona szybkość: RTF 0,34 na 4 vCPU (synteza ok. 3 razy szybsza od czasu mowy).",
            "Piper pl_PL gosia (medium). Speech naturalness is rated with the listeners' mean opinion score (MOS) on a 1–5 scale (ITU-T P.800; crowdsourced variant — P.808); the score for this voice — to be measured in Voice Lab. Measured speed: RTF 0.34 on 4 vCPU (synthesis about 3 times faster than real time).",
        ),
    )
}

fn latency() -> QualityNote {
    note(
        ("Opóźnienie rozmowy", "Conversation delay"),
        "ITU-T G.114",
        (
            "Zalecenie ITU-T G.114 (do 150 ms w jedną stronę) dotyczy połączeń telefonicznych — asystent z modelem lokalnym go nie osiąga. Alfa ma własne cele (PLAN §6.4), od końca Twojej wypowiedzi do pierwszego dźwięku odpowiedzi: lokalnie p50 ≤ 2,0 s i p95 ≤ 3,0 s (profil A), hybrydowo p50 ≤ 1,3 s (profil B) — do zmierzenia.",
            "ITU-T G.114 (up to 150 ms one way) applies to telephone calls — an assistant with a local model does not reach it. Alfa has its own targets (PLAN §6.4), from the end of your utterance to the first sound of the reply: local p50 ≤ 2.0 s and p95 ≤ 3.0 s (profile A), hybrid p50 ≤ 1.3 s (profile B) — to be measured.",
        ),
    )
}

fn reliability() -> QualityNote {
    note(
        ("Niezawodność", "Reliability"),
        ISO_25010,
        (
            "Niezawodność (odporność na błędy, odtwarzalność): każdy plik jest sprawdzany sumą SHA-256; gdy silnik na kartę graficzną nie wystartuje, Alfa przechodzi na wersję CPU; uszkodzony albo źle zainstalowany element naprawisz osobno („Napraw”).",
            "Reliability (fault tolerance, recoverability): every file is checked with SHA-256; when a GPU engine fails to start, Alfa falls back to the CPU build; a damaged or badly installed element can be repaired on its own (“Repair”).",
        ),
    )
}

pub(crate) fn quality_6() -> Vec<QualityNote> {
    let placement = note(
        ASPECT_PERF,
        ISO_25010,
        (
            "Wydajność (zachowanie w czasie, wykorzystanie zasobów): na karcie 8 GB Bielik 4.5B mieści się w całości obok rozpoznawania mowy (plan rozmieszczenia providers-local: 5880 + 1500 MB ≤ 7408 MB). Tempo odpowiedzi na tej maszynie — do zmierzenia.",
            "Performance efficiency (time behaviour, resource utilisation): on an 8 GB GPU Bielik 4.5B fits entirely next to speech recognition (providers-local placement plan: 5880 + 1500 MB ≤ 7408 MB). Reply speed on this machine — to be measured.",
        ),
    );
    let speaker = note(
        ("Weryfikacja głosu", "Voice verification"),
        "ISO/IEC 19795-1:2021",
        (
            "WeSpeaker ResNet34: skuteczność biometrii mierzy się wskaźnikami FAR/FRR i EER według ISO/IEC 19795-1 — dla tego modelu do zmierzenia (okno 200 ramek do potwierdzenia). Cechy słowa wywoławczego (openWakeWord) potrzebują jeszcze własnego klasyfikatora „Hej Alfa”.",
            "WeSpeaker ResNet34: biometric performance is measured with FAR/FRR and EER per ISO/IEC 19795-1 — to be measured for this model (200-frame window to be confirmed). The wake-word features (openWakeWord) still need an own “Hej Alfa” classifier.",
        ),
    );
    vec![
        llm_big(),
        placement,
        stt_turbo(),
        tts(),
        latency(),
        speaker,
        reliability(),
    ]
}

pub(crate) fn quality_5() -> Vec<QualityNote> {
    let placement = note(
        ASPECT_PERF,
        ISO_25010,
        (
            "Wydajność (zachowanie w czasie, wykorzystanie zasobów): na karcie 6 GB (RTX 4050) plan rozmieszczenia kładzie 34 z 60 warstw Bielika 4.5B na kartę, resztę w RAM — wolniej niż w całości na karcie. Dla porównania: na samym procesorze (4 vCPU) zmierzyliśmy 6,8 tok/s (próba generalna 2026-10-06).",
            "Performance efficiency (time behaviour, resource utilisation): on a 6 GB GPU (RTX 4050) the placement plan puts 34 of Bielik 4.5B's 60 layers on the GPU and the rest in RAM — slower than fully on the GPU. For comparison: on the CPU alone (4 vCPU) we measured 6.8 tok/s (dress rehearsal 2026-10-06).",
        ),
    );
    vec![
        llm_big(),
        placement,
        stt_turbo(),
        tts(),
        latency(),
        reliability(),
    ]
}

pub(crate) fn quality_4() -> Vec<QualityNote> {
    vec![
        llm_small(),
        perf_small(),
        stt_turbo(),
        tts(),
        latency(),
        reliability(),
    ]
}

pub(crate) fn quality_3() -> Vec<QualityNote> {
    vec![
        llm_small(),
        perf_small(),
        stt_small(),
        tts(),
        latency(),
        reliability(),
    ]
}

pub(crate) fn quality_2() -> Vec<QualityNote> {
    vec![llm_small(), perf_small(), stt_small(), reliability()]
}

pub(crate) fn quality_1() -> Vec<QualityNote> {
    vec![llm_small(), perf_small(), reliability()]
}
