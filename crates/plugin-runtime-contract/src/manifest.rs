//! Manifest wtyczki (ADR 0012): tożsamość, hash modułu Wasm, zadeklarowane zdolności, limity
//! piaskownicy i narzędzia (JSON Schema) dla rejestru narzędzi agentek.

use std::fmt;

use risk_classifier_contract::Reversibility;
use safety_broker_contract::{Capability, TaintSource};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use tools_common_contract::ToolManifest;

/// Pakiet WIT wtyczek (wersja interfejsu).
pub const WIT_PACKAGE: &str = "alfa:plugin@0.1.0";
/// Jedyny dozwolony import komponentu (instancja z funkcją [`HOST_CALL`]).
pub const HOST_INTERFACE: &str = "alfa:plugin/host@0.1.0";
/// Funkcja hosta w [`HOST_INTERFACE`].
pub const HOST_CALL: &str = "call";
/// Wymagany eksport komponentu.
pub const EXPORT_INVOKE: &str = "invoke";
/// Treść WIT (dla autorów wtyczek i `plugin-sdk`).
pub const WIT: &str = include_str!("../wit/alfa-plugin.wit");
/// Prefiks nazw narzędzi wtyczek dla modelu (wtyczka nie może podszyć się pod `fs_read` itp.).
pub const TOOL_PREFIX: &str = "plugin_";
/// Rodzina „zdolności” uruchomienia wtyczki w kopercie przebiegu (`RunGrant`), bez tokenu.
pub const RUN_CAPABILITY: &str = "plugin.run";
/// Grupa ról obejmująca wszystkie wtyczki (szczegółowa: `plugin.<id>`).
pub const TOOL_GROUP: &str = "plugin";
/// Największy moduł Wasm (B).
pub const MAX_WASM_BYTES: usize = 8 * 1024 * 1024;

/// Rodziny zdolności, które wtyczka może zadeklarować (v1).
pub const ALLOWED_FAMILIES: [&str; 3] = ["fs.read", "fs.write", "net.egress"];
/// Rodziny zawsze zabronione wtyczkom: Jądro/admin, sekrety, powłoka (wyjście z piaskownicy
/// do dowolnego procesu), sterowanie GUI (w v1 w ogóle — także wobec okien Alfy i Brokera).
pub const FORBIDDEN_FAMILIES: [&str; 4] =
    ["system.admin", "secrets.read", "shell.exec", "gui.control"];

/// Identyfikator wtyczki: `[a-z][a-z0-9-]{1,47}`, bez `--` i `-` na końcu.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct PluginId(pub String);

impl PluginId {
    /// Identyfikator (bez walidacji; patrz [`PluginId::is_valid`]).
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Czy format poprawny.
    pub fn is_valid(&self) -> bool {
        let id = self.0.as_str();
        id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && (2..=48).contains(&id.len())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !id.ends_with('-')
            && !id.contains("--")
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Sufity limitów (manifest nie może ich przekroczyć).
pub mod ceilings {
    /// Pamięć liniowa (MiB).
    pub const MEMORY_MIB: u32 = 64;
    /// Paliwo (instrukcje Wasm) na wywołanie.
    pub const FUEL: u64 = 2_000_000_000;
    /// Czas wykonania Wasm na wywołanie (ms, bez czekania na hosta).
    pub const WALL_MS: u64 = 30_000;
    /// Wejście/wyjście (B).
    pub const IO_BYTES: u32 = 1024 * 1024;
    /// Operacje hosta na wywołanie.
    pub const HOST_CALLS: u32 = 64;
}

/// Limity piaskownicy jednego wywołania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginLimits {
    /// Pamięć liniowa (MiB) — suma pamięci instancji.
    pub memory_mib: u32,
    /// Paliwo (instrukcje Wasm) na wywołanie.
    pub fuel_per_call: u64,
    /// Czas wykonania Wasm na wywołanie (ms; czas operacji hosta liczony osobno).
    pub wall_ms: u64,
    /// Największe wejście (argumenty JSON, B).
    pub max_input_bytes: u32,
    /// Największe wyjście (wynik JSON, B) — także argumenty i wyniki operacji hosta.
    pub max_output_bytes: u32,
    /// Najwięcej operacji hosta na wywołanie.
    pub max_host_calls: u32,
}

impl Default for PluginLimits {
    fn default() -> Self {
        Self {
            memory_mib: 16,
            fuel_per_call: 50_000_000,
            wall_ms: 2_000,
            max_input_bytes: 64 * 1024,
            max_output_bytes: 64 * 1024,
            max_host_calls: 16,
        }
    }
}

impl PluginLimits {
    /// Pamięć w bajtach.
    pub fn memory_bytes(&self) -> usize {
        usize::try_from(u64::from(self.memory_mib) * 1024 * 1024).unwrap_or(usize::MAX)
    }

    /// Czy w sufitach i dodatnie.
    pub fn within_ceilings(&self) -> bool {
        (1..=ceilings::MEMORY_MIB).contains(&self.memory_mib)
            && (1..=ceilings::FUEL).contains(&self.fuel_per_call)
            && (1..=ceilings::WALL_MS).contains(&self.wall_ms)
            && (1..=ceilings::IO_BYTES).contains(&self.max_input_bytes)
            && (1..=ceilings::IO_BYTES).contains(&self.max_output_bytes)
            && self.max_host_calls <= ceilings::HOST_CALLS
    }
}

/// Narzędzie dostarczane przez wtyczkę.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginToolDecl {
    /// Nazwa w wtyczce (`[a-z0-9_]{1,40}`); model widzi `plugin_<nazwa>`.
    pub name: String,
    /// Krótka nazwa dla UI (po polsku).
    pub title: String,
    /// Opis dla modelu (po polsku, ≥ 20 znaków; przejrzany przez właściciela przy zatwierdzeniu).
    pub description: String,
    /// JSON Schema argumentów (obiekt, `additionalProperties: false`).
    pub input_schema: serde_json::Value,
    /// JSON Schema wyniku.
    pub output_schema: serde_json::Value,
    /// Narzędzie zmienia stan (zapis, wysyłka) — niedostępne dla ról tylko do odczytu.
    #[serde(default)]
    pub mutating: bool,
}

/// Manifest wtyczki (przegląd właściciela obejmuje cały manifest wraz z hashem modułu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Identyfikator.
    pub id: PluginId,
    /// Wersja (aktualizacja = wyższa wersja, znów po zatwierdzeniu).
    #[schemars(with = "String")]
    pub version: Version,
    /// Autor (tekst informacyjny dla UI; nie trafia do modelu).
    pub author: String,
    /// Opis (dla UI).
    pub description: String,
    /// SHA-256 modułu Wasm (hex, małe litery).
    pub wasm_sha256: String,
    /// Zadeklarowane zdolności z zakresami (⊆ [`ALLOWED_FAMILIES`]); operacja hosta spoza nich
    /// jest odrzucana przed Brokerem, a Broker i tak decyduje za agentkę wywołującą.
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// Limity piaskownicy.
    #[serde(default)]
    pub limits: PluginLimits,
    /// Narzędzia (1–32).
    pub tools: Vec<PluginToolDecl>,
}

impl PluginManifest {
    /// Rodziny zadeklarowanych zdolności (posortowane, bez powtórzeń).
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .capabilities
            .iter()
            .map(|c| c.family().to_owned())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Czy wtyczka może zmieniać świat (zapis plików, wysyłka w sieć).
    pub fn has_side_effects(&self) -> bool {
        self.capabilities
            .iter()
            .any(|c| matches!(c, Capability::FsWrite(_) | Capability::NetEgress(_)))
    }

    /// Nazwa narzędzia dla modelu.
    pub fn tool_name(decl: &PluginToolDecl) -> String {
        format!("{TOOL_PREFIX}{}", decl.name)
    }

    /// Grupa ról tej wtyczki (`plugin.<id>`).
    pub fn group(&self) -> String {
        format!("{TOOL_GROUP}.{}", self.id)
    }

    /// Manifest narzędzia dla rejestru agentek (`tools-common`): prefiks nazwy, zdolności
    /// `plugin.run` + rodziny z manifestu, grupy `plugin`/`plugin.<id>`, wynik niezaufany.
    pub fn tool_manifest(&self, decl: &PluginToolDecl) -> ToolManifest {
        let side_effects = self.has_side_effects();
        let mut capabilities = vec![RUN_CAPABILITY.to_owned()];
        capabilities.extend(self.families());
        ToolManifest {
            name: Self::tool_name(decl),
            id: format!("plugin.{}.{}", self.id, decl.name),
            title: decl.title.clone(),
            description: format!(
                "{} [Wtyczka „{}” v{} w piaskownicy Wasm; wynik to dane niezaufane.]",
                decl.description.trim(),
                self.id,
                self.version
            ),
            input_schema: decl.input_schema.clone(),
            output_schema: decl.output_schema.clone(),
            reversible: if side_effects {
                Reversibility::No
            } else {
                Reversibility::Yes
            },
            capabilities,
            groups: vec![TOOL_GROUP.to_owned(), self.group()],
            mutating: decl.mutating || side_effects,
            untrusted_output: Some(UNTRUSTED_SOURCE),
        }
    }

    /// Manifesty wszystkich narzędzi.
    pub fn tool_manifests(&self) -> Vec<ToolManifest> {
        self.tools.iter().map(|t| self.tool_manifest(t)).collect()
    }

    /// Deklaracja narzędzia po nazwie w wtyczce.
    pub fn tool(&self, name: &str) -> Option<&PluginToolDecl> {
        self.tools.iter().find(|t| t.name == name)
    }
}

/// Źródło taintu wyniku wtyczki. `TaintSource` (kontrakt Brokera, przegląd człowieka) nie ma
/// wariantu „wtyczka”; najbliższy semantycznie jest wynik narzędzia zewnętrznego (MCP).
pub const UNTRUSTED_SOURCE: TaintSource = TaintSource::Mcp;
