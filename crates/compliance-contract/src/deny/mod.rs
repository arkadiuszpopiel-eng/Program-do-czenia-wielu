//! Deny-listy Jądra jako dane (subscription-routes.md §2.1–2.2): ścieżki poświadczeń CLI,
//! profile przeglądarek, magazyny Credential Manager, domeny webowych UI dostawców.

mod domain;
mod path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::ComplianceError;

pub use domain::{host_matches, normalize_host};
pub use path::{NormPath, PathEnv, Root, comp_matches, normalize, percent_decode};

/// Segmenty ścieżek, których nie może usunąć nawet Broker (AGENTS.md: nigdy tokenów CLI).
pub const MANDATORY_PATH_SEGMENTS: [&str; 2] = [".claude", ".codex"];

/// Dowód uprawnień Brokera do zmiany deny-list (dane Jądra).
///
/// Typ bez publicznych pól; jedyny konstruktor jest ukryty i przeznaczony wyłącznie dla crate'a
/// Brokera. To umowa, nie twarda blokada — egzekwowanie (osobny proces Brokera, podpis) przychodzi
/// z modułem `safety-broker` w F3 (ADR 0003). Użycie konstruktora gdziekolwiek indziej jest
/// błędem przeglądu kodu.
#[derive(Debug)]
pub struct KernelAuthority(());

impl KernelAuthority {
    /// WYŁĄCZNIE dla crate'a Brokera (`safety-broker`). Nie wywoływać w innych modułach.
    #[doc(hidden)]
    pub fn __broker_only() -> Self {
        Self(())
    }
}

/// Deny-listy (format wersjonowany).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DenyLists {
    /// Wersja danych (rośnie przy każdej zmianie przez Brokera).
    pub version: u32,
    /// Katalogi/pliki zabronione wraz z zawartością (`%USERPROFILE%\…`, `%LOCALAPPDATA%\…`).
    pub path_prefixes: Vec<String>,
    /// Nazwy komponentów zabronione w dowolnym miejscu ścieżki (np. `.claude`).
    pub path_segments: Vec<String>,
    /// Domeny webowych UI dostawców (dopasowanie obejmuje subdomeny).
    pub domains: Vec<String>,
}

impl Default for DenyLists {
    fn default() -> Self {
        Self::baseline()
    }
}

impl DenyLists {
    /// Lista bazowa z subscription-routes.md §2.
    pub fn baseline() -> Self {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        Self {
            version: 1,
            path_prefixes: own(&[
                r"%LOCALAPPDATA%\Google\Chrome\User Data",
                r"%LOCALAPPDATA%\Microsoft\Edge\User Data",
                r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data",
                r"%APPDATA%\Mozilla\Firefox\Profiles",
                r"%APPDATA%\Opera Software",
                r"%LOCALAPPDATA%\Microsoft\Credentials",
                r"%APPDATA%\Microsoft\Credentials",
                r"%LOCALAPPDATA%\Microsoft\Vault",
                r"%APPDATA%\Microsoft\Vault",
                r"%APPDATA%\Microsoft\Protect",
            ]),
            path_segments: own(&[
                ".claude",
                ".claude.json",
                ".codex",
                ".gemini",
                ".grok",
                ".kimi",
                ".agy",
            ]),
            domains: own(&[
                "claude.ai",
                "chatgpt.com",
                "chat.openai.com",
                "gemini.google.com",
                "aistudio.google.com",
                "grok.com",
                "kimi.com",
                "kimi.moonshot.cn",
                "chat.deepseek.com",
                "chat.qwen.ai",
                "chat.z.ai",
                "chat.mistral.ai",
            ]),
        }
    }

    /// Dopisuje domeny (np. `providers[].deny_domains` z rejestru); duplikaty pomija.
    #[must_use]
    pub fn with_domains<I: IntoIterator<Item = String>>(mut self, extra: I) -> Self {
        for d in extra {
            if let Some(host) = normalize_host(&d)
                && !self.domains.contains(&host)
            {
                self.domains.push(host);
            }
        }
        self
    }

    /// Walidacja: segmenty obowiązkowe obecne, wpisy niepuste, domeny w postaci znormalizowanej.
    pub fn validate(&self) -> Result<(), ComplianceError> {
        let invalid = |m: String| Err(ComplianceError::InvalidDenyList(m));
        if self.version == 0 {
            return invalid("wersja musi być > 0".into());
        }
        let segments: Vec<String> = self
            .path_segments
            .iter()
            .map(|s| normalize(s, &PathEnv::new()).comps.join("\\"))
            .collect();
        for required in MANDATORY_PATH_SEGMENTS {
            if !segments.iter().any(|s| s == required) {
                return invalid(format!("brak obowiązkowego segmentu `{required}`"));
            }
        }
        if let Some(bad) = segments.iter().find(|s| s.is_empty() || s.contains('\\')) {
            return invalid(format!("segment `{bad}` musi być pojedynczą nazwą"));
        }
        if self.path_prefixes.iter().any(|p| p.trim().is_empty()) {
            return invalid("pusty prefiks ścieżki".into());
        }
        for d in &self.domains {
            if normalize_host(d).as_deref() != Some(d.as_str()) {
                return invalid(format!("domena `{d}` nie jest znormalizowana"));
            }
        }
        Ok(())
    }
}

/// Skompilowane deny-listy: gotowe do szybkich zapytań.
#[derive(Debug, Clone)]
pub struct DenyChecker {
    lists: DenyLists,
    prefixes: Vec<NormPath>,
    tails: Vec<Vec<String>>,
    segments: Vec<String>,
}

/// Minimalna długość „ogona” wzorca dopasowywanego w dowolnym miejscu ścieżki.
const MIN_TAIL: usize = 3;

impl DenyChecker {
    /// Kompiluje listy dla danego środowiska (rozwinięcie `%USERPROFILE%` itd.).
    pub fn new(lists: DenyLists, env: &PathEnv) -> Self {
        let prefixes = lists
            .path_prefixes
            .iter()
            .map(|p| normalize(p, env))
            .filter(|p| p.root != Root::Relative)
            .collect();
        let tails = lists
            .path_prefixes
            .iter()
            .flat_map(|p| tails_of(p))
            .collect();
        let segments = lists
            .path_segments
            .iter()
            .filter_map(|s| normalize(s, &PathEnv::new()).comps.pop())
            .collect();
        Self {
            lists,
            prefixes,
            tails,
            segments,
        }
    }

    /// Dane źródłowe.
    pub fn lists(&self) -> &DenyLists {
        &self.lists
    }

    /// Czy ścieżka (dowolnej postaci) jest zabroniona.
    pub fn is_denied_normalized(&self, p: &NormPath) -> bool {
        let segment_hit = p
            .comps
            .iter()
            .any(|c| self.segments.iter().any(|s| comp_matches(c, s)));
        let prefix_hit = self.prefixes.iter().any(|pre| {
            pre.root == p.root
                && pre.comps.len() <= p.comps.len()
                && pre
                    .comps
                    .iter()
                    .zip(&p.comps)
                    .all(|(a, b)| comp_matches(b, a))
        });
        let tail_hit = self.tails.iter().any(|tail| {
            p.comps
                .windows(tail.len())
                .any(|w| w.iter().zip(tail).all(|(b, a)| comp_matches(b, a)))
        });
        segment_hit || prefix_hit || tail_hit
    }

    /// Czy ścieżka jest zabroniona (normalizacja w semantyce Windows).
    pub fn is_denied_path(&self, path: &str, env: &PathEnv) -> bool {
        self.is_denied_normalized(&normalize(path, env))
    }

    /// Czy domena lub URL jest zabroniony. Nierozpoznawalny host → `false` (to nie jest domena).
    pub fn is_denied_domain(&self, input: &str) -> bool {
        normalize_host(input)
            .is_some_and(|host| self.lists.domains.iter().any(|d| host_matches(&host, d)))
    }
}

/// „Ogony” wzorca względem profilu użytkownika, dopasowywane w dowolnym miejscu ścieżki
/// (inny użytkownik, inny dysk, kopia profilu, ścieżka względna).
fn tails_of(pattern: &str) -> Vec<Vec<String>> {
    let env = PathEnv::new()
        .with("USERPROFILE", "")
        .with("LOCALAPPDATA", r"AppData\Local")
        .with("APPDATA", r"AppData\Roaming");
    let full = normalize(pattern, &env).comps;
    let short: Vec<String> = full
        .iter()
        .skip_while(|c| *c == "appdata" || *c == "local" || *c == "roaming")
        .cloned()
        .collect();
    [full, short]
        .into_iter()
        .filter(|t| t.len() >= MIN_TAIL)
        .collect()
}
