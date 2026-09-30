//! Prosty runner migracji: tabela `schema_migrations`, przestrzeń nazw per moduł, każda migracja
//! w osobnym punkcie zapisu (SAVEPOINT) — w całości albo wcale.

use std::collections::BTreeSet;

use rusqlite::{Connection, params};

use crate::error::StoreError;
use crate::unix_millis;

/// Wynik [`migrate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Wersje zastosowane w tym wywołaniu (w kolejności).
    pub applied: Vec<String>,
}

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS schema_migrations(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;";

/// Stosuje brakujące migracje z `steps` (`(wersja, sql)`, wersje rosnąco) w przestrzeni `namespace`.
///
/// Działa także wewnątrz otwartej transakcji (używa SAVEPOINT). Reguły:
/// - wersje niepuste, unikalne i ściśle rosnące (porównanie tekstowe, np. `0001`, `0002`);
/// - wersja zapisana w bazie, a nieznana kodowi → [`StoreError::UnknownMigration`];
/// - oczekująca wersja starsza niż zastosowana → [`StoreError::MigrationOutOfOrder`];
/// - błąd SQL → migracja wycofana w całości, [`StoreError::MigrationFailed`].
pub fn migrate(
    conn: &Connection,
    namespace: &str,
    steps: &[(&str, &str)],
) -> Result<MigrationReport, StoreError> {
    validate_steps(namespace, steps)?;
    conn.execute_batch(CREATE_TABLE)?;
    let applied = applied_versions(conn, namespace)?;
    let known: BTreeSet<&str> = steps.iter().map(|(v, _)| *v).collect();
    if let Some(unknown) = applied.iter().find(|v| !known.contains(v.as_str())) {
        return Err(StoreError::UnknownMigration {
            namespace: namespace.to_owned(),
            version: unknown.clone(),
        });
    }
    let newest = applied.iter().max().cloned();
    let mut report = MigrationReport::default();
    for (version, sql) in steps.iter().filter(|(v, _)| !applied.contains(*v)) {
        if let Some(newest) = newest.as_deref().filter(|n| *version < *n) {
            return Err(StoreError::MigrationOutOfOrder {
                namespace: namespace.to_owned(),
                version: (*version).to_owned(),
                applied: newest.to_owned(),
            });
        }
        apply_one(conn, namespace, version, sql)?;
        report.applied.push((*version).to_owned());
    }
    Ok(report)
}

fn validate_steps(namespace: &str, steps: &[(&str, &str)]) -> Result<(), StoreError> {
    let invalid = |reason: &str| StoreError::InvalidMigrations {
        namespace: namespace.to_owned(),
        reason: reason.to_owned(),
    };
    if namespace.trim().is_empty() {
        return Err(invalid("pusta przestrzeń nazw"));
    }
    for pair in steps.windows(2) {
        if pair[0].0 >= pair[1].0 {
            return Err(invalid(&format!(
                "wersje nie są ściśle rosnące: `{}` ≥ `{}`",
                pair[0].0, pair[1].0
            )));
        }
    }
    if steps.iter().any(|(v, _)| v.trim().is_empty()) {
        return Err(invalid("pusta wersja"));
    }
    Ok(())
}

fn applied_versions(conn: &Connection, namespace: &str) -> Result<BTreeSet<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations WHERE namespace = ?1")?;
    let rows = stmt.query_map(params![namespace], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn apply_one(
    conn: &Connection,
    namespace: &str,
    version: &str,
    sql: &str,
) -> Result<(), StoreError> {
    let failed = |source| StoreError::MigrationFailed {
        namespace: namespace.to_owned(),
        version: version.to_owned(),
        source,
    };
    conn.execute_batch("SAVEPOINT alfa_migration;")?;
    let result = conn.execute_batch(sql).and_then(|()| {
        conn.execute(
            "INSERT INTO schema_migrations(namespace, version, applied_at) VALUES (?1, ?2, ?3)",
            params![namespace, version, unix_millis()],
        )
    });
    match result {
        Ok(_) => {
            conn.execute_batch("RELEASE alfa_migration;")?;
            Ok(())
        }
        Err(e) => {
            conn.execute_batch("ROLLBACK TO alfa_migration; RELEASE alfa_migration;")?;
            Err(failed(e))
        }
    }
}
