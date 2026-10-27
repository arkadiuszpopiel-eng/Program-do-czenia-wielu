//! Heurystyki polecenia powłoki dla faktów Brokera (destrukcja, instalacja, egress z hostami,
//! ścieżki, ścieżki poświadczeń, polecenia interaktywne). Obrona w głąb: twarde reguły Jądra
//! (`check_command`) sprawdza Broker; tutaj — to, czego Broker nie wie bez kontekstu narzędzia.

use risk_classifier_contract::Destructiveness;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const DELETE_VERBS: [&str; 10] = [
    "del",
    "erase",
    "rd",
    "rmdir",
    "remove-item",
    "rm",
    "ri",
    "clear-content",
    "sdelete",
    "cipher",
];
const NETWORK_VERBS: [&str; 31] = [
    "curl",
    "wget",
    "iwr",
    "irm",
    "invoke-webrequest",
    "invoke-restmethod",
    "start-bitstransfer",
    "bitsadmin",
    "ftp",
    "tftp",
    "scp",
    "sftp",
    "ssh",
    "nslookup",
    "resolve-dnsname",
    "ping",
    "test-connection",
    "test-netconnection",
    "tnc",
    "certutil",
    "telnet",
    "nc",
    "ncat",
    "send-mailmessage",
    "enter-pssession",
    "new-pssession",
    "rsync",
    "git",
    "net",
    "mstsc",
    "explorer",
];
const NETWORK_MARKERS: [&str; 7] = [
    "net.webclient",
    "system.net.",
    "downloadstring",
    "downloadfile",
    "uploadstring",
    "httpclient",
    "-computername",
];
const INSTALL_PATTERNS: [&[&str]; 10] = [
    &["winget", "install"],
    &["choco", "install"],
    &["scoop", "install"],
    &["msiexec"],
    &["install-package"],
    &["install-module"],
    &["add-appxpackage"],
    &["pip", "install"],
    &["npm", "-g"],
    &["dism", "/online"],
];
const INTERACTIVE: [&str; 4] = ["runas", "get-credential", "read-host", "pause"];

/// Wynik analizy polecenia.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandAnalysis {
    /// Polecenie usuwa pliki (powłoka — bez Kosza).
    pub deletes: bool,
    /// Instalacja oprogramowania.
    pub install: bool,
    /// Polecenie sieciowe (egress).
    pub network: bool,
    /// Hosty docelowe (z URL, UNC, `user@host`, nazw domenowych po poleceniu sieciowym).
    pub hosts: Vec<String>,
    /// Ścieżki bezwzględne i względne wychodzące w górę (`..`) — bez rozwijania zmiennych; cel
    /// względem katalogu roboczego ustala wykonawca.
    pub paths: Vec<String>,
    /// Polecenie usuwające z celem, którego nie da się ustalić statycznie (zmienna, splat `@x`,
    /// potok do czasownika usuwania) — zawsze nieodwracalne (wymaga zgody).
    #[serde(default)]
    pub opaque_targets: bool,
    /// Któraś ścieżka wskazuje na poświadczenia (`.ssh`, `.claude`, profile przeglądarek…).
    pub credential_path: bool,
    /// Polecenie wymaga interakcji (hasło, potwierdzenie) — niedozwolone bez terminala.
    pub interactive: bool,
}

impl CommandAnalysis {
    /// Destrukcyjność dla faktów: usunięcie w zakresie snapshotu jest odzyskiwalne; cel
    /// nieustalony (`opaque_targets`) albo poza snapshotem — nieodwracalne.
    pub fn destructiveness(&self, all_paths_in_snapshot: bool) -> Destructiveness {
        match (self.deletes, all_paths_in_snapshot && !self.opaque_targets) {
            (false, _) => Destructiveness::None,
            (true, true) => Destructiveness::Recoverable,
            (true, false) => Destructiveness::Permanent,
        }
    }
}

/// Tokeny polecenia (małe litery; cudzysłowy grupują, znaki ucieczki `^` i `` ` `` usunięte).
pub fn tokens(command: &str) -> Vec<String> {
    crate::lex::scan(command).words
}

fn verb(t: &str) -> String {
    let lower = t.to_lowercase();
    let base = lower
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&lower)
        .to_owned();
    base.strip_suffix(".exe").map(str::to_owned).unwrap_or(base)
}

fn host_of_url(t: &str) -> Option<String> {
    let lower = t.to_lowercase();
    let rest = [
        "http://", "https://", "ftp://", "ftps://", "ws://", "wss://", "sftp://", "ssh://",
        "smb://", "file://",
    ]
    .iter()
    .find_map(|s| lower.find(s).map(|i| &lower[i + s.len()..]))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    (!host.is_empty()).then(|| host.to_owned())
}

fn looks_like_domain(t: &str) -> bool {
    const FILE_EXT: [&str; 16] = [
        "txt", "ps1", "exe", "bat", "cmd", "zip", "json", "md", "log", "csv", "xml", "dll", "pdf",
        "png", "jpg", "docx",
    ];
    let t = t.trim_end_matches('.');
    let parts: Vec<&str> = t.split('.').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && parts
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
        && !parts
            .last()
            .is_some_and(|tld| FILE_EXT.contains(&tld.to_lowercase().as_str()))
}

fn is_abs_path(t: &str) -> bool {
    let b = t.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
        || t.starts_with('\\')
        || t.starts_with('/')
        || t.starts_with('~')
        || t.starts_with('%')
        || t.to_lowercase().starts_with("$env:")
}

/// Ścieżka względna z segmentem `..` (cel może leżeć poza katalogiem roboczym).
fn climbs_up(t: &str) -> bool {
    t.split(['\\', '/']).any(|seg| seg == "..")
}

/// Cel usuwania nieustalony statycznie: zmienna (poza `$env:NAZWA` — tę rozwija wykonawca),
/// splat `@x` albo czasownik usuwania za potokiem (cele z wyjścia poprzedniego polecenia).
fn opaque_delete_targets(command: &str, toks: &[String]) -> bool {
    let variable = toks.iter().any(|t| {
        let lower = t.to_lowercase();
        (t.contains('$') && !lower.starts_with("$env:")) || (t.starts_with('@') && t.len() > 1)
    });
    let piped = command.split('|').skip(1).any(|segment| {
        tokens(segment)
            .iter()
            .any(|t| DELETE_VERBS.contains(&verb(t).as_str()))
    });
    variable || piped
}

/// Najgłębsze zagnieżdżenie analizowane (`powershell -Command "cmd /c \"…\""`), jak w Jądrze.
const MAX_DEPTH: u8 = 3;

/// Analizuje polecenie (z poleceniami zagnieżdżonymi w `powershell -Command` / `cmd /c`) bez
/// wiedzy o powłoce — najostrzej: cytowany wzorzec po `-match` też może być ścieżką UNC.
pub fn analyze(command: &str) -> CommandAnalysis {
    analyze_at(command, 0, false)
}

/// Jak [`analyze`], ale dla polecenia wykonywanego przez PowerShell: cytowany literał po
/// `-match`/`-replace` to wzorzec, nie host. W `cmd` `;` i `"…"` znaczą co innego
/// (`type ; $x -match "\\host\share"` otwiera udział), dlatego polecenie zagnieżdżone
/// korzysta z wyjątku tylko, gdy też uruchamia je PowerShell.
pub fn analyze_powershell(command: &str) -> CommandAnalysis {
    analyze_at(command, 0, true)
}

fn merge(a: &mut CommandAnalysis, b: CommandAnalysis) {
    a.deletes |= b.deletes;
    a.install |= b.install;
    a.network |= b.network;
    a.opaque_targets |= b.opaque_targets;
    a.credential_path |= b.credential_path;
    a.interactive |= b.interactive;
    for h in b.hosts {
        push_unique(&mut a.hosts, h);
    }
    for p in b.paths {
        push_unique(&mut a.paths, p);
    }
}

/// Przełącznik narzędzia Windows (`/MIR`, `/Q`, `/S:3`), a nie ścieżka od korzenia.
fn is_switch(t: &str) -> bool {
    t.strip_prefix('/').is_some_and(|rest| {
        let name = rest.split(':').next().unwrap_or_default();
        !name.is_empty()
            && name.len() <= 12
            && name.chars().all(|c| c.is_ascii_alphanumeric())
            && !rest.contains(['/', '\\'])
    })
}

fn analyze_at(command: &str, depth: u8, regex_literals: bool) -> CommandAnalysis {
    let parsed = crate::lex::scan(command);
    let toks = &parsed.words;
    let verbs: Vec<String> = toks.iter().map(|t| verb(t)).collect();
    let lower = command.to_lowercase();
    let has = |w: &str| verbs.iter().any(|v| v == w);
    let mut a = CommandAnalysis {
        deletes: DELETE_VERBS.iter().any(|v| has(v)),
        install: INSTALL_PATTERNS.iter().any(|p| {
            p.iter()
                .all(|w| has(w) || toks.iter().any(|t| t.eq_ignore_ascii_case(w)))
        }),
        interactive: INTERACTIVE.iter().any(|v| has(v)),
        ..CommandAnalysis::default()
    };
    a.opaque_targets = a.deletes && opaque_delete_targets(command, toks);
    let net_verb = NETWORK_VERBS.iter().any(|v| {
        has(v)
            && match *v {
                "net" => has("use"),
                "git" => ["clone", "push", "pull", "fetch", "ls-remote"]
                    .iter()
                    .any(|g| has(g)),
                "explorer" => lower.contains("://"),
                _ => true,
            }
    });
    a.network = net_verb || NETWORK_MARKERS.iter().any(|m| lower.contains(m));
    for (index, t) in toks.iter().enumerate() {
        let regex_data = regex_literals && parsed.literal_regex(command, index);
        if let Some(h) = host_of_url(t) {
            a.network = true;
            push_unique(&mut a.hosts, h);
        } else if let Some(unc) = t
            .strip_prefix("\\\\")
            .filter(|_| !regex_data && !t.starts_with("\\\\?\\") && !t.starts_with("\\\\.\\"))
        {
            if let Some(server) = unc.split(['\\', '/']).next().filter(|s| !s.is_empty()) {
                a.network = true;
                push_unique(&mut a.hosts, server.to_lowercase());
            }
        } else if let Some(h) = net_verb.then(|| user_host(t)).flatten() {
            push_unique(&mut a.hosts, h);
        } else if net_verb && looks_like_domain(t) {
            push_unique(&mut a.hosts, t.trim_end_matches('.').to_lowercase());
        }
        let path_like = t.trim_start_matches('@');
        if !regex_data
            && (is_abs_path(path_like) || climbs_up(path_like))
            && host_of_url(t).is_none()
            && !is_switch(path_like)
        {
            push_unique(&mut a.paths, path_like.to_owned());
        }
        if tools_common_contract::paths::has_credential_segment(t) {
            a.credential_path = true;
        }
    }
    let fx = crate::effects::scan(command, toks, verb);
    a.deletes |= fx.deletes;
    a.network |= fx.network;
    a.opaque_targets |= fx.opaque;
    for target in fx.paths {
        if (is_abs_path(&target) || climbs_up(&target)) && !is_switch(&target) {
            push_unique(&mut a.paths, target.clone());
        }
        if tools_common_contract::paths::has_credential_segment(&target) {
            a.credential_path = true;
        }
    }
    for (inner, powershell) in fx.inner {
        if depth < MAX_DEPTH {
            merge(
                &mut a,
                analyze_at(&inner, depth + 1, regex_literals && powershell),
            );
        } else {
            // Głębiej niż Jądro sprawdza — treść nieznana, najostrzej.
            a.deletes = true;
            a.opaque_targets = true;
            a.network = true;
        }
    }
    a
}

/// `user@host[:ścieżka]` (ssh/scp) → host.
fn user_host(t: &str) -> Option<String> {
    let (user, rest) = t.split_once('@')?;
    if user.is_empty() || user.contains(['\\', '/']) {
        return None;
    }
    let host = rest.split([':', '/']).next()?;
    looks_like_domain(host).then(|| host.to_lowercase())
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
