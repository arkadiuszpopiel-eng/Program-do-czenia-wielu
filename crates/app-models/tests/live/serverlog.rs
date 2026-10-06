//! Przechwycenie logu `llama-server` (stderr → `tracing` z celem `llama_server` w `providers-local`).
//! Do raportu trafiają wiersze z architekturą modelu, KV cache i odciążeniem warstw — potwierdzenie
//! wartości `layers` i `kv_mb_per_1k_ctx` z `models.toml` oznaczonych „do potwierdzenia”.

use std::sync::{Mutex, OnceLock};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

/// Fragmenty wierszy llama.cpp (`print_info: n_layer = 60`, `llama_kv_cache: … size = … MiB`,
/// `load_tensors: offloaded 0/61 layers to GPU`).
const KEYS: [&str; 9] = [
    "n_layer",
    "n_head_kv",
    "n_embd ",
    "n_ctx_train",
    "n_vocab",
    "file type",
    "model params",
    "kv_cache",
    "offloaded",
];

static LINES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

struct ServerLog;

impl Subscriber for ServerLog {
    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        meta.target() == "llama_server"
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut line = Line(String::new());
        event.record(&mut line);
        if KEYS.iter().any(|k| line.0.contains(k))
            && let Some(lines) = LINES.get()
        {
            lines.lock().unwrap().push(line.0);
        }
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

struct Line(String);

impl Visit for Line {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

/// Włącza przechwytywanie (raz na proces).
pub fn install() {
    if LINES.set(Mutex::new(Vec::new())).is_ok() {
        let _ = tracing::subscriber::set_global_default(ServerLog);
    }
}

/// Wiersze zebrane od poprzedniego wywołania.
pub fn drain() -> Vec<String> {
    LINES
        .get()
        .map(|l| std::mem::take(&mut *l.lock().unwrap()))
        .unwrap_or_default()
}

#[test]
fn keeps_only_architecture_lines_of_llama_server() {
    install();
    tracing::debug!(target: "llama_server", "print_info: n_layer          = 60");
    tracing::debug!(target: "llama_server", "srv  log_server_r: request: GET /health 127.0.0.1 200");
    tracing::debug!(target: "inny", "print_info: n_head_kv = 2");
    assert_eq!(drain(), ["print_info: n_layer          = 60"]);
}
