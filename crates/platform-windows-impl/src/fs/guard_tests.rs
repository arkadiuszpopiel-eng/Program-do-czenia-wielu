//! Testy deny-listy i rozwiązywania ścieżek (przenośne; dowiązania tylko na Unix).

use super::*;

fn policy() -> DenyPolicy {
    DenyPolicy::new(
        &DEFAULT_EXTRA_DENY_NAMES.map(String::from),
        &[PathBuf::from(
            r"C:\Users\u\AppData\Roaming\Microsoft\Protect",
        )],
    )
}

#[test]
fn contract_markers_survive_windows_tricks() {
    let p = policy();
    for denied in [
        "/home/u/.claude/creds.json",
        r"C:\Users\u\.CLAUDE.\settings.json",
        r"C:\Users\u\.codex \auth.json",
        r"\\?\C:\Users\u\.ssh\id_ed25519",
        r"\\?\UNC\srv\share\u\.ssh\id_rsa",
        r"C:\x\Cookies:secret",
        r"C:\x\Login Data::$DATA",
        r"C:/Users/u/AppData/Local/Google/Chrome/User Data/Default/Network/Cookies",
        r"C:\Users\u\AppData\Roaming\Microsoft\Credentials\ABC",
        r"C:\Users\u\AppData\Roaming\Mozilla\Firefox\Profiles\x\logins.json",
        r"C:\Users\u\.AWS\credentials",
    ] {
        assert!(p.is_denied(Path::new(denied)), "{denied}");
    }
    for allowed in [
        "/home/u/projects/claude-notes.md",
        r"C:\Users\u\Documents\cookies-recipe.txt",
        r"C:\Users\u\.claudette\x",
        r"C:\data\file.txt:zone",
    ] {
        assert!(!p.is_denied(Path::new(allowed)), "{allowed}");
    }
}

#[test]
fn prefixes_match_whole_segments_and_admin_shares() {
    let p = policy();
    assert!(p.is_denied(Path::new(
        r"C:\USERS\U\AppData\Roaming\Microsoft\Protect\S-1-5-21\key"
    )));
    assert!(p.is_denied(Path::new(
        r"\\localhost\c$\Users\u\AppData\Roaming\Microsoft\Protect\S-1"
    )));
    assert!(!p.is_denied(Path::new(
        r"C:\Users\u\AppData\Roaming\Microsoft\ProtectMe\x"
    )));
    let unix = DenyPolicy::new(&[], &[PathBuf::from("/srv/secret")]);
    assert!(unix.is_denied(Path::new("/srv/secret/a")));
    assert!(unix.is_denied(Path::new("/SRV/Secret./a")));
    assert!(!unix.is_denied(Path::new("/srv/secretive/a")));
}

#[test]
fn segment_keys_and_lexical_normalization() {
    assert_eq!(segment_key("C:"), "c:");
    assert_eq!(segment_key("Cookies:x"), "cookies");
    assert_eq!(segment_key("abc. . "), "abc");
    assert_eq!(segment_key(".."), "");
    assert_eq!(
        keys_of(Path::new(r"\\?\C:\A\b")),
        vec!["c:".to_owned(), "a".into(), "b".into()]
    );
    assert_eq!(
        lexical_normalize(Path::new("/a/b/../c/./d")),
        PathBuf::from("/a/c/d")
    );
    assert_eq!(lexical_normalize(Path::new("/../..")), PathBuf::from("/"));
}

#[test]
fn env_expansion_only_for_defined_variables() {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    if let Ok(home) = std::env::var(var) {
        let expanded = expand_env(Path::new(&format!("%{var}%/x")));
        assert_eq!(expanded, PathBuf::from(format!("{home}/x")));
    }
    let raw = Path::new("/a/%ALFA_NO_SUCH_VAR_42%/100%/b%%c");
    assert_eq!(expand_env(raw), raw.to_path_buf());
}

#[test]
fn resolve_rejects_relative_empty_and_denied() {
    let p = policy();
    assert_eq!(
        p.resolve(Path::new(""), Follow::Final),
        Err(PlatformError::InvalidPath(PathBuf::new()))
    );
    assert_eq!(
        p.resolve(Path::new("rel/x"), Follow::Final),
        Err(PlatformError::InvalidPath("rel/x".into()))
    );
    let tmp = tempfile::tempdir().unwrap();
    let denied = tmp.path().join(".ssh").join("id_rsa");
    assert_eq!(
        p.resolve(&denied, Follow::Final),
        Err(PlatformError::Denylisted(denied.clone()))
    );
}

#[test]
fn resolve_canonicalizes_existing_prefix_and_keeps_missing_rest() {
    let p = policy();
    let tmp = tempfile::tempdir().unwrap();
    let canonical_root = std::fs::canonicalize(tmp.path()).unwrap();
    let target = tmp.path().join("a").join("..").join("b").join("c.txt");
    let resolved = p.resolve(&target, Follow::Final).unwrap();
    assert_eq!(resolved, canonical_root.join("b").join("c.txt"));
    // Korzeń woluminu właściwy dla systemu (`C:\` na Windows, `/` na Uniksie).
    let volume_root = canonical_root.ancestors().last().unwrap();
    let root = p.resolve(volume_root, Follow::NoFinal).unwrap();
    assert!(root.has_root());
}

#[cfg(unix)]
#[test]
fn resolve_follows_symlinks_into_denied_directories() {
    let p = policy();
    let tmp = tempfile::tempdir().unwrap();
    let secret = tmp.path().join(".codex");
    std::fs::create_dir(&secret).unwrap();
    std::fs::write(secret.join("auth.json"), b"{}").unwrap();
    let link = tmp.path().join("innocent");
    std::os::unix::fs::symlink(&secret, &link).unwrap();
    let through = link.join("auth.json");
    assert_eq!(
        p.resolve(&through, Follow::Final),
        Err(PlatformError::Denylisted(through.clone()))
    );
    // Sam wpis-dowiązanie wskazujący na zastrzeżony katalog też jest blokowany.
    assert!(p.resolve(&link, Follow::NoFinal).is_err());
    let dangling = tmp.path().join("dangling");
    std::os::unix::fs::symlink(tmp.path().join("missing"), &dangling).unwrap();
    assert!(matches!(
        p.resolve(&dangling.join("x"), Follow::Final),
        Err(PlatformError::PermissionDenied(_))
    ));
}
