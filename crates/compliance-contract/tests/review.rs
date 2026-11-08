//! Regresje z przeglądu bezpieczeństwa 2026-10 (docs/reviews/2026-10-security-review-1.md):
//! domeny dostawców zapisane znakami zgodności Unicode (mapowanie IDNA/UTS 46 robi to samo co
//! przeglądarka i klient HTTP) oraz katalogi kluczy w bazowej deny-liście Jądra.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::deny::normalize_host;
use compliance_contract::{DenyChecker, DenyLists, MANDATORY_PATH_SEGMENTS, PathEnv};
use proptest::prelude::*;

const PROFILE: &str = r"C:\Users\Test";

fn checker() -> DenyChecker {
    DenyChecker::new(DenyLists::baseline(), &PathEnv::windows_profile(PROFILE))
}

/// SR-04: `ⅽlaude.ai` (U+217D), pełna szerokość i litery matematyczne trafiają w `url`/WebView2
/// do `claude.ai` — deny-lista musi widzieć tę samą domenę.
#[test]
fn unicode_compatibility_forms_hit_provider_domains() {
    let c = checker();
    for host in [
        "\u{217d}laude.ai",
        "ｃｌａｕｄｅ．ａｉ",
        "https://ＣＨＡＴＧＰＴ.com/c/1",
        "\u{1d41c}laude.ai",
        "chat\u{ff0e}openai\u{3002}com",
        "gemini.google.com\u{ff61}",
    ] {
        assert!(c.is_denied_domain(host), "{host}");
    }
    assert_eq!(
        normalize_host("ｃｌａｕｄｅ．ａｉ").as_deref(),
        Some("claude.ai")
    );
    assert_eq!(
        normalize_host("bücher.example").as_deref(),
        Some("xn--bcher-kva.example")
    );
    assert!(!c.is_denied_domain("claude.ai.example.org"));
    assert!(!c.is_denied_domain("notclaude.ai"));
}

proptest! {
    /// Dowolna litera domeny dostawcy zamieniona na odpowiednik pełnej szerokości nie omija listy.
    #[test]
    fn fullwidth_letters_never_bypass(mask in prop::collection::vec(any::<bool>(), 9)) {
        let host: String = "claude.ai"
            .chars()
            .zip(mask.iter().copied().chain(std::iter::repeat(false)))
            .map(|(ch, wide)| {
                if wide && ch.is_ascii_alphabetic() {
                    char::from_u32(u32::from(ch) - 0x61 + 0xff41).unwrap_or(ch)
                } else if wide && ch == '.' {
                    '\u{ff0e}'
                } else {
                    ch
                }
            })
            .collect();
        prop_assert!(checker().is_denied_domain(&host), "{}", host);
    }
}

/// Utwardzenie (a): katalogi i pliki kluczy innych narzędzi w bazowej deny-liście Jądra.
#[test]
fn key_stores_are_denied_by_baseline() {
    let c = checker();
    let env = PathEnv::windows_profile(PROFILE);
    for p in [
        r"C:\Users\Test\.ssh\id_ed25519",
        r"%USERPROFILE%\.SSH\config",
        r"C:\Users\Test\SSH~1\id_rsa",
        r"C:\Users\Test\.gnupg\private-keys-v1.d\x.key",
        r"C:\Users\Test\.aws\credentials",
        r"C:\Users\Test\.azure\msal_token_cache.json",
        r"C:\Users\Test\.kube\config",
        r"C:\Users\Test\.docker\config.json",
        r"C:\Users\Test\.npmrc",
        r"C:\work\proj\.npmrc",
        r"C:\Users\Test\.pypirc",
        r"C:\Users\Test\.netrc",
        r"C:\Users\Test\_netrc",
        r"C:\Users\Test\.git-credentials",
        r"C:\Users\Test\AppData\Roaming\gh\hosts.yml",
        r"%APPDATA%\GH\hosts.yml",
        r"D:\kopia\Users\Test\AppData\Roaming\gh\hosts.yml",
    ] {
        assert!(c.is_denied_path(p, &env), "{p}");
    }
    for p in [
        r"C:\Users\Test\.docker\contexts\meta.json",
        r"C:\Users\Test\Documents\ssh-notes.md",
        r"C:\Users\Test\AppData\Roaming\ghostwriter\x",
    ] {
        assert!(!c.is_denied_path(p, &env), "{p}");
    }
    assert!(DenyLists::baseline().validate().is_ok());
    assert!(MANDATORY_PATH_SEGMENTS.contains(&".claude"));
}
