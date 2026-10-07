//! Artefakty sesji jako dokumenty paczek `.alfa`: nazwy `<sesja>/<artefakt>/<plik>`, odrzucanie
//! ścieżek niebezpiecznych, zapis tylko w katalogu importu, usuwanie wyłącznie stamtąd (plik
//! użytkownika poza importem nigdy nie znika), późne wiązanie rejestru i uzgodnienie po imporcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use app_files::ArtifactDocuments;
use artifacts_contract::{Artifacts, Origin};
use common::Env;
use sessions_contract::{SessionId, Sessions};
use transfer_contract::DocumentStore;

#[test]
fn names_paths_and_removal_stay_inside_import_dir() {
    let env = Env::new();
    let s = env.session("Artefakty");
    let out = env.paths.workdirs().join("Artefakty").join("out");
    std::fs::create_dir_all(&out).unwrap();
    let own = out.join("raport.txt");
    std::fs::write(&own, "mój plik").unwrap();
    let art = env
        .artifacts
        .register(&s, &own, Origin::User, None)
        .unwrap();
    let name = format!("{s}/{}/raport.txt", art.id);
    assert_eq!(env.docs.list().unwrap(), vec![name.clone()]);
    assert_eq!(
        env.docs.read(&name).unwrap().as_deref(),
        Some(&b"m\xc3\xb3j plik"[..])
    );
    assert!(
        !env.docs.remove(&name).unwrap(),
        "plik użytkownika nie jest usuwany"
    );
    assert!(own.exists());
    for bad in ["../x/y/z", "a/b", "a/../b/c", "C:/x/y", "a\\b\\c/d/e"] {
        assert!(env.docs.write(bad, b"x").is_err(), "{bad}");
    }
    assert!(
        env.docs
            .read(&format!("{s}/brak/raport.txt"))
            .unwrap()
            .is_none()
    );
    assert!(
        env.docs
            .read(&format!("{s}/{}/inna-nazwa.txt", art.id))
            .unwrap()
            .is_none(),
        "nazwa pliku musi zgadzać się z rejestrem"
    );

    let imported = format!("{s}/stary-id/wynik.txt");
    env.docs.write(&imported, b"z paczki").unwrap();
    let path = env
        .paths
        .workdirs()
        .join("Import")
        .join(s.as_str())
        .join("stary-id")
        .join("wynik.txt");
    assert_eq!(std::fs::read(&path).unwrap(), b"z paczki");
    assert_eq!(
        env.artifacts.list(&s).unwrap().len(),
        2,
        "sesja istnieje — rejestracja"
    );
    assert!(env.docs.remove(&imported).unwrap());
    assert!(!path.exists());
}

#[test]
fn unbound_store_is_empty_and_late_sessions_are_reconciled() {
    let env = Env::new();
    let docs = ArtifactDocuments::new(env.paths.workdirs().join("Import"), 1 << 20);
    assert!(docs.list().unwrap().is_empty(), "przed wiązaniem — pusto");
    assert!(docs.write("s/a/f.txt", b"x").is_err());
    docs.bind(
        env.artifacts.clone() as Arc<dyn Artifacts>,
        env.sessions.clone() as Arc<dyn Sessions>,
    );
    let future = SessionId::new("sess-0001");
    docs.write(&format!("{future}/a1/plik.txt"), b"tresc")
        .unwrap();
    assert!(docs.list().unwrap().is_empty(), "sesji jeszcze nie ma");
    let s = env.session("Pozniej");
    assert_eq!(s, future);
    assert_eq!(docs.reconcile(), 1);
    assert_eq!(docs.reconcile(), 0, "bez podwójnej rejestracji");
    assert_eq!(docs.list().unwrap().len(), 1);
}
