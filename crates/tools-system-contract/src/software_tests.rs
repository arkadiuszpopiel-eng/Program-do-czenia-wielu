//! Testy wykrywania oprogramowania (wersje, opis, katalog).
use super::*;

#[test]
fn versions_from_tool_output() {
    assert_eq!(parse_version("v20.11.1\n").as_deref(), Some("20.11.1"));
    assert_eq!(
        parse_version("git version 2.45.1.windows.1").as_deref(),
        Some("2.45.1")
    );
    assert_eq!(parse_version("Python 3.12.4").as_deref(), Some("3.12.4"));
    assert_eq!(
        parse_version("openjdk version \"21.0.2\" 2024-01-16").as_deref(),
        Some("21.0.2")
    );
    assert_eq!(parse_version("brak wersji 7"), None);
}

#[test]
fn description_groups_by_kind() {
    let items = vec![
        SoftwareItem {
            id: "git".into(),
            name: "Git".into(),
            kind: SoftwareKind::Vcs,
            version: Some("2.45.1".into()),
            command: "git".into(),
        },
        SoftwareItem {
            id: "edge".into(),
            name: "Microsoft Edge".into(),
            kind: SoftwareKind::Browser,
            version: None,
            command: "C:\\msedge.exe".into(),
        },
    ];
    let d = describe(&items, &["docker".into()]);
    assert!(d.starts_with("Przeglądarki: Microsoft Edge"), "{d}");
    assert!(d.contains("Kontrola wersji: Git 2.45.1"));
    assert!(d.ends_with("Brak: docker"));
}

#[test]
fn catalog_ids_are_unique() {
    let mut ids: Vec<&str> = CLI_CATALOG.iter().map(|e| e.0).collect();
    ids.extend(BROWSER_CATALOG.iter().map(|e| e.0));
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
}
