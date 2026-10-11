//! Strażnik rozpoznawania mowy — reguły strojone na sprzęcie w Delcie AI (zadanie P1):
//! odrzut nagrań, w których nic nie ma, flagi podejrzanego transkryptu, lista znanych zmyśleń
//! (bezpiecznik silnika: `breaker.rs`). Czysta logika bez we/wy.

/// Częstotliwość nagrań trafiających do rozpoznawania.
pub const SAMPLE_RATE: usize = 16_000;
const FRAME: usize = SAMPLE_RATE * 20 / 1000;
const MIN_SECONDS: f32 = 0.25;
const MIN_PEAK: f32 = 0.02;
const MIN_SPEECH_MS: f32 = 120.0;
const ACTIVE_FLOOR: f32 = 0.01;
const EMPTY_DESPITE_SPEECH_MS: f32 = 2000.0;
const TOO_SHORT_SPEECH_MS: f32 = 5000.0;
const REPEAT_RUN: usize = 4;

/// Miary nagrania (mono 16 kHz, zakres −1…1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioStats {
    /// Długość w sekundach.
    pub seconds: f32,
    /// Szczyt amplitudy.
    pub peak: f32,
    /// Ile milisekund faktycznie coś słychać (ramki 20 ms głośniejsze od tła).
    pub speech_ms: f32,
}

/// Powód odrzucenia nagrania przed rozpoznawaniem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Krótsze niż 0,25 s.
    TooShort,
    /// Szczyt poniżej 0,02 — brak sygnału z mikrofonu.
    NoSignal,
    /// Mniej niż 120 ms aktywnego sygnału.
    NoSpeech,
}

/// Miary nagrania. Tło liczone z 10. centyla głośności ramek (przerwy między słowami), więc cichy
/// mikrofon też się liczy, a nagranie pełne mowy nie wychodzi „prawie ciche”.
pub fn audio_stats(pcm: &[f32]) -> AudioStats {
    if pcm.is_empty() {
        return AudioStats::default();
    }
    #[allow(clippy::cast_precision_loss)]
    let seconds = pcm.len() as f32 / SAMPLE_RATE as f32;
    let peak = pcm.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let mut frames: Vec<f32> = pcm
        .chunks_exact(FRAME)
        .map(|f| {
            #[allow(clippy::cast_precision_loss)]
            let mean =
                f.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>() / f.len() as f64;
            #[allow(clippy::cast_possible_truncation)]
            let rms = mean.sqrt() as f32;
            rms
        })
        .collect();
    if frames.is_empty() {
        return AudioStats {
            seconds,
            peak,
            speech_ms: 0.0,
        };
    }
    let mut sorted = frames.clone();
    sorted.sort_by(f32::total_cmp);
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let floor = sorted[((sorted.len() - 1) as f32 * 0.1).round() as usize];
    let threshold = ACTIVE_FLOOR.max(floor * 2.5);
    frames.retain(|rms| *rms >= threshold);
    #[allow(clippy::cast_precision_loss)]
    let speech_ms = frames.len() as f32 * 20.0;
    AudioStats {
        seconds,
        peak,
        speech_ms,
    }
}

/// Czy nagranie w ogóle nadaje się do rozpoznawania. Odrzucamy tylko pustkę — ciche mówienie ma
/// trafić do silnika.
pub fn reject_reason(stats: &AudioStats) -> Option<Reject> {
    if stats.seconds < MIN_SECONDS {
        Some(Reject::TooShort)
    } else if stats.peak < MIN_PEAK {
        Some(Reject::NoSignal)
    } else if stats.speech_ms < MIN_SPEECH_MS {
        Some(Reject::NoSpeech)
    } else {
        None
    }
}

/// Flaga jakości transkryptu (nazwy stałe — trafiają do telemetrii).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Flag {
    /// Wykryty język inny niż żądany i spoza pary polski / angielski.
    UnexpectedLanguage,
    /// Zapętlony dekoder: to samo słowo wiele razy.
    RepeatedTokens,
    /// Słychać było mowę (≥ 2 s), a tekst jest pusty.
    EmptyDespiteSpeech,
    /// Długa mowa (≥ 5 s), a tekst ma najwyżej jedno słowo.
    TooShortForAudio,
}

impl Flag {
    /// Nazwa do dziennika i telemetrii.
    pub fn name(self) -> &'static str {
        match self {
            Flag::UnexpectedLanguage => "unexpected_language",
            Flag::RepeatedTokens => "repeated_tokens",
            Flag::EmptyDespiteSpeech => "speech_detected_but_empty_transcript",
            Flag::TooShortForAudio => "transcript_too_short_for_audio",
        }
    }
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’' || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// To samo słowo ≥ 4 razy z rzędu albo jedno słowo to ≥ 60 % tekstu z co najmniej 8 słów.
pub fn has_repeated_tokens(text: &str) -> bool {
    let words = words(text);
    if words.len() < REPEAT_RUN {
        return false;
    }
    let mut run = 1;
    for pair in words.windows(2) {
        run = if pair[0] == pair[1] { run + 1 } else { 1 };
        if run >= REPEAT_RUN {
            return true;
        }
    }
    if words.len() >= 8 {
        let mut sorted = words.clone();
        sorted.sort();
        let top = sorted
            .chunk_by(|a, b| a == b)
            .map(<[String]>::len)
            .max()
            .unwrap_or(0);
        return top * 10 >= words.len() * 6;
    }
    false
}

/// Język wykryty przez model nie pasuje do żądanego i nie jest polskim ani angielskim — typowa
/// oznaka zmyślenia na ciszy albo szumie. Brak informacji o języku niczego nie flaguje.
pub fn unexpected_language(requested: Option<&str>, detected: Option<&str>) -> bool {
    let short = |l: &str| l.trim().to_lowercase().chars().take(2).collect::<String>();
    match (requested.map(short), detected.map(short)) {
        (Some(req), Some(det)) if !req.is_empty() && !det.is_empty() => {
            det != req && det != "pl" && det != "en"
        }
        _ => false,
    }
}

/// Flagi jakości. Transkrypt częściowy (mowa jeszcze trwa) dostaje tylko dwie pierwsze.
pub fn quality_flags(
    text: &str,
    stats: &AudioStats,
    requested_lang: Option<&str>,
    detected_lang: Option<&str>,
    partial: bool,
) -> Vec<Flag> {
    let clean = text.trim();
    let mut flags = Vec::new();
    if !clean.is_empty() && unexpected_language(requested_lang, detected_lang) {
        flags.push(Flag::UnexpectedLanguage);
    }
    if !clean.is_empty() && has_repeated_tokens(clean) {
        flags.push(Flag::RepeatedTokens);
    }
    if !partial {
        if clean.is_empty() && stats.speech_ms >= EMPTY_DESPITE_SPEECH_MS {
            flags.push(Flag::EmptyDespiteSpeech);
        } else if !clean.is_empty()
            && stats.speech_ms >= TOO_SHORT_SPEECH_MS
            && words(clean).len() <= 1
        {
            flags.push(Flag::TooShortForAudio);
        }
    }
    flags
}

/// Frazy, które modele rozpoznawania „słyszą” w ciszy (podpisy z filmów w danych treningowych).
/// Użytkownik nie mówi ich do asystentki, więc bezpiecznie je odrzucić.
const HALLUCINATIONS: &[&str] = &[
    "dziekuje za ogladanie",
    "dzieki za ogladanie",
    "dziekuje za uwage",
    "dzieki za uwage",
    "napisy stworzone przez spolecznosc amara org",
    "napisy",
    "napisy amara org",
    "subskrybuj",
    "zasubskrybuj",
    "zapraszam do subskrypcji",
    "prosze o subskrypcje",
    "kliknij dzwoneczek",
    "do zobaczenia",
    "do zobaczenia w nastepnym odcinku",
    "zapraszam na kolejny film",
    "to wszystko na dzisiaj",
    "dziekuje za wspolne ogladanie",
    "dziekuje panstwu",
    "dziekuje za obejrzenie",
    "polajkuj i zasubskrybuj",
];
const HALLUCINATION_MARKS: &[&str] = &[
    "napisy stworzone przez",
    "amara org",
    "za ogladanie",
    "subskry",
];

/// Małe litery, bez polskich znaków i interpunkcji, pojedyncze spacje.
pub fn normalize_pl(text: &str) -> String {
    let mapped: String = text
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            c if c.is_ascii_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Znane zmyślenie na ciszy: dokładna fraza z listy albo krótki tekst z jednoznacznym podpisem.
/// Pojedyncze prawdziwe słowa (samo „dziękuję”) nie są tu łapane.
pub fn is_hallucination(text: &str) -> bool {
    let norm = normalize_pl(text);
    norm.is_empty()
        || HALLUCINATIONS.contains(&norm.as_str())
        || (norm.len() <= 60 && HALLUCINATION_MARKS.iter().any(|m| norm.contains(m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: usize, amp: f32) -> Vec<f32> {
        #[allow(clippy::cast_precision_loss)]
        (0..SAMPLE_RATE * ms / 1000)
            .map(|i| (i as f32 * 0.2).sin() * amp)
            .collect()
    }

    #[test]
    fn silence_noise_and_short_clips_are_rejected_quiet_speech_passes() {
        assert_eq!(
            reject_reason(&audio_stats(&tone(100, 0.3))),
            Some(Reject::TooShort)
        );
        assert_eq!(
            reject_reason(&audio_stats(&vec![0.001; SAMPLE_RATE])),
            Some(Reject::NoSignal)
        );
        // Jedno krótkie stuknięcie w sekundzie ciszy: jest szczyt, nie ma mowy.
        let mut click = vec![0.0; SAMPLE_RATE];
        click[100..400].fill(0.5);
        assert_eq!(reject_reason(&audio_stats(&click)), Some(Reject::NoSpeech));
        // Ciche mówienie (0,03) z przerwami przechodzi.
        let mut quiet = tone(400, 0.03);
        quiet.extend(vec![0.0005; SAMPLE_RATE / 2]);
        quiet.extend(tone(400, 0.03));
        let stats = audio_stats(&quiet);
        assert_eq!(reject_reason(&stats), None, "{stats:?}");
        assert!(
            stats.speech_ms >= 700.0 && stats.speech_ms <= 900.0,
            "{stats:?}"
        );
        assert_eq!(audio_stats(&[]), AudioStats::default());
    }

    #[test]
    fn repeated_tokens_detects_loops_but_not_emphasis() {
        assert!(has_repeated_tokens("tak tak tak tak"));
        assert!(!has_repeated_tokens("nie, nie, nie"));
        assert!(has_repeated_tokens("to to jest to to a to to to to"));
        assert!(!has_repeated_tokens(
            "to jest zwykłe zdanie bez żadnych powtórzeń w środku"
        ));
    }

    #[test]
    fn language_flag_ignores_polish_and_english() {
        assert!(unexpected_language(Some("pl"), Some("zh")));
        assert!(unexpected_language(
            Some("pl-PL"),
            Some("Chinese".get(..2).unwrap_or("ch"))
        ));
        assert!(!unexpected_language(Some("pl"), Some("en")));
        assert!(!unexpected_language(Some("pl"), Some("pl")));
        assert!(!unexpected_language(Some("pl"), None));
        assert!(!unexpected_language(None, Some("zh")));
    }

    #[test]
    fn flags_for_final_and_partial_transcripts() {
        let long = AudioStats {
            seconds: 6.0,
            peak: 0.3,
            speech_ms: 5500.0,
        };
        assert_eq!(
            quality_flags("", &long, Some("pl"), Some("pl"), false),
            vec![Flag::EmptyDespiteSpeech]
        );
        assert_eq!(
            quality_flags("tak", &long, Some("pl"), Some("pl"), false),
            vec![Flag::TooShortForAudio]
        );
        assert!(
            quality_flags("", &long, Some("pl"), Some("pl"), true).is_empty(),
            "partial"
        );
        assert_eq!(
            quality_flags("你好 你好 你好 你好", &long, Some("pl"), Some("zh"), true),
            vec![Flag::UnexpectedLanguage, Flag::RepeatedTokens]
        );
        assert!(
            quality_flags(
                "Dzień dobry, co słychać?",
                &long,
                Some("pl"),
                Some("pl"),
                false
            )
            .is_empty()
        );
        assert_eq!(
            Flag::EmptyDespiteSpeech.name(),
            "speech_detected_but_empty_transcript"
        );
    }

    #[test]
    fn hallucinations_are_caught_real_words_are_not() {
        assert!(is_hallucination("Dzięki za oglądanie!"));
        assert!(is_hallucination(
            "Napisy stworzone przez społeczność Amara.org"
        ));
        assert!(is_hallucination("  ...  "));
        assert!(is_hallucination("Zasubskrybuj kanał"));
        assert!(!is_hallucination("Dziękuję"));
        assert!(!is_hallucination("Włącz napisy w filmie, który oglądam"));
        assert_eq!(normalize_pl("Zażółć  GĘŚLĄ, jaźń!"), "zazolc gesla jazn");
    }
}
