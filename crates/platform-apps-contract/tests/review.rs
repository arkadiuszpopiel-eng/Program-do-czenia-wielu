//! Przegląd poprawności fali 3 (`docs/reviews/2026-10-wave3-review.md`, W3-02): deny-lista zapisu
//! zmiennych użytkownika (`system_env_set`) obejmuje wszystkie znane zmienne, przez które nowy
//! proces ładuje cudzy kod albo wyłącza zabezpieczenia (SPEC `tools-system`: „wstrzykujących
//! kod”) — także te spoza ekosystemów z pierwotnej listy (Java, OpenSSL, PowerShell, Python,
//! Perl, Ruby, Cargo, npm). Zapis `HKCU\Environment` działa na każdy proces uruchomiony później
//! przez Eksplorator — to trwałość (persistence) poza zasięgiem Brokera.

use platform_apps_contract::env_write_denied;

#[test]
fn code_loading_variables_are_denied() {
    for name in [
        // JVM: `-javaagent:` ładuje dowolny JAR do każdej maszyny Javy.
        "JAVA_TOOL_OPTIONS",
        "_JAVA_OPTIONS",
        "JDK_JAVA_OPTIONS",
        // OpenSSL (git, curl, Python…): plik konfiguracji i katalogi modułów/silników → DLL.
        "OPENSSL_CONF",
        "OPENSSL_MODULES",
        "OPENSSL_ENGINES",
        // PowerShell: nadpisanie zasad wykonywania skryptów (`Bypass`).
        "PSExecutionPolicyPreference",
        // Python/Perl/Ruby: biblioteka standardowa i moduły z katalogu napastnika.
        "PYTHONHOME",
        "PERL5LIB",
        "PERLLIB",
        "RUBYOPT",
        "RUBYLIB",
        // Narzędzia budowania (właściciel buduje Alfę na tej maszynie): wrapper kompilatora,
        // `runner` celu, powłoka skryptów npm.
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER",
        "CARGO_HOME",
        "npm_config_script_shell",
        "NPM_CONFIG_USERCONFIG",
    ] {
        assert!(
            env_write_denied(name).is_some(),
            "{name} powinna być zabroniona"
        );
    }
    for ok in ["JAVA_HOME", "EDITOR", "MOJA_ZMIENNA", "PYTHONIOENCODING"] {
        assert_eq!(env_write_denied(ok), None, "{ok}");
    }
}

/// W3-05: wartość zmiennej trafia na kartę zatwierdzenia Brokera jako `setx NAZWA "wartość"`
/// (`tools-system::env_command`). Znaki sterujące (nowe linie, CR, ESC, znaki kierunku pisma są
/// osobnym tematem Broker-UI) pozwalały rozbić kartę na wiele linii i przesunąć istotną część
/// wartości poza widok właściciela. Zmienne środowiskowe nie potrzebują znaków sterujących
/// (tabulator — dozwolony).
#[test]
fn env_values_with_control_characters_are_refused() {
    use platform_apps_contract::check_env_value;
    for bad in ["a\nb", "C:\\bin\r\n(zredagowano)", "x\u{1b}[2Ky", "a\u{0}b"] {
        assert!(check_env_value(bad).is_err(), "{bad:?}");
    }
    for ok in [
        "C:\\bin;D:\\narzędzia",
        "a\tb",
        "%USERPROFILE%\\x",
        "zażółć",
    ] {
        assert!(check_env_value(ok).is_ok(), "{ok:?}");
    }
}
