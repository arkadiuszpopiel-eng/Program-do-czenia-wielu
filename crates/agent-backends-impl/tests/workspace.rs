//! Izolowany katalog roboczy: worktree/kopia, zwalnianie, odmowy (źródło w katalogu mostów,
//! kolizja, limit rozmiaru, sesja spoza katalogu mostów).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use agent_backends_contract::{
    BackendError, BridgeKind, SessionRef, TaskId, WorkdirKind, WorkdirMode, WorkdirSpec, Workspace,
};
use agent_backends_impl::GitWorkspace;

fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join(format!("alfa-abi-{}", std::process::id()))
        .join(tag);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=a",
            "-c",
            "user.email=a@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
}

fn spec(source: &Path, mode: WorkdirMode) -> WorkdirSpec {
    WorkdirSpec {
        source: source.to_path_buf(),
        mode,
    }
}

#[tokio::test]
async fn worktree_and_copy_lifecycle() {
    let base = temp("cykl");
    let repo = base.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "x"]);
    let ws = GitWorkspace::new(base.join("wt"));

    let wt = ws
        .prepare(&TaskId("t/1".into()), &spec(&repo, WorkdirMode::Worktree))
        .await
        .unwrap();
    assert_eq!(wt.kind, WorkdirKind::GitWorktree);
    assert!(wt.path.join("a.txt").exists());
    assert!(
        wt.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("t_1")
    );
    let again = ws
        .prepare(&TaskId("t/1".into()), &spec(&repo, WorkdirMode::Worktree))
        .await;
    assert!(matches!(again, Err(BackendError::Workspace(_))));
    ws.release(&wt, false).await.unwrap();
    assert!(!wt.path.exists());

    let copy = ws
        .prepare(&TaskId("t2".into()), &spec(&repo, WorkdirMode::Copy))
        .await
        .unwrap();
    assert_eq!(copy.kind, WorkdirKind::Copy);
    assert!(copy.path.join("a.txt").exists());
    let session = SessionRef {
        bridge: BridgeKind::Codex,
        id: "s".into(),
        workdir: copy.path.clone(),
    };
    let reused = ws.reuse(&session).await.unwrap();
    assert_eq!(reused.kind, WorkdirKind::Resumed);
    ws.release(&reused, false).await.unwrap();
    assert!(
        copy.path.exists(),
        "zwolnienie sesji wznawianej nie usuwa katalogu"
    );
    ws.release(&copy, true).await.unwrap();
    assert!(copy.path.exists());
    ws.release(&copy, false).await.unwrap();
    assert!(!copy.path.exists());
    std::fs::remove_dir_all(&base).unwrap();
}

#[tokio::test]
async fn refusals() {
    let base = temp("odmowy");
    let ws = GitWorkspace::new(base.join("wt")).with_max_copy_bytes(3);
    let plain = base.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("duzy.txt"), "1234").unwrap();
    let big = ws
        .prepare(&TaskId("a".into()), &spec(&plain, WorkdirMode::Worktree))
        .await;
    assert!(matches!(big, Err(BackendError::Workspace(m)) if m.contains("za duży")));
    assert!(
        !base.join("wt").join("a").exists(),
        "niepełna kopia nie została usunięta"
    );

    let inside = base.join("wt").join("zrodlo");
    std::fs::create_dir_all(&inside).unwrap();
    let nested = ws
        .prepare(&TaskId("b".into()), &spec(&inside, WorkdirMode::Copy))
        .await;
    assert!(matches!(nested, Err(BackendError::Workspace(_))));
    let file = plain.join("duzy.txt");
    assert!(
        ws.prepare(&TaskId("c".into()), &spec(&file, WorkdirMode::Copy))
            .await
            .is_err()
    );
    let outside = SessionRef {
        bridge: BridgeKind::ClaudeCode,
        id: "s".into(),
        workdir: plain.clone(),
    };
    assert!(ws.reuse(&outside).await.is_err());
    let root = SessionRef {
        workdir: base.join("wt"),
        ..outside
    };
    assert!(ws.reuse(&root).await.is_err());
    std::fs::remove_dir_all(&base).unwrap();
}
