//! Gramatyka komend PL/EN (edytowalna, R0).
//!
//! Składnia frazy: słowa rozdzielone spacją; `a|b` — alternatywy; `[a|b]` — słowo opcjonalne;
//! `{persona}` — imię persony w dowolnym przypadku (formy z `personas`). Porównania po `fold`
//! (bez wielkości liter i polskich znaków). Wypowiedź jest komendą tylko wtedy, gdy składa się
//! wyłącznie z fraz komend, wypełniaczy i imion — każde inne słowo = zwykła wypowiedź do LLM.

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CommandKind;

/// Reguła: rodzaj komendy + frazy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GrammarRule {
    /// Rodzaj komendy.
    pub command: CommandKind,
    /// Frazy (składnia w opisie modułu).
    pub phrases: Vec<String>,
}

/// Formy imienia persony (mianownik, wołacz, biernik, dopełniacz, narzędnik…).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PersonaForms {
    /// Persona.
    pub persona: PersonaId,
    /// Formy imienia.
    pub forms: Vec<String>,
    /// Formy służące do zwracania się (mianownik/wołacz) — ustawiają adresata.
    pub address_forms: Vec<String>,
}

/// Reguła samodzielnego „nie”.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NieRule {
    /// Słowa uznawane za „nie”.
    pub words: Vec<String>,
    /// Minimalna pauza przed słowem.
    pub min_pause_before_ms: u64,
    /// Minimalna pauza po słowie.
    pub min_pause_after_ms: u64,
}

impl Default for NieRule {
    fn default() -> Self {
        Self {
            words: vec!["nie".into()],
            min_pause_before_ms: 300,
            min_pause_after_ms: 300,
        }
    }
}

/// Gramatyka komend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Grammar {
    /// Reguły komend.
    pub rules: Vec<GrammarRule>,
    /// Wypełniacze dozwolone wokół komendy („proszę”, „dobra”, „yyy”…).
    pub fillers: Vec<String>,
    /// Imiona person.
    pub personas: Vec<PersonaForms>,
    /// Reguła „nie”.
    pub nie: NieRule,
    /// Próg pewności (0–1).
    pub threshold: f32,
    /// Dla transkryptu częściowego: wymagana cisza po ostatnim słowie przed trafieniem.
    pub settle_ms: u64,
}

fn rule(command: CommandKind, phrases: &[&str]) -> GrammarRule {
    GrammarRule {
        command,
        phrases: phrases.iter().map(|p| (*p).to_owned()).collect(),
    }
}

fn persona(id: PersonaId, forms: &[&str], address: &[&str]) -> PersonaForms {
    PersonaForms {
        persona: id,
        forms: forms.iter().map(|f| (*f).to_owned()).collect(),
        address_forms: address.iter().map(|f| (*f).to_owned()).collect(),
    }
}

impl Default for Grammar {
    fn default() -> Self {
        Self::default_pl_en()
    }
}

impl Grammar {
    /// Gramatyka domyślna PL + podstawowe EN.
    pub fn default_pl_en() -> Self {
        use CommandKind as K;
        let rules = vec![
            rule(
                K::StopAll,
                &[
                    "stop|zatrzymaj|wyłącz|zatrzymać wszystko",
                    "stop everything|all",
                    "kill switch",
                ],
            ),
            rule(
                K::Stop,
                &[
                    "stop|stój|sztop|stob|stoj",
                    "zatrzymaj [się]",
                    "przestań [mówić|gadać]",
                    "dość|wystarczy|cisza|koniec|halt",
                    "stop it|talking",
                    "enough",
                ],
            ),
            rule(
                K::Wait,
                &[
                    "czekaj|zaczekaj|poczekaj|czekej [chwilę|chwileczkę|moment|momencik|sekundę|sekundkę]",
                    "chwileczkę|chwilunia|moment|momencik|sekundkę",
                    "wait [a] [second|moment|minute|sec]",
                    "hold on",
                    "one second|moment",
                ],
            ),
            rule(
                K::Pause,
                &["pauza", "zrób|daj pauzę", "wstrzymaj [się]", "pause"],
            ),
            rule(
                K::Resume,
                &[
                    "wznów|kontynuuj|kontynuować",
                    "mów|jedź|leć|czytaj dalej",
                    "dalej",
                    "możesz kontynuować|wznowić",
                    "resume|continue",
                    "go on",
                    "keep going",
                ],
            ),
            rule(
                K::Repeat,
                &[
                    "powtórz [to|proszę] [jeszcze] [raz]",
                    "powtórz ostatnie",
                    "[czy] możesz [to] powtórzyć",
                    "jeszcze raz",
                    "repeat [that]",
                    "say that|it again",
                    "come again",
                ],
            ),
            rule(
                K::VolumeUp,
                &[
                    "głośniej",
                    "mów|zrób|daj głośniej",
                    "podgłośnij|pogłośnij",
                    "louder",
                    "volume up",
                    "speak up",
                ],
            ),
            rule(
                K::VolumeDown,
                &[
                    "ciszej",
                    "mów|zrób|daj ciszej",
                    "ścisz|przycisz",
                    "quieter",
                    "volume down",
                ],
            ),
            rule(
                K::MuteMic,
                &[
                    "wycisz|wyłącz|zablokuj mikrofon|mikrofonik|mic",
                    "mute [the] mic|microphone|mikrofon",
                ],
            ),
            rule(
                K::Cancel,
                &[
                    "anuluj|odwołaj [to]",
                    "nieważne",
                    "zostaw to",
                    "cancel [that|it]",
                    "never mind",
                    "forget it",
                ],
            ),
            rule(
                K::SwitchPersona,
                &[
                    "przełącz|przełączaj|zmień|daj|przejdź|switch [się] [na|do|to] {persona}",
                    "{persona} przejmij",
                ],
            ),
            rule(
                K::DoNotDisturb,
                &[
                    "[włącz] [tryb] nie przeszkadzać|przeszkadzaj [mi]",
                    "do not disturb",
                    "don't disturb",
                ],
            ),
        ];
        let fillers = [
            "proszę",
            "dobra",
            "dobrze",
            "okej",
            "ok",
            "okay",
            "no",
            "hej",
            "ej",
            "już",
            "teraz",
            "natychmiast",
            "yyy",
            "eee",
            "hmm",
            "hm",
            "halo",
            "please",
            "now",
            "hey",
            "sorry",
            "sory",
        ];
        Self {
            rules,
            fillers: fillers.iter().map(|f| (*f).to_owned()).collect(),
            personas: vec![
                persona(
                    PersonaId::alfa(),
                    &["alfa", "alfo", "alfę", "alfy", "alfą", "alfie", "alpha"],
                    &["alfa", "alfo", "alpha"],
                ),
                persona(
                    PersonaId::beta(),
                    &["beta", "beto", "betę", "bety", "betą", "becie"],
                    &["beta", "beto"],
                ),
                persona(
                    PersonaId::gama(),
                    &["gama", "gamo", "gamę", "gamy", "gamą", "gamie", "gamma"],
                    &["gama", "gamo", "gamma"],
                ),
                persona(
                    PersonaId::delta(),
                    &["delta", "delto", "deltę", "delty", "deltą", "delcie"],
                    &["delta", "delto"],
                ),
            ],
            nie: NieRule::default(),
            threshold: 0.6,
            settle_ms: 80,
        }
    }

    /// Frazy dla rodzaju komendy.
    pub fn phrases(&self, kind: CommandKind) -> impl Iterator<Item = &str> {
        self.rules
            .iter()
            .filter(move |r| r.command == kind)
            .flat_map(|r| r.phrases.iter().map(String::as_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grammar_covers_all_commands() {
        let g = Grammar::default_pl_en();
        use CommandKind as K;
        for kind in [
            K::Stop,
            K::Wait,
            K::Pause,
            K::Resume,
            K::Repeat,
            K::Cancel,
            K::VolumeUp,
            K::VolumeDown,
            K::MuteMic,
            K::SwitchPersona,
            K::DoNotDisturb,
            K::StopAll,
        ] {
            assert!(g.phrases(kind).count() > 0, "{kind:?}");
        }
        assert_eq!(g.personas.len(), 4);
        let json = serde_json::to_string(&g).unwrap();
        let back: Grammar = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
    }
}
