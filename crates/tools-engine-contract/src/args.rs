//! Argumenty narzędzi (zamknięte schematy: `deny_unknown_fields`) i ich walidacja — ta sama
//! w `-impl` i `-fake`. Zakresy jak w serwerze silnika (`rpc.rs`, `engine_server.rs`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Scene;

/// Bez argumentów.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

/// `engine_start`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartArgs {
    /// Scena startowa (domyślnie pusta).
    #[serde(default)]
    pub scene: Option<Scene>,
    /// Plik zapisu świata do wczytania (`.json` albo bincode, ≤ 64 MiB) zamiast sceny.
    #[serde(default)]
    pub load: Option<String>,
    /// Czas symulacji stoi do `engine_step` (domyślnie tak — wyniki powtarzalne).
    #[serde(default)]
    pub paused: Option<bool>,
    /// Okno podglądu na ekranie właściciela i zrzut każdej rewizji (`engine_capture`).
    #[serde(default)]
    pub window: Option<bool>,
    /// Wyjście dźwięku.
    #[serde(default)]
    pub audio: Option<bool>,
}

/// Co odczytać w `engine_inspect`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InspectWhat {
    /// Telemetria sceny: encje w promieniu, renderer, fizyka, dźwięk, nawigacja.
    #[default]
    Scene,
    /// Strumieniowanie terenu.
    Streaming,
    /// Zaznaczona encja.
    Selection,
}

/// `engine_inspect`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectArgs {
    /// Co odczytać (domyślnie scena).
    #[serde(default)]
    pub what: Option<InspectWhat>,
    /// Promień w metrach (0–10000, domyślnie 128) — tylko scena.
    #[serde(default)]
    pub radius_meters: Option<f64>,
    /// Środek `[x, y, z]` (domyślnie początek układu) — tylko scena.
    #[serde(default)]
    pub center: Option<[f64; 3]>,
    /// Najwięcej encji (1–256, domyślnie 64) — tylko scena.
    #[serde(default)]
    pub limit: Option<u32>,
}

/// `engine_transaction`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransactionArgs {
    /// Operacje (1–1024) w formacie silnika: obiekty z polem `op`, np.
    /// `{"op":"entity.spawn","entity":{…}}` — opis w `engine_docs`.
    pub operations: Vec<Value>,
    /// Identyfikator (1–128 znaków). Bez niego Alfa nadaje stały identyfikator z treści i kroku,
    /// więc ponowienie tego samego wywołania niczego nie zdubluje.
    #[serde(default)]
    pub transaction_id: Option<String>,
    /// Rewizja, na której opiera się zmiana (konflikt, gdy świat się zmienił).
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

/// `engine_step`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepArgs {
    /// Liczba klatek (1–10000, klatka = 1/60 s).
    pub frames: u32,
}

/// `engine_stream`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamArgs {
    /// Fragmenty terenu na jedno wywołanie (1–32, domyślnie 32).
    #[serde(default)]
    pub budget: Option<u32>,
    /// Powtarzaj, aż teren wokół punktu skupienia będzie gotowy (domyślnie tak, do 30 s).
    #[serde(default)]
    pub until_ready: Option<bool>,
}

/// Wymiar świata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Dimension {
    /// Świat 2D.
    #[serde(rename = "2d")]
    D2,
    /// Świat 3D.
    #[default]
    #[serde(rename = "3d")]
    D3,
}

/// `engine_raycast`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RaycastArgs {
    /// Początek promienia `[x, y, z]`.
    pub origin: [f64; 3],
    /// Kierunek `[x, y, z]` (niezerowy).
    pub direction: [f64; 3],
    /// Najdalej (0–10000 m, domyślnie 1000).
    #[serde(default)]
    pub max_distance: Option<f64>,
    /// Wymiar (domyślnie 3d).
    #[serde(default)]
    pub dimension: Option<Dimension>,
}

/// `engine_height`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeightArgs {
    /// Współrzędna X.
    pub x: f64,
    /// Współrzędna Z.
    pub z: f64,
}

/// `engine_select`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectArgs {
    /// Encja do zaznaczenia; `null` czyści zaznaczenie.
    pub entity_id: Option<u64>,
}

/// `engine_shader`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShaderArgs {
    /// Nazwa shadera.
    pub shader_name: String,
    /// Kod WGSL.
    pub wgsl_code: String,
    /// Identyfikator transakcji (domyślnie stały z treści i kroku).
    #[serde(default)]
    pub transaction_id: Option<String>,
    /// Rewizja, na której opiera się zmiana.
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

/// `engine_preview`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewArgs {
    /// Scena (domyślnie pokazowa).
    #[serde(default)]
    pub scene: Option<Scene>,
    /// Plik zapisu świata zamiast sceny.
    #[serde(default)]
    pub load: Option<String>,
    /// Klatka zrzutu (1–600, domyślnie 3).
    #[serde(default)]
    pub frames: Option<u32>,
    /// Szerokość (64–1920, domyślnie 1280).
    #[serde(default)]
    pub width: Option<u32>,
    /// Wysokość (64–1080, domyślnie 720).
    #[serde(default)]
    pub height: Option<u32>,
}

/// `engine_docs`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocsArgs {
    /// Plik z listy (np. `docs/API_ETAP_4.md`); bez niego — lista plików.
    #[serde(default)]
    pub file: Option<String>,
    /// Od którego znaku czytać (dalsze części długiego pliku).
    #[serde(default)]
    pub offset: Option<u32>,
}

fn range<T: PartialOrd + std::fmt::Display>(
    name: &str,
    v: Option<T>,
    lo: T,
    hi: T,
) -> Result<(), String> {
    match v {
        Some(x) if !(x >= lo && x <= hi) => Err(format!("{name} poza zakresem {lo}–{hi}")),
        _ => Ok(()),
    }
}

fn finite(name: &str, values: &[f64]) -> Result<(), String> {
    if values.iter().all(|v| v.is_finite() && v.abs() <= 1e12) {
        Ok(())
    } else {
        Err(format!("{name}: liczby skończone, |v| ≤ 1e12"))
    }
}

/// Identyfikator transakcji: 1–128 bajtów, bez znaków sterujących.
pub fn check_transaction_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err("transaction_id: 1–128 znaków, bez znaków sterujących".into());
    }
    Ok(())
}

fn parse<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|e| e.to_string())
}

/// Sprawdza argumenty narzędzia.
pub fn check_args(tool: &str, args: &Value) -> Result<(), String> {
    // serde przyjmuje też tablicę jako strukturę (`[1, 2]` → `{x, z}`) — tylko obiekt.
    if !args.is_object() {
        return Err("argumenty muszą być obiektem JSON".into());
    }
    match tool {
        "engine_status" | "engine_stop" | "engine_rollback" | "engine_capture" => {
            parse::<NoArgs>(args).map(|_| ())
        }
        "engine_start" => {
            let a: StartArgs = parse(args)?;
            if a.load.is_some() && a.scene.is_some_and(|s| s != Scene::Empty) {
                return Err("podaj scenę albo plik `load`, nie oba".into());
            }
            Ok(())
        }
        "engine_inspect" => {
            let a: InspectArgs = parse(args)?;
            range("radius_meters", a.radius_meters, 0.0, 10_000.0)?;
            range("limit", a.limit, 1, 256)?;
            finite("center", &a.center.unwrap_or_default())
        }
        "engine_transaction" => {
            let a: TransactionArgs = parse(args)?;
            if a.operations.is_empty() || a.operations.len() > 1024 {
                return Err("operations: 1–1024 operacji".into());
            }
            if let Some(bad) = a.operations.iter().position(|o| {
                o.get("op")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
            }) {
                return Err(format!("operations[{bad}]: obiekt z polem `op`"));
            }
            a.transaction_id
                .as_deref()
                .map_or(Ok(()), check_transaction_id)
        }
        "engine_step" => range("frames", Some(parse::<StepArgs>(args)?.frames), 1, 10_000),
        "engine_stream" => range("budget", parse::<StreamArgs>(args)?.budget, 1, 32),
        "engine_raycast" => {
            let a: RaycastArgs = parse(args)?;
            finite("origin", &a.origin)?;
            finite("direction", &a.direction)?;
            if a.direction.iter().all(|v| *v == 0.0) {
                return Err("direction: wektor niezerowy".into());
            }
            range("max_distance", a.max_distance, 0.0, 10_000.0)
        }
        "engine_height" => {
            let a: HeightArgs = parse(args)?;
            finite("x, z", &[a.x, a.z])
        }
        "engine_select" => parse::<SelectArgs>(args).map(|_| ()),
        "engine_shader" => {
            let a: ShaderArgs = parse(args)?;
            if a.shader_name.trim().is_empty() || a.wgsl_code.trim().is_empty() {
                return Err("shader_name i wgsl_code nie mogą być puste".into());
            }
            a.transaction_id
                .as_deref()
                .map_or(Ok(()), check_transaction_id)
        }
        "engine_preview" => {
            let a: PreviewArgs = parse(args)?;
            if a.load.is_some() && a.scene.is_some() {
                return Err("podaj scenę albo plik `load`, nie oba".into());
            }
            range("frames", a.frames, 1, 600)?;
            range("width", a.width, 64, 1920)?;
            range("height", a.height, 64, 1080)
        }
        "engine_docs" => parse::<DocsArgs>(args).map(|_| ()),
        other => Err(format!("nieznane narzędzie {other}")),
    }
}
