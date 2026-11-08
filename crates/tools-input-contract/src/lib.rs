//! Kontrakt `tools-input` (docs/modules/tools-input/SPEC.md, PLAN §7.2–7.4, §8.2, §16.2 F5/F6).
//!
//! `input_type_text`, `input_keys`, `input_click`, `input_scroll` — wejście syntetyczne do
//! jednego okna: przez Brokera (`gui.control(<aplikacja okna>)`), **nigdy** do okien
//! Alfy/Brokera (strażnik przed każdą paczką zdarzeń w porcie), przerwanie, gdy użytkownik
//! dotknie myszy lub klawiatury (fizyczne wejście ma pierwszeństwo), limit tempa (paczki i liczba
//! wywołań na minutę). Akcje nieodwracalne (`reversible: no`). Wpisywany tekst nigdy nie trafia
//! do zdarzeń ani logów; wynik nie niesie treści ekranu (tylko liczby i nazwę aplikacji).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use platform_contract::{ElementRef, KeyChord};
use risk_classifier_contract::Reversibility;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{ToolManifest, schema_of};

/// Zdarzenie: wysłano wejście (narzędzie, okno, aplikacja, liczba paczek — bez treści).
pub const EVENT_SENT: &str = "tool.input.sent";
/// Maksymalna liczba skrótów w jednym wywołaniu.
pub const MAX_KEYS: usize = 10;

/// `input_type_text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypeArgs {
    /// Okno (`window` z `window_list`) — dostanie fokus.
    pub window: u64,
    /// Tekst (Unicode; `\n` = Enter, `\t` = Tab).
    pub text: String,
}

/// `input_keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeysArgs {
    /// Okno — dostanie fokus.
    pub window: u64,
    /// Skróty po kolei, np. `["Ctrl+A", "Delete"]`, `["Ctrl+S"]`, `["Alt+F4"]` (1–10).
    pub keys: Vec<String>,
}

/// Przycisk myszy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ButtonArg {
    /// Lewy.
    Left,
    /// Prawy.
    Right,
    /// Środkowy.
    Middle,
}

/// `input_click`: punkt względem lewego górnego rogu okna (`x`, `y`) albo środek elementu UIA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClickArgs {
    /// Okno.
    pub window: u64,
    /// X względem okna (px).
    #[serde(default)]
    pub x: Option<i32>,
    /// Y względem okna (px).
    #[serde(default)]
    pub y: Option<i32>,
    /// Element (`element` z `uia_tree`/`uia_find`) zamiast współrzędnych.
    #[serde(default)]
    pub element: Option<String>,
    /// Przycisk (domyślnie lewy).
    #[serde(default)]
    pub button: Option<ButtonArg>,
    /// Podwójne kliknięcie.
    #[serde(default)]
    pub double: Option<bool>,
}

/// `input_scroll`: kółko myszy w punkcie okna (domyślnie środek).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScrollArgs {
    /// Okno.
    pub window: u64,
    /// Ząbki: dodatnie = w górę, ujemne = w dół (±1–±20).
    pub notches: i32,
    /// Przewijanie poziome.
    #[serde(default)]
    pub horizontal: Option<bool>,
    /// X względem okna.
    #[serde(default)]
    pub x: Option<i32>,
    /// Y względem okna.
    #[serde(default)]
    pub y: Option<i32>,
}

/// Wynik narzędzi wejścia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputOutput {
    /// Okno.
    pub window: u64,
    /// Aplikacja.
    pub app: String,
    /// Wysłane paczki zdarzeń.
    pub batches: u32,
    /// Wysłane zdarzenia.
    pub events: u32,
    /// Weryfikacja: okno docelowe nadal na pierwszym planie (klawiatura) / pod punktem (mysz).
    pub verified: bool,
}

/// Limity (`[tools.input]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputToolsConfig {
    /// Wywołań na minutę na sesję (limit tempa).
    pub max_calls_per_minute: u32,
    /// Maksymalny tekst (znaki).
    pub max_text_chars: usize,
}

impl Default for InputToolsConfig {
    fn default() -> Self {
        Self {
            max_calls_per_minute: 60,
            max_text_chars: 5_000,
        }
    }
}

/// Parsuje i sprawdza skróty (zakaz skrótów systemowych — Win, Alt+Tab, Ctrl+Esc, kill-switch).
pub fn parse_keys(keys: &[String]) -> Result<Vec<KeyChord>, String> {
    if keys.is_empty() || keys.len() > MAX_KEYS {
        return Err(format!("podaj 1–{MAX_KEYS} skrótów"));
    }
    keys.iter()
        .map(|k| {
            let chord = KeyChord::parse(k).map_err(|e| e.to_string())?;
            match chord.system_scope() {
                Some(why) => Err(format!("skrót {k}: {why}")),
                None => Ok(chord),
            }
        })
        .collect()
}

/// Sprawdza `input_click`: dokładnie jedno — punkt (`x`+`y`) albo `element`.
pub fn check_click(a: &ClickArgs) -> Result<(), String> {
    match (a.x, a.y, &a.element) {
        (Some(_), Some(_), None) => Ok(()),
        (None, None, Some(e)) => ElementRef::parse(e).map(|_| ()).map_err(|e| e.to_string()),
        _ => Err("podaj `x` i `y` albo `element`".into()),
    }
}

fn manifest(name: &str, title: &str, description: &str, input: serde_json::Value) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        id: format!("tools-input.{}", name.trim_start_matches("input_")),
        title: title.into(),
        description: description.into(),
        input_schema: input,
        output_schema: schema_of::<InputOutput>(),
        reversible: Reversibility::No,
        capabilities: vec!["gui.control".into()],
        groups: vec!["gui.control".into(), "gui.input".into()],
        mutating: true,
        untrusted_output: None,
    }
}

/// Manifesty zestawu.
pub fn manifests() -> Vec<ToolManifest> {
    vec![
        manifest(
            "input_type_text",
            "Wpisz tekst",
            "Wpisuje tekst do okna (najpierw nadaje mu fokus), jakby z klawiatury. Przerywa, gdy użytkownik użyje myszy lub klawiatury. Nie wpisuje do okien Alfy. Najpierw spróbuj `uia_act` z `set_value`.",
            schema_of::<TypeArgs>(),
        ),
        manifest(
            "input_keys",
            "Skróty klawiszowe",
            "Naciska skróty w oknie (np. `Ctrl+S`, `Enter`, `Alt+F4`). Skróty systemowe (klawisz Win, Alt+Tab, Ctrl+Esc) są zablokowane.",
            schema_of::<KeysArgs>(),
        ),
        manifest(
            "input_click",
            "Kliknięcie",
            "Klika w oknie: w punkt względem lewego górnego rogu okna (`x`, `y`, piksele ekranu) albo w środek elementu (`element`). Odmowa, gdy w tym miejscu jest inne okno. Najpierw spróbuj `uia_act` z `invoke`.",
            schema_of::<ClickArgs>(),
        ),
        manifest(
            "input_scroll",
            "Przewijanie",
            "Przewija kółkiem myszy w punkcie okna (domyślnie środek): `notches` dodatnie w górę, ujemne w dół.",
            schema_of::<ScrollArgs>(),
        ),
    ]
}

/// Sprawdza argumenty (ten sam parser i reguły co implementacja).
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    let v = args.clone();
    let err = |e: serde_json::Error| e.to_string();
    match tool {
        "input_type_text" => {
            let a: TypeArgs = serde_json::from_value(v).map_err(err)?;
            if a.text.is_empty() {
                Err("pusty tekst".into())
            } else {
                Ok(())
            }
        }
        "input_keys" => {
            parse_keys(&serde_json::from_value::<KeysArgs>(v).map_err(err)?.keys).map(|_| ())
        }
        "input_click" => check_click(&serde_json::from_value(v).map_err(err)?),
        "input_scroll" => {
            let a: ScrollArgs = serde_json::from_value(v).map_err(err)?;
            if a.notches == 0 || a.notches.abs() > 20 {
                Err("`notches`: ±1–±20".into())
            } else {
                Ok(())
            }
        }
        other => Err(format!("nieznane narzędzie {other}")),
    }
}

/// Przykładowe poprawne argumenty.
pub fn sample_args(tool: &str) -> serde_json::Value {
    match tool {
        "input_type_text" => serde_json::json!({"window": 1, "text": "kontrakt"}),
        "input_keys" => serde_json::json!({"window": 1, "keys": ["Ctrl+S"]}),
        "input_click" => serde_json::json!({"window": 1, "x": 10, "y": 10}),
        _ => serde_json::json!({"window": 1, "notches": -2}),
    }
}

/// Testy kontraktowe zestawu `tools-input` (feature `contract-tests`).
#[cfg(feature = "contract-tests")]
pub mod contract_tests {
    use std::sync::Arc;

    use tools_common_contract::{Tool, contract_tests as common};

    use super::{manifests, sample_args};

    /// Wszystkie narzędzia: manifest, złe argumenty, brak mutacji przy anulowaniu, zakaz Win+R.
    pub async fn run_all(tools: &[Arc<dyn Tool>]) {
        assert_eq!(tools.len(), manifests().len());
        for m in manifests() {
            let tool = tools
                .iter()
                .find(|t| t.manifest().name == m.name)
                .unwrap_or_else(|| panic!("brak narzędzia {}", m.name));
            assert_eq!(tool.manifest(), &m);
            common::run_all(tool.as_ref(), "/", sample_args(&m.name)).await;
            if m.name == "input_keys" {
                let win_r = serde_json::json!({"window": 1, "keys": ["Win+R"]});
                assert!(
                    !tool.call(win_r, &common::ctx("/")).await.is_ok(),
                    "skrót systemowy"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_and_args() {
        for m in manifests() {
            m.validate().unwrap();
            assert_eq!(
                check_args(&m.name, &sample_args(&m.name)),
                Ok(()),
                "{}",
                m.name
            );
            assert!(check_args(&m.name, &serde_json::json!({"zzz": 1})).is_err());
            assert_eq!(m.reversible, Reversibility::No);
            assert!(
                !m.allowed_for(&["gui.control".into()], true),
                "tylko do odczytu = brak wejścia"
            );
        }
        for bad in [
            (
                "input_keys",
                serde_json::json!({"window": 1, "keys": ["Win+R"]}),
            ),
            (
                "input_keys",
                serde_json::json!({"window": 1, "keys": ["Alt+Tab"]}),
            ),
            ("input_keys", serde_json::json!({"window": 1, "keys": []})),
            ("input_click", serde_json::json!({"window": 1, "x": 1})),
            (
                "input_click",
                serde_json::json!({"window": 1, "x": 1, "y": 1, "element": "w1:1"}),
            ),
            (
                "input_click",
                serde_json::json!({"window": 1, "element": "zly"}),
            ),
            (
                "input_scroll",
                serde_json::json!({"window": 1, "notches": 0}),
            ),
            (
                "input_type_text",
                serde_json::json!({"window": 1, "text": ""}),
            ),
            ("nope", serde_json::json!({})),
        ] {
            assert!(check_args(bad.0, &bad.1).is_err(), "{bad:?}");
        }
        assert!(
            check_args(
                "input_click",
                &serde_json::json!({"window": 1, "element": "w1:42.1"})
            )
            .is_ok()
        );
        assert_eq!(
            parse_keys(&["Ctrl+A".into(), "Delete".into()])
                .unwrap()
                .len(),
            2
        );
        assert_eq!(InputToolsConfig::default().max_calls_per_minute, 60);
    }
}
