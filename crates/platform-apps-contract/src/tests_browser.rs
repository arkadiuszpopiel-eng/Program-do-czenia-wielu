//! Testy jednostkowe kontraktu przeglądarki: profil, argumenty uruchomienia, adresy, filtr egressu.

use std::path::{Path, PathBuf};

use super::*;

fn spec(
    root: impl Into<PathBuf>,
    profile: impl Into<PathBuf>,
    quarantine: impl Into<PathBuf>,
) -> BrowserSpec {
    BrowserSpec {
        kind: BrowserKind::Edge,
        executable: None,
        alfa_root: root.into(),
        profile_dir: profile.into(),
        quarantine_dir: quarantine.into(),
        headless: true,
    }
}

/// Ścieżka bezwzględna właściwa dla systemu, składana segment po segmencie (`rel` — segmenty
/// rozdzielone `/`): na Windows `C:\a\Alfa`, gdzie indziej `/a/Alfa`. Samo `/a/Alfa` na Windows
/// nie ma litery dysku, więc **nie** jest bezwzględne i `validate` słusznie je odrzuca.
fn abs(rel: &str) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" });
    rel.split('/')
        .filter(|s| !s.is_empty())
        .fold(root, |acc, seg| acc.join(seg))
}

#[test]
fn browser_spec_never_uses_user_profiles() {
    let ok = spec(
        abs("a/Alfa"),
        abs("a/Alfa/browser/profile"),
        abs("a/Alfa/browser/quarantine"),
    );
    assert!(ok.validate().is_ok(), "{ok:?}");
    let win_ok = spec(
        abs("Users/ala/AppData/Local/Alfa"),
        abs("Users/ala/AppData/Local/Alfa/browser/profile"),
        abs("Users/ala/AppData/Local/Alfa/browser/quarantine"),
    );
    assert!(win_ok.validate().is_ok(), "{win_ok:?}");
    // Ścieżka od korzenia bez litery dysku jest bezwzględna tylko poza Windows.
    let rooted = spec("/a/Alfa", "/a/Alfa/p", "/a/Alfa/q");
    assert_eq!(rooted.validate().is_ok(), !cfg!(windows), "{rooted:?}");
    for bad in [
        spec(
            abs("a/Alfa"),
            abs("a/Google/Chrome/User Data"),
            abs("a/Alfa/q"),
        ),
        spec(
            abs("a/Alfa"),
            abs("a/Alfa/../Microsoft/Edge/User Data"),
            abs("a/Alfa/q"),
        ),
        spec(abs("a/Alfa"), abs("a/Alfa/p"), abs("a/Alfa/p/q")),
        spec(abs("a/Alfa"), abs("a/Alfa/p"), abs("a/Alfa/p")),
        spec(abs("a/Alfa"), abs("a/Alfa"), abs("a/Alfa/q")),
        spec(abs("a/Alfa"), abs("a/Inne/p"), abs("a/Alfa/q")),
        spec(
            abs("home/u/.config/google-chrome"),
            abs("home/u/.config/google-chrome/x"),
            abs("home/u/.config/google-chrome/q"),
        ),
        spec(
            abs("Users/ala/AppData/Local/Alfa"),
            abs("Users/ala/AppData/Local/Alfa/../Google/Chrome/User Data/Default"),
            abs("Users/ala/AppData/Local/Alfa/q"),
        ),
        spec(
            abs("Users/ala/AppData/Local/Microsoft/Edge/User Data"),
            abs("Users/ala/AppData/Local/Microsoft/Edge/User Data/Profile 1"),
            abs("Users/ala/AppData/Local/Microsoft/Edge/User Data/q"),
        ),
        spec("rel", "rel/p", "rel/q"),
        spec(r"\a\Alfa", r"\a\Alfa\p", r"\a\Alfa\q"),
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    for user in [
        r"C:\Users\ala\AppData\Local\Google\Chrome\User Data",
        r"C:\Users\ala\AppData\Local\Microsoft\Edge\User Data\Profile 1",
        r"C:\Users\ala\AppData\Roaming\Mozilla\Firefox\Profiles\x.default",
        r"\\?\C:\Users\ala\AppData\Local\Google\Chrome\User Data\Default",
        r"\\serwer\udzial\ala\AppData\Local\BraveSoftware\Brave-Browser\User Data",
        "C:/Users/ala/AppData/Local/Microsoft/Edge/User Data",
        r"c:\USERS\ALA\APPDATA\LOCAL\GOOGLE\CHROME",
        "/home/u/.mozilla/firefox",
    ] {
        assert!(is_user_browser_profile(Path::new(user)), "{user}");
    }
    for alfa in [
        r"C:\Users\ala\AppData\Local\Alfa\browser\profile",
        r"\\?\C:\Users\ala\AppData\Local\Alfa\browser\quarantine",
        "/home/u/.local/share/alfa/browser/profile",
    ] {
        assert!(!is_user_browser_profile(Path::new(alfa)), "{alfa}");
    }
}

#[test]
fn chromium_args_use_pipe_and_isolated_profile() {
    let s = spec("/a/Alfa", "/a/Alfa/browser/profile", "/a/Alfa/browser/q");
    let args = chromium_args(&s);
    assert!(args.contains(&"--remote-debugging-pipe".to_owned()));
    assert!(
        args.iter()
            .all(|a| !a.starts_with("--remote-debugging-port")
                && !a.starts_with("--remote-debugging-address"))
    );
    assert!(args.contains(&"--user-data-dir=/a/Alfa/browser/profile".to_owned()));
    assert!(args.contains(&"--headless=new".to_owned()));
    let prefs = profile_preferences(Path::new("/a/Alfa/browser/q"));
    assert_eq!(prefs["credentials_enable_service"], false);
    assert_eq!(prefs["profile"]["password_manager_enabled"], false);
    assert_eq!(prefs["autofill"]["profile_enabled"], false);
    assert_eq!(prefs["download"]["default_directory"], "/a/Alfa/browser/q");
}

struct Only(&'static str);
impl EgressFilter for Only {
    fn allows(&self, host: &str) -> bool {
        host == self.0
    }
}

#[test]
fn urls_and_egress_filter() {
    assert_eq!(
        check_navigation_url("https://Example.com/a?b").unwrap(),
        "example.com"
    );
    assert_eq!(check_navigation_url("http://[::1]:8080/").unwrap(), "::1");
    for bad in [
        "file:///C:/x",
        "javascript:alert(1)",
        "chrome://settings",
        "edge://passwords",
        "https://user:pass@example.com/",
        "https://",
        "ftp://x",
    ] {
        assert!(check_navigation_url(bad).is_err(), "{bad}");
    }
    assert_eq!(
        url_host("wss://Chat.Example.com:443/s"),
        Some("chat.example.com".into())
    );
    assert_eq!(url_host("https://a.com./x"), Some("a.com".into()));
    // Ukośnik wsteczny = separator (jak w Chromium): host to `good.com`, nie `evil.com`.
    assert_eq!(
        url_host(r"https://good.com\@evil.com/"),
        Some("good.com".into())
    );
    assert_eq!(
        check_navigation_url(r"https://good.com\@evil.com/").unwrap(),
        "good.com"
    );
    assert_eq!(url_host("data:text/html,x"), None);
    let f = Only("example.com");
    assert!(request_allowed(&f, "https://example.com/img.png"));
    assert!(request_allowed(&f, "data:image/png;base64,AA"));
    assert!(request_allowed(&f, "about:blank"));
    assert!(!request_allowed(&f, "https://cdn.other.net/x.js"));
    assert!(!request_allowed(&f, "file:///etc/passwd"));
    assert!(!request_allowed(&f, "chrome-extension://x/y"));
    assert_eq!(BrowserKind::Chrome.exe(), "chrome.exe");
}
