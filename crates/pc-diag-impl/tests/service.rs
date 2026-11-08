//! Usługa diagnostyki na atrapie sondy: zapis w historii, plan testów, przebieg z postępem,
//! porównanie z poprzednim wynikiem, dysk na katalogu tymczasowym, zajętość, przerwanie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use pc_diag_contract::{CheckKind, DiagError, Verdict};
use pc_diag_fake::{FakeProbe, sample};
use pc_diag_impl::{PcDiagDeps, PcDiagService, Progress};

fn service(dir: &std::path::Path, probe: Arc<FakeProbe>) -> PcDiagService {
    let mut deps = PcDiagDeps::new(probe, dir.join("history"), dir.join("work"));
    deps.disk_sample_mb = 16;
    deps.cpu_window = Duration::from_millis(60);
    PcDiagService::new(deps)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn record_plan_run_compare_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeProbe::default());
    let svc = service(dir.path(), probe.clone());
    assert!(svc.latest().is_none());
    assert_eq!(svc.available().len(), 5, "bez zapisu — bez dysków");
    let s = svc.record().await.unwrap();
    assert_eq!(s.cpu.cores, 8);
    let plan = svc.available();
    assert!(plan.contains(&CheckKind::Disk("C:".into())));
    assert!(plan.contains(&CheckKind::Disk("E:".into())));

    let seen = Arc::new(Mutex::new(Vec::<Progress>::new()));
    let sink = seen.clone();
    let run = svc
        .run(
            vec![
                CheckKind::Integrity,
                CheckKind::CpuSingle,
                CheckKind::CpuMulti,
                CheckKind::Disk("C:".into()),
                CheckKind::Disk("Q:".into()),
            ],
            Some(Arc::new(move |p| sink.lock().unwrap().push(p))),
        )
        .await
        .unwrap();
    assert_eq!(run.results.len(), 5);
    {
        let progress = seen.lock().unwrap();
        assert_eq!(progress.len(), 10, "początek i koniec każdego testu");
        assert!(progress[9].result.is_some() && progress[0].result.is_none());
    }
    assert_eq!(
        run.results[0].verdict,
        Verdict::Warn,
        "dysk USB z ostrzeżeniem i błędami"
    );
    assert!(run.results[2].metric("scaling").is_some());
    let disk_c = &run.results[3];
    assert_ne!(disk_c.verdict, Verdict::Skipped, "{:?}", disk_c.notes);
    assert_eq!(disk_c.metric("verify_errors"), Some(0.0));
    assert!(!dir.path().join("work").exists(), "próba dysku usunięta");
    assert_eq!(run.results[4].verdict, Verdict::Skipped, "nieznany wolumin");

    let again = service(dir.path(), probe);
    assert!(again.latest().is_some(), "ostatni zapis z historii");
    assert_eq!(again.history().runs().len(), 1);
    assert!(again.history().last_result("cpu_single").is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn low_space_probe_error_busy_and_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeProbe::default());
    let mut s = sample();
    s.volumes[0].free_mb = 100;
    probe.set(Ok(s));
    let svc = Arc::new(service(dir.path(), probe.clone()));
    svc.record().await.unwrap();
    let run = svc
        .run(vec![CheckKind::Disk("C:".into())], None)
        .await
        .unwrap();
    assert_eq!(run.results[0].verdict, Verdict::Skipped);
    assert!(run.results[0].notes[0].contains("Za mało wolnego miejsca"));

    probe.set(Err(DiagError::Probe("brak WMI".into())));
    assert!(svc.record().await.is_err());
    assert!(svc.run(vec![CheckKind::Integrity], None).await.is_err());
    assert!(!svc.busy(), "błąd zwalnia blokadę");

    probe.set(Ok(sample()));
    let slow = svc.clone();
    let task = tokio::spawn(async move {
        slow.run(
            vec![CheckKind::CpuSingle, CheckKind::CpuMulti, CheckKind::Memory],
            None,
        )
        .await
    });
    for _ in 0..500 {
        if svc.busy() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(matches!(
        svc.run(vec![CheckKind::CpuSingle], None).await,
        Err(DiagError::Busy)
    ));
    svc.cancel();
    let run = task.await.unwrap().unwrap();
    assert!(
        run.results.iter().any(|r| r.verdict == Verdict::Skipped),
        "przerwane pominięte"
    );
}
