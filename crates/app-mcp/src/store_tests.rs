//! Testy zapisu serwerów i zgód.

use super::*;

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("alfa-mcp-store-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn entry(id: &str) -> ServerEntry {
    ServerEntry {
        id: id.into(),
        name: "Serwer".into(),
        command: "npx".into(),
        args: vec!["-y".into(), "pakiet".into()],
        cwd: None,
        env: BTreeMap::from([("LOG_LEVEL".into(), "info".into())]),
        secret_env: vec!["API_TOKEN".into()],
        trust: TrustLevel::Untrusted,
        enabled: true,
        timeout_ms: 30_000,
    }
}

#[test]
fn servers_roundtrip_and_bad_file_is_empty() {
    let d = dir("servers");
    let path = d.join(SERVERS_FILE);
    assert!(load_servers(&path).is_empty());
    save_servers(&path, &[entry("a"), entry("b")]).unwrap();
    let back = load_servers(&path);
    assert_eq!(back, vec![entry("a"), entry("b")]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("API_TOKEN") && !text.contains("sekret"));
    std::fs::write(&path, "{zepsute").unwrap();
    assert!(load_servers(&path).is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn defaults_fill_missing_fields() {
    let d = dir("defaults");
    let path = d.join(SERVERS_FILE);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        &path,
        r#"{"servers":[{"id":"x","name":"X","command":"uvx"}]}"#,
    )
    .unwrap();
    let s = &load_servers(&path)[0];
    assert!(s.enabled && s.args.is_empty() && s.secret_env.is_empty());
    assert_eq!(s.trust, TrustLevel::Untrusted);
    assert_eq!(s.timeout_ms, DEFAULT_REQUEST_TIMEOUT_MS);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn pins_persist_revoke_and_forget() {
    let d = dir("pins");
    let path = d.join(PINS_FILE);
    let fp = ToolFingerprint::parse(&format!("sha256:{}", "a".repeat(64))).unwrap();
    let store = FilePinStore::open(path.clone());
    for (server, tool) in [("s1", "t1"), ("s1", "t2"), ("s2", "t1")] {
        store.put(ToolPin {
            server: server.into(),
            tool: tool.into(),
            fingerprint: fp.clone(),
        });
    }
    let again = FilePinStore::open(path.clone());
    assert_eq!(again.all().len(), 3);
    assert_eq!(again.get("s1", "t2"), Some(fp.clone()));
    assert!(again.revoke("s1", "t2"));
    assert!(!again.revoke("s1", "t2"));
    again.forget_server("s1");
    let last = FilePinStore::open(path);
    assert_eq!(last.all().len(), 1);
    assert_eq!(last.get("s2", "t1"), Some(fp));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn env_names_secrets_and_slugs() {
    assert!(valid_env_name("GITHUB_TOKEN") && valid_env_name("_x1"));
    assert!(!valid_env_name("1A") && !valid_env_name("A-B") && !valid_env_name(""));
    assert!(looks_secret("GITHUB_PERSONAL_ACCESS_TOKEN") && looks_secret("api_key"));
    assert!(!looks_secret("LOG_LEVEL"));
    assert_eq!(slug("Pliki na dysku Ż"), "pliki-na-dysku-z");
    assert_eq!(slug("  GitHub (praca) "), "github-praca");
    assert_eq!(slug("!!!"), "serwer");
    assert!(slug(&"a".repeat(80)).len() <= 48);
}
