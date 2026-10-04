//! `BrokerPort` nad dowolnym `Broker` + `KillSwitch`: silnikiem w procesie (tryb deweloperski)
//! albo klientem IPC Brokera poza procesem (`app-broker`: usługa / tryb przenośny), i dziennikiem
//! cofania `undo-journal`. Zasady bez zmian: podniesienie poziomu i akcje wymagające zgody idą
//! wyłącznie przez okno Brokera (`ApprovalWindow`; bez niego — odmowa), obniżenie działa od razu,
//! każda decyzja trafia do Audytu z łańcuchem SHA-256.

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::PathEnv;
use risk_classifier_contract::{
    AutonomyLevel as Level, CommandOrigin, Destructiveness, Reversibility,
};
use safety_broker_contract::{
    AutonomyChangeRequest, AutonomyTarget, Broker, BrokerError, Capability, ChangeOrigin, Decision,
    DeclaredFacts, DenyReason, Holder, PathScope,
};
use safety_broker_impl::BrokerEngine;
use sessions_contract::SessionId;
use undo_journal_contract::{StepId, UndoError, UndoJournal};
use undo_journal_impl::UndoService;
use watchdog_contract::{KillReason, KillSwitch};

use app_api::dto::{AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView};
use app_api::error::{AppError, ErrorCode};
use app_api::ports::{ApprovalWindow, AutonomyView, BrokerPort, KillOrigin};

/// Sesja-sonda poziomu globalnego (identyfikatory sesji to UUIDv7 — brak kolizji).
const GLOBAL_PROBE: &str = "__alfa_global__";

/// Broker (w procesie albo poza nim) + dziennik cofania.
pub struct InprocBroker {
    engine: Arc<dyn Broker>,
    kill: Arc<dyn KillSwitch>,
    undo: Option<Arc<UndoService>>,
    window: Arc<dyn ApprovalWindow>,
    env: PathEnv,
    /// Zakres katalogów roboczych sesji (`%USERPROFILE%\Alfa\Sesje`).
    workdirs: String,
}

fn to_level(level: AutonomyLevel) -> Level {
    match level {
        AutonomyLevel::L0 => Level::L0,
        AutonomyLevel::L1 => Level::L1,
        AutonomyLevel::L2 => Level::L2,
        AutonomyLevel::L3 => Level::L3,
        AutonomyLevel::L4 => Level::L4,
    }
}

fn from_level(level: Level) -> AutonomyLevel {
    match level {
        Level::L0 => AutonomyLevel::L0,
        Level::L1 => AutonomyLevel::L1,
        Level::L2 => AutonomyLevel::L2,
        Level::L3 => AutonomyLevel::L3,
        Level::L4 => AutonomyLevel::L4,
    }
}

/// Błąd Brokera → błąd komendy.
fn broker_error(e: BrokerError) -> AppError {
    let code = match &e {
        BrokerError::KernelBlock(_) | BrokerError::Unauthorized(_) => ErrorCode::Forbidden,
        BrokerError::AuditUnavailable(_) => ErrorCode::Unavailable,
        BrokerError::UnknownApproval(_) | BrokerError::UnknownSession(_) => ErrorCode::NotFound,
        _ => ErrorCode::InvalidInput,
    };
    AppError::new(code, format!("Broker: {e}"))
}

fn undo_error(e: UndoError) -> AppError {
    let code = match &e {
        UndoError::UnknownStep(_) => ErrorCode::NotFound,
        UndoError::Conflict { .. } | UndoError::Expired(_) | UndoError::BadState { .. } => {
            ErrorCode::Forbidden
        }
        _ => ErrorCode::Storage,
    };
    AppError::new(code, format!("Cofnięcie: {e}"))
}

impl InprocBroker {
    /// Broker w procesie z dziennikiem cofania i oknem zatwierdzeń (`NoApprovalWindow` w dev).
    pub fn new(
        engine: Arc<BrokerEngine>,
        undo: Option<Arc<UndoService>>,
        window: Arc<dyn ApprovalWindow>,
        env: PathEnv,
    ) -> Self {
        Self::over(engine.clone(), engine, undo, window, env)
    }

    /// `BrokerPort` nad dowolnym Brokerem i kill-switchem (Broker poza procesem: `app-broker`).
    pub fn over(
        engine: Arc<dyn Broker>,
        kill: Arc<dyn KillSwitch>,
        undo: Option<Arc<UndoService>>,
        window: Arc<dyn ApprovalWindow>,
        env: PathEnv,
    ) -> Self {
        Self {
            engine,
            kill,
            undo,
            window,
            env,
            workdirs: r"%USERPROFILE%\Alfa\Sesje".into(),
        }
    }

    /// Prośba czeka na zatwierdzenie → karta w oknie Brokera (bez okna — odmowa).
    fn present(&self, approval: u64) -> Result<BrokerIntentResult, AppError> {
        let id = approval.to_string();
        self.window.present(&id)?;
        Ok(BrokerIntentResult {
            status: BrokerIntentStatus::OpenedBroker,
            request_id: id,
        })
    }

    fn global(&self) -> Level {
        self.engine.autonomy(&SessionId::new(GLOBAL_PROBE), None)
    }
}

#[async_trait]
impl BrokerPort for InprocBroker {
    async fn levels(&self, session: Option<&SessionId>) -> Option<AutonomyView> {
        let global = self.global();
        let session = session
            .map(|s| self.engine.autonomy(s, None))
            .filter(|l| *l != global)
            .map(from_level);
        Some(AutonomyView {
            global: from_level(global),
            session,
        })
    }

    async fn request_level(
        &self,
        level: AutonomyLevel,
        session: Option<&SessionId>,
    ) -> Result<BrokerIntentResult, AppError> {
        let target = match session {
            Some(s) => AutonomyTarget::Session { session: s.clone() },
            None => AutonomyTarget::Global,
        };
        let request = AutonomyChangeRequest {
            target,
            level: to_level(level),
            until_ms: None,
            origin: ChangeOrigin::UserInterface,
        };
        match self.engine.request_autonomy_change(request).await {
            Ok(None) => Ok(BrokerIntentResult {
                status: BrokerIntentStatus::Applied,
                request_id: String::new(),
            }),
            Ok(Some(approval)) => self.present(approval.0),
            Err(e) => Err(broker_error(e)),
        }
    }

    async fn open_approval(&self, approval: &str) -> Result<BrokerIntentResult, AppError> {
        let id: u64 = approval
            .parse()
            .map_err(|_| AppError::invalid(format!("Nieznana prośba „{approval}”.")))?;
        self.present(id)
    }

    async fn run_code(
        &self,
        session: &SessionId,
        lang: Option<&str>,
        code: &str,
    ) -> Result<BrokerIntentResult, AppError> {
        let scope = PathScope::tree(&self.workdirs, &self.env)
            .map_err(|e| AppError::internal(format!("zakres sesji: {e}")))?;
        // Język bloku tylko w nazwie narzędzia; reguły Jądra sprawdzają samo polecenie.
        let tool = lang.map_or_else(|| "shell.exec".to_owned(), |l| format!("shell.exec:{l}"));
        let mut facts = DeclaredFacts::new(&tool);
        facts.reversible = Reversibility::No;
        facts.destructive = Destructiveness::None;
        facts.command = Some(code.to_owned());
        let action = safety_broker_contract::ActionRequest {
            holder: Holder {
                session: session.clone(),
                agent: None,
                role: None,
            },
            capability: Capability::ShellExec(scope),
            facts,
            origin: CommandOrigin::UserText,
            ttl_ms: None,
        };
        match self.engine.decide(action).await.map_err(broker_error)? {
            Decision::Allow(token) => {
                // Brak wykonawcy (`tools-shell`) — zgoda nie może zostać użyta, więc ją cofamy.
                if let Err(e) = self.engine.revoke(token.id).await {
                    tracing::warn!(error = %e, "unieważnienie nieużytego tokenu nie powiodło się");
                }
                Err(AppError::unavailable(
                    "Uruchomienie kodu w terminalu (Broker zezwolił)",
                    "tools-shell",
                ))
            }
            Decision::NeedsApproval(ticket) => self.present(ticket.id.0),
            Decision::Deny(DenyReason::KernelBlock(rule)) => Err(AppError::forbidden(format!(
                "Twarda blokada Jądra: {rule:?} — tego nie da się uruchomić na żadnym poziomie."
            ))),
            Decision::Deny(DenyReason::AuditUnavailable) => Err(AppError::new(
                ErrorCode::Unavailable,
                "Audyt niedostępny — Broker nie wydaje zgód (bezpieczna odmowa).",
            )),
        }
    }

    async fn undo_step(&self, session: &SessionId, step: u64) -> Result<String, AppError> {
        let undo = self
            .undo
            .as_ref()
            .ok_or_else(|| AppError::unavailable("Cofnięcie kroku", "undo-journal"))?;
        let summary = undo
            .steps(session)
            .into_iter()
            .find(|s| s.step == StepId(step))
            .ok_or_else(|| AppError::not_found("Ten krok nie należy do tej sesji."))?;
        let result = undo.undo(StepId(step));
        undo.flush_events().await;
        let report = result.map_err(undo_error)?;
        Ok(format!(
            "{} — przywrócono operacji: {}",
            summary.text, report.restored
        ))
    }

    fn approval_window(&self) -> bool {
        self.window.available()
    }

    fn status(&self) -> BrokerStatusView {
        self.window
            .status()
            .unwrap_or_else(|| BrokerStatusView::in_process(self.window.available()))
    }

    async fn kill_all(&self, origin: KillOrigin) -> Result<(), AppError> {
        let reason = match origin {
            KillOrigin::Hotkey => KillReason::Hotkey,
            KillOrigin::Tray => KillReason::TrayButton,
            KillOrigin::Ui => KillReason::CapsuleButton,
            KillOrigin::Voice => KillReason::VoiceStop,
        };
        let report = self.kill.kill_all(reason).await;
        tracing::warn!(
            tokeny = report.tokens_revoked,
            procesy = report.jobs_killed,
            audyt = report.audited,
            "kill-switch Brokera"
        );
        Ok(())
    }
}

// Środowisko ścieżek i katalog Brokera w procesie żyją w `app-broker` (wybór Brokera dla
// `AppOptions`); reeksport dla dotychczasowych użytkowników.
pub use app_broker::inproc::{dev_dir, path_env, path_env_for};
