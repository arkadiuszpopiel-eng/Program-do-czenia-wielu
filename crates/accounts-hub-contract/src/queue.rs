//! Kolejka modeli konta: dla każdej klasy zadań do dziesięciu modeli w kolejności prób. Router
//! bierze pierwszy; następny dopiero, gdy poprzedni zawiedzie (błąd, limit, brak dostępu). Pusta
//! kolejka = wybór automatyczny. Tu są reguły wspólne dla huba i interfejsu: co pasuje do której
//! klasy i jak wygląda propozycja kolejki dla wybranej strategii.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::TaskClass;
use crate::model_use::{ModelUse, chat_rank, is_snapshot, model_use};

/// Najwięcej modeli w kolejce jednej klasy zadań.
pub const MAX_QUEUE: usize = 10;

/// Strategia propozycji kolejki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueueStrategy {
    /// Najmocniejsze modele najpierw.
    Quality,
    /// Modele główne najpierw, potem mniejsze jako zapas.
    Balanced,
    /// Małe i szybkie modele najpierw.
    Economy,
}

/// Wielkość modelu rozpoznana po nazwie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Model główny rodziny.
    Standard,
    /// Odmiana mniejsza i tańsza (`mini`, `flash`, `haiku`, `small`).
    Small,
    /// Odmiana najmniejsza (`nano`, `lite`).
    Tiny,
    /// Odmiana najdroższa (`pro`, `opus`, `ultra`).
    Premium,
}

fn has_part(id: &str, word: &str) -> bool {
    id.to_ascii_lowercase()
        .split(['-', '_', '.', '/', ':'])
        .any(|part| part == word)
}

/// Wielkość modelu po nazwie.
pub fn model_tier(id: &str) -> ModelTier {
    let any = |words: &[&str]| words.iter().any(|word| has_part(id, word));
    if any(&["nano", "lite", "tiny"]) {
        ModelTier::Tiny
    } else if any(&["mini", "flash", "haiku", "small", "fast"]) {
        ModelTier::Small
    } else if any(&["pro", "opus", "ultra", "max"]) {
        ModelTier::Premium
    } else {
        ModelTier::Standard
    }
}

/// Czy model o tym przeznaczeniu nadaje się do klasy zadań.
pub fn use_fits(class: TaskClass, kind: ModelUse) -> bool {
    match class {
        TaskClass::Embeddings => kind == ModelUse::Embedding,
        // Szybka odpowiedź głosowa i sterowanie ekranem nie czekają na model rozumujący.
        TaskClass::VoiceFast | TaskClass::GuiVision => kind == ModelUse::Chat,
        TaskClass::Conversation | TaskClass::Code | TaskClass::Planning | TaskClass::Summarize => {
            kind.is_chat()
        }
    }
}

/// Czy model (po nazwie) nadaje się do klasy zadań; `Err` niesie rozpoznane przeznaczenie.
pub fn fits_class(id: &str, class: TaskClass) -> Result<(), ModelUse> {
    let kind = model_use(id);
    if use_fits(class, kind) {
        Ok(())
    } else {
        Err(kind)
    }
}

/// Po ludzku: do czego służy model o tym przeznaczeniu (do komunikatu odmowy).
fn use_name(kind: ModelUse) -> &'static str {
    match kind {
        ModelUse::Chat => "model rozmowy",
        ModelUse::Reasoning => "model rozumujący (wolny)",
        ModelUse::Embedding => "model embeddingów",
        ModelUse::Speech => "model syntezy mowy",
        ModelUse::Transcription => "model transkrypcji mowy",
        ModelUse::Realtime => "model rozmowy głosowej na żywo",
        ModelUse::Image => "model generowania obrazów",
        ModelUse::Video => "model generowania wideo",
        ModelUse::Moderation => "model moderacji treści",
        ModelUse::Other => "model wyspecjalizowany, bez zwykłej rozmowy",
    }
}

/// Sprawdza kolejkę przed zapisem; `Err` to komunikat dla użytkownika.
pub fn validate_queue(class: TaskClass, models: &[String]) -> Result<(), String> {
    if models.len() > MAX_QUEUE {
        return Err(format!("Kolejka może mieć najwyżej {MAX_QUEUE} modeli."));
    }
    for (at, id) in models.iter().enumerate() {
        if models[..at].contains(id) {
            return Err(format!("Model „{id}” jest w kolejce dwa razy."));
        }
        if let Err(kind) = fits_class(id, class) {
            return Err(format!(
                "„{id}” to {} — nie nadaje się do tej klasy zadań.",
                use_name(kind)
            ));
        }
    }
    Ok(())
}

/// Ocena modelu w danej klasie i strategii (większa = wcześniej w kolejce).
fn score(id: &str, class: TaskClass, strategy: QueueStrategy) -> Option<i64> {
    let kind = model_use(id);
    if !use_fits(class, kind) {
        return None;
    }
    if class == TaskClass::Embeddings {
        let size = match (has_part(id, "large"), has_part(id, "small")) {
            (true, _) => 20,
            (_, true) => 0,
            _ => 10,
        };
        let size = if strategy == QueueStrategy::Economy {
            -size
        } else {
            size
        };
        return Some(size + i64::from(!is_snapshot(id)));
    }
    let base = chat_rank(id)?;
    // `chat_rank` karze modele rozumujące (nie są domyślne w rozmowie); w planowaniu i kodzie
    // to zaleta, więc karę zdejmujemy z nawiązką.
    let reasoning = match (kind, class) {
        (ModelUse::Reasoning, TaskClass::Planning) => 420,
        (ModelUse::Reasoning, TaskClass::Code) => 310,
        _ => 0,
    };
    let tier = model_tier(id);
    // Klasy, w których liczy się czas i koszt, zaczynają od małych modeli nawet przy „jakości”.
    let quick = matches!(class, TaskClass::VoiceFast | TaskClass::Summarize);
    let shape = match (strategy, quick, tier) {
        (QueueStrategy::Economy, _, ModelTier::Tiny) => 400,
        (QueueStrategy::Economy, _, ModelTier::Small) => 300,
        (QueueStrategy::Economy, _, ModelTier::Standard) => 0,
        (QueueStrategy::Economy, _, ModelTier::Premium) => -200,
        (QueueStrategy::Balanced, false, ModelTier::Standard) => 60,
        (QueueStrategy::Balanced, false, ModelTier::Premium) => -40,
        (QueueStrategy::Balanced, true, ModelTier::Small) | (_, true, ModelTier::Small) => 120,
        (QueueStrategy::Balanced, true, ModelTier::Tiny) => 100,
        (QueueStrategy::Quality, false, ModelTier::Premium) => 30,
        _ => 0,
    };
    Some(base + reasoning + shape)
}

/// Propozycja kolejki: najlepiej oceniane modele pasujące do klasy, najwyżej [`MAX_QUEUE`].
/// Przypięte migawki (`…-2024-08-06`) pomijamy, gdy jest dość modeli bez daty — inaczej kolejkę
/// wypełniłyby kopie tego samego modelu.
pub fn suggest_queue<'a>(
    ids: impl IntoIterator<Item = &'a str>,
    class: TaskClass,
    strategy: QueueStrategy,
) -> Vec<&'a str> {
    let mut ranked: Vec<(i64, usize, &'a str)> = ids
        .into_iter()
        .enumerate()
        .filter_map(|(at, id)| score(id, class, strategy).map(|s| (s, at, id)))
        .collect();
    let undated = ranked.iter().filter(|(_, _, id)| !is_snapshot(id)).count();
    if undated >= 3 {
        ranked.retain(|(_, _, id)| !is_snapshot(id));
    }
    // Stabilnie: przy remisie kolejność z listy dostawcy.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    ranked.dedup_by(|a, b| a.2 == b.2);
    ranked
        .into_iter()
        .take(MAX_QUEUE)
        .map(|(_, _, id)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENAI: [&str; 22] = [
        "tts-1",
        "whisper-1",
        "text-embedding-3-small",
        "text-embedding-3-large",
        "text-embedding-ada-002",
        "dall-e-3",
        "gpt-4o-realtime-preview",
        "gpt-3.5-turbo",
        "gpt-4o",
        "gpt-4o-2024-08-06",
        "gpt-4o-mini",
        "gpt-4o-mini-2024-07-18",
        "gpt-4.1",
        "gpt-4.1-mini",
        "gpt-4.1-nano",
        "gpt-5",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-5-pro",
        "o3",
        "o4-mini",
        "omni-moderation-latest",
    ];

    fn queue(class: TaskClass, strategy: QueueStrategy) -> Vec<&'static str> {
        suggest_queue(OPENAI, class, strategy)
    }

    #[test]
    fn suggestions_fit_the_task_and_never_contain_speech_or_image_models() {
        for class in [
            TaskClass::Conversation,
            TaskClass::Code,
            TaskClass::Planning,
            TaskClass::VoiceFast,
            TaskClass::GuiVision,
            TaskClass::Summarize,
            TaskClass::Embeddings,
        ] {
            for strategy in [
                QueueStrategy::Quality,
                QueueStrategy::Balanced,
                QueueStrategy::Economy,
            ] {
                let q = queue(class, strategy);
                assert!(
                    !q.is_empty() && q.len() <= MAX_QUEUE,
                    "{class:?} {strategy:?}"
                );
                for id in &q {
                    assert!(fits_class(id, class).is_ok(), "{id} w {class:?}");
                    assert!(!is_snapshot(id), "{id}: migawka w propozycji");
                }
            }
        }
    }

    #[test]
    fn strategies_and_classes_order_models_differently() {
        let quality = queue(TaskClass::Conversation, QueueStrategy::Quality);
        assert_eq!(quality[0], "gpt-5-pro");
        assert!(quality.contains(&"gpt-5") && !quality.contains(&"tts-1"));
        let balanced = queue(TaskClass::Conversation, QueueStrategy::Balanced);
        assert_eq!(balanced[0], "gpt-5");
        let economy = queue(TaskClass::Conversation, QueueStrategy::Economy);
        assert_eq!(economy[0], "gpt-5-nano");
        // Planowanie zaczyna od modeli rozumujących; szybka odpowiedź głosowa ich nie dopuszcza.
        let at = |list: &[&str], id: &str| list.iter().position(|m| *m == id);
        let planning = queue(TaskClass::Planning, QueueStrategy::Quality);
        assert!(
            at(&planning, "o3") < at(&planning, "gpt-4o"),
            "{planning:?}"
        );
        assert!(at(&planning, "o3").is_some());
        assert!(at(&quality, "o3").is_none_or(|o3| Some(o3) > at(&quality, "gpt-4o")));
        let voice = queue(TaskClass::VoiceFast, QueueStrategy::Quality);
        assert_eq!(voice[0], "gpt-5-mini");
        assert!(!voice.contains(&"o3"));
        assert_eq!(
            queue(TaskClass::Embeddings, QueueStrategy::Quality),
            [
                "text-embedding-3-large",
                "text-embedding-ada-002",
                "text-embedding-3-small"
            ]
        );
        assert_eq!(
            queue(TaskClass::Embeddings, QueueStrategy::Economy)[0],
            "text-embedding-3-small"
        );
    }

    #[test]
    fn wrong_kind_is_named_and_snapshots_fill_in_when_nothing_else_exists() {
        assert_eq!(
            fits_class("tts-1", TaskClass::Conversation),
            Err(ModelUse::Speech)
        );
        assert_eq!(
            fits_class("gpt-4o", TaskClass::Embeddings),
            Err(ModelUse::Chat)
        );
        assert_eq!(
            fits_class("o3", TaskClass::VoiceFast),
            Err(ModelUse::Reasoning)
        );
        assert!(fits_class("nieznany-model", TaskClass::Code).is_ok());
        let only_dated = suggest_queue(
            ["gpt-4o-2024-08-06", "gpt-4o-mini-2024-07-18"],
            TaskClass::Conversation,
            QueueStrategy::Quality,
        );
        assert_eq!(only_dated, ["gpt-4o-2024-08-06", "gpt-4o-mini-2024-07-18"]);
        let list = |ids: &[&str]| ids.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(validate_queue(TaskClass::Conversation, &list(&["gpt-4o", "gpt-4o-mini"])).is_ok());
        let speech = validate_queue(TaskClass::Conversation, &list(&["gpt-4o", "tts-1"]));
        assert!(speech.unwrap_err().contains("syntezy mowy"));
        let twice = validate_queue(TaskClass::Code, &list(&["gpt-4o", "gpt-4o"]));
        assert!(twice.unwrap_err().contains("dwa razy"));
        let many: Vec<String> = (0..11).map(|n| format!("model-{n}")).collect();
        assert!(validate_queue(TaskClass::Code, &many).is_err());
        assert_eq!(model_tier("gpt-5-nano"), ModelTier::Tiny);
        assert_eq!(model_tier("claude-opus-5-5"), ModelTier::Premium);
        assert_eq!(model_tier("gpt-4o"), ModelTier::Standard);
    }
}
