//! Prawdziwe testy na tym komputerze (ręcznie: `cargo test -p app-diag --test live -- --ignored --nocapture`).
//! Dysk tylko C: (dysk systemowy, dane lokalne Alfy); inne dyski — z Ustawień.

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "obciąża procesor, pamięć i dysk C: (ok. minuty)"]
async fn live_quick_and_system_disk() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_diag::DiagApp::new(app_diag::DiagAppDeps {
        probe: app_diag::system_probe(),
        local: dir.path().join("local"),
        reports: dir.path().join("Diagnostyka"),
    });
    app.snapshot().await.unwrap();
    let keys = [
        "integrity",
        "cpu_single",
        "cpu_multi",
        "memory",
        "network",
        "disk:C:",
    ];
    let run = app
        .run(keys.iter().map(|k| (*k).to_owned()).collect())
        .await
        .unwrap();
    for r in &run.results {
        eprintln!("{} {:?} {} ms", r.kind.key(), r.verdict, r.duration_ms);
        for m in &r.metrics {
            eprintln!("   {} = {:.2} {}", m.label, m.value, m.unit);
        }
        for n in &r.notes {
            eprintln!("   - {n}");
        }
    }
    eprintln!("RAPORT: {}", app.export().await.unwrap());
}
