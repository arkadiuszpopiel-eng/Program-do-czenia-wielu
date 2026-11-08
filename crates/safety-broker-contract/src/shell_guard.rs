//! Leksykalne reguły Jądra dla poleceń powłoki (cmd/PowerShell). Obrona w głąb: polecenia
//! wykonuje proces o ograniczonym tokenie (≤ L3), a narzędzia i platforma sprawdzają ścieżki
//! ponownie. Polecenie nieczytelne (zakodowane, `iex`) jest blokowane, bo nie da się go ocenić.

use compliance_contract::DenyChecker;
use compliance_contract::deny::{NormPath, PathEnv, Root, normalize};
use risk_classifier_contract::KernelRule;

use crate::policy::{PROTECTED_PROCESSES, PROTECTED_SERVICES};
use crate::scope::PathScope;

/// Kontekst oceny polecenia.
pub struct ShellContext<'a> {
    /// Środowisko do rozwijania zmiennych (`%USERPROFILE%`, `%SystemRoot%`, …).
    pub env: &'a PathEnv,
    /// Deny-listy poświadczeń.
    pub deny: &'a DenyChecker,
    /// Litera dysku systemowego.
    pub system_drive: char,
    /// Ścieżki Jądra.
    pub kernel_paths: &'a [PathScope],
    /// Katalog roboczy (zakres `shell.exec`) — do ścieżek względnych.
    pub cwd: Option<&'a PathScope>,
}

const DELETE_VERBS: [&str; 10] = [
    "del",
    "erase",
    "rd",
    "rmdir",
    "remove-item",
    "rm",
    "ri",
    "takeown",
    "icacls",
    "cipher",
];
const SERVICE_VERBS: [&str; 5] = ["stop", "delete", "config", "pause", "failure"];
const PS_SERVICE_CMDLETS: [&str; 5] = [
    "stop-service",
    "set-service",
    "remove-service",
    "suspend-service",
    "spsv",
];
const KILL_VERBS: [&str; 5] = ["taskkill", "stop-process", "kill", "spps", "pskill"];
const BOOT_TOOLS: [&str; 4] = ["bcdedit", "bcdboot", "bootrec", "bootsect"];

/// Tokeny polecenia wraz z poleceniami zagnieżdżonymi w cudzysłowach (`cmd /c "…"`,
/// `powershell -Command "…"`) — rozwijane do głębokości 3.
fn tokens(command: &str) -> Vec<String> {
    let mut all = lex(command);
    let mut frontier: Vec<String> = all.iter().filter(|t| t.contains(' ')).cloned().collect();
    for _ in 0..3 {
        let nested: Vec<String> = frontier.iter().flat_map(|t| lex(t)).collect();
        frontier = nested.iter().filter(|t| t.contains(' ')).cloned().collect();
        all.extend(nested);
    }
    all
}

/// Lekser: znaki ucieczki (`^`, `` ` ``) usuwane, cudzysłowy grupują (także ze spacjami)
/// i znikają, separatory poza cudzysłowami dzielą tokeny.
fn lex(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in command.to_lowercase().chars() {
        match (c, quote) {
            ('^' | '`', _) => {}
            ('"' | '\'', None) => quote = Some(c),
            (q, Some(open)) if q == open => quote = None,
            (c, None) if c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')' | ',') => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (c, _) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .map(|t| t.strip_suffix(".exe").map(str::to_owned).unwrap_or(t))
        .collect()
}

fn has(tokens: &[String], word: &str) -> bool {
    tokens.iter().any(|t| t == word)
}

fn has_any(tokens: &[String], words: &[&str]) -> bool {
    words.iter().any(|w| has(tokens, w))
}

fn is_protected_name(token: &str) -> bool {
    let t = token
        .trim_start_matches("/im")
        .trim_start_matches("-name")
        .trim_start_matches(['=', ':']);
    let t = t.strip_suffix(".exe").unwrap_or(t);
    PROTECTED_PROCESSES
        .iter()
        .any(|p| p.strip_suffix(".exe") == Some(t))
        || PROTECTED_SERVICES.contains(&t)
}

fn opaque(tokens: &[String]) -> bool {
    let powershell = has_any(tokens, &["powershell", "pwsh"]);
    let encoded = tokens.iter().any(|t| {
        let flag = t.strip_prefix('-').or_else(|| t.strip_prefix('/'));
        flag.is_some_and(|f| !f.is_empty() && ("encodedcommand".starts_with(f) || f == "ec"))
    });
    (powershell && encoded)
        || has_any(tokens, &["iex", "invoke-expression"])
        || tokens.iter().any(|t| t.contains("frombase64string"))
}

fn path_tokens(tokens: &[String], ctx: &ShellContext<'_>) -> Vec<NormPath> {
    tokens
        .iter()
        .filter(|t| {
            t.contains(['\\', '/', ':'])
                || t.starts_with(['%', '~', '$'])
                || t.starts_with("..")
                || t.starts_with('.')
        })
        .map(|t| {
            let p = normalize(t, ctx.env);
            match (&p.root, ctx.cwd) {
                (Root::Relative, Some(cwd)) => {
                    normalize(&format!("{}\\{t}", cwd.canonical()), ctx.env)
                }
                _ => p,
            }
        })
        .collect()
}

fn inside(p: &NormPath, root: &NormPath) -> bool {
    p.root == root.root
        && root.comps.len() <= p.comps.len()
        && root.comps.iter().zip(&p.comps).all(|(a, b)| a == b)
}

/// Katalogi systemowe chronione: `%SystemRoot%` i obszary rozruchu.
pub fn system_areas(system_drive: char) -> (NormPath, Vec<NormPath>) {
    let env = PathEnv::new();
    let windows = normalize(&format!("{system_drive}:\\windows"), &env);
    let boot = ["boot", "efi", "bootmgr", "windows\\boot"]
        .iter()
        .map(|p| normalize(&format!("{system_drive}:\\{p}"), &env))
        .collect();
    (windows, boot)
}

/// Twarda reguła Jądra naruszona przez polecenie (pierwsza wykryta), albo `None`.
pub fn check_command(command: &str, ctx: &ShellContext<'_>) -> Option<KernelRule> {
    let t = tokens(command);
    if opaque(&t) {
        return Some(KernelRule::OpaqueShellCommand);
    }
    let sd = ctx.system_drive;
    let drive_tokens = [format!("{sd}:"), format!("{sd}:\\"), format!("{sd}:/")];
    let names_system_drive = t.iter().any(|x| drive_tokens.contains(x));
    if (has_any(&t, &["format", "format.com"]) && names_system_drive)
        || (has(&t, "format-volume") && (has(&t, &sd.to_string()) || names_system_drive))
        || has_any(&t, &["clear-disk", "initialize-disk"])
        || (has(&t, "diskpart") && (has(&t, "clean") || has(&t, "/s") || has(&t, "format")))
    {
        return Some(KernelRule::SystemDiskFormat);
    }
    if has_any(&t, &BOOT_TOOLS) || (has(&t, "mountvol") && has(&t, "/s")) {
        return Some(KernelRule::BootloaderModification);
    }
    let audit_off = (has(&t, "wevtutil")
        && has_any(&t, &["cl", "clear-log", "sl", "set-log", "um"]))
        || has_any(&t, &["clear-eventlog", "remove-eventlog"])
        || (has(&t, "auditpol") && has_any(&t, &["/clear", "/remove", "/set"]));
    if audit_off {
        return Some(KernelRule::AuditDisable);
    }
    let protected_target = t.iter().any(|x| is_protected_name(x));
    let service_stop = (has(&t, "sc") && has_any(&t, &SERVICE_VERBS))
        || (has(&t, "net") && has(&t, "stop"))
        || has_any(&t, &PS_SERVICE_CMDLETS)
        || has_any(&t, &KILL_VERBS);
    if protected_target && service_stop {
        return Some(KernelRule::KillSwitchDisable);
    }
    if has_any(&t, &["cmdkey", "vaultcmd"]) || t.iter().any(|x| x.contains("keymgr")) {
        return Some(KernelRule::CredentialDenylist);
    }
    let paths = path_tokens(&t, ctx);
    if paths.iter().any(|p| ctx.deny.is_denied_normalized(p)) {
        return Some(KernelRule::CredentialDenylist);
    }
    let kernel: Vec<NormPath> = ctx.kernel_paths.iter().map(PathScope::norm).collect();
    if paths.iter().any(|p| kernel.iter().any(|k| inside(p, k))) {
        return Some(KernelRule::KernelPolicyChange);
    }
    let (windows, boot) = system_areas(sd);
    if paths.iter().any(|p| boot.iter().any(|b| inside(p, b))) {
        return Some(KernelRule::BootloaderModification);
    }
    let deletes = has_any(&t, &DELETE_VERBS);
    let drive_root = normalize(&format!("{sd}:\\"), &PathEnv::new());
    if deletes
        && paths
            .iter()
            .any(|p| inside(p, &windows) || *p == drive_root)
    {
        return Some(KernelRule::SystemRootDeletion);
    }
    None
}
