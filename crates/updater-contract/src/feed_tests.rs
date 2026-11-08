use super::*;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[test]
fn channels_and_modes_from_settings() {
    assert_eq!(Channel::from_setting("preview"), Channel::Beta);
    assert_eq!(Channel::from_setting("beta"), Channel::Beta);
    assert_eq!(Channel::from_setting("stable"), Channel::Stable);
    assert_eq!(Channel::from_setting("cokolwiek"), Channel::Stable);
    assert!(Channel::Stable.accepts(&v("1.2.0")));
    assert!(!Channel::Stable.accepts(&v("1.3.0-beta.1")));
    assert!(Channel::Beta.accepts(&v("1.3.0-beta.1")));
    let preview: Channel = serde_json::from_str("\"preview\"").unwrap();
    assert_eq!(preview, Channel::Beta);
    assert_eq!(serde_json::to_string(&Channel::Beta).unwrap(), "\"beta\"");
    assert_eq!(UpdateMode::from_setting("auto"), UpdateMode::Auto);
    assert_eq!(UpdateMode::from_setting("manual"), UpdateMode::Manual);
    assert_eq!(UpdateMode::from_setting("x"), UpdateMode::Ask);
    assert!(!UpdateMode::Manual.checks_automatically());
    assert!(UpdateMode::Ask.checks_automatically());
}

#[test]
fn downgrade_only_as_explicit_user_rollback() {
    let cur = v("1.2.0");
    assert!(check_install_allowed(&v("1.3.0"), &cur, InstallIntent::Update).is_ok());
    for older in ["1.2.0", "1.1.9", "1.2.0-rc.1"] {
        assert!(matches!(
            check_install_allowed(&v(older), &cur, InstallIntent::Update),
            Err(UpdaterError::Downgrade { .. })
        ));
        assert!(check_install_allowed(&v(older), &cur, InstallIntent::UserRollback).is_ok());
    }
}

#[test]
fn urls_are_https_or_loopback_in_tests() {
    assert!(is_allowed_url("https://repo.example/alfa", false));
    assert!(!is_allowed_url("https://", false));
    assert!(!is_allowed_url("http://repo.example/alfa", true));
    assert!(!is_allowed_url("http://127.0.0.1:8080/x", false));
    assert!(is_allowed_url("http://127.0.0.1:8080/x", true));
    assert!(is_allowed_url("http://[::1]:8080/x", true));
    assert!(!is_allowed_url("http://127.0.0.1.evil.example/x", true));
    assert!(!is_allowed_url("http://localhost@evil.example/x", true));
    assert!(!is_allowed_url("https://a.example/ x", false));
    assert!(!is_allowed_url("file:///C:/alfa.zip", true));
    assert_eq!(
        manifest_url("https://repo.example/wydania/", Channel::Beta, false).unwrap(),
        "https://repo.example/wydania/beta.json"
    );
    assert!(matches!(
        manifest_url("http://repo.example", Channel::Stable, false),
        Err(UpdaterError::NotConfigured { .. })
    ));
    let m = "https://repo.example/wydania/stable.json";
    assert_eq!(
        resolve_url(m, "alfa-1.2.0-x64.zip", false).unwrap(),
        "https://repo.example/wydania/alfa-1.2.0-x64.zip"
    );
    assert_eq!(
        resolve_url(m, "https://cdn.example/a.zip", false).unwrap(),
        "https://cdn.example/a.zip"
    );
    for bad in [
        "../alfa.zip",
        "/alfa.zip",
        "a//b.zip",
        "a\\b.zip",
        "http://cdn.example/a.zip",
        "a.zip?x=1",
        "",
    ] {
        assert!(resolve_url(m, bad, false).is_err(), "{bad}");
    }
}

#[test]
fn manifest_must_match_channel_and_schema() {
    let mut m = ReleaseManifest {
        schema: RELEASES_SCHEMA,
        channel: "stable".into(),
        releases: Vec::new(),
    };
    assert!(validate_manifest(&m, Channel::Stable).is_ok());
    assert!(validate_manifest(&m, Channel::Beta).is_err());
    m.schema = 2;
    assert!(validate_manifest(&m, Channel::Stable).is_err());
}
