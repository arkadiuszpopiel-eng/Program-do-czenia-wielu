//! Reguły planisty: dla każdego rodzaju awarii — kroki cofalne, ryzyko, uzasadnienie,
//! zadanie dla człowieka.

use serde_json::{Value, json};

use crate::catalog::FailureKind;
use crate::classify::Detection;
use crate::plan::{Risk, key_segment, plain_token, signal_key};
use crate::ports::RepairContext;
use crate::step::{RepairStep, is_kernel_module};

pub(crate) struct Builder<'a> {
    pub(crate) d: &'a Detection,
    pub(crate) ctx: &'a dyn RepairContext,
    pub(crate) steps: Vec<RepairStep>,
    pub(crate) human: Option<String>,
    pub(crate) risk: Risk,
    pub(crate) why: String,
}

impl Builder<'_> {
    fn detail(&self, key: &str) -> Option<String> {
        self.d.details.get(key).cloned()
    }

    /// Klucz z szczegółów sygnału — tylko w przestrzeni nazw modułu (SR2-03).
    fn key(&self, detail: &str, default: String) -> String {
        signal_key(&self.d.module, self.detail(detail), default)
    }

    fn set(&mut self, key: &str, new: Value) {
        let old = self.ctx.config(key);
        if old.as_ref() != Some(&new) {
            self.steps.push(RepairStep::SetConfig {
                key: key.to_owned(),
                old,
                new: Some(new),
            });
        }
    }

    fn restart(&mut self, module: &str) {
        self.steps.push(RepairStep::RestartModule {
            module: module.to_owned(),
        });
    }

    fn quarantine(&mut self, path: &str) -> String {
        let q = self.ctx.quarantine_path(path);
        self.steps.push(RepairStep::MoveFile {
            from: path.to_owned(),
            to: q.clone(),
        });
        q
    }

    fn restore_backup(&mut self, path: &str) -> bool {
        let Some(backup) = self.ctx.latest_backup(path) else {
            return false;
        };
        self.quarantine(path);
        self.steps.push(RepairStep::CopyFile {
            from: backup,
            to: path.to_owned(),
        });
        true
    }

    fn human(&mut self, text: impl Into<String>) {
        self.human = Some(text.into());
    }

    fn because(&mut self, risk: Risk, why: impl Into<String>) {
        self.risk = risk;
        self.why = why.into();
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn build(b: &mut Builder<'_>) {
    let (t, m) = (b.d.target.clone(), b.d.module.clone());
    let (ts, ms) = (key_segment(&t), key_segment(&m));
    match b.d.kind {
        FailureKind::ModuleStartFailure if is_kernel_module(&m) => {
            b.restart(&m);
            b.because(Risk::Medium, format!("Moduł Jądra {m} nie startuje — tylko ponowne uruchomienie przez Brokera/watchdoga (nigdy wyłączenie)."));
            b.human(format!(
                "Moduł Jądra {m} nie startuje — sprawdź logi; o safe-mode decyduje watchdog."
            ));
        }
        FailureKind::ModuleStartFailure => {
            match (b.ctx.current_revision(), b.ctx.last_good_revision()) {
                (Some(cur), Some(good)) if cur != good => {
                    b.steps.push(RepairStep::RollbackConfig {
                        from_revision: cur,
                        to_revision: good,
                    });
                    b.because(Risk::Medium, format!("Moduł {m} nie startuje od zmiany konfiguracji — wracam do ostatniej dobrej rewizji."));
                }
                _ => {
                    let key = b.key("enabled_key", format!("modules.{ms}.enabled"));
                    b.set(&key, json!(false));
                    b.because(Risk::Medium, format!("Moduł {m} nie startuje, a konfiguracja się nie zmieniła — wyłączam go do czasu diagnozy (reszta działa)."));
                    b.human(format!(
                        "Sprawdź logi modułu {m} (paczka diagnostyczna) i włącz go ponownie."
                    ));
                }
            }
        }
        FailureKind::ConfigCorrupted | FailureKind::KernelConfigTampered => {
            if b.restore_backup(&t) {
                b.because(Risk::Medium, format!("Plik {t} nie przechodzi walidacji — uszkodzony trafia do kwarantanny, wraca ostatnia dobra kopia."));
            } else {
                b.human(format!(
                    "Brak kopii {t} — popraw plik ręcznie (Ustawienia → edytor surowy)."
                ));
            }
            if b.d.kind == FailureKind::KernelConfigTampered {
                b.because(Risk::High, format!("Polityka Jądra {t} zmieniona poza Brokerem — przywrócenie podpisanej kopii wymaga Broker-UI."));
            }
        }
        FailureKind::SessionDbCorrupted => {
            if b.restore_backup(&t) {
                b.because(Risk::High, format!("Baza {t} jest uszkodzona — przywracam najnowszą kopię; uszkodzona zostaje w kwarantannie."));
                b.human("Zmiany od czasu kopii są w bazie w kwarantannie — sprawdź, czy czegoś brakuje.");
            } else {
                b.human(format!("Brak kopii bazy {t} — potrzebna ręczna naprawa (eksport `.alfa`, narzędzia SQLite)."));
            }
        }
        FailureKind::SessionDbLocked => {
            let key = b.key("busy_timeout_key", "sessions.busy_timeout_ms".into());
            let old = b.ctx.config(&key).and_then(|v| v.as_u64()).unwrap_or(5_000);
            b.set(&key, json!(old.saturating_mul(3).max(15_000)));
            b.restart(&m);
            b.because(Risk::Low, format!("Baza {t} jest zajęta przez inny proces — dłuższy czas oczekiwania i ponowne otwarcie."));
        }
        FailureKind::DiskFull => {
            for (from, to) in b.ctx.reclaimable(&t) {
                b.steps.push(RepairStep::MoveFile { from, to });
            }
            b.set("logs.min_level", json!("warn"));
            b.set("downloads.paused", json!(true));
            b.because(Risk::Medium, format!("Na {t} brakuje miejsca — przenoszę pamięć podręczną na inny wolumin, ograniczam logi, wstrzymuję pobieranie."));
            b.human(format!(
                "Zwolnij miejsce na {t} (Alfa niczego nie usuwa sama)."
            ));
        }
        FailureKind::SidecarCrashLoop => {
            let key = b.key("safe_mode_key", format!("sidecars.{ms}.safe_mode"));
            b.set(&key, json!(true));
            b.restart(&m);
            b.because(Risk::Low, format!("Proces {m} wysypuje się w pętli — uruchamiam go w trybie zachowawczym (CPU, mniej wątków)."));
        }
        FailureKind::GpuLost => {
            let key = b.key("device_key", format!("{ms}.device"));
            b.set(&key, json!(plain_token(b.detail("fallback"), "cpu")));
            b.restart(&m);
            b.because(
                Risk::Low,
                format!("{m} utracił GPU — przełączam na CPU (wolniej, ale działa)."),
            );
        }
        FailureKind::ApiKeyRevoked => {
            b.set(&format!("router.routes.{ts}.enabled"), json!(false));
            b.because(
                Risk::Low,
                format!("Dostawca {t} odrzuca klucz — wyłączam trasę, Router użyje zapasowej."),
            );
            b.human(format!(
                "Dodaj nowy klucz dla {t} w hubie kont (Diagnosta nie dotyka sekretów)."
            ));
        }
        FailureKind::RateLimitLoop => {
            let backoff = 60_000u64.saturating_mul(1 << b.d.count.min(6));
            b.set(
                &format!("router.routes.{ts}.paused_until_ms"),
                json!(b.ctx.now_ms().saturating_add(backoff)),
            );
            b.because(
                Risk::Low,
                format!(
                    "{t} odpowiada 429 w pętli — wstrzymuję trasę na {} s (wykładniczo).",
                    backoff / 1000
                ),
            );
        }
        FailureKind::BudgetExhausted => {
            b.set("router.prefer_local", json!(true));
            b.because(
                Risk::Low,
                "Budżet wyczerpany — Router przechodzi na model lokalny (budżetu nie zmieniam).",
            );
            b.human(
                "Limit budżetu jest polityką Jądra — podnosisz go tylko Ty (Ustawienia → Koszty).",
            );
        }
        FailureKind::PortInUse => {
            let key = b.key("port_key", format!("{ms}.port"));
            let old = b
                .detail("port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(0u16);
            match b.ctx.free_port(old) {
                Some(port) => {
                    b.set(&key, json!(port));
                    b.restart(&m);
                    b.because(
                        Risk::Low,
                        format!("Port {old} modułu {m} jest zajęty — przenoszę na {port}."),
                    );
                }
                None => b.human(format!(
                    "Brak wolnego portu dla {m} — zamknij program, który go zajmuje."
                )),
            }
        }
        FailureKind::ModelMissing | FailureKind::ModelCorrupted => {
            if b.d.kind == FailureKind::ModelCorrupted {
                b.quarantine(&t);
            }
            let model = b.detail("model").unwrap_or_else(|| t.clone());
            if let Some(fallback) = b.ctx.fallback_model(&model) {
                let key = b.key("model_key", format!("{ms}.model"));
                b.set(&key, json!(fallback));
            }
            // Hash z sygnału jest niezaufany — tylko poprawny SHA-256 trafia do kolejki pobrań.
            let sha = b
                .detail("sha256")
                .filter(|h| h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit()));
            match sha {
                Some(sha) => b.steps.push(RepairStep::QueueDownload {
                    item: model.clone(),
                    sha256: sha,
                }),
                None => b.human(format!(
                    "Pobierz model {model} ponownie (brak znanego hasha)."
                )),
            }
            b.because(Risk::Medium, format!("Model {model} jest niedostępny albo uszkodzony — model zastępczy i ponowne pobranie ze sprawdzeniem SHA-256."));
        }
        FailureKind::WebViewBroken => {
            if b.detail("runtime_missing").as_deref() == Some("true") {
                b.set("ui.fallback_mode", json!("tray"));
                b.human("Zainstaluj środowisko WebView2 (Microsoft Edge WebView2 Runtime).");
                b.because(
                    Risk::Low,
                    "Brak WebView2 — Alfa działa z zasobnika i głosem do czasu instalacji.",
                );
            } else {
                b.quarantine(&t);
                b.restart(&m);
                b.because(Risk::Medium, format!("Profil WebView2 {t} jest uszkodzony — przenoszę go do kwarantanny, WebView2 utworzy nowy."));
            }
        }
        FailureKind::SchemaMismatch => {
            b.set(&format!("{ms}.read_only"), json!(true));
            b.because(
                Risk::Low,
                format!(
                    "Dane {t} mają inną wersję schematu niż kod — otwieram je tylko do odczytu."
                ),
            );
            b.human("Zaktualizuj Alfę do wersji obsługującej te dane (albo zgłoś błąd migracji).");
        }
        FailureKind::UndoJournalFull | FailureKind::LogDiskLimit => {
            let entries = b
                .detail("entries")
                .and_then(|e| e.parse().ok())
                .unwrap_or(1_000);
            let archive = b.ctx.archive_path(&t);
            b.steps.push(RepairStep::ArchiveEntries {
                store: t.clone(),
                entries,
                archive,
            });
            b.because(Risk::Low, format!("{t} jest pełny — przenoszę {entries} najstarszych wpisów do archiwum (nic nie jest usuwane)."));
        }
        FailureKind::FileLocked => {
            b.set("io.defer_locked_writes", json!(true));
            b.because(
                Risk::Low,
                format!("Plik {t} trzyma inny program — zapisy czekają w kolejce i są ponawiane."),
            );
        }
        FailureKind::DirPermissionDenied => match b.ctx.fallback_dir(&t) {
            Some(dir) => {
                let key = b.key("dir_key", format!("{ms}.dir"));
                b.set(&key, json!(dir));
                b.because(
                    Risk::Low,
                    format!("Brak prawa zapisu do {t} — używam katalogu zastępczego {dir}."),
                );
            }
            None => b.human(format!(
                "Nadaj prawo zapisu do {t} albo wskaż inny katalog."
            )),
        },
        FailureKind::ClockSkew => {
            let skew: i64 = b
                .detail("skew_ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            b.set("time.offset_ms", json!(-skew));
            b.because(
                Risk::Low,
                format!(
                    "Zegar systemowy jest przesunięty o {skew} ms — Alfa koryguje znaczniki czasu."
                ),
            );
            b.human("Włącz synchronizację czasu w Windows (Ustawienia → Czas i język).");
        }
        FailureKind::NetworkDown => {
            b.set("router.offline", json!(true));
            b.because(
                Risk::Low,
                "Brak sieci — Router przechodzi w tryb offline (model lokalny).",
            );
        }
        FailureKind::UpdatePackageCorrupted => {
            b.quarantine(&t);
            b.because(Risk::Medium, format!("Paczka aktualizacji {t} ma zły podpis/hash — do kwarantanny (Broker), pobranie ponowi updater."));
        }
        FailureKind::ResourceBudgetExceeded => {
            let key = b.key("limit_key", format!("modules.{ms}.lifecycle"));
            b.set(&key, json!(plain_token(b.detail("limit_value"), "lazy")));
            b.restart(&m);
            b.because(
                Risk::Low,
                format!("{m} przekracza budżet zasobów — tryb oszczędny i ponowne uruchomienie."),
            );
        }
    }
}
