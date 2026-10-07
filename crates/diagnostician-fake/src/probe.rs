//! Sonda zdrowia świata chaosowego: czy awaria wykrycia nie występuje w bieżącym stanie.

use diagnostician_contract::{Detection, FailureKind};
use serde_json::{Value, json};

use crate::state::{MIN_FREE_MB, WorldState};

fn cfg<'a>(st: &'a WorldState, key: &str) -> Option<&'a Value> {
    st.config.get(key)
}

fn detail_key(d: &Detection, name: &str, default: String) -> String {
    d.details.get(name).cloned().unwrap_or(default)
}

/// Sonda zdrowia: czy awaria wykrycia nie występuje w bieżącym stanie.
pub fn healthy(st: &WorldState, d: &Detection, now_ms: u64) -> bool {
    let t = d.target.as_str();
    let valid = |p: &str| st.files.get(p).is_some_and(|f| f.valid);
    match d.kind {
        FailureKind::ModuleStartFailure => cfg(st, "voice.tts.engine") != Some(&json!("nieznany")),
        FailureKind::ConfigCorrupted
        | FailureKind::SessionDbCorrupted
        | FailureKind::KernelConfigTampered => valid(t),
        FailureKind::SessionDbLocked => {
            cfg(st, "sessions.busy_timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                >= 15_000
        }
        FailureKind::DiskFull => st.free_mb(t) >= MIN_FREE_MB,
        FailureKind::SidecarCrashLoop => {
            !st.sidecar_faults.contains(t)
                || cfg(st, &format!("sidecars.{}.safe_mode", t.replace('-', "_")))
                    == Some(&json!(true))
        }
        FailureKind::GpuLost => {
            st.gpu_ok || cfg(st, &detail_key(d, "device_key", String::new())) == Some(&json!("cpu"))
        }
        FailureKind::ApiKeyRevoked => {
            !st.revoked_routes.contains(t)
                || cfg(st, &format!("router.routes.{t}.enabled")) == Some(&json!(false))
        }
        FailureKind::RateLimitLoop => {
            !st.throttled_routes.contains(t)
                || cfg(st, &format!("router.routes.{t}.paused_until_ms"))
                    .and_then(Value::as_u64)
                    .is_some_and(|u| u > now_ms)
        }
        FailureKind::BudgetExhausted => {
            !st.budget_exhausted || cfg(st, "router.prefer_local") == Some(&json!(true))
        }
        FailureKind::PortInUse => cfg(st, "providers.local.port")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .is_some_and(|p| !st.ports_taken.contains(&p)),
        FailureKind::ModelMissing | FailureKind::ModelCorrupted => {
            let key = detail_key(d, "model_key", String::new());
            let model = cfg(st, &key).and_then(Value::as_str).unwrap_or_default();
            let available =
                valid(&format!("models/{model}.gguf")) || valid(&format!("models/{model}.bin"));
            let corrupt_gone = d.kind == FailureKind::ModelMissing || !st.files.contains_key(t);
            available && corrupt_gone
        }
        FailureKind::WebViewBroken => st.files.get(t).is_none_or(|f| f.valid),
        FailureKind::SchemaMismatch => cfg(st, "memory.read_only") == Some(&json!(true)),
        FailureKind::UndoJournalFull | FailureKind::LogDiskLimit => {
            st.entries.get(t).copied().unwrap_or(0) < WorldState::capacity(t)
        }
        FailureKind::FileLocked => cfg(st, "io.defer_locked_writes") == Some(&json!(true)),
        FailureKind::DirPermissionDenied => cfg(st, &detail_key(d, "dir_key", String::new()))
            .and_then(Value::as_str)
            .is_some_and(|dir| !st.read_only_dirs.contains(dir)),
        FailureKind::ClockSkew => {
            let offset = cfg(st, "time.offset_ms")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            (st.clock_skew_ms + offset).abs() < 120_000
        }
        FailureKind::NetworkDown => {
            st.network_up || cfg(st, "router.offline") == Some(&json!(true))
        }
        FailureKind::UpdatePackageCorrupted => !st.files.contains_key(t),
        FailureKind::ResourceBudgetExceeded => {
            cfg(st, "modules.memory_consolidation.lifecycle") == Some(&json!("lazy"))
        }
    }
}
