//! Testy property-based deny-list: obejścia przez `..`, wielkość liter, ukośniki, prefiksy
//! urządzeń, zmienne środowiskowe, końcowe kropki, strumienie ADS i aliasy 8.3.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::deny::{normalize, normalize_host};
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use proptest::prelude::*;

const PROFILE: &str = r"C:\Users\Test";

fn env() -> PathEnv {
    PathEnv::windows_profile(PROFILE)
}

fn checker() -> DenyChecker {
    DenyChecker::new(DenyLists::baseline(), &env())
}

const DENIED_BASES: [&str; 7] = [
    r"C:\Users\Test\.claude",
    r"C:\Users\Test\.codex",
    r"C:\Users\Test\.claude.json",
    r"C:\Users\Test\AppData\Local\Google\Chrome\User Data",
    r"C:\Users\Test\AppData\Local\Microsoft\Edge\User Data",
    r"C:\Users\Test\AppData\Roaming\Microsoft\Credentials",
    r"C:\Users\Test\AppData\Roaming\Mozilla\Firefox\Profiles",
];

#[derive(Debug, Clone)]
struct Mangle {
    case_mask: Vec<bool>,
    sep: u8,
    detour_at: Option<usize>,
    prefix: u8,
    env_form: u8,
    trailing: u8,
    stream: bool,
}

fn mangle_strategy() -> impl Strategy<Value = Mangle> {
    (
        prop::collection::vec(any::<bool>(), 64),
        0u8..4,
        prop::option::of(0usize..8),
        0u8..6,
        0u8..4,
        0u8..3,
        any::<bool>(),
    )
        .prop_map(
            |(case_mask, sep, detour_at, prefix, env_form, trailing, stream)| Mangle {
                case_mask,
                sep,
                detour_at,
                prefix,
                env_form,
                trailing,
                stream,
            },
        )
}

/// Buduje wariant ścieżki, który w Windows wskazuje to samo miejsce co `base\suffix`.
fn apply(base: &str, suffix: &[String], m: &Mangle) -> String {
    let body = &base[PROFILE.len()..];
    let head = match m.env_form {
        0 => PROFILE.to_owned(),
        1 => "%USERPROFILE%".to_owned(),
        2 => "~".to_owned(),
        _ => "$env:USERPROFILE".to_owned(),
    };
    let head = if m.env_form == 0 {
        match m.prefix {
            1 => format!(r"\\?\{head}"),
            2 => format!(r"\\.\{head}"),
            3 => format!(r"\??\{head}"),
            4 => format!(r"\\localhost\c$\{}", &head[3..]),
            5 => format!(r"\\?\UNC\127.0.0.1\C$\{}", &head[3..]),
            _ => head,
        }
    } else {
        head
    };
    let mut comps: Vec<String> = body
        .split('\\')
        .filter(|c| !c.is_empty())
        .map(str::to_owned)
        .collect();
    comps.extend(suffix.iter().cloned());
    if let Some(at) = m.detour_at {
        let at = at.min(comps.len());
        comps.insert(at, "..".into());
        comps.insert(at, "zzz".into());
        comps.insert(at, ".".into());
    }
    if let Some(first) = comps.iter_mut().find(|c| c.starts_with('.') && c.len() > 2) {
        match m.trailing {
            1 => first.push('.'),
            2 => first.push_str(". "),
            _ => {}
        }
    }
    if m.stream
        && let Some(last) = comps.last_mut()
    {
        last.push_str(":secret");
    }
    let sep = match m.sep {
        0 => "\\",
        1 => "/",
        2 => "\\\\",
        _ => "/\\",
    };
    let joined = format!("{head}{sep}{}", comps.join(sep));
    joined
        .chars()
        .zip(m.case_mask.iter().cycle())
        .map(|(c, up)| {
            if *up {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .replace("$ENV:USERPROFILE", "$env:USERPROFILE")
}

fn name() -> impl Strategy<Value = String> {
    "p[a-z0-9]{0,8}"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn mangled_denied_paths_stay_denied(
        base in 0usize..DENIED_BASES.len(),
        suffix in prop::collection::vec(name(), 0..3),
        m in mangle_strategy(),
    ) {
        let path = apply(DENIED_BASES[base], &suffix, &m);
        prop_assert!(checker().is_denied_path(&path, &env()), "{path} -> {:?}", normalize(&path, &env()));
    }

    #[test]
    fn safe_paths_stay_allowed(
        dirs in prop::collection::vec(name(), 1..4),
        m in mangle_strategy(),
    ) {
        let base = format!(r"{PROFILE}\projects");
        let path = apply(&base, &dirs, &m);
        prop_assert!(!checker().is_denied_path(&path, &env()), "{path}");
    }

    #[test]
    fn denied_domains_resist_formatting(
        idx in 0usize..12,
        sub in prop::option::of("[a-z]{1,6}"),
        upper in any::<bool>(),
        scheme in 0u8..4,
        port in prop::option::of(1u16..65535),
        trailing_dot in any::<bool>(),
        path in "[a-z0-9/]{0,10}",
    ) {
        let lists = DenyLists::baseline();
        let domain = &lists.domains[idx % lists.domains.len()];
        let mut host = match &sub { Some(s) => format!("{s}.{domain}"), None => domain.clone() };
        if trailing_dot { host.push('.'); }
        if upper { host = host.to_uppercase(); }
        let port = port.map(|p| format!(":{p}")).unwrap_or_default();
        let url = match scheme {
            0 => format!("{host}{port}"),
            1 => format!("https://{host}{port}/{path}"),
            2 => format!("HTTP://user@{host}{port}/{path}"),
            _ => format!("https:\\\\{host}{port}\\{path}"),
        };
        prop_assert!(checker().is_denied_domain(&url), "{url}");
        let lookalike = format!("{domain}.evil.example");
        prop_assert!(!checker().is_denied_domain(&lookalike));
        let glued = format!("x{domain}");
        prop_assert!(!checker().is_denied_domain(&glued));
    }

    #[test]
    fn safe_domains_allowed(host in "p[a-z0-9]{0,10}\\.example\\.(com|pl)") {
        prop_assert!(!checker().is_denied_domain(&host));
        prop_assert_eq!(normalize_host(&host), Some(host.clone()));
    }
}

#[test]
fn short_names_and_relative_paths() {
    let c = checker();
    let e = env();
    for p in [
        r"C:\Users\Test\CLAUDE~1\x",
        r"C:\Users\Test\AppData\Local\Google\Chrome\USERDA~1\Default",
        r"..\..\.claude\settings.json",
        r"Google\Chrome\User Data\Local State",
        r"file:///C:/Users/Test/.codex/auth.json",
        r"\\?\GLOBALROOT\Device\HarddiskVolume3\Users\Test\.codex",
        r"%LocalAppData%\Microsoft\Vault\x",
        r"${env:APPDATA}\Microsoft\Protect\S-1-5",
    ] {
        assert!(c.is_denied_path(p, &e), "{p}");
    }
    for p in [r"C:\Users\Test\CL~1.txt", "", r"C:\", r"%UNKNOWN%\x"] {
        assert!(!c.is_denied_path(p, &e), "{p}");
    }
}

#[test]
fn normalization_examples() {
    let e = env();
    let n = normalize(r"\\?\C:\Users\TEST\.\x\..\.Claude.  \a:b", &e);
    assert_eq!(n.comps, vec!["users", "test", ".claude", "a"]);
    let rel = normalize(r"a\..\..\b", &e);
    assert_eq!(rel.comps, vec!["b"]);
}
