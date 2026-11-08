//! Przypadki: round-trip (test przywracania), dry-run, tryby i kolizje, snapshot + rollback.

use accounts_hub_contract::SecretString;
use sessions_contract::{NewTurn, SessionId};

use super::fixtures::seed;
use super::{Harness, ok, wipe, world};
use crate::report::{DryRunReport, ItemDiff, ItemRef, ItemState, PlannedAction};
use crate::scope::{
    Category, CollisionResolution, ExportRequest, ExportScope, ImportMode, ImportOptions, ModeMap,
    Selection,
};

/// Hasło testowe.
pub(crate) fn pwd() -> SecretString {
    SecretString::from("hasło testowe 123")
}

/// Pełny zakres (wszystko, łącznie z logami, nakładką i sesjami prywatnymi).
pub(crate) fn full_scope() -> ExportScope {
    ExportScope {
        sessions: Selection::All,
        memory: Selection::All,
        artifacts: true,
        logs: true,
        config_machine: true,
        include_private: true,
        ..ExportScope::default()
    }
}

pub(crate) fn find<'a>(report: &'a DryRunReport, item: &ItemRef) -> &'a ItemDiff {
    report
        .items
        .iter()
        .find(|i| &i.item == item)
        .unwrap_or_else(|| panic!("brak elementu {item} w raporcie"))
}

pub(crate) fn doc(category: Category, name: &str) -> ItemRef {
    ItemRef::Document {
        category,
        name: name.to_owned(),
    }
}

pub(crate) fn sess(id: &SessionId) -> ItemRef {
    ItemRef::Session { id: id.clone() }
}

/// Test przywracania (`ACC-F7-transfer-04`, `ACC-F1-transfer-01`): eksport pełny → wyczyszczenie
/// → import → stan identyczny; ponowny dry-run nie ma nic do zrobienia.
pub fn round_trip_restores_identical_state(h: &dyn Harness) {
    seed(h, false);
    let before = world(h);
    let pkg = h.path("pelny.alfa");
    let mut req = ExportRequest::new(full_scope(), &pkg);
    req.password = Some(pwd());
    let report = ok(h.transfer().export(&req));
    assert_eq!(report.manifest.scope.counts.sessions, 3);
    assert_eq!(report.manifest.redactions, 0);
    assert!(report.manifest.encryption.is_some());
    wipe(h);
    assert!(world(h).sessions.is_empty() && world(h).documents.is_empty());
    let opts = ImportOptions {
        password: Some(pwd()),
        include_machine_overlay: true,
        ..ImportOptions::default()
    };
    let imported = ok(h.transfer().import(&pkg, &opts));
    assert_eq!(imported.failed, 0, "{imported:?}");
    assert_eq!(
        imported.added as usize,
        before.sessions.len() + before.documents.len()
    );
    assert_eq!(world(h), before);
    let again = ok(h.transfer().inspect(&pkg, &opts));
    assert_eq!(again.report.writes(), 0, "{:?}", again.report);
}

/// Dry-run pokazuje różnice i niczego nie zapisuje.
pub fn dry_run_writes_nothing(h: &dyn Harness) {
    let seeded = seed(h, false);
    let pkg = h.path("zakres.alfa");
    let scope = ExportScope {
        sessions: Selection::Only(vec![seeded.s1.clone(), seeded.s2.clone()]),
        ..ExportScope::default()
    };
    ok(h.transfer().export(&ExportRequest::new(scope, &pkg)));
    ok(h.store(Category::ConfigCommon)
        .write("shared.toml", b"[voice]\nengine = \"pocket\"\n"));
    ok(h.sessions()
        .append_turn(&seeded.s1, Some(seeded.s1_leaf), NewTurn::user("lokalnie")));
    ok(h.sessions().delete_session(&seeded.s2));
    let mid = world(h);

    let add = ok(h.transfer().inspect(&pkg, &ImportOptions::default()));
    assert_eq!(world(h), mid, "dry-run niczego nie zapisuje");
    assert_eq!(add.manifest.scope.counts.sessions, 2);
    let config = find(&add.report, &doc(Category::ConfigCommon, "shared.toml"));
    assert_eq!(config.state, ItemState::Changed);
    assert!(matches!(config.action, PlannedAction::Skip { .. }));
    let s1 = find(&add.report, &sess(&seeded.s1));
    assert_eq!(s1.state, ItemState::Changed);
    assert!(matches!(s1.action, PlannedAction::Skip { .. }));
    let s2 = find(&add.report, &sess(&seeded.s2));
    assert_eq!(
        (s2.state, &s2.action),
        (ItemState::New, &PlannedAction::Add)
    );
    let personas = find(&add.report, &doc(Category::Personas, "personas.json"));
    assert_eq!(
        (personas.state, &personas.action),
        (ItemState::Same, &PlannedAction::Keep)
    );
    assert_eq!(add.report.writes(), 1);

    let merge = ImportOptions {
        modes: ModeMap::all(ImportMode::Merge),
        ..ImportOptions::default()
    };
    let merged = ok(h.transfer().inspect(&pkg, &merge));
    assert_eq!(
        find(&merged.report, &doc(Category::ConfigCommon, "shared.toml")).action,
        PlannedAction::Merge
    );
    assert_eq!(
        find(&merged.report, &sess(&seeded.s1)).action,
        PlannedAction::Merge
    );
    assert_eq!(world(h), mid);
}

/// Tryby dodaj/scal/zastąp i kolizje identyfikatorów sesji (scal, kopia z mapą, zastąp).
pub fn modes_and_collisions(h: &dyn Harness) {
    let seeded = seed(h, false);
    let only_s1 = || ExportScope {
        sessions: Selection::Only(vec![seeded.s1.clone()]),
        ..ExportScope::default()
    };
    let base = h.path("baza.alfa");
    ok(h.transfer().export(&ExportRequest::new(only_s1(), &base)));
    let remote_turn = ok(h.sessions().append_turn(
        &seeded.s1,
        Some(seeded.s1_leaf),
        NewTurn::user("z drugiej maszyny"),
    ));
    let remote = h.path("zdalna.alfa");
    ok(h.transfer().export(&ExportRequest::new(only_s1(), &remote)));
    // Powrót do bazy i lokalna, rozbieżna kontynuacja (ta sama sesja na dwóch maszynach).
    let replace = ImportOptions {
        modes: ModeMap::all(ImportMode::Replace),
        ..ImportOptions::default()
    };
    ok(h.transfer().import(&base, &replace));
    let local_turn =
        ok(h.sessions()
            .append_turn(&seeded.s1, Some(seeded.s1_leaf), NewTurn::user("lokalna")));
    let local_before = ok(h.sessions().all_turns(&seeded.s1));
    ok(h.store(Category::ConfigCommon)
        .write("shared.toml", b"[voice]\nengine = \"pocket\"\nrate = 2\n"));

    let inspected = ok(h.transfer().inspect(&remote, &ImportOptions::default()));
    let item = find(&inspected.report, &sess(&seeded.s1));
    assert_eq!(item.state, ItemState::Collision);
    assert_eq!(inspected.report.collisions().len(), 1);

    // Scal: suma drzew, lokalne tury bajtowo bez zmian, tura zdalna jako nowy wariant.
    let merge = ImportOptions {
        modes: ModeMap::all(ImportMode::Merge),
        ..ImportOptions::default()
    };
    let merged = ok(h.transfer().import(&remote, &merge));
    assert_eq!((merged.merged, merged.failed), (2, 0), "{merged:?}");
    let after = ok(h.sessions().all_turns(&seeded.s1));
    assert_eq!(after.len(), local_before.len() + 1);
    assert_eq!(&after[..local_before.len()], &local_before[..]);
    let added = &after[local_before.len()];
    assert_eq!(added.content, remote_turn.content);
    assert_eq!(added.parent, Some(seeded.s1_leaf));
    assert_ne!(added.branch, local_turn.branch);
    let config = ok(h.store(Category::ConfigCommon).read("shared.toml")).unwrap_or_default();
    let config = String::from_utf8(config).unwrap_or_default();
    assert!(
        config.contains("piper") && config.contains("rate = 2"),
        "{config}"
    );

    // Kopia: nowy identyfikator z mapą, tytuł z sufiksem, oryginał nietknięty.
    let mut copy = ImportOptions::default();
    copy.resolutions
        .insert(seeded.s1.clone(), CollisionResolution::Copy);
    let copied = ok(h.transfer().import(&remote, &copy));
    assert_eq!(copied.copied, 1);
    let new_id = copied
        .id_map
        .get(&seeded.s1)
        .cloned()
        .unwrap_or_else(|| panic!("brak mapy id"));
    assert_ne!(new_id, seeded.s1);
    let copy_meta = ok(h.sessions().session(&new_id));
    assert!(copy_meta.title.contains("(import z"), "{}", copy_meta.title);
    assert_eq!(ok(h.sessions().all_turns(&seeded.s1)), after);
    assert_eq!(
        ok(h.sessions().turn_count(&new_id)),
        local_before.len() as u64
    );

    // Zastąp: sesja dokładnie jak w paczce.
    let replaced = ok(h.transfer().import(&remote, &replace));
    assert_eq!(replaced.failed, 0);
    let s1 = ok(h.sessions().all_turns(&seeded.s1));
    assert_eq!(
        s1.last().map(|t| t.content.clone()),
        Some(remote_turn.content)
    );
    assert_eq!(s1.len(), local_before.len());
}

/// Automatyczny snapshot przed importem i rollback jednym kliknięciem (stan sprzed importu).
pub fn snapshot_and_rollback(h: &dyn Harness) {
    let seeded = seed(h, false);
    let pkg = h.path("do-rollbacku.alfa");
    let scope = ExportScope {
        sessions: Selection::Only(vec![seeded.s1.clone(), seeded.s2.clone()]),
        memory: Selection::All,
        ..ExportScope::default()
    };
    ok(h.transfer().export(&ExportRequest::new(scope, &pkg)));
    ok(h.store(Category::ConfigCommon)
        .write("shared.toml", b"[ui]\ntheme = \"light\"\n"));
    ok(h.store(Category::Memory).remove("global.ndjson"));
    ok(h.sessions().append_turn(
        &seeded.s1,
        Some(seeded.s1_leaf),
        NewTurn::user("po eksporcie"),
    ));
    ok(h.sessions().delete_session(&seeded.s2));
    let before = world(h);
    assert!(ok(h.transfer().snapshots()).is_empty());

    let replace = ImportOptions {
        modes: ModeMap::all(ImportMode::Replace),
        ..ImportOptions::default()
    };
    let report = ok(h.transfer().import(&pkg, &replace));
    assert!(report.replaced >= 2 && report.added >= 2, "{report:?}");
    assert_ne!(world(h), before);
    let snapshot = report.snapshot.unwrap_or_else(|| panic!("brak snapshotu"));
    let list = ok(h.transfer().snapshots());
    assert_eq!(list.last().map(|s| s.id.clone()), Some(snapshot.clone()));
    let rolled = ok(h.transfer().rollback(&snapshot));
    assert!(rolled.failed.is_empty(), "{rolled:?}");
    assert!(rolled.removed >= 2);
    assert_eq!(world(h), before);
    assert!(h.transfer().rollback(&"snap-nie-ma".to_owned()).is_err());
}
