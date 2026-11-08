//! Instalator z atrapą pobierania: pełna instalacja (TOFU), odmowa bez przypiętego hasha, wznawianie
//! z `.part` po przerwaniu, serwer bez `Range`, niezgodny hash, limit rozmiaru, anulowanie, ponowna
//! instalacja bez pobierania.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use lib_embed::catalog::{CatalogEntry, CatalogFile, find};
use lib_embed::manifest::sha256_hex;
use lib_embed::{EmbedError, EmbedManifest, Fetched, Fetcher, HashPolicy, install, installed};

/// Strumień, który po `cut` bajtach zwraca błąd (przerwane połączenie).
struct Cut {
    inner: Cursor<Vec<u8>>,
    left: usize,
}

impl Read for Cut {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.left == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "zerwane",
            ));
        }
        let n = buf.len().min(self.left);
        let read = self.inner.read(&mut buf[..n])?;
        self.left -= read;
        Ok(read)
    }
}

#[derive(Default)]
struct FakeFetcher {
    files: BTreeMap<String, Vec<u8>>,
    /// Kolejne odpowiedzi przerywane po tylu bajtach (FIFO; pusta = bez przerw).
    cuts: Mutex<Vec<usize>>,
    ranges: bool,
    opens: Mutex<Vec<(String, u64)>>,
}

impl Fetcher for FakeFetcher {
    fn open(&self, url: &str, offset: u64) -> Result<Fetched, EmbedError> {
        self.opens.lock().unwrap().push((url.to_owned(), offset));
        let data = self
            .files
            .get(url)
            .ok_or_else(|| EmbedError::Fetch(format!("404 {url}")))?;
        let start = if self.ranges { offset as usize } else { 0 };
        let body = data[start..].to_vec();
        let mut cuts = self.cuts.lock().unwrap();
        let left = if cuts.is_empty() {
            usize::MAX
        } else {
            cuts.remove(0)
        };
        Ok(Fetched {
            resumed: self.ranges && offset > 0,
            total: Some(data.len() as u64),
            body: Box::new(Cut {
                inner: Cursor::new(body),
                left,
            }),
        })
    }
}

const MODEL_URL: &str = "https://huggingface.co/test/resolve/main/onnx/model.onnx";
const TOK_URL: &str = "https://huggingface.co/test/resolve/main/tokenizer.json";

fn entry(model_pin: Option<&'static str>) -> CatalogEntry {
    let base = *find("multilingual-e5-small").unwrap();
    CatalogEntry {
        id: "test-model",
        model: CatalogFile {
            path: "onnx/model.onnx",
            url: MODEL_URL,
            sha256: model_pin,
            size_mb: 1,
        },
        tokenizer: CatalogFile {
            path: "tokenizer.json",
            url: TOK_URL,
            sha256: None,
            size_mb: 1,
        },
        ..base
    }
}

fn fetcher(ranges: bool) -> FakeFetcher {
    let model: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    FakeFetcher {
        files: BTreeMap::from([
            (MODEL_URL.to_owned(), model),
            (TOK_URL.to_owned(), b"{\"tokenizer\": true}".to_vec()),
        ]),
        ranges,
        ..FakeFetcher::default()
    }
}

fn run(
    entry: &CatalogEntry,
    dir: &std::path::Path,
    f: &FakeFetcher,
) -> Result<lib_embed::Installed, EmbedError> {
    install(
        entry,
        dir,
        f,
        HashPolicy::TrustOnFirstUse,
        &AtomicBool::new(false),
        &|_| {},
    )
}

#[test]
fn trust_on_first_use_writes_manifest_with_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(true);
    let e = entry(None);
    assert!(matches!(
        install(
            &e,
            dir.path(),
            &f,
            HashPolicy::PinnedOnly,
            &AtomicBool::new(false),
            &|_| {}
        ),
        Err(EmbedError::Manifest(_))
    ));
    assert!(
        f.opens.lock().unwrap().is_empty(),
        "bez zgody nic nie pobrano"
    );
    let progress = Mutex::new(Vec::new());
    let out = install(
        &e,
        dir.path(),
        &f,
        HashPolicy::TrustOnFirstUse,
        &AtomicBool::new(false),
        &|p| progress.lock().unwrap().push(p),
    )
    .unwrap();
    assert!(!out.pinned);
    let (m, _) = EmbedManifest::load(&out.manifest_path).unwrap();
    assert_eq!(m.model.sha256, sha256_hex(&f.files[MODEL_URL]));
    assert_eq!(m.tokenizer.sha256, sha256_hex(&f.files[TOK_URL]));
    assert_eq!(m.query_prefix, "query: ");
    assert_eq!(installed(dir.path()), Some(out.manifest_path.clone()));
    assert!(progress.lock().unwrap().iter().any(|p| p.done == 300_000));
    // Ponowna instalacja: pliki są — bez pobierania.
    let opens = f.opens.lock().unwrap().len();
    run(&e, dir.path(), &f).unwrap();
    assert_eq!(f.opens.lock().unwrap().len(), opens);
}

#[test]
fn interrupted_download_resumes_from_part_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(true);
    *f.cuts.lock().unwrap() = vec![100_000, 50_000];
    let pin: &'static str = Box::leak(sha256_hex(&f.files[MODEL_URL]).into_boxed_str());
    let out = run(&entry(Some(pin)), dir.path(), &f).unwrap();
    assert_eq!(out.manifest.model.sha256, pin);
    let opens = f.opens.lock().unwrap().clone();
    assert_eq!(opens[0], (MODEL_URL.to_owned(), 0));
    assert_eq!(opens[1], (MODEL_URL.to_owned(), 100_000));
    assert_eq!(opens[2], (MODEL_URL.to_owned(), 150_000));
    assert!(!dir.path().join("onnx/model.onnx.part").exists());
}

#[test]
fn server_without_range_restarts_from_zero() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(false);
    *f.cuts.lock().unwrap() = vec![70_000];
    let out = run(&entry(None), dir.path(), &f).unwrap();
    assert_eq!(out.manifest.model.sha256, sha256_hex(&f.files[MODEL_URL]));
    assert_eq!(
        std::fs::read(dir.path().join("onnx/model.onnx")).unwrap(),
        f.files[MODEL_URL]
    );
}

#[test]
fn hash_mismatch_removes_partial_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(true);
    let wrong: &'static str = Box::leak("0".repeat(64).into_boxed_str());
    let err = run(&entry(Some(wrong)), dir.path(), &f).unwrap_err();
    assert!(matches!(err, EmbedError::Hash { .. }), "{err}");
    assert!(!dir.path().join("onnx/model.onnx").exists());
    assert!(!dir.path().join("onnx/model.onnx.part").exists());
    assert_eq!(installed(dir.path()), None);
}

#[test]
fn persistent_failures_and_limits_stop_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(true);
    *f.cuts.lock().unwrap() = vec![10, 10, 10, 10, 10];
    assert!(matches!(
        run(&entry(None), dir.path(), &f),
        Err(EmbedError::Fetch(_))
    ));
    assert!(
        dir.path().join("onnx/model.onnx.part").exists(),
        "zostaje do wznowienia"
    );
    let mut big = fetcher(true);
    big.files
        .insert(MODEL_URL.to_owned(), vec![0; 3 * 1024 * 1024]);
    let dir = tempfile::tempdir().unwrap();
    let err = run(&entry(None), dir.path(), &big).unwrap_err();
    assert!(err.to_string().contains("limit"), "{err}");
    let mut missing = fetcher(true);
    missing.files.remove(TOK_URL);
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        run(&entry(None), dir.path(), &missing),
        Err(EmbedError::Fetch(_))
    ));
}

#[test]
fn cancel_keeps_part_for_later() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(true);
    let cancel = AtomicBool::new(true);
    let err = install(
        &entry(None),
        dir.path(),
        &f,
        HashPolicy::TrustOnFirstUse,
        &cancel,
        &|_| {},
    )
    .unwrap_err();
    assert_eq!(err, EmbedError::Cancelled);
    cancel.store(false, Ordering::SeqCst);
    run(&entry(None), dir.path(), &f).unwrap();
}

#[test]
fn existing_file_with_wrong_pin_is_redownloaded() {
    let dir = tempfile::tempdir().unwrap();
    let f = fetcher(true);
    std::fs::create_dir_all(dir.path().join("onnx")).unwrap();
    std::fs::write(dir.path().join("onnx/model.onnx"), b"stary").unwrap();
    let pin: &'static str = Box::leak(sha256_hex(&f.files[MODEL_URL]).into_boxed_str());
    run(&entry(Some(pin)), dir.path(), &f).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("onnx/model.onnx")).unwrap(),
        f.files[MODEL_URL]
    );
}
