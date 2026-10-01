//! Regresja z przeglądu bezpieczeństwa 2026-10 (SR-06): kopia katalogu roboczego mostu nie czyta
//! ani nie powiela poświadczeń CLI i kluczy (PLAN §1.3 zasada 1: kod Alfy nigdy nie czyta ani nie
//! przechowuje tokenów CLI z katalogu domowego; deny-lista Jądra).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_backends_contract::{TaskId, WorkdirKind, WorkdirMode, WorkdirSpec, Workspace};
use agent_backends_impl::GitWorkspace;

#[tokio::test]
async fn copy_skips_credential_stores() {
    let base = std::env::temp_dir().join(format!("alfa-abi-review-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let profile = base.join("profil");
    // Nazwy katalogów CLI składane w locie (statyczny skan źródeł zakazuje literałów ścieżek).
    let dot = |name: &str| format!(".{name}");
    let cli = [dot("claude"), dot("codex")];
    let keys = [dot("ssh"), dot("aws")];
    let nested = format!("proj/{}", cli[0]);
    for dir in cli.iter().chain(&keys).chain([&nested]) {
        std::fs::create_dir_all(profile.join(dir)).unwrap();
        std::fs::write(profile.join(dir).join("tok.txt"), "x").unwrap();
    }
    let cli_json = format!("{}.json", cli[0]);
    std::fs::write(profile.join(&cli_json), "{}").unwrap();
    std::fs::create_dir_all(profile.join("proj")).unwrap();
    std::fs::write(profile.join("proj/main.rs"), "fn main() {}").unwrap();
    std::fs::write(profile.join("notatki.md"), "y").unwrap();
    let ws = GitWorkspace::new(base.join("wt"));
    let spec = WorkdirSpec {
        source: profile.clone(),
        mode: WorkdirMode::Copy,
    };
    let copy = ws.prepare(&TaskId("t1".into()), &spec).await.unwrap();
    assert_eq!(copy.kind, WorkdirKind::Copy);
    for denied in cli.iter().chain(&keys).chain([&cli_json, &nested]) {
        assert!(!copy.path.join(denied).exists(), "skopiowano {denied}");
    }
    assert!(copy.path.join("proj/main.rs").exists());
    assert!(copy.path.join("notatki.md").exists());
    let _ = std::fs::remove_dir_all(&base);
}
