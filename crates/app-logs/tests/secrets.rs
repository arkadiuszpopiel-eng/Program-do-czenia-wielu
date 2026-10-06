//! Test szpiegowski (ACC-F1-core-log-04 dla dziennika procesu): klucze API i treść rozmowy
//! wpisane w zdarzenia `tracing` nie trafiają do pliku; jedno zdarzenie = jedna linia;
//! poziomy z filtra (także zmiana w czasie działania i biblioteki spoza Alfy).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;

use app_logs::{LogConfig, Rotation, build};

const KEY: &str = "sk-ant-api03-Zx9Qw8Er7Ty6Ui5Op4As3Df2Gh1Jk0Lz";
const KEY_TAIL: &str = "Zx9Qw8Er7Ty6Ui5Op";
const PLAIN: &str = "zwykle-haslo-bez-wzorca";
const OPAQUE: &str = "Mq7Lp2Xv9Rk4Tz8Wn1Bc6Hd3Fs5Gj0Yt";

fn config(dir: &Path, filter: &str) -> LogConfig {
    LogConfig {
        process: "alfa".into(),
        dir: Some(dir.to_path_buf()),
        filter: filter.into(),
        filter_from_env: false,
        stderr: false,
        rotation: Rotation::default(),
        clock: Arc::new(|| chrono::DateTime::from_timestamp(1_791_331_200, 0).unwrap()),
        panic_hook: false,
    }
}

fn read_logs(dir: &Path) -> String {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "log"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

#[derive(Debug)]
#[allow(dead_code)]
struct ProviderConfig {
    base_url: String,
    api_key: String,
}

#[derive(Debug)]
struct HttpError(String);

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP 401: {}", self.0)
    }
}

impl std::error::Error for HttpError {}

#[test]
fn api_keys_and_conversation_never_reach_the_file() {
    let dir = tempfile::tempdir().unwrap();
    // Cel zdarzeń = nazwa binarki testu (`secrets`) — spoza Alfy, więc wskazany jawnie.
    let (subscriber, handle) = build(config(dir.path(), "info,secrets=info"));
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(
            api_key = PLAIN,
            dostawca = "anthropic",
            "połączenie z dostawcą"
        );
        tracing::warn!(klucz = KEY, "klucz w polu o nazwie sekretu");
        tracing::warn!(wartosc = KEY, "klucz w zwykłym polu");
        tracing::error!(naglowek = %format!("Authorization: Bearer {KEY}"), "odrzucono");
        tracing::info!("wpisany klucz {KEY} w komunikacie");
        let cfg = ProviderConfig {
            base_url: "https://api.anthropic.com".into(),
            api_key: KEY.into(),
        };
        tracing::info!(config = ?cfg, "konfiguracja dostawcy");
        let err = HttpError(format!("{{\"error\":\"invalid x-api-key {KEY}\"}}"));
        tracing::error!(
            error = &err as &(dyn std::error::Error + 'static),
            "błąd dostawcy"
        );
        tracing::info!(token = OPAQUE, sesja = "s1", "token bez prefiksu");
        tracing::info!(obcy = OPAQUE, "token bez prefiksu w zwykłym polu");
        tracing::info!(
            prompt = "Ala ma kota i tajny plan",
            text = "treść rozmowy użytkownika",
            screenshot_png = "iVBORw0KGgo",
            "tura rozmowy"
        );
    });
    let text = read_logs(dir.path());
    for secret in [KEY, KEY_TAIL, PLAIN, OPAQUE] {
        assert!(
            !text.contains(secret),
            "sekret „{secret}” w dzienniku:\n{text}"
        );
    }
    for content in ["Ala ma kota", "treść rozmowy", "iVBORw0KGgo"] {
        assert!(
            !text.contains(content),
            "treść „{content}” w dzienniku:\n{text}"
        );
    }
    assert_eq!(text.lines().count(), 10, "{text}");
    assert!(text.contains("[REDACTED]"));
    assert!(text.contains("połączenie z dostawcą"));
    assert!(text.contains("dostawca=anthropic"));
    assert!(text.contains("prompt=\"[pominięto: 24 znaków]\""), "{text}");
    assert!(text.contains("https://api.anthropic.com"), "{text}");
    assert!(handle.problem().is_none());
    assert!(handle.current_file().unwrap().starts_with(dir.path()));
}

#[test]
fn every_event_is_one_line_with_timestamp_level_and_target() {
    let dir = tempfile::tempdir().unwrap();
    let (subscriber, _handle) = build(config(dir.path(), "info"));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(
            target: "app_core::kernel",
            sciezka = "C:\\Users\\Ala\\Alfa",
            "linia\n2026-01-01T00:00:00.000Z ERROR fałszywa\r"
        );
    });
    let text = read_logs(dir.path());
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(
        text.starts_with("2026-10-07T00:00:00.000Z  WARN app_core::kernel: linia\\n2026"),
        "{text}"
    );
    assert!(text.contains("sciezka=C:\\Users\\Ala\\Alfa"), "{text}");
    let name = handle_file_name(dir.path());
    assert_eq!(name, "alfa.2026-10-07.000.log");
}

fn handle_file_name(dir: &Path) -> String {
    let files = app_logs::list_files(dir, "alfa").unwrap();
    assert_eq!(files.len(), 1);
    files[0]
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn levels_follow_filter_and_change_at_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let (subscriber, handle) = build(config(dir.path(), "info"));
    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "app_core::x", "ukryty-debug");
        tracing::info!(target: "app_core::x", "widoczny-info");
        tracing::info!(target: "hyper::proto", "ukryty-hyper-info");
        tracing::warn!(target: "hyper::proto", "widoczny-hyper-warn");
        handle.set_filter("debug,hyper=info").unwrap();
        tracing::debug!(target: "app_core::x", "widoczny-debug");
        tracing::info!(target: "hyper::proto", "widoczny-hyper-info");
        tracing::trace!(target: "app_core::x", "ukryty-trace");
        assert!(handle.set_filter("głośno").is_err());
        tracing::debug!(target: "app_core::x", "nadal-debug");
    });
    let text = read_logs(dir.path());
    for hidden in ["ukryty-debug", "ukryty-hyper-info", "ukryty-trace"] {
        assert!(!text.contains(hidden), "{hidden}:\n{text}");
    }
    for shown in [
        "widoczny-info",
        "widoczny-hyper-warn",
        "widoczny-debug",
        "widoczny-hyper-info",
        "nadal-debug",
    ] {
        assert!(text.contains(shown), "{shown}:\n{text}");
    }
}

#[test]
fn settings_respect_env_override_and_report_bad_values() {
    let dir = tempfile::tempdir().unwrap();
    let (_s, handle) = build(config(dir.path(), "info"));
    assert!(handle.apply_settings(Some("debug"), Some(3)).is_empty());
    assert_eq!(handle.filter(), app_logs::Filter::parse("debug").unwrap());
    let problems = handle.apply_settings(Some("bardzo"), None);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("logs.level"));
    assert_eq!(handle.filter(), app_logs::Filter::parse("debug").unwrap());

    let mut env = config(dir.path(), "warn");
    env.filter_from_env = true;
    let (_s, from_env) = build(env);
    assert!(from_env.apply_settings(Some("trace"), None).is_empty());
    assert_eq!(from_env.filter(), app_logs::Filter::parse("warn").unwrap());
}

#[test]
fn bad_filter_and_missing_dir_fall_back_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let (_s, handle) = build(config(dir.path(), "x=głośno"));
    assert_eq!(handle.filter(), app_logs::Filter::default());
    assert!(handle.problem().unwrap().contains("ALFA_LOG"));

    let blocker = dir.path().join("plik");
    std::fs::write(&blocker, b"x").unwrap();
    let (subscriber, handle) = build(config(&blocker.join("logs"), "info"));
    tracing::subscriber::with_default(subscriber, || tracing::info!("bez pliku"));
    assert!(handle.dir().is_none());
    assert!(handle.problem().unwrap().contains("katalog dziennika"));
}
