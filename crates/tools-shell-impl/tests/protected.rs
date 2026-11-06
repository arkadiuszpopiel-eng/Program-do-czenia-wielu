//! Ścieżki chronione w poleceniach powłoki (plan 50, przegląd 3.11).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::harness;
use tools_common_contract::{DenialReason, ToolStatus};

/// Plan 50 (przegląd 3.11): zapis do ścieżki chronionej wewnątrz katalogu roboczego (`.git\hooks`,
/// `.vscode`) pyta Sędziego Cieni także na „Automatycznie” — zakres samego katalogu by ją ukrył.
#[tokio::test]
async fn protected_path_inside_cwd_needs_approval() {
    let h = harness(&[("/Users/ala/Projekt/a.txt", "a")]);
    for cmd in [
        r"Set-Content .git\hooks\pre-commit 'echo x'",
        r"Copy-Item a.txt .vscode\settings.json",
    ] {
        let out = h.sh(cmd).await;
        assert!(
            matches!(
                out.status,
                ToolStatus::Denied {
                    reason: DenialReason::ApprovalTimeout { .. }
                }
            ),
            "{cmd}: {out:?}"
        );
    }
    assert!(h.exec.runs().is_empty());
    let out = h.sh(r"Copy-Item a.txt b.txt").await;
    assert!(
        !matches!(out.status, ToolStatus::Denied { .. }),
        "zwykły zapis w katalogu roboczym: {out:?}"
    );
}
