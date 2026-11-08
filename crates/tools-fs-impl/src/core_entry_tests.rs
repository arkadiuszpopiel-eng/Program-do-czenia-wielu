//! Porównywanie wpisów katalogu: reguły hosta i separatory Windows.

use super::*;

/// `fs_stat` zwracał rozmiar 0, gdy model podał ścieżkę z `/` i inną wielkością liter niż na dysku.
#[cfg(windows)]
#[test]
fn separators_and_case_do_not_matter_when_matching_a_listed_entry() {
    let listed = Path::new(r"C:\Dane\docs").join("readme.md");
    assert!(same_ignoring_case(
        &listed,
        Path::new(r"C:\Dane\DOCS\README.MD")
    ));
    if cfg!(windows) {
        assert!(same_ignoring_case(
            &listed,
            Path::new("C:/Dane/docs/README.MD")
        ));
        assert!(same_entry(&listed, Path::new("c:/dane/docs/Readme.md")));
    }
    assert!(!same_ignoring_case(
        &listed,
        Path::new(r"C:\Dane\docs\readme.txt")
    ));
    assert!(!same_ignoring_case(
        &listed,
        Path::new(r"C:\Dane\readme.md")
    ));
}

#[test]
fn native_components_keep_the_hosts_case_matching_rule() {
    let listed = Path::new("Dane").join("docs").join("readme.md");
    let other_case = Path::new("Dane").join("DOCS").join("README.MD");
    assert!(same_entry(&listed, &listed));
    assert!(same_ignoring_case(&listed, &other_case));
    assert_eq!(same_entry(&listed, &other_case), cfg!(windows));
    for other in [
        Path::new("Dane").join("docs").join("readme.txt"),
        Path::new("Dane").join("readme.md"),
    ] {
        assert!(!same_ignoring_case(&listed, &other));
        assert!(!same_entry(&listed, &other));
    }
}
