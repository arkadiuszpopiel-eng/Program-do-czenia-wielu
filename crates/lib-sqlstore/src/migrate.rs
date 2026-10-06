//! Prosty runner migracji: tabela `schema_migrations`, przestrzeń nazw per moduł, każda migracja
//! w osobnym punkcie zapisu (SAVEPOINT) — w całości albo wcale.
//!
//! **Powrót do starszej wersji programu** (rollback aktualizatora, ADR 0007; fala 5, m-23): baza
//! z migracjami nieznanymi kodowi, ale nowszymi od każdej znanej, nie jest odrzucana ani zmieniana.
//! Gdy nowsza wersja oznaczyła je jako **addytywne** (tabela `schema_compat`), starsza pracuje
//! normalnie; inaczej połączenie przechodzi w tryb tylko do odczytu (`PRAGMA query_only`) —
//! dane czytelne, żaden zapis starszego kodu nie zepsuje schematu nowszego. Raport mówi o tym
//! wywołującemu ([`MigrationReport::newer`], [`MigrationReport::read_only`]).

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::StoreError;
use crate::unix_millis;

/// Wynik [`migrate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Wersje zastosowane w tym wywołaniu (w kolejności).
    pub applied: Vec<String>,
    /// Migracje zapisane przez **nowszą** wersję programu, nieznane temu kodowi (rosnąco) — baza
    /// po powrocie do starszej wersji. Wywołujący powinien pokazać ostrzeżenie.
    pub newer: Vec<String>,
    /// Połączenie jest tylko do odczytu (`PRAGMA query_only`) — któraś z `newer` nie jest
    /// addytywna albo zrobiła to wcześniej inna przestrzeń nazw na tym samym połączeniu.
    pub read_only: bool,
}

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS schema_migrations(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;";

/// Migracje oznaczone przez program, który je zastosował, jako addytywne (zgodne wstecz).
const CREATE_COMPAT: &str = "CREATE TABLE IF NOT EXISTS schema_compat(
    namespace TEXT NOT NULL,
    version TEXT NOT NULL,
    PRIMARY KEY(namespace, version)
) WITHOUT ROWID;";

/// Stosuje brakujące migracje z `steps` (`(wersja, sql)`, wersje rosnąco) w przestrzeni `namespace`.
///
/// Działa także wewnątrz otwartej transakcji (używa SAVEPOINT). Reguły:
/// - wersje niepuste, unikalne i ściśle rosnące (porównanie tekstowe, np. `0001`, `0002`);
/// - wersje zapisane w bazie, nieznane kodowi i **nowsze** od każdej znanej (baza z nowszej wersji
///   programu) → `Ok` bez zmian: [`MigrationReport::newer`], a gdy nie wszystkie są addytywne —
///   połączenie tylko do odczytu ([`MigrationReport::read_only`]);
/// - wersja nieznana kodowi, a nie nowsza od znanych (luka, rozwidlenie) → [`StoreError::UnknownMigration`];
/// - oczekująca wersja starsza niż zastosowana → [`StoreError::MigrationOutOfOrder`];
/// - oczekująca migracja na połączeniu tylko do odczytu → [`StoreError::ReadOnly`];
/// - błąd SQL → migracja wycofana w całości, [`StoreError::MigrationFailed`].
pub fn migrate(
    conn: &Connection,
    namespace: &str,
    steps: &[(&str, &str)],
) -> Result<MigrationReport, StoreError> {
    migrate_with(conn, namespace, steps, &[])
}

/// Jak [`migrate`], a wersje z `additive` (podzbiór `steps`) przy zastosowaniu zapisuje w
/// `schema_compat` jako **addytywne**: starsza wersja programu, która ich nie zna, pracuje na bazie
/// normalnie (odczyt i zapis). Addytywna = tylko nowe tabele, indeksy albo kolumny z wartością
/// domyślną; bez nowych wyzwalaczy, ograniczeń na starych tabelach i niezmienników łączących stare
/// tabele z nowymi (wiersze dopisane przez starszy kod nowsza wersja musi przyjąć po ponownej
/// aktualizacji). Zasady i lista migracji: `docs/modules/sessions/SPEC.md` („Fala 5”).
pub fn migrate_with(
    conn: &Connection,
    namespace: &str,
    steps: &[(&str, &str)],
    additive: &[&str],
) -> Result<MigrationReport, StoreError> {
    validate_steps(namespace, steps, additive)?;
    ensure_table(conn, namespace, steps)?;
    let applied = applied_versions(conn, namespace)?;
    let known: BTreeSet<&str> = steps.iter().map(|(v, _)| *v).collect();
    let unknown: Vec<String> = applied
        .iter()
        .filter(|v| !known.contains(v.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return tolerate_newer(conn, namespace, steps, &applied, unknown);
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
        if query_only(conn)? {
            return Err(read_only(namespace, version));
        }
        apply_one(conn, namespace, version, sql, additive.contains(version))?;
        report.applied.push((*version).to_owned());
    }
    report.read_only = query_only(conn)?;
    Ok(report)
}

/// Baza z nowszej wersji programu: nieznane wersje muszą być nowsze od każdej znanej, a znane —
/// wszystkie zastosowane. Nic nie zapisuje; nieaddytywne → `query_only`.
fn tolerate_newer(
    conn: &Connection,
    namespace: &str,
    steps: &[(&str, &str)],
    applied: &BTreeSet<String>,
    unknown: Vec<String>,
) -> Result<MigrationReport, StoreError> {
    let newest_known = steps.iter().map(|(v, _)| *v).max().unwrap_or_default();
    if let Some(gap) = unknown.iter().find(|v| v.as_str() <= newest_known) {
        return Err(StoreError::UnknownMigration {
            namespace: namespace.to_owned(),
            version: gap.clone(),
        });
    }
    if let Some((pending, _)) = steps.iter().find(|(v, _)| !applied.contains(*v)) {
        return Err(StoreError::MigrationOutOfOrder {
            namespace: namespace.to_owned(),
            version: (*pending).to_owned(),
            applied: unknown.iter().max().cloned().unwrap_or_default(),
        });
    }
    let compatible = additive_versions(conn, namespace)?;
    if !unknown.iter().all(|v| compatible.contains(v)) {
        conn.pragma_update(None, "query_only", true)?;
    }
    Ok(MigrationReport {
        applied: Vec::new(),
        newer: unknown,
        read_only: query_only(conn)?,
    })
}

fn validate_steps(
    namespace: &str,
    steps: &[(&str, &str)],
    additive: &[&str],
) -> Result<(), StoreError> {
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
    if let Some(v) = additive
        .iter()
        .find(|a| !steps.iter().any(|(v, _)| v == *a))
    {
        return Err(invalid(&format!(
            "addytywna `{v}` nie jest na liście migracji"
        )));
    }
    Ok(())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, StoreError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn query_only(conn: &Connection) -> Result<bool, StoreError> {
    Ok(conn.query_row("PRAGMA query_only", [], |r| r.get::<_, i64>(0))? != 0)
}

fn read_only(namespace: &str, version: &str) -> StoreError {
    StoreError::ReadOnly {
        namespace: namespace.to_owned(),
        version: version.to_owned(),
    }
}

/// Tworzy `schema_migrations` tylko, gdy jej brak (na połączeniu tylko do odczytu nic nie pisze).
fn ensure_table(
    conn: &Connection,
    namespace: &str,
    steps: &[(&str, &str)],
) -> Result<(), StoreError> {
    if table_exists(conn, "schema_migrations")? {
        return Ok(());
    }
    if query_only(conn)? {
        return match steps.first() {
            Some((v, _)) => Err(read_only(namespace, v)),
            None => Ok(()),
        };
    }
    conn.execute_batch(CREATE_TABLE)?;
    Ok(())
}

fn applied_versions(conn: &Connection, namespace: &str) -> Result<BTreeSet<String>, StoreError> {
    if !table_exists(conn, "schema_migrations")? {
        return Ok(BTreeSet::new());
    }
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations WHERE namespace = ?1")?;
    let rows = stmt.query_map(params![namespace], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn additive_versions(conn: &Connection, namespace: &str) -> Result<BTreeSet<String>, StoreError> {
    if !table_exists(conn, "schema_compat")? {
        return Ok(BTreeSet::new());
    }
    let mut stmt = conn.prepare("SELECT version FROM schema_compat WHERE namespace = ?1")?;
    let rows = stmt.query_map(params![namespace], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn apply_one(
    conn: &Connection,
    namespace: &str,
    version: &str,
    sql: &str,
    additive: bool,
) -> Result<(), StoreError> {
    let failed = |source| StoreError::MigrationFailed {
        namespace: namespace.to_owned(),
        version: version.to_owned(),
        source,
    };
    conn.execute_batch("SAVEPOINT alfa_migration;")?;
    let result = conn
        .execute_batch(sql)
        .and_then(|()| {
            conn.execute(
                "INSERT INTO schema_migrations(namespace, version, applied_at) VALUES (?1, ?2, ?3)",
                params![namespace, version, unix_millis()],
            )
        })
        .and_then(|_| {
            if additive {
                conn.execute_batch(CREATE_COMPAT)?;
                conn.execute(
                    "INSERT OR IGNORE INTO schema_compat(namespace, version) VALUES (?1, ?2)",
                    params![namespace, version],
                )?;
            }
            Ok(())
        });
    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE alfa_migration;")?;
            Ok(())
        }
        Err(e) => {
            conn.execute_batch("ROLLBACK TO alfa_migration; RELEASE alfa_migration;")?;
            Err(failed(e))
        }
    }
}
