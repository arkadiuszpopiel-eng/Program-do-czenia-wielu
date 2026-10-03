//! Pełny cykl aktualizacji na lokalnym serwerze HTTP i parach kluczy minisign z testu:
//! dobra aktualizacja (sprawdź → pobierz → zweryfikuj → rozpakuj → przełącz → launcher wybiera
//! nową), przerwane pobieranie → wznowienie (HTTP Range), anulowanie → wznowienie, kanał
//! testowy, sprzątanie (zostają 2 wersje), „Co nowego” raz po aktualizacji, rollback ręczny
//! i przez watchdoga.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::flow::{Flow, noise, package, v};
use updater_contract::{
    Channel, InstallIntent, UpdateMode, UpdatePhase, UpdateStatus, Updater, UpdaterError,
};
use updater_impl::{ServiceOptions, UpdateService, WatchdogSignal};
use watchdog_contract::UpdaterSignal;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn good_update_full_cycle() {
    let f = Flow::new("1.0.0").await;
    let seen: Arc<Mutex<Vec<UpdatePhase>>> = Arc::default();
    let log = seen.clone();
    f.service.set_listener(Arc::new(move |s: &UpdateStatus| {
        let mut l = log.lock().unwrap();
        if l.last() != Some(&s.phase) {
            l.push(s.phase);
        }
    }));
    let launcher_bytes = b"MZ nowy launcher".to_vec();
    let bytes = package("1.1.0", &[("alfa.exe", &launcher_bytes)]);
    let release = f.signed("1.1.0", "1.1.0", &bytes);
    f.publish("stable", &[(release, bytes)]);

    let s = f.service.check().await.unwrap();
    assert_eq!(s.phase, UpdatePhase::Available);
    let available = s.available.unwrap();
    assert_eq!(
        (available.version, available.notes),
        (v("1.1.0"), "Co nowego w 1.1.0".to_owned())
    );
    assert!(s.last_check.is_some());

    let s = f.service.download(InstallIntent::Update).await.unwrap();
    assert_eq!(
        (s.phase, s.ready.clone()),
        (UpdatePhase::Ready, Some(v("1.1.0")))
    );
    assert_eq!(s.previous, Some(v("1.0.0")));
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            UpdatePhase::Checking,
            UpdatePhase::Available,
            UpdatePhase::Downloading,
            UpdatePhase::Verifying,
            UpdatePhase::Installing,
            UpdatePhase::Ready,
        ]
    );
    let layout = f.updater.layout().clone();
    let dir = layout.version_dir(&v("1.1.0"));
    assert_eq!(
        std::fs::read_to_string(dir.join("notes.md")).unwrap(),
        "- Nowości w 1.1.0"
    );
    let state = f.updater.state().unwrap().unwrap();
    assert_eq!(
        (state.active, state.previous, state.pending),
        (v("1.1.0"), Some(v("1.0.0")), true)
    );
    assert!(f.staging_files().is_empty(), "plik częściowy usunięty");
    // Launcher przy następnym starcie wybiera nową wersję.
    assert_eq!(f.updater.select_launch().unwrap().version, v("1.1.0"));
    // Ponowne sprawdzenie nie proponuje tej samej wersji.
    let s = f.service.check().await.unwrap();
    assert_eq!(s.phase, UpdatePhase::Ready);

    // Nowa wersja wystartowała zdrowo: mark_good przygotowuje nowy launcher do zamiany.
    f.updater.mark_good(&v("1.1.0")).unwrap();
    let staged = updater_impl::selfupdate::new_path(&layout);
    assert_eq!(std::fs::read(staged).unwrap(), launcher_bytes);

    // „Co nowego” raz po aktualizacji (uruchomiona 1.1.0; wcześniej widziana 1.0.0).
    let old = UpdateService::new(
        f.updater.clone(),
        None,
        v("1.0.0"),
        Channel::Stable,
        UpdateMode::Ask,
        ServiceOptions::default(),
    );
    assert_eq!(old.whats_new(), None, "pierwszy start — zapamiętana wersja");
    let new = UpdateService::new(
        f.updater.clone(),
        None,
        v("1.1.0"),
        Channel::Stable,
        UpdateMode::Ask,
        ServiceOptions::default(),
    );
    assert_eq!(
        new.whats_new(),
        Some((v("1.1.0"), "- Nowości w 1.1.0".to_owned()))
    );
    new.mark_whats_new_seen().unwrap();
    assert_eq!(new.whats_new(), None, "tylko raz");
    assert_eq!(new.status().phase, UpdatePhase::Disabled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupted_download_resumes_with_range() {
    let f = Flow::new("1.0.0").await;
    let big = noise(300_000, 1);
    let bytes = package("1.1.0", &[("zasoby/duzy.bin", &big)]);
    let release = f.signed("1.1.0", "1.1.0", &bytes);
    f.publish("stable", &[(release.clone(), bytes.clone())]);
    f.service.check().await.unwrap();
    f.server.with(|s| s.cut_next_after = Some(1000));
    let s = f.service.download(InstallIntent::Update).await.unwrap();
    assert_eq!(s.phase, UpdatePhase::Ready);
    let zip_requests: Vec<_> = f
        .server
        .requests()
        .into_iter()
        .filter(|(p, _)| p.ends_with(".zip"))
        .collect();
    assert_eq!(zip_requests.len(), 2, "{zip_requests:?}");
    assert_eq!(zip_requests[0].1, None);
    assert_eq!(zip_requests[1].1.as_deref(), Some("bytes=1000-"));
    assert_eq!(f.active(), v("1.1.0"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_download_keeps_part_and_resumes_later() {
    let f = Flow::with_options(
        "1.0.0",
        ServiceOptions {
            max_resumes: 0,
            retry_delay: Duration::ZERO,
        },
    )
    .await;
    let bytes = package("1.1.0", &[("zasoby/a.bin", &noise(50_000, 2))]);
    let release = f.signed("1.1.0", "1.1.0", &bytes);
    f.publish("stable", &[(release, bytes.clone())]);
    f.service.check().await.unwrap();
    f.server.with(|s| s.cut_next_after = Some(4096));
    let err = f.service.download(InstallIntent::Update).await.unwrap_err();
    assert!(matches!(err, UpdaterError::Network { .. }), "{err:?}");
    let s = f.service.status();
    assert_eq!(s.phase, UpdatePhase::Failed);
    assert_eq!(s.progress.map(|p| p.downloaded), Some(4096));
    assert_eq!(f.staging_files().len(), 1, "częściowy plik zostaje");
    assert_eq!(f.active(), v("1.0.0"));
    // Serwer bez obsługi zakresów: pobieranie od początku też kończy się poprawnie.
    f.server.with(|s| s.no_range = true);
    let s = f.service.download(InstallIntent::Update).await.unwrap();
    assert_eq!(s.phase, UpdatePhase::Ready);
    let last = f.server.requests().pop().unwrap();
    assert_eq!(last.1.as_deref(), Some("bytes=4096-"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_keeps_part_for_resume() {
    let f = Flow::new("1.0.0").await;
    let bytes = package("1.1.0", &[("zasoby/a.bin", &noise(100_000, 3))]);
    let release = f.signed("1.1.0", "1.1.0", &bytes);
    f.publish("stable", &[(release, bytes)]);
    f.service.check().await.unwrap();
    f.server.with(|s| s.stall_next_after = Some(2048));
    let service = f.service.clone();
    let task = tokio::spawn(async move { service.download(InstallIntent::Update).await });
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let part_size = || {
        let dir = f.updater.layout().root.join("staging");
        std::fs::read_dir(dir)
            .ok()
            .and_then(|mut d| d.next())
            .and_then(|e| e.ok())
            .and_then(|e| e.metadata().ok())
            .map_or(0, |m| m.len())
    };
    while part_size() < 2048 {
        assert!(std::time::Instant::now() < deadline, "brak postępu");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    f.service.cancel();
    let err = task.await.unwrap().unwrap_err();
    assert_eq!(err, UpdaterError::Cancelled);
    let s = f.service.status();
    assert_eq!(s.phase, UpdatePhase::Available);
    assert_eq!(s.progress.map(|p| p.downloaded), Some(2048));
    assert_eq!(f.staging_files().len(), 1);
    let s = f.service.download(InstallIntent::Update).await.unwrap();
    assert_eq!(s.phase, UpdatePhase::Ready);
    let last = f.server.requests().pop().unwrap();
    assert_eq!(last.1.as_deref(), Some("bytes=2048-"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn beta_channel_and_prune_keeps_two_versions() {
    let f = Flow::new("1.0.0").await;
    let stable = package("1.1.0", &[]);
    let beta = package("1.2.0-beta.1", &[]);
    let r_stable = f.signed("1.1.0", "1.1.0", &stable);
    let r_beta = f.signed("1.2.0-beta.1", "1.2.0-beta.1", &beta);
    // Wersja przedpremierowa w manifeście stabilnym jest ignorowana.
    f.publish(
        "stable",
        &[
            (r_stable.clone(), stable.clone()),
            (r_beta.clone(), beta.clone()),
        ],
    );
    let s = f.service.check().await.unwrap();
    assert_eq!(s.available.map(|a| a.version), Some(v("1.1.0")));
    f.service.download(InstallIntent::Update).await.unwrap();
    // Kanał testowy (ustawienie „preview”) — inny manifest.
    f.publish("beta", &[(r_beta, beta)]);
    f.service
        .set_preferences(Channel::from_setting("preview"), UpdateMode::Auto);
    let s = f.service.check().await.unwrap();
    assert_eq!(s.available.map(|a| a.version), Some(v("1.2.0-beta.1")));
    f.service.download(InstallIntent::Update).await.unwrap();
    // Uruchomiona 1.0.0 jest chroniona; zostaje aktywna i poprzednia.
    // Sprzątanie po instalacji: wersji nieuruchomionej 1.1.0 nie ma już po co trzymać.
    let installed = f.updater.installed().unwrap();
    assert_eq!(installed, vec![v("1.0.0"), v("1.2.0-beta.1")]);
    let state = f.updater.state().unwrap().unwrap();
    assert_eq!(
        (state.active, state.previous),
        (v("1.2.0-beta.1"), Some(v("1.0.0"))),
        "poprzednią jest uruchomiona (sprawdzona) wersja, nie nieuruchomiona 1.1.0"
    );
    assert!(
        f.updater
            .prune_except(2, &[v("1.2.0-beta.1")])
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_rollback_and_watchdog_rollback() {
    let f = Flow::new("1.0.0").await;
    f.publish_good("1.1.0");
    f.service.check().await.unwrap();
    f.service.download(InstallIntent::Update).await.unwrap();
    // „Przywróć poprzednią wersję” przed restartem = anulowanie przygotowanej aktualizacji.
    let s = f.service.rollback().await.unwrap();
    assert_eq!((s.phase, s.ready), (UpdatePhase::Idle, None));
    let state = f.updater.state().unwrap().unwrap();
    assert_eq!((state.active, state.bad), (v("1.0.0"), vec![v("1.1.0")]));
    let s = f.service.check().await.unwrap();
    assert_eq!(s.phase, UpdatePhase::UpToDate, "wycofana nie wraca sama");

    // Watchdog: po pętli awarii nowej wersji zleca powrót do ostatniej dobrej.
    f.updater.switch_to(&v("1.1.0")).unwrap();
    let signal = WatchdogSignal::new(f.updater.clone(), v("1.1.0"));
    assert_eq!(signal.current_version(), "1.1.0");
    assert!(
        signal.request_rollback("0.5.0").is_err(),
        "tylko o jeden krok"
    );
    signal.request_rollback("1.0.0").unwrap();
    assert_eq!(signal.current_version(), "1.0.0");
    let state = f.updater.state().unwrap().unwrap();
    assert!(state.bad.contains(&v("1.1.0")));
    assert_eq!(f.updater.select_launch().unwrap().version, v("1.0.0"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_without_feed_or_key() {
    let f = Flow::new("1.0.0").await;
    let service = UpdateService::from_config(
        f.updater.clone(),
        v("1.0.0"),
        Channel::Stable,
        UpdateMode::Auto,
    );
    assert_eq!(service.status().phase, UpdatePhase::Disabled);
    assert!(service.status().error.unwrap().contains("adresu wydań"));
    assert!(matches!(
        service.check().await,
        Err(UpdaterError::NotConfigured { .. })
    ));
    assert!(matches!(
        updater_impl::HttpFeed::new("http://repo.example/alfa", false),
        Err(UpdaterError::NotConfigured { .. })
    ));
    assert!(updater_impl::HttpFeed::new("http://127.0.0.1:1/alfa", false).is_err());
}
