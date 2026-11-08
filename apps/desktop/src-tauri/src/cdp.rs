//! Port debugowania WebView2 (CDP) — F3-13 / PT-34 (PLAN §8.2, THREAT_MODEL S13).
//!
//! Port istnieje **wyłącznie** w buildzie testowym z cechą `e2e` (Playwright łączy się przez
//! `localhost:9222`). Build produkcyjny nie dodaje oknom żadnych argumentów przeglądarki i nie
//! startuje, gdy środowisko procesu próbuje je wstrzyknąć: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`
//! z flagą zdalnego debugowania (tak WebView2 otwiera CDP bez zmiany kodu) albo
//! `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` (podmieniony silnik WebView2). Agentki nie mogą ustawić
//! zmiennych `WEBVIEW2_*` (`platform-apps-contract::env_write_denied`); ta kontrola zamyka drogę
//! przez inne narzędzia i ręczne `setx`. Polityka rejestru `…\Policies\Microsoft\Edge\WebView2`
//! nie jest tu sprawdzana (wymaga `windows-rs` — poza powłoką).

/// Zmienna z dodatkowymi argumentami przeglądarki WebView2.
pub const ENV_ARGS: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
/// Zmienna z katalogiem innego silnika WebView2.
pub const ENV_BROWSER: &str = "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER";

/// Rdzeń flag zdalnego debugowania (`…-port`, `…-pipe`, `…-address`; bez wielkości liter).
const REMOTE_DEBUGGING: &str = "remote-debugging";

/// Domyślny port CDP buildu testowego.
#[cfg(feature = "e2e")]
const DEFAULT_PORT: u16 = 9222;
/// Zmienna z portem CDP instancji testowej (tylko build `e2e`; docs/plan/24 §2.7).
#[cfg(feature = "e2e")]
pub const ENV_PORT: &str = "ALFA_E2E_CDP_PORT";
/// Zmienna oznaczająca instancję testową uruchamianą obok Alfy właściciela (tylko build `e2e`).
#[cfg(feature = "e2e")]
pub const ENV_INSTANCE: &str = "ALFA_E2E_INSTANCE";

/// Port CDP z wartości zmiennej: tylko porty nieuprzywilejowane; śmieci → domyślny.
#[cfg(feature = "e2e")]
fn port_from(value: Option<&str>) -> u16 {
    value
        .and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(DEFAULT_PORT)
}

/// Argumenty przeglądarki okien w buildzie testowym (`e2e`): port CDP dla Playwrighta
/// (domyślnie 9222; instancja testowa obok Alfy właściciela bierze własny z [`ENV_PORT`]).
#[cfg(feature = "e2e")]
pub fn browser_args() -> Option<String> {
    let port = port_from(std::env::var(ENV_PORT).ok().as_deref());
    Some(format!(
        // Okno poza ekranem (`hidden_instance`) dalej rysuje: bez wykrywania zasłonięcia okna
        // i bez usypiania stron w tle (inaczej zrzuty i animacje czekają w nieskończoność).
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion \
         --disable-backgrounding-occluded-windows --disable-renderer-backgrounding \
         --remote-debugging-port={port}"
    ))
}

/// Build produkcyjny: bez dodatkowych argumentów przeglądarki — bez portu CDP.
#[cfg(not(feature = "e2e"))]
pub fn browser_args() -> Option<String> {
    None
}

/// Instancja testowa (`e2e` + [`ENV_INSTANCE`]`=1`): działa obok uruchomionej Alfy właściciela na
/// własnym profilu, więc nie przekazuje argumentów istniejącej instancji i się nie zamyka.
#[cfg(feature = "e2e")]
pub fn test_instance() -> bool {
    std::env::var(ENV_INSTANCE).is_ok_and(|v| v == "1")
}

/// Build produkcyjny nie ma instancji testowych — zawsze jedna instancja.
#[cfg(not(feature = "e2e"))]
pub fn test_instance() -> bool {
    false
}

/// Zmienna, która pokazuje okna instancji testowej na ekranie (do podglądu testów; domyślnie ukryte).
#[cfg(feature = "e2e")]
pub const ENV_VISIBLE: &str = "ALFA_E2E_VISIBLE";

/// Instancja testowa pracuje poza ekranem (właściciel 1.11: „testy mają mi nie przeszkadzać”):
/// okna poza obszarem monitorów, bez paska zadań, bez fokusu i powiadomień systemowych.
/// [`ENV_VISIBLE`]`=1` pokazuje je jak zwykle.
#[cfg(feature = "e2e")]
pub fn hidden_instance() -> bool {
    test_instance() && std::env::var(ENV_VISIBLE).ok().is_none_or(|v| v != "1")
}

/// Build produkcyjny: okna zawsze na ekranie.
#[cfg(not(feature = "e2e"))]
pub fn hidden_instance() -> bool {
    false
}

/// Powód odmowy startu, gdy środowisko otwiera CDP albo podmienia silnik WebView2 (`None` —
/// bezpieczne). W buildzie `e2e` zawsze `None`.
pub fn environment_violation(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    if cfg!(feature = "e2e") {
        return None;
    }
    let opens_cdp = get(ENV_ARGS).is_some_and(|args| {
        args.to_ascii_lowercase()
            .replace('_', "-")
            .contains(REMOTE_DEBUGGING)
    });
    if opens_cdp {
        return Some(format!(
            "{ENV_ARGS} włącza zdalne debugowanie WebView2 (port CDP) — usuń zmienną i uruchom Alfę ponownie"
        ));
    }
    if get(ENV_BROWSER).is_some_and(|dir| !dir.trim().is_empty()) {
        return Some(format!(
            "{ENV_BROWSER} podmienia silnik WebView2 — usuń zmienną i uruchom Alfę ponownie"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flaga składana z części, żeby ten test nie był jej wystąpieniem w źródle.
    const FLAG: &str = concat!("--remote-", "debugging-port");

    #[cfg(not(feature = "e2e"))]
    #[test]
    fn production_build_adds_no_browser_args() {
        assert_eq!(browser_args(), None);
        assert!(
            !test_instance(),
            "build produkcyjny zawsze pilnuje jednej instancji"
        );
    }

    #[cfg(feature = "e2e")]
    #[test]
    fn e2e_build_opens_cdp_for_playwright() {
        assert!(browser_args().is_some_and(|args| args.contains(FLAG)));
        assert_eq!(port_from(None), 9222);
        assert_eq!(port_from(Some("9333")), 9333);
        assert_eq!(port_from(Some(" 9444 ")), 9444);
        // Porty uprzywilejowane i śmieci nie przechodzą.
        assert_eq!(port_from(Some("80")), 9222);
        assert_eq!(port_from(Some("abc")), 9222);
        assert_eq!(port_from(Some("99999")), 9222);
        assert_eq!(environment_violation(|_| Some(FLAG.to_owned())), None);
    }

    #[cfg(not(feature = "e2e"))]
    #[test]
    fn production_refuses_environment_that_opens_cdp() {
        fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
            let owned: Vec<(String, String)> = pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect();
            move |name| {
                owned
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| v.clone())
            }
        }
        assert_eq!(environment_violation(env(&[])), None);
        assert_eq!(
            environment_violation(env(&[(ENV_ARGS, "--disable-features=msSmartScreen")])),
            None
        );
        assert_eq!(environment_violation(env(&[(ENV_BROWSER, " ")])), None);
        for bad in [
            "--Remote-Debugging-Port=9222",
            "--remote-debugging-pipe",
            "--remote_debugging_address=0.0.0.0",
        ] {
            let why = environment_violation(env(&[(ENV_ARGS, bad)]));
            assert!(why.is_some_and(|w| w.contains(ENV_ARGS)), "{bad}");
        }
        let why = environment_violation(env(&[(ENV_BROWSER, "C:\\inny-silnik")]));
        assert!(why.is_some_and(|w| w.contains(ENV_BROWSER)));
    }

    /// Jedyne wystąpienie flagi CDP w powłoce i jej konfiguracji: `browser_args` za
    /// `#[cfg(feature = "e2e")]` w tym pliku (regresja = port w wydaniu, CVSS 9,3).
    #[test]
    fn cdp_flag_exists_only_behind_e2e_feature() {
        let others = [
            ("lib.rs", include_str!("lib.rs")),
            ("main.rs", include_str!("main.rs")),
            ("windows.rs", include_str!("windows.rs")),
            ("shell.rs", include_str!("shell.rs")),
            ("commands.rs", include_str!("commands.rs")),
            ("kernel.rs", include_str!("kernel.rs")),
            ("logs.rs", include_str!("logs.rs")),
            ("pump.rs", include_str!("pump.rs")),
            ("shortcuts.rs", include_str!("shortcuts.rs")),
            ("tray.rs", include_str!("tray.rs")),
            ("tauri.conf.json", include_str!("../tauri.conf.json")),
            (
                "tauri.bundle.conf.json",
                include_str!("../tauri.bundle.conf.json"),
            ),
        ];
        for (name, text) in others {
            assert!(!text.contains(REMOTE_DEBUGGING), "flaga CDP w {name}");
        }
        let own = include_str!("cdp.rs");
        let hits: Vec<usize> = own.match_indices(FLAG).map(|(i, _)| i).collect();
        assert_eq!(hits.len(), 1, "jedno wystąpienie flagi w cdp.rs");
        let before = &own[..hits[0]];
        let Some(gate) = before.rfind("#[cfg(feature = \"e2e\")]") else {
            panic!("flaga CDP bez bramki cechy e2e");
        };
        let gated = &before[gate..];
        assert!(
            gated.contains("pub fn browser_args"),
            "flaga CDP poza `browser_args`"
        );
        assert!(
            !gated.contains("#[cfg(not("),
            "flaga CDP w gałęzi produkcyjnej"
        );
    }
}
