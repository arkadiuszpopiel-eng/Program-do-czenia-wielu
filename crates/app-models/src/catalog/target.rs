//! Wymiar system / architektura katalogu (etap B.6, plan 02 „Potrzebny szew”): binarki silników
//! (llama-server, whisper-server, Piper, ONNX Runtime, DirectML) są budowane pod konkretny system
//! i procesor, a modele (GGUF, ONNX, wagi) — nie. Pozycja z pustą listą celów działa wszędzie;
//! pozycja z listą trafia do katalogu tylko na pasującym komputerze. Nowy system albo ARM64 to
//! nowe wiersze w katalogu z własnym archiwum i sumą — bez zmian w rdzeniu.

/// System operacyjny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// Windows.
    Windows,
    /// Linux.
    Linux,
    /// macOS.
    MacOs,
}

/// Architektura procesora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    /// x86-64.
    X64,
    /// ARM64 (aarch64).
    Arm64,
}

/// Para system / architektura, pod którą zbudowano plik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// System.
    pub os: Os,
    /// Procesor.
    pub arch: Arch,
}

impl Target {
    /// Windows na x86-64 — wszystkie dzisiejsze binarki katalogu.
    pub const WINDOWS_X64: Self = Self {
        os: Os::Windows,
        arch: Arch::X64,
    };

    /// Cel z nazw `std::env::consts::{OS, ARCH}`; `None` — system albo procesor spoza listy.
    pub fn parse(os: &str, arch: &str) -> Option<Self> {
        let os = match os {
            "windows" => Os::Windows,
            "linux" => Os::Linux,
            "macos" => Os::MacOs,
            _ => return None,
        };
        let arch = match arch {
            "x86_64" => Arch::X64,
            "aarch64" => Arch::Arm64,
            _ => return None,
        };
        Some(Self { os, arch })
    }

    /// Ten komputer (`None` — nieznany: pozycje związane z systemem nie trafią do katalogu).
    pub fn host() -> Option<Self> {
        Self::parse(std::env::consts::OS, std::env::consts::ARCH)
    }
}

/// Czy pozycja z celami `targets` działa na `host` (pusta lista — wszędzie, także na nieznanym).
pub fn runs_on(targets: &[Target], host: Option<Target>) -> bool {
    targets.is_empty() || host.is_some_and(|h| targets.contains(&h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_pairs_only() {
        assert_eq!(
            Target::parse("windows", "x86_64"),
            Some(Target::WINDOWS_X64)
        );
        let arm = Target::parse("linux", "aarch64");
        assert_eq!(
            arm,
            Some(Target {
                os: Os::Linux,
                arch: Arch::Arm64
            })
        );
        assert_eq!(Target::parse("windows", "x86"), None);
        assert_eq!(Target::parse("freebsd", "x86_64"), None);
    }

    #[test]
    fn empty_list_runs_everywhere_and_bound_item_only_on_its_target() {
        let linux = Target::parse("linux", "x86_64");
        assert!(runs_on(&[], linux));
        assert!(runs_on(&[], None));
        assert!(runs_on(&[Target::WINDOWS_X64], Some(Target::WINDOWS_X64)));
        assert!(!runs_on(&[Target::WINDOWS_X64], linux));
        assert!(!runs_on(&[Target::WINDOWS_X64], None));
    }

    #[test]
    fn host_is_known_on_supported_builds() {
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert_eq!(Target::host(), Some(Target::WINDOWS_X64));
        }
    }
}
