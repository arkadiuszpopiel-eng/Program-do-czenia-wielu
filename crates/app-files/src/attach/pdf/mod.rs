//! Tekst z PDF dla modelu (C2.11). `lopdf` czyta strukturę pliku (xref, strumienie obiektów,
//! odszyfrowanie pustym hasłem użytkownika), a treść stron, czcionki i mapy `ToUnicode`
//! interpretujemy sami — strumieniowo i z limitami: rozmiar wejścia, liczba obiektów i stron,
//! rozpakowane bajty, operacje treści, głębokość formularzy, znaki wyniku i czas. Ekstrakcja idzie
//! w osobnym wątku z terminem — wrogi plik nie zawiesi wywołującego; wynik trzymamy w małej
//! pamięci podręcznej (po SHA-256), bo projekcja historii pyta o te same pliki co turę.
//! PDF z hasłem i skany (bez warstwy tekstowej) dają notkę po polsku zamiast tekstu; OCR skanów
//! to osobny, opcjonalny dodatek (roadmapa C2.11).

mod cmap;
mod encoding;
mod fonts;
mod lexer;
mod load;
mod sink;
mod state;
mod walk;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use super::extract::Extracted;
use load::LoadError;

const MIB: usize = 1024 * 1024;
/// Ile wyników trzymamy w pamięci podręcznej.
const CACHE_ENTRIES: usize = 16;
/// Najwięcej jednoczesnych wątków ekstrakcji (także tych, które przekroczyły termin).
const MAX_RUNNING: usize = 3;
/// Stos wątku ekstrakcji.
const STACK: usize = 16 * MIB;
/// Zapas na notkę o uciętym tekście (znaki).
const NOTE_RESERVE: usize = 200;

const ENCRYPTED: &str = "PDF jest zabezpieczony hasłem, więc Alfa nie odczyta jego tekstu. Otwórz go z hasłem, \
     zapisz kopię bez zabezpieczenia i załącz ją ponownie.";
const SCAN: &str = "PDF nie ma warstwy tekstowej — to najpewniej skan albo same obrazy. Rozpoznawanie tekstu \
     ze skanów (OCR) będzie osobnym, opcjonalnym dodatkiem.";
const NO_TEXT: &str =
    "PDF nie zawiera tekstu do odczytania (puste strony albo tekst zapisany jako krzywe).";
const BROKEN: &str = "PDF jest uszkodzony albo niepełny — nie udało się odczytać jego struktury.";
const TOO_COMPLEX: &str = "PDF jest zbyt złożony (limit obiektów albo rozpakowanych danych) — tekstu nie \
     wyciągnięto, żeby nie obciążać Alfy.";
const BUSY: &str = "Wyciąganie tekstu z PDF jest chwilowo niedostępne (trwają inne ekstrakcje) — spróbuj ponownie \
     za chwilę.";

/// Limity ekstrakcji.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    /// Największy plik (bajty).
    pub(super) max_bytes: usize,
    /// Najwięcej obiektów przy wczytywaniu.
    pub(super) max_objects: usize,
    /// Największy pojedynczy strumień obiektów / xref po rozpakowaniu.
    pub(super) load_stream: usize,
    /// Suma rozpakowanych strumieni obiektów.
    pub(super) load_total: usize,
    /// Najwięcej czytanych stron.
    pub(super) max_pages: usize,
    /// Największa treść jednej strony albo formularza po rozpakowaniu.
    pub(super) page_content: usize,
    /// Suma rozpakowanych treści (strony, formularze, czcionki).
    pub(super) content_total: usize,
    /// Najwięcej operatorów treści.
    pub(super) max_ops: usize,
    /// Najgłębsze zagnieżdżenie formularzy.
    pub(super) form_depth: usize,
    /// Najwięcej wywołań formularzy.
    pub(super) max_forms: usize,
    /// Najwięcej znaków wyniku.
    pub(super) max_chars: usize,
    /// Termin.
    pub(super) time: Duration,
}

impl Limits {
    /// Limity domyślne z limitem znaków `max_chars`.
    pub(super) fn with_chars(max_chars: usize) -> Self {
        Self {
            max_bytes: 25 * MIB,
            max_objects: 500_000,
            load_stream: 8 * MIB,
            load_total: 64 * MIB,
            max_pages: 1000,
            page_content: 8 * MIB,
            content_total: 128 * MIB,
            max_ops: 5_000_000,
            form_depth: 8,
            max_forms: 2000,
            max_chars,
            time: Duration::from_secs(10),
        }
    }
}

/// Powód przerwania ekstrakcji w trakcie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop {
    Chars,
    Pages,
    Ops,
    Content,
    Time,
}

impl Stop {
    fn reason(self, limits: &Limits) -> String {
        match self {
            Self::Chars => format!("limit {} znaków tekstu", limits.max_chars),
            Self::Pages => format!("limit {} stron", limits.max_pages),
            Self::Ops => "limit operacji na treści stron".into(),
            Self::Content => "limit rozpakowanych danych".into(),
            Self::Time => format!("limit czasu ({} s)", limits.time.as_secs()),
        }
    }
}

/// Czy nazwa pliku wskazuje PDF.
pub fn is_pdf(name: &str) -> bool {
    name.to_lowercase().ends_with(".pdf")
}

/// Tekst albo notka dla pliku `.pdf` (`None` dla innych nazw). Wynik z pamięci podręcznej, jeśli
/// ten sam plik (SHA-256) był już czytany z tym samym limitem znaków.
pub fn text(name: &str, bytes: &[u8], max_chars: usize) -> Option<Extracted> {
    if !is_pdf(name) {
        return None;
    }
    let limits = Limits::with_chars(max_chars);
    if bytes.len() > limits.max_bytes {
        return Some(Extracted::Note(format!(
            "PDF jest za duży do wyciągnięcia tekstu (limit {} MB).",
            limits.max_bytes / MIB
        )));
    }
    let key = (artifacts_contract::sha256_hex(bytes), max_chars);
    if let Some(hit) = cache_get(&key) {
        return Some(hit);
    }
    let (out, cacheable) = isolated(bytes.to_vec(), limits);
    if cacheable {
        cache_put(key, out.clone());
    }
    Some(out)
}

type Key = (String, usize);
static CACHE: Mutex<Vec<(Key, Extracted)>> = Mutex::new(Vec::new());
static RUNNING: AtomicUsize = AtomicUsize::new(0);

fn cache_get(key: &Key) -> Option<Extracted> {
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    let at = cache.iter().position(|(k, _)| k == key)?;
    let entry = cache.remove(at);
    let value = entry.1.clone();
    cache.push(entry);
    Some(value)
}

fn cache_put(key: Key, value: Extracted) {
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    cache.retain(|(k, _)| k != &key);
    if cache.len() >= CACHE_ENTRIES {
        cache.remove(0);
    }
    cache.push((key, value));
}

/// Zmniejsza licznik wątków także po panice w ekstrakcji.
struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Ekstrakcja w osobnym wątku z terminem; `bool` — czy wynik można zapamiętać.
fn isolated(bytes: Vec<u8>, limits: Limits) -> (Extracted, bool) {
    if RUNNING.fetch_add(1, Ordering::SeqCst) >= MAX_RUNNING {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
        return (Extracted::Note(BUSY.into()), false);
    }
    let guard = Running;
    let (tx, rx) = mpsc::sync_channel(1);
    let deadline = Instant::now() + limits.time;
    let spawned = std::thread::Builder::new()
        .name("alfa-pdf".into())
        .stack_size(STACK)
        .spawn(move || {
            let _guard = guard;
            let _ = tx.send(extract(&bytes, &limits, deadline));
        });
    if spawned.is_err() {
        return (Extracted::Note(BUSY.into()), false);
    }
    match rx.recv_timeout(limits.time + Duration::from_secs(1)) {
        Ok(out) => (out, true),
        Err(mpsc::RecvTimeoutError::Timeout) => (
            Extracted::Note(format!(
                "Wyciąganie tekstu z PDF przekroczyło limit czasu ({} s) i zostało przerwane.",
                limits.time.as_secs()
            )),
            true,
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => (Extracted::Note(BROKEN.into()), true),
    }
}

/// Czy w pierwszym KiB jest nagłówek `%PDF-`.
fn has_header(bytes: &[u8]) -> bool {
    bytes
        .get(..bytes.len().min(1024))
        .is_some_and(|head| head.windows(5).any(|w| w == b"%PDF-"))
}

/// Ekstrakcja w bieżącym wątku (z terminem `deadline` sprawdzanym w pętli).
pub(super) fn extract(bytes: &[u8], limits: &Limits, deadline: Instant) -> Extracted {
    if !has_header(bytes) {
        return Extracted::Note(BROKEN.into());
    }
    let doc = match load::load(bytes, limits) {
        Ok(doc) => doc,
        Err(LoadError::Encrypted) => return Extracted::Note(ENCRYPTED.into()),
        Err(LoadError::TooComplex) => return Extracted::Note(TOO_COMPLEX.into()),
        Err(LoadError::Broken) => return Extracted::Note(BROKEN.into()),
    };
    let pages = doc.get_pages();
    let total = pages.len();
    if total == 0 {
        return Extracted::Note(BROKEN.into());
    }
    let reserve = NOTE_RESERVE.min(limits.max_chars / 4);
    let mut walker = walk::Walker::new(&doc, limits, deadline, limits.max_chars - reserve);
    let mut read = 0usize;
    let mut saw_image = false;
    for (&number, &page) in &pages {
        if read >= limits.max_pages {
            walker.stop = Some(Stop::Pages);
            break;
        }
        if Instant::now() >= deadline {
            walker.stop = Some(Stop::Time);
            break;
        }
        saw_image |= walker.page(number, page);
        read += 1;
        if walker.sink.full && walker.stop.is_none() {
            walker.stop = Some(Stop::Chars);
        }
        if walker.stop.is_some() {
            break;
        }
    }
    let (stop, skipped) = (walker.stop, walker.skipped);
    let any_text = walker.sink.any_text;
    let mut text = walker.sink.finish();
    match (any_text, stop) {
        (false, Some(stop)) => Extracted::Note(format!(
            "Nie udało się wyciągnąć tekstu z PDF — {}.",
            stop.reason(limits)
        )),
        (false, None) if skipped > 0 => Extracted::Note(TOO_COMPLEX.into()),
        (false, None) => Extracted::Note(if saw_image { SCAN } else { NO_TEXT }.into()),
        (true, Some(stop)) => {
            text.push_str(&format!(
                "\n\n[… tekst niepełny — {}; przeczytane strony: {read} z {total}]",
                stop.reason(limits)
            ));
            Extracted::Text {
                text,
                truncated: true,
            }
        }
        (true, None) => Extracted::Text {
            text,
            truncated: skipped > 0,
        },
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_build;
