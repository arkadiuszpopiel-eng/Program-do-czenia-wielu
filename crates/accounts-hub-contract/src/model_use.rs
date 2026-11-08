//! Do czego służy model o danej nazwie. Lista modeli dostawcy (`GET /models`) miesza modele rozmowy
//! z syntezą mowy, transkrypcją, obrazami i embeddingami, a nie mówi, który jest który — OpenAI
//! zwraca ponad sto pozycji. Rozpoznajemy to po nazwie: wystarczająco, żeby rozmowa nie trafiła do
//! `tts-1`, i żeby zaproponować sensowną kolejność. Ręczny wybór użytkownika zawsze wygrywa.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Przeznaczenie modelu rozpoznane po nazwie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelUse {
    /// Rozmowa i praca na tekście (także z obrazem na wejściu).
    Chat,
    /// Rozumowanie krok po kroku (wolniejsze, droższe; nadal rozmowa).
    Reasoning,
    /// Embeddingi.
    Embedding,
    /// Synteza mowy.
    Speech,
    /// Transkrypcja mowy.
    Transcription,
    /// Rozmowa głosowa na żywo i modele z dźwiękiem na wejściu lub wyjściu.
    Realtime,
    /// Generowanie obrazów.
    Image,
    /// Generowanie wideo.
    Video,
    /// Moderacja treści.
    Moderation,
    /// Starsze modele dopełniania tekstu i wyspecjalizowane (wyszukiwanie, sterowanie komputerem):
    /// nie obsługują zwykłej rozmowy.
    Other,
}

impl ModelUse {
    /// Czy model nadaje się do zwykłej rozmowy (API czatu).
    pub fn is_chat(self) -> bool {
        matches!(self, ModelUse::Chat | ModelUse::Reasoning)
    }
}

/// Nazwa bez przedrostka katalogu (`models/gemini-…` u Google) i małymi literami.
fn normalized(id: &str) -> String {
    let id = id.trim();
    id.strip_prefix("models/")
        .unwrap_or(id)
        .to_ascii_lowercase()
}

/// Czy `id` zawiera `word` jako osobny człon (między `-`, `_`, `.`, `/`, `:` albo na brzegu).
fn has_part(id: &str, word: &str) -> bool {
    id.split(['-', '_', '.', '/', ':']).any(|part| part == word)
}

/// Przeznaczenie modelu po nazwie.
pub fn model_use(id: &str) -> ModelUse {
    let id = normalized(id);
    let has = |needle: &str| id.contains(needle);
    if has("embed") {
        return ModelUse::Embedding;
    }
    if has("moderation") || has("guard") {
        return ModelUse::Moderation;
    }
    if has("whisper") || has("transcribe") || has_part(&id, "stt") || has_part(&id, "asr") {
        return ModelUse::Transcription;
    }
    if has_part(&id, "tts") || has("speech") {
        return ModelUse::Speech;
    }
    if has("realtime") || has_part(&id, "audio") || has_part(&id, "live") || has("native-audio") {
        return ModelUse::Realtime;
    }
    if has("dall-e") || has_part(&id, "image") || has("imagen") || has("image-") {
        return ModelUse::Image;
    }
    if has("sora") || has_part(&id, "veo") || has_part(&id, "video") {
        return ModelUse::Video;
    }
    let legacy = ["babbage", "davinci", "curie"];
    if legacy.iter().any(|name| has(name))
        || has_part(&id, "ada")
        || has_part(&id, "instruct")
        || has_part(&id, "search")
        || has("computer-use")
        || has("deep-research")
        || has_part(&id, "codex")
        || has_part(&id, "rerank")
        || has_part(&id, "ocr")
        || has_part(&id, "aqa")
    {
        return ModelUse::Other;
    }
    let reasoning = id
        .strip_prefix('o')
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        || has_part(&id, "reasoner")
        || has_part(&id, "thinking");
    if reasoning {
        ModelUse::Reasoning
    } else {
        ModelUse::Chat
    }
}

/// Pierwsza liczba w nazwie jako wersja ×10 (`gpt-5.2` → 52, `gpt-4o` → 40, `claude-opus-5-5` → 55).
fn version_tenths(id: &str) -> i64 {
    let bytes = id.as_bytes();
    let Some(start) = bytes.iter().position(u8::is_ascii_digit) else {
        return 0;
    };
    let digits = |from: usize| -> (i64, usize) {
        let len = bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let value = id
            .get(from..from + len)
            .and_then(|text| text.parse::<i64>().ok())
            .unwrap_or(0);
        (value, from + len)
    };
    let (major, after) = digits(start);
    // Rok albo data na początku (`2024-08-06`) to nie numer wersji.
    if major > 99 {
        return 0;
    }
    let minor = match bytes.get(after) {
        Some(b'.' | b'-') if bytes.get(after + 1).is_some_and(u8::is_ascii_digit) => {
            let (value, end) = digits(after + 1);
            // Jedna cyfra to wersja (`4.1`, `5-5`); dłuższy ciąg to data wydania.
            if end - (after + 1) == 1 { value } else { 0 }
        }
        _ => 0,
    };
    major * 10 + minor
}

/// Czy nazwa kończy się datą wydania (`-2024-08-06`, `-20250219`, `-0125`) — przypięta migawka.
pub fn is_snapshot(id: &str) -> bool {
    let id = normalized(id);
    let tail: Vec<&str> = id.rsplit('-').take(3).collect();
    let digits =
        |part: &&str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
    match tail.as_slice() {
        [day, month, year, ..] if digits(day, 2) && digits(month, 2) && digits(year, 4) => true,
        [last, ..] => digits(last, 8) || digits(last, 4),
        [] => false,
    }
}

/// Ocena modelu jako domyślnego do rozmowy (większa = lepszy wybór). `None` dla modeli, które
/// rozmowy nie obsługują. Nowsza wersja wygrywa; odmiany okrojone, podglądowe i przypięte migawki
/// przegrywają z modelem głównym tej samej wersji; modele rozumujące nie są wyborem domyślnym.
pub fn chat_rank(id: &str) -> Option<i64> {
    let kind = model_use(id);
    if !kind.is_chat() {
        return None;
    }
    let id = normalized(id);
    let mut score = version_tenths(&id) * 10;
    if kind == ModelUse::Reasoning {
        score -= 300;
    }
    let penalties: [(&str, i64); 12] = [
        ("nano", 60),
        ("lite", 50),
        ("mini", 40),
        ("haiku", 40),
        ("small", 40),
        ("flash", 20),
        ("preview", 25),
        ("beta", 25),
        ("exp", 25),
        ("pro", 15),
        ("latest", 3),
        ("turbo", 2),
    ];
    for (word, cost) in penalties {
        if has_part(&id, word) {
            score -= cost;
        }
    }
    if is_snapshot(&id) {
        score -= 5;
    }
    Some(score)
}

/// Najlepszy model rozmowy z listy (przy remisie — wcześniejszy na liście dostawcy).
pub fn best_chat_model<'a>(ids: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut best: Option<(i64, &'a str)> = None;
    for id in ids {
        if let Some(rank) = chat_rank(id)
            && best.is_none_or(|(top, _)| rank > top)
        {
            best = Some((rank, id));
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zgłoszenie właściciela (2026-10-09): konto OpenAI ze 133 modelami, a rozmowa szła do
    /// `tts-1`, bo brany był pierwszy model z listy.
    #[test]
    fn speech_and_other_non_chat_models_are_never_picked_for_conversation() {
        let listed = [
            "tts-1",
            "tts-1-hd",
            "whisper-1",
            "text-embedding-3-large",
            "dall-e-3",
            "gpt-image-1",
            "omni-moderation-latest",
            "gpt-4o-realtime-preview",
            "gpt-4o-audio-preview",
            "gpt-4o-mini-tts",
            "gpt-4o-transcribe",
            "babbage-002",
            "davinci-002",
            "gpt-3.5-turbo-instruct",
            "gpt-4o-search-preview",
            "computer-use-preview",
            "sora-2",
            "gpt-4o-mini",
            "gpt-4o",
            "gpt-4o-2024-08-06",
            "o3-mini",
        ];
        for id in &listed[..17] {
            assert!(!model_use(id).is_chat(), "{id}");
            assert_eq!(chat_rank(id), None, "{id}");
        }
        assert_eq!(best_chat_model(listed), Some("gpt-4o"));
        assert_eq!(best_chat_model(["tts-1", "whisper-1"]), None);
    }

    #[test]
    fn uses_are_recognised_across_providers() {
        for (id, expected) in [
            ("gpt-4o", ModelUse::Chat),
            ("gpt-5.2", ModelUse::Chat),
            ("chatgpt-4o-latest", ModelUse::Chat),
            ("o3", ModelUse::Reasoning),
            ("o4-mini", ModelUse::Reasoning),
            ("deepseek-reasoner", ModelUse::Reasoning),
            ("deepseek-chat", ModelUse::Chat),
            ("claude-opus-5-5", ModelUse::Chat),
            ("models/gemini-2.5-flash", ModelUse::Chat),
            ("models/text-embedding-004", ModelUse::Embedding),
            ("models/gemini-2.5-flash-preview-tts", ModelUse::Speech),
            ("mistral-embed", ModelUse::Embedding),
            ("mistral-large-latest", ModelUse::Chat),
            ("mistral-ocr-latest", ModelUse::Other),
            ("grok-4", ModelUse::Chat),
            ("openai/gpt-4o", ModelUse::Chat),
            ("model-testowy", ModelUse::Chat),
        ] {
            assert_eq!(model_use(id), expected, "{id}");
        }
    }

    #[test]
    fn newer_full_models_rank_above_small_dated_and_reasoning_ones() {
        let better = |a: &str, b: &str| {
            assert!(chat_rank(a) > chat_rank(b), "{a} powinien wygrać z {b}");
        };
        better("gpt-5", "gpt-4.1");
        better("gpt-4.1", "gpt-4o");
        better("gpt-4o", "gpt-4o-mini");
        better("gpt-4o", "gpt-4o-2024-08-06");
        better("gpt-4o-mini", "gpt-3.5-turbo");
        better("gpt-5-mini", "gpt-5-nano");
        better("gpt-4o", "o3");
        better("claude-opus-5-5", "claude-haiku-5-5");
        better("gemini-2.5-pro", "gemini-2.5-flash-lite");
        assert!(is_snapshot("gpt-4o-2024-08-06") && is_snapshot("claude-3-5-sonnet-20241022"));
        assert!(is_snapshot("gpt-3.5-turbo-0125") && !is_snapshot("gpt-4o") && !is_snapshot("o3"));
        assert_eq!(version_tenths("gpt-4o-2024-08-06"), 40);
        assert_eq!(version_tenths("claude-opus-5-5"), 55);
    }
}
