//! Heurystyczny klasyfikator intencji przerwania (reguły PL + podstawowe EN).
//!
//! Kolejność: backchannel → stop/anuluj → zmiana tematu → kontynuuj → korekta → uzupełnienie →
//! pytanie doprecyzowujące → domyślnie korekta (niska pewność). Docelowo mały model lub LLM (SPEC).

use voice_dialog_contract::{
    DialogConfig, IntentResult, InterruptClassifier, InterruptContext, InterruptIntent,
};

use crate::backchannel::words;

const STOP: &[&str] = &[
    "stop",
    "stoj",
    "dosc",
    "wystarczy",
    "przestan",
    "anuluj",
    "zostaw",
    "zostaw to",
    "niewazne",
    "daj spokoj",
    "koniec",
    "cisza",
    "nie trzeba",
    "nie teraz",
    "stop stop",
    "dobra stop",
    "przestan mowic",
    "zatrzymaj sie",
    "nie chce",
    "cancel",
    "never mind",
    "forget it",
    "stop it",
];
const TOPIC: &[&str] = &[
    "a co z",
    "co z",
    "sluchaj",
    "zmienmy temat",
    "inna sprawa",
    "a propos",
    "przy okazji",
    "tak w ogole",
    "wracajac do",
    "zostaw powiedz",
    "zostaw to powiedz",
    "powiedz mi o",
    "opowiedz mi o",
    "zapomnij o tym",
    "a teraz",
    "nowy temat",
    "zajmijmy sie",
    "what about",
    "by the way",
    "change of topic",
];
const CONTINUE: &[&str] = &[
    "dalej",
    "mow dalej",
    "kontynuuj",
    "wznow",
    "no i",
    "no i co",
    "i co dalej",
    "co dalej",
    "tak tak dalej",
    "mhm mow dalej",
    "ok dalej",
    "okej dalej",
    "dobra dalej",
    "jedz dalej",
    "prosze kontynuuj",
    "sorry kontynuuj",
    "przepraszam kontynuuj",
    "go on",
    "continue",
    "mhm dalej",
    "tak dalej",
    "leci dalej",
    "sluchaj dalej",
];
const CORRECTION: &[&str] = &[
    "nie",
    "chodzilo mi o",
    "chodzi mi o",
    "mialem na mysli",
    "mialam na mysli",
    "nie to",
    "zle",
    "blad",
    "pomylka",
    "poprawka",
    "raczej",
    "to nie",
    "nie o to",
    "mowilem o",
    "mowilam o",
    "nie tamten",
    "nie ten",
    "nie ta",
    "tylko ze",
    "no nie",
    "no not",
    "i meant",
    "no i meant",
    "not that",
];
const ADDITION: &[&str] = &[
    "a jeszcze",
    "i jeszcze",
    "jeszcze",
    "dodaj",
    "dodatkowo",
    "oprocz tego",
    "poza tym",
    "i tez",
    "a takze",
    "dorzuc",
    "rowniez",
    "i dodaj",
    "a i",
    "aha i",
    "aha jeszcze",
    "oraz",
    "plus",
    "also",
    "and also",
];
const CLARIFY: &[&str] = &[
    "czyli",
    "czy",
    "a to",
    "to znaczy",
    "znaczy",
    "jak",
    "co to",
    "ktory",
    "ktora",
    "ktore",
    "ile",
    "gdzie",
    "kiedy",
    "dlaczego",
    "po co",
    "a dlaczego",
    "a ile",
    "a kiedy",
    "a gdzie",
    "a jak",
    "a czy",
    "co znaczy",
    "w sensie",
    "what",
    "which",
    "how",
    "why",
    "when",
    "where",
];

fn starts_with(w: &[String], phrase: &str) -> bool {
    let p: Vec<&str> = phrase.split(' ').collect();
    w.len() >= p.len() && w.iter().zip(&p).all(|(a, b)| a == b)
}

fn exact(w: &[String], phrase: &str) -> bool {
    starts_with(w, phrase) && w.len() == phrase.split(' ').count()
}

/// Klasyfikator regułowy.
#[derive(Debug, Clone)]
pub struct HeuristicClassifier {
    backchannel: Vec<String>,
}

impl Default for HeuristicClassifier {
    fn default() -> Self {
        Self {
            backchannel: DialogConfig::default().backchannel_phrases,
        }
    }
}

impl HeuristicClassifier {
    /// Klasyfikator z własną listą fraz backchannelu.
    pub fn new(backchannel: Vec<String>) -> Self {
        Self { backchannel }
    }
}

fn result(intent: InterruptIntent, confidence: f32) -> IntentResult {
    IntentResult { intent, confidence }
}

impl InterruptClassifier for HeuristicClassifier {
    fn classify(&self, ctx: &InterruptContext<'_>) -> IntentResult {
        use InterruptIntent as I;
        let raw = ctx.utterance.trim();
        let w = words(raw);
        if w.is_empty() {
            return result(I::Backchannel, 0.5);
        }
        if self.backchannel.iter().any(|p| words(p) == w) {
            return result(I::Backchannel, 0.9);
        }
        if STOP.iter().any(|p| exact(&w, p)) {
            return result(I::StopCancel, 0.95);
        }
        if TOPIC.iter().any(|p| starts_with(&w, p)) {
            return result(I::TopicChange, 0.85);
        }
        let continue_like = CONTINUE.iter().any(|p| exact(&w, p))
            || (w.len() <= 4 && w.iter().any(|x| x == "dalej" || x == "kontynuuj"));
        if continue_like {
            return result(I::Continue, 0.9);
        }
        let nie_tylko = w.iter().any(|x| x == "nie") && w.iter().any(|x| x == "tylko");
        if CORRECTION.iter().any(|p| starts_with(&w, p)) || nie_tylko {
            return result(I::Correction, 0.85);
        }
        if ADDITION.iter().any(|p| starts_with(&w, p)) {
            return result(I::Addition, 0.8);
        }
        if raw.ends_with('?') || CLARIFY.iter().any(|p| starts_with(&w, p)) {
            return result(I::Clarify, 0.8);
        }
        result(I::Correction, 0.4)
    }
}
