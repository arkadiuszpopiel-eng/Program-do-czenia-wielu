//! `system_software` — co jest zainstalowane na komputerze (przeglądarki, środowiska
//! uruchomieniowe, narzędzia programisty, menedżery pakietów), żeby agentka dobierała narzędzia
//! do maszyny zamiast zgadywać. Tylko odczyt: wykrycie w PATH i `--version` w czystym
//! środowisku, przeglądarki po znanych ścieżkach instalacji.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Rodzaj programu.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareKind {
    /// Przeglądarka internetowa.
    Browser,
    /// Środowisko uruchomieniowe języka (node, python, java, .NET…).
    Runtime,
    /// Kompilator albo narzędzie budowania (rustc, cargo, gcc, cmake…).
    Build,
    /// Kontrola wersji (git, gh).
    Vcs,
    /// Menedżer pakietów (npm, pnpm, pip, winget…).
    PackageManager,
    /// Edytor albo IDE.
    Editor,
    /// Kontenery i maszyny wirtualne (docker, wsl).
    Container,
    /// Inne narzędzia (ffmpeg, curl, 7-Zip…).
    Tool,
}

/// `system_software`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SoftwareArgs {
    /// Tylko jeden rodzaj programów.
    #[serde(default)]
    pub kind: Option<SoftwareKind>,
    /// Wykryj na nowo (zamiast wyniku sprzed kilku minut).
    #[serde(default)]
    pub refresh: Option<bool>,
}

/// Wykryty program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SoftwareItem {
    /// Identyfikator z katalogu (`node`, `chrome`…).
    pub id: String,
    /// Nazwa do pokazania.
    pub name: String,
    /// Rodzaj.
    pub kind: SoftwareKind,
    /// Wersja (`x.y.z`), gdy narzędzie ją podało.
    pub version: Option<String>,
    /// Polecenie do użycia w powłoce (program w PATH) albo ścieżka pliku wykonywalnego.
    pub command: String,
}

/// Wynik `system_software`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SoftwareOut {
    /// Wykryte programy.
    pub items: Vec<SoftwareItem>,
    /// Nazwy popularnych narzędzi, których nie ma (agentka nie powinna ich zakładać).
    pub missing: Vec<String>,
}

/// Wykrywanie oprogramowania (implementacja: PATH + `--version`, ścieżki przeglądarek).
pub trait SoftwareProbe: Send + Sync {
    /// Pełne wykrycie (wolne — kilkadziesiąt krótkich procesów; wywołujący trzyma wynik w pamięci).
    fn scan(&self) -> Vec<SoftwareItem>;
}

/// Program wykrywany przez PATH: identyfikator, nazwa, rodzaj, plik wykonywalny, argumenty wersji.
pub type CliEntry = (
    &'static str,
    &'static str,
    SoftwareKind,
    &'static str,
    &'static [&'static str],
);

/// Katalog programów sprawdzanych w PATH.
pub const CLI_CATALOG: &[CliEntry] = &[
    (
        "node",
        "Node.js",
        SoftwareKind::Runtime,
        "node",
        &["--version"],
    ),
    (
        "deno",
        "Deno",
        SoftwareKind::Runtime,
        "deno",
        &["--version"],
    ),
    ("bun", "Bun", SoftwareKind::Runtime, "bun", &["--version"]),
    (
        "python",
        "Python",
        SoftwareKind::Runtime,
        "python",
        &["--version"],
    ),
    (
        "py",
        "Python Launcher",
        SoftwareKind::Runtime,
        "py",
        &["--version"],
    ),
    ("java", "Java", SoftwareKind::Runtime, "java", &["-version"]),
    (
        "dotnet",
        ".NET",
        SoftwareKind::Runtime,
        "dotnet",
        &["--version"],
    ),
    (
        "pwsh",
        "PowerShell 7",
        SoftwareKind::Runtime,
        "pwsh",
        &["--version"],
    ),
    ("go", "Go", SoftwareKind::Build, "go", &["version"]),
    (
        "rustc",
        "Rust",
        SoftwareKind::Build,
        "rustc",
        &["--version"],
    ),
    (
        "cargo",
        "Cargo",
        SoftwareKind::Build,
        "cargo",
        &["--version"],
    ),
    ("gcc", "GCC", SoftwareKind::Build, "gcc", &["--version"]),
    (
        "clang",
        "Clang",
        SoftwareKind::Build,
        "clang",
        &["--version"],
    ),
    (
        "cmake",
        "CMake",
        SoftwareKind::Build,
        "cmake",
        &["--version"],
    ),
    ("make", "Make", SoftwareKind::Build, "make", &["--version"]),
    ("git", "Git", SoftwareKind::Vcs, "git", &["--version"]),
    ("gh", "GitHub CLI", SoftwareKind::Vcs, "gh", &["--version"]),
    (
        "npm",
        "npm",
        SoftwareKind::PackageManager,
        "npm",
        &["--version"],
    ),
    (
        "pnpm",
        "pnpm",
        SoftwareKind::PackageManager,
        "pnpm",
        &["--version"],
    ),
    (
        "yarn",
        "Yarn",
        SoftwareKind::PackageManager,
        "yarn",
        &["--version"],
    ),
    (
        "pip",
        "pip",
        SoftwareKind::PackageManager,
        "pip",
        &["--version"],
    ),
    (
        "uv",
        "uv",
        SoftwareKind::PackageManager,
        "uv",
        &["--version"],
    ),
    (
        "winget",
        "winget",
        SoftwareKind::PackageManager,
        "winget",
        &["--version"],
    ),
    (
        "choco",
        "Chocolatey",
        SoftwareKind::PackageManager,
        "choco",
        &["--version"],
    ),
    (
        "scoop",
        "Scoop",
        SoftwareKind::PackageManager,
        "scoop",
        &["--version"],
    ),
    (
        "code",
        "Visual Studio Code",
        SoftwareKind::Editor,
        "code",
        &["--version"],
    ),
    (
        "docker",
        "Docker",
        SoftwareKind::Container,
        "docker",
        &["--version"],
    ),
    (
        "ffmpeg",
        "FFmpeg",
        SoftwareKind::Tool,
        "ffmpeg",
        &["-version"],
    ),
    ("curl", "curl", SoftwareKind::Tool, "curl", &["--version"]),
    ("7z", "7-Zip", SoftwareKind::Tool, "7z", &[]),
    (
        "sqlite3",
        "SQLite",
        SoftwareKind::Tool,
        "sqlite3",
        &["--version"],
    ),
];

/// Przeglądarki po znanych ścieżkach (względem `%ProgramFiles%`, `%ProgramFiles(x86)%`,
/// `%LOCALAPPDATA%`): identyfikator, nazwa, ścieżki względne.
pub const BROWSER_CATALOG: &[(&str, &str, &[&str])] = &[
    (
        "edge",
        "Microsoft Edge",
        &[r"Microsoft\Edge\Application\msedge.exe"],
    ),
    (
        "chrome",
        "Google Chrome",
        &[r"Google\Chrome\Application\chrome.exe"],
    ),
    (
        "firefox",
        "Mozilla Firefox",
        &[r"Mozilla Firefox\firefox.exe"],
    ),
    (
        "brave",
        "Brave",
        &[r"BraveSoftware\Brave-Browser\Application\brave.exe"],
    ),
    (
        "opera",
        "Opera",
        &[r"Programs\Opera\opera.exe", r"Opera\opera.exe"],
    ),
    ("vivaldi", "Vivaldi", &[r"Vivaldi\Application\vivaldi.exe"]),
];

/// Popularne narzędzia, o których brak warto powiedzieć agentce wprost.
pub const COMMON: &[&str] = &["node", "python", "git", "dotnet", "java", "docker"];

/// Wersja z wyjścia narzędzia: pierwszy ciąg `cyfry.cyfry[.cyfry…]` (np. `v20.11.1` → `20.11.1`,
/// `git version 2.45.1.windows.1` → `2.45.1`).
pub fn parse_version(text: &str) -> Option<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit()
                    || (bytes[i] == '.' && bytes.get(i + 1).is_some_and(char::is_ascii_digit)))
            {
                if bytes[i] == '.' {
                    dots += 1;
                }
                i += 1;
            }
            if dots >= 1 {
                let v: String = bytes[start..i].iter().collect();
                // `git version 2.45.1.windows.1`: najwyżej trzy człony.
                return Some(v.split('.').take(3).collect::<Vec<_>>().join("."));
            }
        }
        i += 1;
    }
    None
}

/// Zwięzły opis maszyny dla agentki (jedna linia na rodzaj).
pub fn describe(items: &[SoftwareItem], missing: &[String]) -> String {
    let label = |k: SoftwareKind| match k {
        SoftwareKind::Browser => "Przeglądarki",
        SoftwareKind::Runtime => "Środowiska",
        SoftwareKind::Build => "Kompilatory i budowanie",
        SoftwareKind::Vcs => "Kontrola wersji",
        SoftwareKind::PackageManager => "Menedżery pakietów",
        SoftwareKind::Editor => "Edytory",
        SoftwareKind::Container => "Kontenery",
        SoftwareKind::Tool => "Narzędzia",
    };
    let mut kinds: Vec<SoftwareKind> = items.iter().map(|i| i.kind).collect();
    kinds.sort();
    kinds.dedup();
    let mut lines: Vec<String> = kinds
        .into_iter()
        .map(|k| {
            let names: Vec<String> = items
                .iter()
                .filter(|i| i.kind == k)
                .map(|i| match &i.version {
                    Some(v) => format!("{} {v}", i.name),
                    None => i.name.clone(),
                })
                .collect();
            format!("{}: {}", label(k), names.join(", "))
        })
        .collect();
    if !missing.is_empty() {
        lines.push(format!("Brak: {}", missing.join(", ")));
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "software_tests.rs"]
mod tests;
