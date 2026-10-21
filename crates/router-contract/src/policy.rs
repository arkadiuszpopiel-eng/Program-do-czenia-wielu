//! Polityka tras: klasa → uporządkowana lista kandydatów, terminy pierwszego zdarzenia,
//! obwód, Mówczyni + Myślicielka (§6.9 — tylko typy i konfiguracja; orkiestracja: agent-runtime).

use std::collections::BTreeMap;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TaskClass;
use crate::breaker::BreakerConfig;
use crate::candidate::Candidate;

/// Wszystkie klasy zadań (kolejność stała).
pub const ALL_CLASSES: [TaskClass; 7] = [
    TaskClass::VoiceFast,
    TaskClass::Conversation,
    TaskClass::Code,
    TaskClass::Planning,
    TaskClass::GuiVision,
    TaskClass::Summarize,
    TaskClass::Embeddings,
];

/// Tempo pracy agentki w trybie „Mówczyni + Myślicielka".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tempo {
    /// Mówczyni: szybka odpowiedź, krótkie raporty postępu.
    Fast,
    /// Myślicielka: głęboka praca w tle.
    Deep,
}

/// Mówczyni + Myślicielka (PLAN §6.9): persony i klasy tras dla obu temp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DuoConfig {
    /// Persona Mówczyni (domyślnie Alfa).
    pub speaker: String,
    /// Persona Myślicielki / Krytyczki (domyślnie Gama).
    pub thinker: String,
    /// Klasa trasy tempa szybkiego.
    pub fast: TaskClass,
    /// Klasa trasy tempa głębokiego.
    pub deep: TaskClass,
}

impl Default for DuoConfig {
    fn default() -> Self {
        Self {
            speaker: "alfa".into(),
            thinker: "gama".into(),
            fast: TaskClass::VoiceFast,
            deep: TaskClass::Planning,
        }
    }
}

impl DuoConfig {
    /// Klasa trasy dla tempa.
    pub fn class_for(&self, tempo: Tempo) -> TaskClass {
        match tempo {
            Tempo::Fast => self.fast,
            Tempo::Deep => self.deep,
        }
    }
}

/// Polityka Routera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoutePolicy {
    /// Klasa → kandydaci w kolejności preferencji.
    pub classes: BTreeMap<TaskClass, Vec<Candidate>>,
    /// Termin pierwszego zdarzenia strumienia (ms) — po nim fallback (ACC F1-04: ≤ 2 s).
    pub first_event_ms: BTreeMap<TaskClass, u64>,
    /// Obwód.
    pub breaker: BreakerConfig,
    /// Mówczyni + Myślicielka.
    pub duo: DuoConfig,
}

/// Dodatek do terminu pierwszego zdarzenia na 1000 tokenów wejścia ponad
/// [`DEADLINE_FREE_TOKENS`] (ms): adaptery zgodne z OpenAI emitują `Started` z pierwszą porcją
/// SSE, czyli po przetworzeniu promptu — duży kontekst agentki przekraczał 1,5 s i przebieg spadał
/// na model lokalny (przegląd 2026-10-21).
pub const DEADLINE_MS_PER_KTOK: u64 = 250;
/// Kontekst bez dodatku do terminu — zwykła rozmowa (prompt systemowy, pamięć, narzędzia)
/// zostaje przy terminie klasy, więc przełączenie po awarii trzyma ACC F1-04 (≤ 2 s).
pub const DEADLINE_FREE_TOKENS: u64 = 8_000;
/// Najdłuższy termin pierwszego zdarzenia po dodatku za długi prompt (ms).
pub const MAX_FIRST_EVENT_MS: u64 = 30_000;

/// Domyślne terminy pierwszego zdarzenia: interaktywne ≤ 1,5 s (głos 1,2 s), tło 5 s.
pub fn default_deadlines() -> BTreeMap<TaskClass, u64> {
    ALL_CLASSES
        .iter()
        .map(|c| {
            let ms = match c {
                TaskClass::VoiceFast => 1_200,
                TaskClass::Summarize | TaskClass::Embeddings => 5_000,
                _ => 1_500,
            };
            (*c, ms)
        })
        .collect()
}

impl RoutePolicy {
    /// Polityka domyślna (PLAN §5.4, ADR 0014): bez kluczy API wszystko na model lokalny
    /// (profil A); z kluczem — rozmowa, kod, planowanie, wizja i streszczanie przez API
    /// (lokalny jako fallback/offline), głos-szybka i embeddingi najpierw lokalnie.
    pub fn defaults(local: Option<&Candidate>, api: &[Candidate]) -> Self {
        let local: Vec<Candidate> = local.cloned().into_iter().collect();
        let classes = ALL_CLASSES
            .iter()
            .map(|class| {
                let list: Vec<Candidate> = match class {
                    TaskClass::VoiceFast | TaskClass::Embeddings => {
                        local.iter().chain(api).cloned().collect()
                    }
                    _ => api.iter().chain(&local).cloned().collect(),
                };
                (*class, list)
            })
            .collect();
        Self {
            classes,
            first_event_ms: default_deadlines(),
            breaker: BreakerConfig::default(),
            duo: DuoConfig::default(),
        }
    }

    /// Kandydaci klasy.
    pub fn candidates(&self, class: TaskClass) -> &[Candidate] {
        self.classes.get(&class).map_or(&[], Vec::as_slice)
    }

    /// Termin pierwszego zdarzenia klasy (`None` = wyłączony).
    pub fn first_event_deadline(&self, class: TaskClass) -> Option<Duration> {
        self.first_event_ms
            .get(&class)
            .map(|ms| Duration::from_millis(*ms))
    }

    /// Termin pierwszego zdarzenia dla żądania z `input_tokens` tokenami wejścia: termin klasy
    /// plus [`DEADLINE_MS_PER_KTOK`] na 1000 tokenów ponad [`DEADLINE_FREE_TOKENS`], najwyżej
    /// [`MAX_FIRST_EVENT_MS`] (albo termin klasy, gdy jest dłuższy). Zwykła rozmowa — bez zmian
    /// (ACC F1-04: ≤ 2 s).
    pub fn first_event_deadline_for(
        &self,
        class: TaskClass,
        input_tokens: u64,
    ) -> Option<Duration> {
        self.first_event_ms.get(&class).map(|ms| {
            let extra = input_tokens
                .saturating_sub(DEADLINE_FREE_TOKENS)
                .saturating_mul(DEADLINE_MS_PER_KTOK)
                / 1000;
            let ms = ms.saturating_add(extra).min(MAX_FIRST_EVENT_MS.max(*ms));
            Duration::from_millis(ms)
        })
    }

    /// Nakłada sekcję `[router]` z TOML na tę politykę (klasy z pliku zastępują domyślne).
    pub fn with_toml(mut self, text: &str) -> Result<Self, String> {
        let t: RouterToml = toml::from_str(text).map_err(|e| format!("[router]: {e}"))?;
        for (class, c) in t.class {
            self.classes.insert(class, c.prefer);
        }
        for (class, value) in t.deadline {
            if value == "off" {
                self.first_event_ms.remove(&class);
            } else {
                let d = parse_duration(&value)
                    .ok_or_else(|| format!("[router.deadline] niepoprawny czas `{value}`"))?;
                self.first_event_ms
                    .insert(class, u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            }
        }
        if let Some(b) = t.breaker {
            let ms = |v: Option<String>, dflt: u64| -> Result<u64, String> {
                v.map_or(Ok(dflt), |s| {
                    parse_duration(&s)
                        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
                        .ok_or_else(|| format!("[router.breaker] niepoprawny czas `{s}`"))
                })
            };
            self.breaker = BreakerConfig {
                failures: b.failures.unwrap_or(self.breaker.failures).max(1),
                window_ms: ms(b.window, self.breaker.window_ms)?,
                cooldown_ms: ms(b.cooldown, self.breaker.cooldown_ms)?,
            };
        }
        if let Some(s) = t.speaker {
            self.duo.speaker = s;
        }
        if let Some(s) = t.thinker {
            self.duo.thinker = s;
        }
        Ok(self)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouterToml {
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    thinker: Option<String>,
    #[serde(default)]
    breaker: Option<BreakerToml>,
    #[serde(default)]
    deadline: BTreeMap<TaskClass, String>,
    #[serde(default)]
    class: BTreeMap<TaskClass, ClassToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BreakerToml {
    failures: Option<u32>,
    window: Option<String>,
    cooldown: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassToml {
    prefer: Vec<Candidate>,
}

/// Czas w formacie `"500ms"`, `"30s"`, `"10m"`, `"1h"`.
pub fn parse_duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit())?;
    let (num, unit) = text.split_at(split);
    let n: u64 = num.parse().ok()?;
    match unit {
        "ms" => Some(Duration::from_millis(n)),
        "s" => Some(Duration::from_secs(n)),
        "m" => Some(Duration::from_secs(n.checked_mul(60)?)),
        "h" => Some(Duration::from_secs(n.checked_mul(3_600)?)),
        _ => None,
    }
}
