//! Przegląd bezpieczeństwa #3 (2026-10), propozycja P3-04: menedżer modeli pobiera **pliki
//! wykonywalne** sidecarów (`llama-server`, `whisper-server`, `piper` — archiwa z wydań GitHub,
//! uruchamiane potem przez Alfę) bez przypiętego SHA-256, wyłącznie ze zgodą TOFU na policzony
//! skrót (właściciel nie ma czym go sprawdzić). THREAT_MODEL S23: kod z łańcucha dostaw tylko
//! z przypiętymi skrótami/podpisem. Test pokazuje stan obecny; zostaje `#[ignore]` do decyzji
//! człowieka (przypięcie skrótów potwierdzonych wydań albo blokada pobierania nieprzypiętych
//! sidecarów).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_api::dto::ModelItemKind;

#[test]
#[ignore = "P3-04: sidecary wykonywalne bez przypiętego SHA-256 (decyzja człowieka)"]
fn executable_sidecars_are_pinned() {
    let unpinned: Vec<String> = app_models::builtin()
        .into_iter()
        .filter(|s| s.kind == ModelItemKind::Sidecar && s.downloadable() && !s.pinned())
        .map(|s| s.id)
        .collect();
    assert!(
        unpinned.is_empty(),
        "sidecary pobierane bez przypiętego SHA-256 (tylko TOFU): {unpinned:?}"
    );
}
