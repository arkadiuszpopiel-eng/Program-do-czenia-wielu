//! Kill-switch i rejestr Job Objects (PLAN §8.6): typy wspólne dla watchdoga i Brokera.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use platform_contract::{ProcessHandle, ProcessPort};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Zdarzenie: cisza audio natychmiast (voice-audio/voice-tts przerywają odtwarzanie).
pub const EVENT_AUDIO_SILENCE: &str = "kernel.audio.silence";
/// Zdarzenie (Audyt): kill-switch wykonany (`latency_us`, liczba zabitych drzew).
pub const EVENT_KILL_SWITCH: &str = "watchdog.kill_switch";

/// Kto wywołał kill-switch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", content = "detail", rename_all = "snake_case")]
pub enum KillReason {
    /// Skrót globalny (`Ctrl+Shift+F12`).
    Hotkey,
    /// Przycisk w zasobniku.
    TrayButton,
    /// Przycisk w kapsule aktywności.
    CapsuleButton,
    /// „Stop” głosem (przez `voice-cmd` → Broker).
    VoiceStop,
    /// Decyzja Brokera (np. naruszenie polityki).
    Broker(String),
    /// Decyzja watchdoga (np. pętla awarii).
    Watchdog(String),
}

impl fmt::Display for KillReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hotkey => f.write_str("skrót globalny"),
            Self::TrayButton => f.write_str("przycisk w zasobniku"),
            Self::CapsuleButton => f.write_str("przycisk w kapsule"),
            Self::VoiceStop => f.write_str("„stop” głosem"),
            Self::Broker(why) => write!(f, "Broker: {why}"),
            Self::Watchdog(why) => write!(f, "watchdog: {why}"),
        }
    }
}

/// Rola procesu nadzorowanego (właściciel drzewa procesów).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "role", content = "id", rename_all = "snake_case")]
pub enum ProcessRole {
    /// Jądro Alfy (proces główny).
    Core,
    /// Sidecar modułu (`isolation = "process"`), np. `voice-stt`.
    Sidecar(String),
    /// Okno Broker-UI.
    BrokerUi,
    /// Usługa Brokera.
    Broker,
    /// Most CLI („opaque worker”).
    CliBridge(String),
    /// Proces narzędzia (shell, przeglądarka).
    Tool(String),
}

impl fmt::Display for ProcessRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => f.write_str("core"),
            Self::Sidecar(id) => write!(f, "sidecar:{id}"),
            Self::BrokerUi => f.write_str("broker-ui"),
            Self::Broker => f.write_str("broker"),
            Self::CliBridge(id) => write!(f, "cli:{id}"),
            Self::Tool(id) => write!(f, "tool:{id}"),
        }
    }
}

/// Wpis rejestru Job Objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRecord {
    /// Uchwyt drzewa procesów (Job Object).
    pub job: ProcessHandle,
    /// Właściciel.
    pub owner: ProcessRole,
    /// Opis (do raportu).
    pub label: String,
}

/// Nieudane zabicie drzewa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobFailure {
    /// Numer uchwytu.
    pub job: u32,
    /// Błąd.
    pub error: String,
}

/// Raport kill-switcha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KillReport {
    /// Powód.
    pub reason: KillReason,
    /// Liczba unieważnionych tokenów zdolności (0 dla watchdoga bez Brokera).
    pub tokens_revoked: u64,
    /// Liczba zabitych drzew procesów.
    pub jobs_killed: u32,
    /// Drzewa, których nie udało się zabić (nigdy cicho).
    pub jobs_failed: Vec<JobFailure>,
    /// Czy wysłano zdarzenie ciszy audio.
    pub audio_silenced: bool,
    /// Czy zapisano rekord Audytu (best effort — nie blokuje kill-switcha).
    pub audited: bool,
    /// Czas od wywołania do zakończenia (µs).
    pub latency_us: u64,
}

/// Kill-switch: nigdy nie wymaga zatwierdzenia i nie może być zablokowany brakiem Brokera.
#[async_trait]
pub trait KillSwitch: Send + Sync {
    /// Zabija wszystko: tokeny (Broker), drzewa procesów, audio; zwraca raport.
    async fn kill_all(&self, reason: KillReason) -> KillReport;
}

/// Rejestr drzew procesów do zabicia przy kill-switchu.
pub trait JobRegistry: Send + Sync {
    /// Rejestruje drzewo procesów.
    fn register_job(&self, job: ProcessHandle, owner: ProcessRole, label: &str);
    /// Wyrejestrowuje (proces zakończył się normalnie); `false`, gdy nieznany.
    fn unregister_job(&self, job: ProcessHandle) -> bool;
    /// Zarejestrowane drzewa (rosnąco po uchwycie).
    fn jobs(&self) -> Vec<JobRecord>;
}

/// Prosta tabela Job Objects (wspólna dla `-impl` watchdoga, Brokera i atrap).
#[derive(Debug, Default)]
pub struct JobTable {
    jobs: Mutex<BTreeMap<u32, JobRecord>>,
}

impl JobTable {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<u32, JobRecord>> {
        self.jobs.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zabija wszystkie zarejestrowane drzewa przez `ProcessPort::kill_tree` i czyści tabelę.
    /// Zwraca (liczba zabitych, błędy). Błąd jednego drzewa nie zatrzymuje pozostałych.
    pub fn kill_all(&self, port: &dyn ProcessPort) -> (u32, Vec<JobFailure>) {
        let jobs: Vec<JobRecord> = std::mem::take(&mut *self.lock()).into_values().collect();
        let mut killed = 0u32;
        let mut failed = Vec::new();
        for record in jobs {
            match port.kill_tree(record.job) {
                Ok(()) => killed = killed.saturating_add(1),
                Err(e) => failed.push(JobFailure {
                    job: record.job.0,
                    error: e.to_string(),
                }),
            }
        }
        (killed, failed)
    }
}

impl JobRegistry for JobTable {
    fn register_job(&self, job: ProcessHandle, owner: ProcessRole, label: &str) {
        self.lock().insert(
            job.0,
            JobRecord {
                job,
                owner,
                label: label.to_owned(),
            },
        );
    }

    fn unregister_job(&self, job: ProcessHandle) -> bool {
        self.lock().remove(&job.0).is_some()
    }

    fn jobs(&self) -> Vec<JobRecord> {
        self.lock().values().cloned().collect()
    }
}
