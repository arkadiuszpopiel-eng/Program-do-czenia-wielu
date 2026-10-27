use super::*;
use tools_fs_contract::{FsToolKind, manifest};

#[test]
fn transposed_name_points_to_the_right_operation() {
    let entries = [
        manifest(FsToolKind::Delete),
        manifest(FsToolKind::List),
        manifest(FsToolKind::Read),
    ];
    assert_eq!(
        Catalog::new(entries.iter()).rank("fs_lsit", false, 3)[0],
        "fs_list"
    );
}

#[test]
fn exact_short_command_needs_only_one_result_and_readonly_hides_writes() {
    let entries = [
        manifest(FsToolKind::Write),
        manifest(FsToolKind::List),
        manifest(FsToolKind::Read),
    ];
    let index = Catalog::new(entries.iter());
    assert_eq!(index.rank("czytaj", false, 8), ["fs_read"]);
    assert!(
        !index
            .rank("zapisz", true, 8)
            .contains(&"fs_write".to_owned())
    );
}

#[test]
#[allow(clippy::print_stdout)] // Jawny wynik benchmarku uruchamianego z --nocapture.
fn local_index_lookup_measurement() {
    let entries: Vec<_> = (0..80)
        .map(|i| {
            let mut m = manifest(FsToolKind::Read);
            m.name = format!("device_status_{i}");
            m.title = format!("Stan urządzenia {i}");
            m
        })
        .collect();
    let index = Catalog::new(entries.iter());
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        assert_eq!(
            index.rank("device_status_79", false, 8)[0],
            "device_status_79"
        );
    }
    let exact_us = start.elapsed().as_micros() as f64 / 1000.0;
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        assert!(!index.rank("odczytaj stan urządzenia", false, 8).is_empty());
    }
    let query_us = start.elapsed().as_micros() as f64 / 1000.0;
    println!(
        "LOCAL_CATALOG_BENCHMARK dataset=synthetic_80_tools build=debug exact_us={exact_us:.2} description_us={query_us:.2}"
    );
}

#[test]
fn game_engine_question_finds_engine_tools() {
    let mut engine = manifest(FsToolKind::Read);
    engine.name = "engine_inspect".into();
    engine.title = "Odczyt sceny".into();
    engine.groups = vec!["engine".into(), "shell".into()];
    let entries = [manifest(FsToolKind::Read), engine];
    let found = Catalog::new(entries.iter()).rank("co jest w silniku gry", false, 8);
    assert_eq!(found.first().map(String::as_str), Some("engine_inspect"));
}
