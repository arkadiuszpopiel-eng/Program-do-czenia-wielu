//! Biblioteka pipeline'ów: wbudowane wzorce + własne zapisane przez właściciela (klucz konfiguracji
//! `pipelines.saved`, JSON). Walidacja i operacje na liście są czyste (testowalne bez rdzenia).

use app_api::dto::{PipelineDef, PipelineStage};
use router_contract::Candidate;

/// Klucz konfiguracji z zapisanymi pipeline'ami (JSON w tekście).
pub const SAVED_KEY: &str = "pipelines.saved";
/// Najwięcej etapów w pipeline.
pub const MAX_STAGES: usize = 6;
const MAX_SAVED: usize = 50;
const NAME_MAX: usize = 60;
const GOAL_MAX: usize = 4000;

fn st(title: &str, role: &str, goal: &str) -> PipelineStage {
    PipelineStage {
        title: title.into(),
        role: role.into(),
        persona: None,
        model: None,
        goal: goal.into(),
    }
}

fn def(name: &str, description: &str, stages: Vec<PipelineStage>) -> PipelineDef {
    PipelineDef {
        name: name.into(),
        description: description.into(),
        stages,
        builtin: true,
    }
}

/// Wzorce wbudowane (modele dobiera Router wg roli albo wpisuje właściciel w kopii).
pub fn builtins() -> Vec<PipelineDef> {
    vec![
        def(
            "ui-design",
            "Szkic/opis → wytyczne wizualne → architektura komponentów → kod React/Tailwind.",
            vec![
                st(
                    "Styl i typografia",
                    "thinker",
                    "Na podstawie szkicu/opisu dobierz styl, paletę kolorów, typografię i spacing; wypisz konkretne wytyczne wizualne (tokeny Tailwind, kontrasty).\n\nSzkic / opis:\n{{input}}",
                ),
                st(
                    "Architektura komponentów",
                    "thinker",
                    "Na podstawie wytycznych wizualnych zaprojektuj architekturę komponentów React: drzewo komponentów, propsy, stany (loading/empty/error/disabled), dostępność (role ARIA, fokus, klawiatura).\n\nWytyczne wizualne:\n{{prev}}\n\nOryginalny opis:\n{{input}}",
                ),
                st(
                    "Kod React/Tailwind",
                    "coder",
                    "Wygeneruj finalny kod React + Tailwind zgodny z architekturą i wytycznymi. Jeśli masz katalog roboczy, zapisz pliki i podaj ich ścieżki.\n\nArchitektura i wytyczne:\n{{prev}}\n\nOryginalny opis:\n{{input}}",
                ),
            ],
        ),
        def(
            "code-review",
            "Implementacja → krytyczny przegląd → poprawki z testami.",
            vec![
                st(
                    "Implementacja",
                    "coder",
                    "Zaimplementuj zadanie w repozytorium i opisz zmiany.\n\nZadanie:\n{{input}}",
                ),
                st(
                    "Przegląd",
                    "critic",
                    "Zrób krytyczny przegląd zmian: błędy, przypadki brzegowe, bezpieczeństwo, testy. Wypisz poprawki.\n\nZmiany:\n{{prev}}",
                ),
                st(
                    "Poprawki",
                    "coder",
                    "Wprowadź poprawki z przeglądu i potwierdź wynik testami.\n\nPrzegląd:\n{{prev}}\n\nZadanie:\n{{input}}",
                ),
            ],
        ),
        def(
            "research-report",
            "Badanie tematu → weryfikacja wiarygodności → zwięzły raport.",
            vec![
                st(
                    "Badanie",
                    "researcher",
                    "Zbierz i zweryfikuj informacje do tematu; podaj źródła.\n\nTemat:\n{{input}}",
                ),
                st(
                    "Weryfikacja",
                    "critic",
                    "Oceń wiarygodność i luki w zebranym materiale; popraw lub oznacz wątpliwości.\n\nMateriał:\n{{prev}}",
                ),
                st(
                    "Raport",
                    "writer",
                    "Napisz zwięzły, uporządkowany raport z materiału.\n\nMateriał po weryfikacji:\n{{prev}}\n\nTemat:\n{{input}}",
                ),
            ],
        ),
    ]
}

/// Zapisane pipeline'y z wartości konfiguracji (tekst JSON); błędny zapis = pusta lista.
pub fn parse_saved(value: Option<&serde_json::Value>) -> Vec<PipelineDef> {
    let Some(text) = value.and_then(serde_json::Value::as_str) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<PipelineDef>>(text)
        .map(|v| {
            v.into_iter()
                .map(|d| PipelineDef {
                    builtin: false,
                    ..d
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Wartość do zapisu w konfiguracji.
pub fn encode_saved(saved: &[PipelineDef]) -> String {
    serde_json::to_string(saved).unwrap_or_else(|_| "[]".into())
}

/// Pełna biblioteka: własne (kolejność zapisu) + wbudowane, których nikt nie nadpisał.
pub fn library(saved: &[PipelineDef]) -> Vec<PipelineDef> {
    let mut all: Vec<PipelineDef> = saved.to_vec();
    for b in builtins() {
        if !all.iter().any(|d| d.name.eq_ignore_ascii_case(&b.name)) {
            all.push(b);
        }
    }
    all
}

/// Znajduje pipeline po nazwie (bez rozróżniania wielkości liter, również po samej nazwie bez spacji).
pub fn find<'a>(library: &'a [PipelineDef], name: &str) -> Option<&'a PipelineDef> {
    let n = name.trim();
    library.iter().find(|d| d.name.eq_ignore_ascii_case(n))
}

fn clean_opt(v: Option<&String>) -> Option<String> {
    v.map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
        .map(str::to_owned)
}

/// Model w zapisie `dostawca:model` (pusty / `auto` = dobór Routera).
pub fn clean_model(raw: &str) -> Result<Option<String>, String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    Candidate::parse(t)
        .map(|c| Some(c.qualified()))
        .ok_or_else(|| {
            format!("„{t}” nie jest modelem w zapisie dostawca:model (np. openai:gpt-5).")
        })
}

/// Waliduje i normalizuje pipeline z interfejsu; `Err` — komunikat dla użytkownika.
pub fn validate(def: &PipelineDef) -> Result<PipelineDef, String> {
    let name = def.name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return Err(format!("Nazwa pipeline'u: 1–{NAME_MAX} znaków."));
    }
    if def.stages.is_empty() || def.stages.len() > MAX_STAGES {
        return Err(format!("Pipeline ma mieć od 1 do {MAX_STAGES} etapów."));
    }
    let mut stages = Vec::new();
    for (i, s) in def.stages.iter().enumerate() {
        let n = i + 1;
        let goal = s.goal.trim();
        if goal.is_empty() || goal.chars().count() > GOAL_MAX {
            return Err(format!("Etap {n}: zadanie ma mieć 1–{GOAL_MAX} znaków."));
        }
        let role = s.role.trim();
        let persona = clean_opt(s.persona.as_ref());
        if role.is_empty() && persona.is_none() {
            return Err(format!("Etap {n}: wybierz rolę lub agentkę."));
        }
        let model = match &s.model {
            Some(m) => clean_model(m).map_err(|e| format!("Etap {n}: {e}"))?,
            None => None,
        };
        stages.push(PipelineStage {
            title: s.title.trim().chars().take(60).collect(),
            role: role.to_owned(),
            persona,
            model,
            goal: goal.to_owned(),
        });
    }
    Ok(PipelineDef {
        name: name.to_owned(),
        description: def.description.trim().chars().take(300).collect(),
        stages,
        builtin: false,
    })
}

/// Zapis pipeline'u na liście własnych; `previous` — dotychczasowa nazwa (zmiana nazwy).
pub fn upsert(
    saved: &[PipelineDef],
    def: PipelineDef,
    previous: Option<&str>,
) -> Result<Vec<PipelineDef>, String> {
    let mut list: Vec<PipelineDef> = saved.to_vec();
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    if let Some(prev) = previous.map(str::trim).filter(|p| !p.is_empty())
        && !same(prev, &def.name)
    {
        if list.iter().any(|d| same(&d.name, &def.name)) {
            return Err(format!("Pipeline „{}” już istnieje.", def.name));
        }
        list.retain(|d| !same(&d.name, prev));
    }
    match list.iter().position(|d| same(&d.name, &def.name)) {
        Some(i) => list[i] = def,
        None => {
            if list.len() >= MAX_SAVED {
                return Err(format!("Limit {MAX_SAVED} własnych pipeline'ów."));
            }
            list.push(def);
        }
    }
    Ok(list)
}

/// Usuwa własny pipeline (nadpisany wzorzec wraca do wersji wbudowanej).
pub fn remove(saved: &[PipelineDef], name: &str) -> Vec<PipelineDef> {
    saved
        .iter()
        .filter(|d| !d.name.eq_ignore_ascii_case(name.trim()))
        .cloned()
        .collect()
}

impl crate::engine::ChatEngine {
    /// Biblioteka pipeline'ów (własne z konfiguracji + wbudowane).
    pub(crate) async fn pipeline_library(&self) -> Vec<PipelineDef> {
        library(&parse_saved(self.config_value(SAVED_KEY).await.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> PipelineDef {
        PipelineDef {
            name: name.into(),
            description: String::new(),
            stages: vec![st("A", "coder", "zrób {{input}}")],
            builtin: false,
        }
    }

    #[test]
    fn library_adds_unshadowed_builtins_and_override_wins() {
        assert_eq!(library(&[]).len(), 3);
        let lib = library(&[sample("UI-Design")]);
        assert_eq!(lib.len(), 3);
        assert!(!find(&lib, "ui-design").unwrap().builtin);
    }

    #[test]
    fn validate_normalizes_and_rejects() {
        let mut d = sample("  Mój  ");
        d.stages[0].model = Some("openai:gpt-5".into());
        let v = validate(&d).unwrap();
        assert_eq!(v.name, "Mój");
        assert_eq!(v.stages[0].model.as_deref(), Some("openai:gpt-5"));
        d.stages[0].model = Some("bez-dostawcy".into());
        assert!(validate(&d).is_err());
        d.stages.clear();
        assert!(validate(&d).is_err());
        assert!(validate(&sample(" ")).is_err());
    }

    #[test]
    fn upsert_rename_remove_and_roundtrip() {
        let a = upsert(&[], sample("A"), None).unwrap();
        let b = upsert(&a, sample("B"), None).unwrap();
        assert!(
            upsert(&b, sample("B"), Some("A")).is_err(),
            "kolizja nazw przy zmianie nazwy"
        );
        let c = upsert(&b, sample("C"), Some("A")).unwrap();
        assert_eq!(
            c.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            ["B", "C"]
        );
        let json = serde_json::Value::String(encode_saved(&c));
        assert_eq!(parse_saved(Some(&json)), c);
        assert_eq!(remove(&c, "b").len(), 1);
        assert!(parse_saved(None).is_empty());
    }
}
