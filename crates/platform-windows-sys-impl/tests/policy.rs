//! Spójność bazowej deny-listy obserwacji z deny-listą Jądra (`compliance`): każdy segment Jądra
//! jest odrzucany przez `WatchPolicy::baseline()`, a obserwacja z prefiksami Jądra (po rozwinięciu
//! zmiennych) odrzuca profile przeglądarek i magazyny poświadczeń — na każdej platformie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use compliance_contract::DenyLists;
use platform_contract::{BASELINE_DENY_SEGMENTS, WatchPolicy};

#[test]
fn baseline_covers_every_kernel_segment() {
    let policy = WatchPolicy::baseline();
    let base = if cfg!(windows) {
        PathBuf::from(r"C:\Users\Ja")
    } else {
        PathBuf::from("/home/ja")
    };
    let kernel = DenyLists::baseline();
    for seg in &kernel.path_segments {
        assert!(
            BASELINE_DENY_SEGMENTS.contains(&seg.as_str()),
            "segment Jądra spoza listy obserwacji: {seg}"
        );
        assert!(policy.is_denied(&base.join(seg)), "{seg}");
        assert!(
            policy.is_denied(&base.join(seg.to_uppercase()).join("x")),
            "{seg}"
        );
    }
    assert!(!policy.is_denied(&base.join("Pobrane")));
}

#[test]
fn kernel_prefixes_extend_the_policy() {
    let kernel = DenyLists::baseline();
    let local = Path::new("/u/AppData/Local");
    let roaming = Path::new("/u/AppData/Roaming");
    let expand = |p: &str| -> PathBuf {
        let p = p.replace('\\', "/");
        let p = p
            .replace("%LOCALAPPDATA%", &local.to_string_lossy())
            .replace("%APPDATA%", &roaming.to_string_lossy())
            .replace("%USERPROFILE%", "/u");
        PathBuf::from(p)
    };
    let prefixes: Vec<PathBuf> = kernel.path_prefixes.iter().map(|p| expand(p)).collect();
    let policy = WatchPolicy::baseline().with_denylist(kernel.path_segments.clone(), prefixes);
    assert!(policy.is_denied(&local.join("Google/Chrome/User Data/Default")));
    assert!(policy.is_denied(&roaming.join("Mozilla/Firefox/Profiles/x")));
    assert!(policy.is_denied(&local.join("MICROSOFT/Vault")));
    assert!(!policy.is_denied(&local.join("Google/Drive")));
    assert!(!policy.is_denied(&local.join("Google/Chrome/User Data2")));
}
