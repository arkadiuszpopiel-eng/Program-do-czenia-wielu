//! Usuwanie sesji równolegle z otwieraniem jej bazy: czytelnik widzi albo działającą sesję,
//! albo `NotFound` — nigdy błędu sejfu (klucz już zniszczony, a wpis katalogu jeszcze jest).
//! Regresja: `memory_scopes` po `sessions_remove` zgłaszał „brak klucza sesji”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};

use sessions_contract::{NewSession, SessionCatalog, SessionDbProvider, SessionError};

#[test]
fn opening_a_session_while_it_is_deleted_never_reports_a_vault_error() {
    let h = common::harness();
    let ids: Vec<_> = (0..40)
        .map(|i| {
            h.create_session(NewSession {
                title: format!("S{i}"),
                ..NewSession::default()
            })
            .unwrap()
            .id
        })
        .collect();
    let done = AtomicBool::new(false);
    let unexpected = std::thread::scope(|scope| {
        let readers: Vec<_> = (0..3)
            .map(|_| {
                scope.spawn(|| {
                    let mut bad = Vec::new();
                    while !done.load(Ordering::Acquire) {
                        for id in &ids {
                            match h.session_db(id) {
                                Ok(_) | Err(SessionError::NotFound { .. }) => {}
                                Err(e) => bad.push(e.to_string()),
                            }
                        }
                    }
                    bad
                })
            })
            .collect();
        for id in &ids {
            h.delete_session(id).unwrap();
        }
        done.store(true, Ordering::Release);
        readers
            .into_iter()
            .flat_map(|r| r.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        unexpected.is_empty(),
        "błędy inne niż NotFound: {unexpected:?}"
    );
    for id in &ids {
        assert!(matches!(
            h.session_db(id),
            Err(SessionError::NotFound { .. })
        ));
    }
}
