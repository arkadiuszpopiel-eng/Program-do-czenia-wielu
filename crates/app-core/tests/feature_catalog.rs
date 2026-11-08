//! Spis narzędzi agentek w katalogu funkcji (`docs/features/narzedzia.txt`, docs/plan/45): nazwy
//! z rdzenia złożonego jak w aplikacji. Nowe albo usunięte narzędzie zmienia spis — test jest
//! czerwony, dopóki spis nie zostanie odświeżony (`ALFA_UPDATE_FEATURES=1`), a wtedy bramka
//! katalogu (`scripts/features.mjs`) wymaga testu dla nowej pozycji. Narzędzia serwerów MCP
//! i wtyczek użytkownika nie należą do spisu (pochodzą z konfiguracji, nie z kodu Alfy).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use common::agents::agents;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn agent_tool_catalog_matches_the_feature_list() {
    let a = agents().await;
    let names = a.h.core.agent_tool_names();
    assert!(names.len() >= 20, "za mało narzędzi: {names:?}");
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/features/narzedzia.txt");
    let current = format!("{}\n", names.join("\n"));
    if std::env::var("ALFA_UPDATE_FEATURES").is_ok_and(|v| v == "1") {
        std::fs::write(&file, &current).unwrap();
        return;
    }
    let saved = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert_eq!(
        saved, current,
        "spis narzędzi agentek się zmienił — odśwież go: ALFA_UPDATE_FEATURES=1 cargo test -p app-core \
         --test feature_catalog, potem dopisz testy nowych narzędzi (node scripts/features.mjs)"
    );
}
