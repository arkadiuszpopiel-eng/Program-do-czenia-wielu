//! Polityki portu systemu wspólne dla atrapy i implementacji Windows (i narzędzi `tools-system`,
//! które sprawdzają je wcześniej — obrona w głąb): procesy i usługi krytyczne, nazwy zmiennych
//! środowiskowych (sekrety ukrywane, zapis zmiennych przekierowujących dane Alfy albo
//! wstrzykujących kod — odmowa), dostawca zdarzeń i zapytanie XPath bez wstrzyknięcia.

use platform_contract::{LinkRole, ProcessLink, TargetGuard, ancestors_of, image_file_name};

use crate::sys::{EnvVar, EventQuery, SysError};

/// Procesy, których zakończenie destabilizuje system albo wyłącza ochronę — zawsze odmowa.
pub const CRITICAL_PROCESSES: [&str; 26] = [
    "system",
    "registry",
    "memory compression",
    "secure system",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "svchost.exe",
    "dwm.exe",
    "fontdrvhost.exe",
    "logonui.exe",
    "sihost.exe",
    "explorer.exe",
    "spoolsv.exe",
    "audiodg.exe",
    "msmpeng.exe",
    "nissrv.exe",
    "mpdefendercoreservice.exe",
    "securityhealthservice.exe",
    "securityhealthsystray.exe",
    "smartscreen.exe",
    "consent.exe",
];

/// Usługi, których zatrzymanie/restart wyłącza ochronę albo system — odmowa narzędzia (start
/// dozwolony). Usługi Jądra (`alfabroker`, `alfawatchdog`, `eventlog`) blokuje też Broker.
pub const CRITICAL_SERVICES: [&str; 24] = [
    "alfabroker",
    "alfawatchdog",
    "eventlog",
    "windefend",
    "wdnissvc",
    "sense",
    "mpssvc",
    "bfe",
    "wscsvc",
    "securityhealthservice",
    "rpcss",
    "rpceptmapper",
    "dcomlaunch",
    "lsm",
    "samss",
    "winmgmt",
    "cryptsvc",
    "wuauserv",
    "usosvc",
    "trustedinstaller",
    "appidsvc",
    "gpsvc",
    "schedule",
    "vaultsvc",
];

/// Fragmenty nazw zmiennych z sekretami (wartość nigdy nie opuszcza portu).
const SECRET_NAME_PARTS: [&str; 12] = [
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "PWD",
    "CREDENTIAL",
    "AUTH",
    "COOKIE",
    "SESSION",
    "PRIVATE",
    "HASLO",
];

/// Zmienne, których agentka nie zapisuje: przekierowują dane i ścieżki Alfy, interpreter
/// poleceń, ładowanie modułów, ruch sieciowy i zaufanie TLS albo wstrzykują kod do procesów
/// (przegląd fali 3, W3-02: JVM, OpenSSL, PowerShell, Python, Perl, Ruby).
const ENV_WRITE_DENIED: &[&str] = &[
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "HOME",
    "TEMP",
    "TMP",
    "SYSTEMROOT",
    "WINDIR",
    "SYSTEMDRIVE",
    "PROGRAMDATA",
    "COMSPEC",
    "PATHEXT",
    "PSMODULEPATH",
    "SSLKEYLOGFILE",
    "NODE_OPTIONS",
    "NODE_EXTRA_CA_CERTS",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "PYTHONSTARTUP",
    "PYTHONPATH",
    "PERL5OPT",
    "__COMPAT_LAYER",
    "ONEDRIVE",
    "PUBLIC",
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "OPENSSL_CONF",
    "OPENSSL_MODULES",
    "OPENSSL_ENGINES",
    "PSEXECUTIONPOLICYPREFERENCE",
    "PYTHONHOME",
    "PERL5LIB",
    "PERLLIB",
    "RUBYOPT",
    "RUBYLIB",
];

/// Prefiksy zmiennych zabronionych do zapisu (`CARGO_`, `NPM_CONFIG_` — wrapper kompilatora,
/// `runner` celu, powłoka skryptów: kod przy budowaniu na tej maszynie; W3-02).
const ENV_WRITE_DENIED_PREFIXES: &[&str] = &[
    "ALFA",
    "WEBVIEW2_",
    "COR_",
    "CORECLR_",
    "COMPLUS_",
    "DOTNET_",
    "SSL_CERT",
    "PROGRAMFILES",
    "COMMONPROGRAMFILES",
    "GIT_",
    "LD_",
    "DYLD_",
    "RUST",
    "CARGO_",
    "NPM_CONFIG_",
];

/// Najdłuższa nazwa zmiennej.
pub const MAX_ENV_NAME: usize = 255;
/// Najdłuższa wartość zmiennej (limit `SetEnvironmentVariable`).
pub const MAX_ENV_VALUE: usize = 32_767;

/// Czy proces jest krytyczny dla systemu (nazwa obrazu bez rozróżniania wielkości liter).
pub fn is_critical_process(image: &str) -> bool {
    let name = image_file_name(image);
    CRITICAL_PROCESSES.contains(&name.as_str())
}

/// Czy proces jest chroniony: PID systemowy (≤ 4), strażnik celów (Alfa, jej drzewo — przodkowie
/// liczeni w chwili sprawdzenia, Broker, watchdog, obraz nieznany) albo proces krytyczny.
pub fn protected_process(
    guard: &TargetGuard,
    pid: u32,
    image: &str,
    parent_of: impl Fn(u32) -> Option<u32>,
) -> bool {
    let link = ProcessLink {
        role: LinkRole::Root,
        pid,
        image: image.to_owned(),
        ancestors: ancestors_of(pid, parent_of),
    };
    pid <= 4 || is_critical_process(image) || guard.is_protected_link(&link)
}

/// Czy usługa jest krytyczna (zatrzymanie/restart = odmowa).
pub fn is_critical_service(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    CRITICAL_SERVICES.contains(&n.as_str()) || n.starts_with("alfa")
}

/// Nazwa usługi: 1–256 znaków bez separatorów ścieżek, cudzysłowów i znaków sterujących.
pub fn check_service_name(name: &str) -> Result<(), SysError> {
    let ok = !name.trim().is_empty()
        && name.chars().count() <= 256
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '"' | '\''));
    ok.then_some(())
        .ok_or_else(|| SysError::Invalid(format!("nazwa usługi „{name}”")))
}

/// Czy nazwa zmiennej wygląda na sekret (wartość ukrywana).
pub fn is_secret_env_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_PARTS.iter().any(|p| upper.contains(p))
}

/// Nazwa zmiennej: 1–255 znaków, bez `=`, NUL i znaków sterujących.
pub fn check_env_name(name: &str) -> Result<(), SysError> {
    let ok = !name.is_empty()
        && name.len() <= MAX_ENV_NAME
        && !name.chars().any(|c| c.is_control() || c == '=');
    ok.then_some(())
        .ok_or_else(|| SysError::Invalid(format!("nazwa zmiennej „{name}”")))
}

/// Powód odmowy zapisu zmiennej użytkownika przez agentkę (`None` = dozwolony).
pub fn env_write_denied(name: &str) -> Option<&'static str> {
    if check_env_name(name).is_err() {
        return Some("niepoprawna nazwa zmiennej");
    }
    let upper = name.to_ascii_uppercase();
    if is_secret_env_name(&upper) {
        return Some("zmienne z sekretami ustawia wyłącznie właściciel");
    }
    if ENV_WRITE_DENIED.contains(&upper.as_str())
        || ENV_WRITE_DENIED_PREFIXES
            .iter()
            .any(|p| upper.starts_with(p))
    {
        return Some(
            "zmienna steruje ścieżkami Alfy, interpreterem, zaufaniem TLS albo ładowaniem kodu",
        );
    }
    if upper.ends_with("_PROXY") || upper == "NO_PROXY" {
        return Some("zmienna przekierowuje ruch sieciowy");
    }
    None
}

/// Wartość zmiennej: ≤ 32 767 znaków, bez NUL i znaków sterujących poza tabulatorem (W3-05:
/// karta Brokera `setx NAZWA "wartość"` nie może rozpaść się na wiele linii).
pub fn check_env_value(value: &str) -> Result<(), SysError> {
    let ok = value.chars().count() <= MAX_ENV_VALUE
        && !value.chars().any(|c| c.is_control() && c != '\t');
    ok.then_some(()).ok_or_else(|| {
        SysError::Invalid("wartość zmiennej za długa albo zawiera znaki sterujące".into())
    })
}

/// Ukrywa wartości zmiennych o nazwach sekretów (port stosuje przed zwróceniem listy).
pub fn guard_env(vars: Vec<(String, String)>) -> Vec<EnvVar> {
    let mut out: Vec<EnvVar> = vars
        .into_iter()
        .map(|(name, value)| EnvVar {
            value: (!is_secret_env_name(&name)).then_some(value),
            name,
        })
        .collect();
    out.sort_by_key(|v| v.name.to_ascii_lowercase());
    out
}

/// Dostawca zdarzeń: 1–128 znaków z listy dozwolonej (litery, cyfry, spacja, `.`, `-`, `_`, `/`)
/// — bez cudzysłowów i nawiasów (wstrzyknięcie XPath).
pub fn check_provider(provider: &str) -> Result<(), SysError> {
    let ok = !provider.trim().is_empty()
        && provider.chars().count() <= 128
        && provider
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_' | '/'));
    ok.then_some(())
        .ok_or_else(|| SysError::Invalid(format!("dostawca zdarzeń „{provider}”")))
}

/// Zapytanie XPath dla `EvtQuery` (poziomy, dostawca, okno czasu) — z wartości sprawdzonych.
pub fn event_xpath(query: &EventQuery) -> Result<String, SysError> {
    let mut conds: Vec<String> = Vec::new();
    if let Some(min) = query.min_level {
        let levels: Vec<String> = (1..=min.value())
            .map(|l| format!("Level={l}"))
            .chain((min.value() >= 4).then(|| "Level=0".to_owned()))
            .collect();
        conds.push(format!("({})", levels.join(" or ")));
    }
    if let Some(p) = &query.provider {
        check_provider(p)?;
        conds.push(format!("Provider[@Name='{p}']"));
    }
    if let Some(ms) = query.since_ms {
        conds.push(format!("TimeCreated[timediff(@SystemTime) <= {ms}]"));
    }
    Ok(if conds.is_empty() {
        "*".to_owned()
    } else {
        format!("*[System[{}]]", conds.join(" and "))
    })
}

/// Obcina tekst do `max` znaków (na granicy znaku).
pub fn clip_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
