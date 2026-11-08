//! Czyste reguły zgodności mostów (wspólne dla `-impl` i `-fake`, PLAN §1.3):
//! pochodzenie uruchomienia, przypięte wersje CLI, środowisko procesu potomnego.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{BackendError, LaunchRefusal};
use crate::task::{BridgeKind, LaunchOrigin};

/// Jawna zgoda użytkownika na uruchamianie trasy z harmonogramu (zapisana w konfiguracji).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScheduleConsent {
    /// Dzienny limit uruchomień z harmonogramu (0 = brak zgody).
    pub max_per_day: u32,
}

/// Polityka uruchamiania (klucz `[agent_backends.launch]`; zmienia ją tylko użytkownik).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LaunchPolicy {
    /// Zgody na harmonogram per trasa (`claude-code-cli`, `codex-cli`). Brak wpisu = brak zgody.
    #[serde(default)]
    pub scheduled: BTreeMap<String, ScheduleConsent>,
}

/// Czy most wolno uruchomić z danego źródła. `scheduled_today` — liczba uruchomień tej trasy
/// z harmonogramu w bieżącej dobie. Wyzwalacze i Ulepszacz — nigdy (niezależnie od konfiguracji).
pub fn check_origin(
    origin: &LaunchOrigin,
    bridge: BridgeKind,
    policy: &LaunchPolicy,
    scheduled_today: u32,
) -> Result<(), LaunchRefusal> {
    match origin {
        LaunchOrigin::UserRequest => Ok(()),
        LaunchOrigin::Improver => Err(LaunchRefusal::Improver),
        LaunchOrigin::Trigger { .. } => Err(LaunchRefusal::Trigger),
        LaunchOrigin::Scheduled { .. } => {
            let consent = policy
                .scheduled
                .get(bridge.route_id_str())
                .filter(|c| c.max_per_day > 0)
                .ok_or(LaunchRefusal::ScheduleWithoutConsent)?;
            if scheduled_today >= consent.max_per_day {
                return Err(LaunchRefusal::ScheduleDailyLimit {
                    limit: consent.max_per_day,
                });
            }
            Ok(())
        }
    }
}

/// Przypięcie CLI: lista zweryfikowanych wersji (spike (b)) i opcjonalny hash pliku wykonywalnego.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CliPin {
    /// Dozwolone wersje (dokładne dopasowanie tekstu z `--version`). Pusta lista = trasa wyłączona.
    #[serde(default)]
    pub versions: Vec<String>,
    /// SHA-256 pliku wykonywalnego (hex, małe litery); `None` = bez sprawdzania hasha.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Sprawdza wersję CLI: musi być na liście przypiętych **i** — jeśli rejestr zgodności ma
/// `cli_pinned_version` — równa przypięciu z rejestru (przecięcie obu list = bezpieczniej).
pub fn check_version(
    bridge: BridgeKind,
    found: Option<&str>,
    pin: &CliPin,
    registry_pin: Option<&str>,
) -> Result<String, BackendError> {
    let refuse = || BackendError::VersionNotPinned {
        program: bridge.program().to_owned(),
        found: found.map(str::to_owned),
    };
    let version = found.ok_or_else(refuse)?;
    if !pin.versions.iter().any(|v| v == version) {
        return Err(refuse());
    }
    if registry_pin.is_some_and(|r| r != version) {
        return Err(refuse());
    }
    Ok(version.to_owned())
}

/// Zmienne przekazywane procesom CLI (reszta środowiska Alfy, w tym klucze API, jest czyszczona).
/// `HOME`/`USERPROFILE`/`APPDATA` są potrzebne CLI do odnalezienia **własnego** logowania —
/// Alfa ich zawartości nie czyta.
pub const CHILD_ENV_ALLOWLIST: [&str; 36] = [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "TMPDIR",
    "HOME",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "USERNAME",
    "USER",
    "LOGNAME",
    "COMPUTERNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_RUNTIME_DIR",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "ALL_PROXY",
    "SSL_CERT_FILE",
    "NODE_EXTRA_CA_CERTS",
];

const SECRET_MARKERS: [&str; 9] = [
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "COOKIE",
    "SESSION",
    "AUTH",
];

const SECRET_PREFIXES: [&str; 20] = [
    "ALFA_",
    "ANTHROPIC_",
    "OPENAI_",
    "CODEX_",
    "CLAUDE_",
    "AWS_",
    "AZURE_",
    "GOOGLE_",
    "GEMINI_",
    "XAI_",
    "GROK_",
    "DEEPSEEK_",
    "MOONSHOT_",
    "KIMI_",
    "DASHSCOPE_",
    "ZAI_",
    "MISTRAL_",
    "OPENROUTER_",
    "GITHUB_",
    "GH_",
];

/// Czy nazwa zmiennej wygląda na sekret albo należy do Alfy/dostawcy (nigdy nie przekazywana).
pub fn is_secret_env_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_MARKERS.iter().any(|m| upper.contains(m))
        || SECRET_PREFIXES.iter().any(|p| upper.starts_with(p))
}

/// Środowisko procesu CLI: tylko lista dozwolona (bez rozróżniania wielkości liter, jak na
/// Windows) i nigdy nic, co wygląda na sekret.
pub fn child_env<I>(parent: I) -> BTreeMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    parent
        .into_iter()
        .filter(|(name, _)| {
            let upper = name.to_ascii_uppercase();
            CHILD_ENV_ALLOWLIST.contains(&upper.as_str()) && !is_secret_env_name(name)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        let mut policy = LaunchPolicy::default();
        let b = BridgeKind::ClaudeCode;
        assert_eq!(
            check_origin(&LaunchOrigin::UserRequest, b, &policy, 0),
            Ok(())
        );
        assert_eq!(
            check_origin(&LaunchOrigin::Improver, b, &policy, 0),
            Err(LaunchRefusal::Improver)
        );
        let trig = LaunchOrigin::Trigger {
            trigger_id: "t".into(),
        };
        let sched = LaunchOrigin::Scheduled {
            schedule_id: "s".into(),
        };
        assert_eq!(
            check_origin(&trig, b, &policy, 0),
            Err(LaunchRefusal::Trigger)
        );
        assert_eq!(
            check_origin(&sched, b, &policy, 0),
            Err(LaunchRefusal::ScheduleWithoutConsent)
        );
        policy
            .scheduled
            .insert("claude-code-cli".into(), ScheduleConsent { max_per_day: 2 });
        assert_eq!(check_origin(&sched, b, &policy, 1), Ok(()));
        assert_eq!(
            check_origin(&sched, b, &policy, 2),
            Err(LaunchRefusal::ScheduleDailyLimit { limit: 2 })
        );
        assert_eq!(
            check_origin(&trig, b, &policy, 0),
            Err(LaunchRefusal::Trigger)
        );
        assert_eq!(
            check_origin(&sched, BridgeKind::Codex, &policy, 0),
            Err(LaunchRefusal::ScheduleWithoutConsent)
        );
        policy
            .scheduled
            .insert("codex-cli".into(), ScheduleConsent { max_per_day: 0 });
        assert!(check_origin(&sched, BridgeKind::Codex, &policy, 0).is_err());
        assert_eq!(
            check_origin(&LaunchOrigin::Improver, b, &policy, 0),
            Err(LaunchRefusal::Improver)
        );
    }

    #[test]
    fn versions() {
        let pin = CliPin {
            versions: vec!["2.1.0".into()],
            sha256: None,
        };
        let b = BridgeKind::ClaudeCode;
        assert_eq!(
            check_version(b, Some("2.1.0"), &pin, None),
            Ok("2.1.0".into())
        );
        assert!(check_version(b, Some("2.1.1"), &pin, None).is_err());
        assert!(check_version(b, None, &pin, None).is_err());
        assert!(check_version(b, Some("2.1.0"), &pin, Some("2.0.0")).is_err());
        assert!(check_version(b, Some("2.1.0"), &pin, Some("2.1.0")).is_ok());
        assert!(check_version(b, Some("2.1.0"), &CliPin::default(), None).is_err());
    }

    #[test]
    fn env_is_allowlisted_and_scrubbed() {
        let parent = [
            ("PATH", "/bin"),
            ("Path", "C:\\x"),
            ("HOME", "/home/u"),
            ("ANTHROPIC_API_KEY", "sk-ant"),
            ("OPENAI_API_KEY", "sk"),
            ("CODEX_HOME", "/x"),
            ("ALFA_MCP_TOKEN", "t"),
            ("GITHUB_TOKEN", "g"),
            ("RANDOM", "r"),
            ("HTTPS_PROXY", "http://proxy"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()));
        let env = child_env(parent);
        let names: Vec<&str> = env.keys().map(String::as_str).collect();
        assert_eq!(names, vec!["HOME", "HTTPS_PROXY", "PATH", "Path"]);
        assert!(is_secret_env_name("my_api_key"));
        assert!(is_secret_env_name("CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(!is_secret_env_name("PATH"));
        for name in CHILD_ENV_ALLOWLIST {
            assert!(!is_secret_env_name(name), "{name}");
        }
    }
}
