//! Runner scenariuszy `sqlite.sessions` z `evals/F7/migrations/scenarios.json` (F7-08; fala 5,
//! m-23): skrypt SQL stanu bazy wykonany na pustej bazie SQLCipher, potem `lib_sqlstore::migrate`
//! z migracjami `sessions`. Oczekiwanie porównywane jako podzbiór (pola nieobecne nie są
//! sprawdzane), jak opisuje `evals/F7/migrations/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use lib_sqlstore::rusqlite::Connection;
use lib_sqlstore::{DbKey, migrate, open_connection};
use serde_json::Value;
use sessions_impl::SESSION_MIGRATIONS;

const SCENARIOS: &str = include_str!("../../../evals/F7/migrations/scenarios.json");

fn eval_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/F7/migrations")
}

fn schema_objects(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
        .unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

/// Sprawdza flagi zachowania bazy po migracji (tury czytelne, append-only, zapis zablokowany).
fn check_behaviour(id: &str, conn: &Connection, expected: &Value) {
    if expected["turns_decoded"] == true {
        let mut stmt = conn.prepare("SELECT body FROM turns ORDER BY id").unwrap();
        let bodies: Vec<Vec<u8>> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!bodies.is_empty(), "{id}: brak tur");
        for b in bodies {
            let v: Value = serde_json::from_slice(&b).unwrap();
            assert!(
                v["role"].is_string() && v["content"].is_object(),
                "{id}: {v}"
            );
        }
    }
    for (flag, sql) in [
        ("update_blocked", "UPDATE turns SET role = 'user'"),
        ("delete_blocked", "DELETE FROM turns"),
    ] {
        if expected[flag] == true {
            let err = conn.execute(sql, []).unwrap_err().to_string();
            assert!(err.contains("append-only"), "{id} {flag}: {err}");
        }
    }
    if expected["write_blocked"] == true {
        let write = conn.execute(
            "INSERT INTO session_state(key, value) VALUES ('f7', 'x')",
            [],
        );
        assert!(write.is_err(), "{id}: zapis powinien być zablokowany");
    }
}

#[test]
fn f7_sessions_sql_scenarios() {
    let doc: Value = serde_json::from_str(SCENARIOS).unwrap();
    let mut ran = Vec::new();
    for s in doc["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["entity"] == "sqlite.sessions")
    {
        let id = s["id"].as_str().unwrap();
        let (input, expected) = (&s["input"], &s["expected"]);
        assert_eq!(input["namespace"], "sessions", "{id}");
        let dir = tempfile::tempdir().unwrap();
        let conn = open_connection(&dir.path().join("s.db"), &DbKey::generate().unwrap()).unwrap();
        let sql = std::fs::read_to_string(eval_dir().join(input["sql"].as_str().unwrap())).unwrap();
        conn.execute_batch(&sql).unwrap();
        let before = schema_objects(&conn);
        let result = migrate(&conn, "sessions", SESSION_MIGRATIONS);
        if expected["outcome"] == "ok" {
            let report = result.unwrap_or_else(|e| panic!("{id}: {e:?}"));
            assert_eq!(report.applied, strings(&expected["applied"]), "{id}");
            if let Some(newer) = expected.get("newer") {
                assert_eq!(report.newer, strings(newer), "{id}");
            }
            if let Some(ro) = expected.get("read_only") {
                assert_eq!(Some(report.read_only), ro.as_bool(), "{id}");
            }
            check_behaviour(id, &conn, expected);
        } else {
            let err = result.expect_err(id);
            let name = format!("{err:?}");
            let want = expected["error"].as_str().unwrap();
            assert!(name.starts_with(want), "{id}: {name} ≠ {want}");
        }
        if expected["schema_unchanged"] == true {
            assert_eq!(schema_objects(&conn), before, "{id}: schemat zmieniony");
        }
        ran.push(id.to_owned());
    }
    for must in ["m-20", "m-23"] {
        assert!(ran.iter().any(|i| i.starts_with(must)), "{must} ∉ {ran:?}");
    }
}
