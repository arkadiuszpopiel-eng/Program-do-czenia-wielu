//! Filtr poziomów dziennika: `ALFA_LOG` albo `[logs] level` w konfiguracji.
//!
//! Składnia (uproszczony `RUST_LOG`): `poziom` albo lista `cel=poziom` rozdzielona przecinkami,
//! np. `info`, `debug`, `warn,app_core=debug`. Poziomy: `off`, `error`, `warn`, `info`, `debug`,
//! `trace` (wielkość liter bez znaczenia). Wygrywa najdłuższy pasujący cel (`app_core` pasuje do
//! `app_core` i `app_core::…`, ale nie do `app_core_x`).
//!
//! Cele spoza Alfy (biblioteki: `hyper`, `h2`, `reqwest`, `wry`, `tao`…) bez jawnego wpisu mają
//! co najwyżej `warn` — ich `debug`/`trace` potrafią zawierać nagłówki i treść zapytań.

use tracing::Level;
use tracing::level_filters::LevelFilter;

/// Poziom domyślny (PLAN §13, SPEC `core-log`: `[logs] level = "info"`).
pub const DEFAULT_LEVEL: &str = "info";

/// Błąd składni filtra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterError(pub String);

impl std::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "błędny poziom dziennika: {}", self.0)
    }
}

impl std::error::Error for FilterError {}

/// Filtr: poziom domyślny i wpisy per cel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    default: LevelFilter,
    directives: Vec<(String, LevelFilter)>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            default: LevelFilter::INFO,
            directives: Vec::new(),
        }
    }
}

fn level(text: &str) -> Result<LevelFilter, FilterError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "off" => Ok(LevelFilter::OFF),
        "error" => Ok(LevelFilter::ERROR),
        "warn" | "warning" => Ok(LevelFilter::WARN),
        "info" => Ok(LevelFilter::INFO),
        "debug" => Ok(LevelFilter::DEBUG),
        "trace" => Ok(LevelFilter::TRACE),
        other => Err(FilterError(format!("nieznany poziom „{other}”"))),
    }
}

fn valid_target(target: &str) -> bool {
    !target.is_empty()
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':'))
}

/// Czy cel należy do Alfy (crate'y workspace, binaria `alfa-*`, cele nadawane ręcznie).
pub fn is_alfa_target(target: &str) -> bool {
    let krate = target.split("::").next().unwrap_or(target);
    ["alfa", "app_", "core_", "lib_", "platform_"]
        .iter()
        .any(|p| krate.starts_with(p))
        || ["_impl", "_contract", "_fake"]
            .iter()
            .any(|s| krate.ends_with(s))
}

fn matches(target: &str, prefix: &str) -> bool {
    target == prefix
        || target
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with("::"))
}

impl Filter {
    /// Parsuje specyfikację (pusta = domyślny `info`).
    pub fn parse(spec: &str) -> Result<Self, FilterError> {
        let mut filter = Self::default();
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                Some((target, lvl)) => {
                    let target = target.trim();
                    if !valid_target(target) {
                        return Err(FilterError(format!("niepoprawny cel „{target}”")));
                    }
                    filter.directives.push((target.to_owned(), level(lvl)?));
                }
                None => filter.default = level(part)?,
            }
        }
        // Najdłuższy cel pierwszy — pierwsze dopasowanie wygrywa.
        filter
            .directives
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Ok(filter)
    }

    /// Poziom dla celu.
    pub fn level_for(&self, target: &str) -> LevelFilter {
        if let Some((_, lvl)) = self.directives.iter().find(|(p, _)| matches(target, p)) {
            return *lvl;
        }
        if is_alfa_target(target) {
            self.default
        } else {
            self.default.min(LevelFilter::WARN)
        }
    }

    /// Czy zdarzenie celu `target` na poziomie `level` jest zapisywane.
    pub fn enabled(&self, target: &str, level: &Level) -> bool {
        self.level_for(target) >= *level
    }

    /// Najwyższy poziom, jaki może przejść (podpowiedź dla `tracing`).
    pub fn max_level(&self) -> LevelFilter {
        self.directives
            .iter()
            .map(|(_, l)| *l)
            .fold(self.default, LevelFilter::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_level_and_case() {
        let f = Filter::parse("DEBUG").unwrap();
        assert!(f.enabled("app_core::x", &Level::DEBUG));
        assert!(!f.enabled("app_core::x", &Level::TRACE));
        assert_eq!(f.max_level(), LevelFilter::DEBUG);
        assert_eq!(Filter::parse("").unwrap(), Filter::default());
        assert_eq!(
            Filter::parse("warning").unwrap().level_for("alfa"),
            LevelFilter::WARN
        );
    }

    #[test]
    fn external_targets_are_capped_at_warn() {
        let f = Filter::parse("trace").unwrap();
        assert!(f.enabled("providers_local_impl::sidecar", &Level::TRACE));
        assert!(f.enabled("alfa-watchdog", &Level::TRACE));
        assert!(!f.enabled("hyper::proto", &Level::INFO));
        assert!(f.enabled("hyper::proto", &Level::WARN));
        let explicit = Filter::parse("info,hyper=debug").unwrap();
        assert!(explicit.enabled("hyper::proto", &Level::DEBUG));
        let quiet = Filter::parse("error").unwrap();
        assert!(!quiet.enabled("h2", &Level::WARN));
    }

    #[test]
    fn longest_prefix_wins_on_segment_boundary() {
        let f = Filter::parse("info,app_core=warn,app_core::parts=trace").unwrap();
        assert_eq!(f.level_for("app_core::parts::kernel"), LevelFilter::TRACE);
        assert_eq!(f.level_for("app_core::core"), LevelFilter::WARN);
        assert_eq!(f.level_for("app_core"), LevelFilter::WARN);
        assert_eq!(f.level_for("app_core_extra"), LevelFilter::INFO);
        assert_eq!(f.max_level(), LevelFilter::TRACE);
        assert_eq!(Filter::parse("off").unwrap().max_level(), LevelFilter::OFF);
    }

    #[test]
    fn errors_are_reported() {
        assert!(Filter::parse("głośno").is_err());
        assert!(Filter::parse("a b=info").is_err());
        assert!(Filter::parse("=info").is_err());
        let e = Filter::parse("x=loud").unwrap_err();
        assert!(e.to_string().contains("loud"), "{e}");
    }

    #[test]
    fn alfa_targets() {
        for t in [
            "alfa_desktop_lib::kernel",
            "alfa-broker",
            "app_broker::notice",
            "core_log_impl",
            "lib_openai_compat::retry",
            "platform_windows_impl",
            "providers_local_impl::sidecar",
            "voice_stt_contract",
        ] {
            assert!(is_alfa_target(t), "{t}");
        }
        for t in [
            "hyper",
            "h2::codec",
            "tao::platform",
            "reqwest::connect",
            "wry",
        ] {
            assert!(!is_alfa_target(t), "{t}");
        }
    }
}
