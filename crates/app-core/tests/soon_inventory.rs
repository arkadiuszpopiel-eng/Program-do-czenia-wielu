//! Spis funkcji „wkrótce” (docs/plan/14-funkcje-wkrotce.md) musi obejmować każdą stronę Ustawień,
//! która w programie ma znak „wkrótce” (`wave` > 1 w `data/settings-pages.json`), a każda taka
//! strona musi mieć zadanie w roadmapie. Zgłoszenie właściciela 2026-10-11: funkcje oznaczone
//! „wkrótce” nie mogą istnieć tylko w programie — mają być zapisane w planie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn every_soon_page_is_listed_in_the_plan_with_a_task() {
    let pages: serde_json::Value = serde_json::from_str(&read("data/settings-pages.json")).unwrap();
    let inventory = read("../../docs/plan/14-funkcje-wkrotce.md");
    let roadmap = read("../../docs/plan/03-roadmapa.md");
    let mut soon = 0;
    for page in pages.as_array().unwrap() {
        let id = page["id"].as_str().unwrap();
        let wave = page["wave"].as_u64().unwrap();
        let upcoming = page["upcoming"].as_array().unwrap();
        if wave <= 1 {
            assert!(
                upcoming.is_empty(),
                "strona `{id}` działa (wave 1), a ma zapowiedzi „wkrótce”"
            );
            continue;
        }
        soon += 1;
        assert!(
            !upcoming.is_empty(),
            "strona „wkrótce” `{id}` nie mówi, co na niej będzie"
        );
        let row = inventory
            .lines()
            .find(|line| line.starts_with('|') && line.contains(&format!("(`{id}`)")))
            .unwrap_or_else(|| panic!("strony `{id}` nie ma w docs/plan/14-funkcje-wkrotce.md"));
        for item in upcoming {
            let text = item["pl"].as_str().unwrap().to_lowercase();
            assert!(
                row.to_lowercase().contains(&text.replace('"', "”"))
                    || row.to_lowercase().contains(&text),
                "zapowiedź „{text}” strony `{id}` nie jest wpisana w spisie"
            );
        }
        let task = row
            .split('|')
            .map(str::trim)
            .rfind(|cell| !cell.is_empty())
            .unwrap();
        assert!(
            roadmap
                .lines()
                .any(|line| line.starts_with(&format!("| {task} "))),
            "zadania {task} (strona `{id}`) nie ma w roadmapie"
        );
    }
    assert!(
        soon > 0 || !inventory.contains("(`router`)"),
        "spis jest nieaktualny"
    );
}
