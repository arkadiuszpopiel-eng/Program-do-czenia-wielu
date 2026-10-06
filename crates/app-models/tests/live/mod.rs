//! Narzędzia testu na żywo `live_catalog.rs`: raport JSON (dla człowieka — przypięcie SHA-256 i
//! potwierdzenie układu archiwów), instalacja pozycji przez `ModelsApp` ze zgodą TOFU, opis drzewa
//! sidecara, sprawdzenie flag CLI w `--help`, miara podobieństwa tekstu (WER).

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod probe;
pub mod setup;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use app_api::dto::{ModelItem, ModelItemState, TrustedHashes};
use app_models::ModelsApp;
use app_models::catalog::ItemSpec;
use app_models::store::Store;
use serde_json::{Value, json};

/// Raport (`alfa-live-catalog-v1`), zapisywany po każdym etapie — awaria dalszego etapu nie
/// gubi hashy pobranych plików.
pub struct Report {
    pub path: PathBuf,
    pub value: Value,
}

impl Report {
    pub fn new(path: PathBuf) -> Self {
        let value = json!({
            "format": "alfa-live-catalog-v1",
            "os": std::env::consts::OS,
            "unix_time": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            "items": [],
            "problems": [],
        });
        Self { path, value }
    }

    pub fn set(&mut self, key: &str, v: Value) {
        self.value[key] = v;
        self.save();
    }

    pub fn item(&mut self, v: Value) {
        if let Some(items) = self.value["items"].as_array_mut() {
            items.push(v);
        }
        self.save();
    }

    /// Problem do rozwiązania (test kończy się porażką po zapisaniu całego raportu).
    pub fn problem(&mut self, what: impl Into<String>) {
        let what = what.into();
        eprintln!("PROBLEM: {what}");
        if let Some(p) = self.value["problems"].as_array_mut() {
            p.push(Value::String(what));
        }
        self.save();
    }

    pub fn problems(&self) -> Vec<String> {
        self.value["problems"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = serde_json::to_string_pretty(&self.value).unwrap();
        std::fs::write(&self.path, text).unwrap();
    }
}

pub fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

pub async fn view(app: &ModelsApp, id: &str) -> ModelItem {
    let list = app.list().await.unwrap();
    list.items.into_iter().find(|i| i.id == id).unwrap()
}

/// Czeka na stan końcowy (zainstalowana, zgoda TOFU, błąd); `None` po przekroczeniu czasu.
async fn settle(app: &ModelsApp, id: &str, limit: Duration) -> Option<ModelItem> {
    let t0 = Instant::now();
    while t0.elapsed() < limit {
        let item = view(app, id).await;
        match item.state {
            ModelItemState::Installed
            | ModelItemState::NeedsTrust
            | ModelItemState::Failed
            | ModelItemState::Corrupt => return Some(item),
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    None
}

/// Instaluje pobraną pozycję (zgoda TOFU z hashami policzonymi przy pobraniu — jak kliknięcie
/// „Ufam temu plikowi” w UI) i opisuje ją w raporcie: rozmiar i SHA-256 pobranych plików, surowy
/// układ archiwów (przed zgodą pliki leżą w katalogu roboczym), drzewo po instalacji.
pub async fn install(
    app: &Arc<ModelsApp>,
    store: &Store,
    spec: &ItemSpec,
    started: Instant,
    report: &mut Report,
) -> bool {
    let id = spec.id.as_str();
    let limit = Duration::from_secs(45 * 60);
    let mut item = settle(app, id, limit).await;
    let download_ms = ms(started.elapsed());
    let staged = staged_files(store, spec);
    let mut shown = TrustedHashes::new();
    let t_install = Instant::now();
    if let Some(pending) = item
        .as_ref()
        .filter(|i| i.state == ModelItemState::NeedsTrust)
    {
        shown = pending
            .files
            .iter()
            .filter_map(|f| f.sha256.clone().map(|h| (f.name.clone(), h)))
            .collect();
        if let Err(e) = app.trust_hash(id, shown.clone()).await {
            report.problem(format!("{id}: zgoda TOFU odrzucona: {e}"));
        }
        item = settle(app, id, limit).await;
    }
    let state = item.as_ref().map(|i| i.state);
    let error = item.as_ref().and_then(|i| i.error.clone());
    let receipt = store.receipt(id);
    let files: Vec<Value> = spec
        .files
        .iter()
        .map(|f| {
            let sha = receipt
                .as_ref()
                .and_then(|r| r.downloads.get(&f.name).cloned())
                .or_else(|| shown.get(&f.name).cloned());
            let mut v =
                json!({ "name": f.name, "url": f.url, "catalog_bytes": f.size, "sha256": sha });
            if let Some(s) = staged.get(&f.name) {
                v["bytes"] = s["bytes"].clone();
                v["archive"] = s["archive"].clone();
            }
            v
        })
        .collect();
    let target = spec.target(store.paths());
    let ok = state == Some(ModelItemState::Installed);
    let mut entry = json!({
        "id": id,
        "name": spec.name,
        "state": state.map_or_else(|| "timeout".to_owned(), |s| format!("{s:?}")),
        "error": error,
        "download_ms": download_ms,
        "install_ms": ms(t_install.elapsed()),
        "target": target.display().to_string(),
        "files": files,
    });
    if let Some(r) = &receipt {
        entry["tree"] = tree(&target, &r.files);
    }
    report.item(entry);
    if !ok {
        report.problem(format!("{id}: nie zainstalowano ({state:?}): {error:?}"));
    }
    ok
}

/// Drzewo zainstalowanej pozycji: liczba plików, wpisy najwyższego poziomu, rozmiary i SHA-256
/// plików w korzeniu (do przypięcia przez człowieka).
fn tree(target: &Path, files: &std::collections::BTreeMap<String, String>) -> Value {
    let mut top: Vec<String> = files
        .keys()
        .map(|k| {
            k.split_once('/')
                .map_or_else(|| k.clone(), |(d, _)| format!("{d}/"))
        })
        .collect();
    top.dedup();
    let key_files: serde_json::Map<String, Value> = files
        .iter()
        .filter(|(k, _)| !k.contains('/'))
        .map(|(k, sha)| {
            let bytes = std::fs::metadata(target.join(k))
                .map(|m| m.len())
                .unwrap_or(0);
            (k.clone(), json!({ "bytes": bytes, "sha256": sha }))
        })
        .collect();
    json!({ "files": files.len(), "top_level": top, "root_files": key_files })
}

/// Pobrane pliki w katalogu roboczym (przed zgodą TOFU): rozmiar i — dla ZIP — surowy układ
/// (liczba wpisów, katalogi najwyższego poziomu, pierwsze nazwy dokładnie jak w archiwum).
fn staged_files(store: &Store, spec: &ItemSpec) -> serde_json::Map<String, Value> {
    let staging = store.staging(&spec.id);
    let mut out = serde_json::Map::new();
    for f in &spec.files {
        let path = staging.join(&f.name);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        let archive = std::fs::File::open(&path)
            .ok()
            .and_then(|file| zip::ZipArchive::new(std::io::BufReader::new(file)).ok())
            .map(|mut zip| {
                let names: Vec<String> = (0..zip.len())
                    .filter_map(|i| zip.by_index_raw(i).ok().map(|e| e.name().to_owned()))
                    .collect();
                let mut top: Vec<String> = names
                    .iter()
                    .map(|n| n.split(['/', '\\']).next().unwrap_or_default().to_owned())
                    .collect();
                top.sort();
                top.dedup();
                top.truncate(40);
                let backslash = names.iter().any(|n| n.contains('\\'));
                let first: Vec<&String> = names.iter().take(60).collect();
                json!({ "entries": names.len(), "top_level": top, "backslash_separators": backslash, "first": first })
            });
        out.insert(
            f.name.clone(),
            json!({ "bytes": meta.len(), "archive": archive }),
        );
    }
    out
}

/// Słowa do porównania: małe litery, bez interpunkcji (polskie znaki zostają).
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Odległość edycyjna ciągów.
fn distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, y) in b.iter().enumerate() {
            let cost = usize::from(x != y);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// WER (word error rate) hipotezy względem wzorca.
pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let (r, h) = (words(reference), words(hypothesis));
    distance(&r, &h) as f64 / r.len().max(1) as f64
}

/// Podobieństwo znakowe (1 − odległość / dłuższy) tekstów po normalizacji.
pub fn similarity(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (
        words(a).join(" ").chars().collect(),
        words(b).join(" ").chars().collect(),
    );
    1.0 - distance(&a, &b) as f64 / a.len().max(b.len()).max(1) as f64
}

#[test]
fn text_measures() {
    assert_eq!(
        wer("Dzień dobry, jestem Alfa.", "dzień dobry jestem alfa"),
        0.0
    );
    assert!((wer("Dzień dobry, jestem Alfa.", "Dzień dobry, jestem Alpha.") - 0.25).abs() < 1e-9);
    assert!(similarity("Dzień dobry, jestem Alfa.", "Dzień dobry, jestem Alpha.") > 0.85);
    assert!(similarity("abc", "xyz") < 0.01);
}
