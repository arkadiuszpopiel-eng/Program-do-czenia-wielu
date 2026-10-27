//! Odczyt poziomu głównego katalogu do arkusza faktów zadania (poza wątkami async).
use crate::{FileFact, WorkdirFacts};

/// Pobiera metadane katalogu zadania w tle. Brak katalogu albo błąd odczytu = brak danych.
pub async fn read_workdir_facts(path: Option<String>) -> Option<WorkdirFacts> {
    let path = path?;
    tokio::task::spawn_blocking(move || list_workdir(path))
        .await
        .ok()
        .flatten()
}

/// Poziom główny katalogu roboczego do arkusza faktów (nazwy i rozmiary; katalogi najpierw).
fn list_workdir(path: String) -> Option<WorkdirFacts> {
    let mut all: Vec<FileFact> = std::fs::read_dir(&path)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| {
            let meta = entry.metadata().ok();
            FileFact {
                name: entry.file_name().to_string_lossy().into_owned(),
                dir: meta.as_ref().is_some_and(std::fs::Metadata::is_dir),
                bytes: meta.map_or(0, |m| m.len()),
            }
        })
        .collect();
    all.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    let total = all.len();
    all.truncate(crate::MAX_FILES);
    Some(WorkdirFacts {
        path,
        entries: all,
        total,
    })
}
