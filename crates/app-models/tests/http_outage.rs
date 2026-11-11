//! Chwilowy HTTP504: odstęp przed ponowieniem, niezmienny budżet i anulowanie.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use app_models::fetch::{FetchError, Fetcher, MAX_RESUMES, part_path};
use common::http::Server;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_504_cooldown_preserves_range_and_verified_bytes() {
    let server = Server::start().await;
    let data = b"public-fixture-for-range-and-hash";
    server.put("/file", data.to_vec());
    server.with(|s| s.outage = Some((504, Duration::from_millis(200))));
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file.bin");
    std::fs::write(part_path(&file), &data[..7]).unwrap();
    let fetcher = Fetcher::new(true).unwrap();
    let result = fetcher
        .fetch(
            &format!("{}/file", server.base),
            &file,
            data.len() as u64,
            &CancellationToken::new(),
            &|_, _| {},
        )
        .await;
    let (hash, size) = result.expect("chwilowa awaria powinna ustąpić w istniejącym budżecie prób");
    assert_eq!(size, data.len() as u64);
    assert_eq!(hash, common::sha(data));
    assert_eq!(std::fs::read(&file).unwrap(), data);
    assert!(!part_path(&file).exists());
    let requests = server.requests();
    assert!(requests.len() > 1 && requests.len() <= 4);
    assert!(
        requests
            .iter()
            .all(|(_, range)| range.as_deref() == Some("bytes=7-"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persistent_504_exhausts_exactly_four_attempts_and_keeps_partial() {
    assert_eq!(MAX_RESUMES, 3, "budżet nie został zwiększony");
    let server = Server::start().await;
    server.with(|s| s.outage = Some((504, Duration::from_secs(60))));
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file.bin");
    std::fs::write(part_path(&file), b"prefix").unwrap();
    let fetcher = Fetcher::new(true).unwrap();
    let cancel = CancellationToken::new();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        fetcher.fetch(
            &format!("{}/file", server.base),
            &file,
            100,
            &cancel,
            &|_, _| {},
        ),
    )
    .await
    .unwrap();
    assert_eq!(result, Err(FetchError::Network("HTTP 504".into())));
    assert_eq!(server.requests().len(), 4);
    assert_eq!(std::fs::read(part_path(&file)).unwrap(), b"prefix");
    assert!(!file.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_stops_http_429_backoff_without_another_attempt() {
    let server = Server::start().await;
    server.with(|s| s.outage = Some((429, Duration::from_secs(60))));
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file.bin");
    let cancel = CancellationToken::new();
    let request_cancel = cancel.clone();
    let url = format!("{}/file", server.base);
    let task = tokio::spawn(async move {
        Fetcher::new(true)
            .unwrap()
            .fetch(&url, &file, 100, &request_cancel, &|_, _| {})
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.requests().len() < 3 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    let result = tokio::time::timeout(Duration::from_millis(500), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result, Err(FetchError::Cancelled));
    assert_eq!(server.requests().len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_retryable_http_404_remains_one_request() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file.bin");
    let result = Fetcher::new(true)
        .unwrap()
        .fetch(
            &format!("{}/missing", server.base),
            &file,
            100,
            &CancellationToken::new(),
            &|_, _| {},
        )
        .await;
    assert!(matches!(result, Err(FetchError::Rejected(_))));
    assert_eq!(server.requests().len(), 1);
    assert!(!file.exists());
}
