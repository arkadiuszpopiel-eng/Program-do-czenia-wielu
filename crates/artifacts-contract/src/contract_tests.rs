//! Współdzielone testy kontraktowe (`ACC-F1-artifacts-01`): rejestracja, wersje, podgląd, diff,
//! intencje. Pliki powstają w katalogu tymczasowym testu.

use std::ops::Deref;
use std::path::{Path, PathBuf};

use core_bus_contract::AgentId;
use sessions_contract::{SessionId, TurnId};

use crate::types::{ArtifactAction, ArtifactError, ArtifactId, Artifacts, Origin, Preview};
use crate::util::sha256_hex;

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("nieoczekiwany błąd: {e}"))
}

fn sid(s: &str) -> SessionId {
    SessionId::new(s)
}

fn write(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    ok(std::fs::write(&path, content));
    path
}

fn agent() -> Origin {
    Origin::Agent {
        agent: AgentId::new("delta"),
    }
}

fn tmp() -> tempfile::TempDir {
    ok(tempfile::tempdir())
}

/// Rejestracja, deduplikacja identycznej treści, kolejne wersje.
pub fn register_and_versions(a: &dyn Artifacts) {
    let dir = tmp();
    let path = write(dir.path(), "raport.md", b"linia 1\nlinia 2\n");
    let art = ok(a.register(&sid("S1"), &path, agent(), Some(TurnId(3))));
    assert_eq!(art.name, "raport.md");
    assert_eq!(art.origin, agent());
    let v1 = &art.versions[0];
    assert_eq!(
        (v1.version, v1.bytes, v1.mime.as_str()),
        (1, 16, "text/markdown")
    );
    assert_eq!(v1.sha256, sha256_hex(b"linia 1\nlinia 2\n"));
    assert_eq!(v1.source_turn, Some(TurnId(3)));
    assert!(v1.snapshot);
    let same = ok(a.register(&sid("S1"), &path, agent(), None));
    assert_eq!((same.id.clone(), same.versions.len()), (art.id.clone(), 1));
    write(dir.path(), "raport.md", b"linia 1\nLINIA 2\nlinia 3\n");
    let v2 = ok(a.register(&sid("S1"), &path, agent(), Some(TurnId(5))));
    assert_eq!((v2.id.clone(), v2.versions.len()), (art.id.clone(), 2));
    let other = write(dir.path(), "raport-kopia.md", "inna treść\n".as_bytes());
    let v3 = ok(a.add_version(&sid("S1"), &art.id, &other, None));
    assert_eq!(v3.latest().map(|v| v.version), Some(3));
    assert_eq!(
        v3.versions[..2],
        v2.versions[..],
        "wcześniejsze wersje niezmienne"
    );
    assert_eq!(ok(a.get(&sid("S1"), &art.id)), v3);
}

/// Podgląd (także starej wersji po nadpisaniu pliku) i diff.
pub fn preview_and_diff(a: &dyn Artifacts) {
    let dir = tmp();
    let path = write(dir.path(), "notatka.txt", "żółw\nkot\n".as_bytes());
    let art = ok(a.register(&sid("S1"), &path, Origin::User, None));
    write(dir.path(), "notatka.txt", "żółw\npies\nkot\n".as_bytes());
    ok(a.register(&sid("S1"), &path, Origin::User, None));
    let old = ok(a.preview(&sid("S1"), &art.id, Some(1), 1024));
    assert_eq!(
        old,
        Preview::Text {
            text: "żółw\nkot\n".into(),
            truncated: false
        }
    );
    match ok(a.preview(&sid("S1"), &art.id, None, 3)) {
        Preview::Text { text, truncated } => assert!(text == "ż" && truncated),
        other => panic!("{other:?}"),
    }
    let diff = ok(a.diff(&sid("S1"), &art.id, 1, 2));
    assert_eq!((diff.added, diff.removed), (1, 0));
    assert!(diff.unified.contains("+pies"));
    assert!(matches!(
        a.diff(&sid("S1"), &art.id, 1, 9),
        Err(ArtifactError::VersionNotFound { version: 9, .. })
    ));
    let bin_path = write(dir.path(), "obraz.png", &[0x89, b'P', b'N', b'G', 0, 0, 1]);
    let bin = ok(a.register(&sid("S1"), &bin_path, Origin::User, None));
    assert!(matches!(
        ok(a.preview(&sid("S1"), &bin.id, None, 1024)),
        Preview::Binary { bytes: 7, .. }
    ));
    write(dir.path(), "obraz.png", &[0x89, b'P', b'N', b'G', 0, 0, 2]);
    ok(a.register(&sid("S1"), &bin_path, Origin::User, None));
    assert_eq!(
        a.diff(&sid("S1"), &bin.id, 1, 2),
        Err(ArtifactError::NotText)
    );
}

/// Intencje akcji UI są walidowane i wskazują wersję.
pub fn intents(a: &dyn Artifacts) {
    let dir = tmp();
    let path = write(dir.path(), "wynik.csv", b"a;b\n1;2\n");
    let art = ok(a.register(&sid("S1"), &path, agent(), None));
    let reveal = ok(a.intent(&sid("S1"), &art.id, None, ArtifactAction::Reveal));
    assert_eq!((reveal.version, reveal.path.clone()), (1, path.clone()));
    assert_eq!(reveal.sha256, art.versions[0].sha256);
    let target = ArtifactAction::SaveAs {
        target: dir.path().join("kopia.csv"),
    };
    assert_eq!(
        ok(a.intent(&sid("S1"), &art.id, Some(1), target.clone())).action,
        target
    );
    let handoff = ArtifactAction::SendToSession { target: sid("S2") };
    assert!(a.intent(&sid("S1"), &art.id, None, handoff).is_ok());
    let to_self = ArtifactAction::SendToSession { target: sid("S1") };
    assert!(matches!(
        a.intent(&sid("S1"), &art.id, None, to_self),
        Err(ArtifactError::Invalid { .. })
    ));
    assert!(matches!(
        a.intent(&sid("S1"), &art.id, Some(7), ArtifactAction::Open),
        Err(ArtifactError::VersionNotFound { .. })
    ));
    let missing = ArtifactId("brak".into());
    assert!(matches!(
        a.intent(&sid("S1"), &missing, None, ArtifactAction::Open),
        Err(ArtifactError::NotFound { .. })
    ));
}

/// Lista w kolejności rejestracji; izolacja sesji; brakujące pliki.
pub fn list_isolation_and_missing_files(a: &dyn Artifacts) {
    let dir = tmp();
    let first = ok(a.register(
        &sid("S1"),
        &write(dir.path(), "1.txt", b"1"),
        Origin::User,
        None,
    ));
    let second = ok(a.register(
        &sid("S1"),
        &write(dir.path(), "2.txt", b"2"),
        Origin::User,
        None,
    ));
    ok(a.register(
        &sid("S2"),
        &write(dir.path(), "3.txt", b"3"),
        Origin::User,
        None,
    ));
    let ids: Vec<ArtifactId> = ok(a.list(&sid("S1"))).into_iter().map(|x| x.id).collect();
    assert_eq!(ids, vec![first.id.clone(), second.id]);
    assert!(matches!(
        a.get(&sid("S2"), &first.id),
        Err(ArtifactError::NotFound { .. })
    ));
    assert!(matches!(
        a.register(
            &sid("S1"),
            &dir.path().join("nie-ma.txt"),
            Origin::User,
            None
        ),
        Err(ArtifactError::FileNotFound { .. })
    ));
    assert!(matches!(
        a.register(&sid("S1"), dir.path(), Origin::User, None),
        Err(ArtifactError::FileNotFound { .. })
    ));
    assert!(
        a.out_dir("Raport (2)")
            .ends_with(Path::new("Sesje").join("Raport (2)").join("out"))
    );
}

/// Uruchamia cały zestaw; `factory` daje świeży rejestr (sesje `S1`, `S2` dostępne).
pub fn run_all<H, A>(factory: impl Fn() -> H)
where
    H: Deref<Target = A>,
    A: Artifacts,
{
    let cases: [fn(&dyn Artifacts); 4] = [
        register_and_versions,
        preview_and_diff,
        intents,
        list_isolation_and_missing_files,
    ];
    for case in cases {
        let harness = factory();
        case(&*harness);
    }
}
