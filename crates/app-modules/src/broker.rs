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
    AutonomyChangeRequest, AutonomyTarget, Broker, Capability, ChangeOrigin, Decision,
    DeclaredFacts, DenyReason, Holder, PathScope, UNTIL_REVOKED,
};
use safety_broker_impl::BrokerEngine;
use sessions_contract::SessionId;
use undo_journal_contract::{StepId, UndoError, UndoJournal};
use undo_journal_impl::UndoService;
use watchdog_contract::{KillReason, KillSwitch};

use app_api::dto::{
    AutonomyLevel, BrokerIntentResult, BrokerIntentStatus, BrokerStatusView, GrantView, MaxAccess,
};
use app_api::error::{AppError, ErrorCode};
use app_api::ports::{ApprovalOutcome, ApprovalWindow, AutonomyView, BrokerPort, KillOrigin};
use app_broker::convert::{applied, broker_error, from_level, from_scope, to_level, to_scope};
use broker_ui_contract::{place_pl, verb};

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

fn undo_error(e: UndoError) -> AppError {
    if let UndoError::Partial(report) = &e {
        // Audyt A-06: częściowe cofnięcie mówi, co nie wróciło, i że krok można cofnąć ponownie.
        let paths: Vec<String> = report
            .failed
            .iter()
            .take(5)
            .map(|(path, why)| format!("„{}” ({why})", path.display()))
            .collect();
        let more = report.failed.len().saturating_sub(paths.len());
        let rest = if more > 0 {
            format!(" i {more} więcej")
        } else {
            String::new()
        };
        return AppError::new(
            ErrorCode::Storage,
            format!(
                "Cofnięcie częściowe: przywrócono {}, nie wróciło: {}{rest}. Kliknij „Cofnij”                  jeszcze raz, żeby spróbować przywrócić resztę.",
                report.restored,
                paths.join(", ")
            ),
        );
    }
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

    /// Zmiana poziomu z interfejsu: obniżenie od razu, podniesienie — karta w oknie Sędziego.
    async fn change(
        &self,
        target: AutonomyTarget,
        level: Level,
    ) -> Result<BrokerIntentResult, AppError> {
        let request = AutonomyChangeRequest {
            target,
            level,
            until_ms: None,
            origin: ChangeOrigin::UserInterface,
        };
        match self.engine.request_autonomy_change(request).await {
            Ok(None) => Ok(applied()),
            Ok(Some(approval)) => self.present(approval.0),
            Err(e) => Err(broker_error(e)),
        }
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
        let metrics = self.engine.metrics();
        Some(AutonomyView {
            global: from_level(global),
            session,
            max_access: metrics.max_access.map(from_scope),
            standing_global: metrics.standing_global.map(from_level),
        })
    }

    /// „Max dostęp” (plan 50 §8.1): włączenie (także zmiana zasięgu) — karta Sędziego;
    /// wyłączenie — obniżenie bieżącego zasięgu, od razu. Stan trzyma i zapamiętuje Sędzia.
    async fn request_max_access(
        &self,
        scope: Option<MaxAccess>,
    ) -> Result<BrokerIntentResult, AppError> {
        let (scope, level) = match scope {
            Some(s) => (to_scope(s), Level::L4),
            None => match self.engine.metrics().max_access {
                Some(on) => (on, Level::L3),
                None => return Ok(applied()),
            },
        };
        self.change(AutonomyTarget::Models { scope }, level).await
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
        self.change(target, to_level(level)).await
    }

    async fn open_approval(&self, approval: &str) -> Result<BrokerIntentResult, AppError> {
        let id: u64 = approval
            .parse()
            .map_err(|_| AppError::invalid(format!("Nieznana prośba „{approval}”.")))?;
        self.present(id)
    }

    async fn grants(&self) -> Vec<GrantView> {
        self.engine
            .grants()
            .into_iter()
            .map(|g| GrantView {
                id: g.id.to_string(),
                session_id: g.session.as_str().to_owned(),
                agent: g.agent.map(|a| a.as_str().to_owned()),
                scope: format!("{}: {}", verb(&g.scope), place_pl(&g.scope)),
                granted_at_ms: g.granted_at_ms,
                until_ms: (g.until_ms != UNTIL_REVOKED).then_some(g.until_ms),
            })
            .collect()
    }

    async fn revoke_grant(&self, grant: &str) -> Result<bool, AppError> {
        let id: u64 = grant
            .parse()
            .map_err(|_| AppError::invalid(format!("Nieznana zgoda „{grant}”.")))?;
        self.engine.revoke_grant(id).await.map_err(broker_error)
    }

    async fn approval_outcome(&self, approval: &str) -> Option<ApprovalOutcome> {
        app_broker::levels::approval_outcome(&*self.engine, approval)
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
                    "Uruchomienie kodu w terminalu (Sędzia Cieni zezwolił)",
                    "tools-shell",
                ))
            }
            Decision::NeedsApproval(ticket) => self.present(ticket.id.0),
            Decision::Deny(DenyReason::KernelBlock(rule)) => Err(AppError::forbidden(format!(
                "Twarda blokada Jądra: {rule:?} — tego nie da się uruchomić na żadnym poziomie."
            ))),
            Decision::Deny(DenyReason::AuditUnavailable) => Err(AppError::new(
                ErrorCode::Unavailable,
                "Audyt niedostępny — Sędzia Cieni nie wydaje zgód (bezpieczna odmowa).",
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

    async fn kill_all(&self, origin: KillOrigin) -> Result<Option<String>, AppError> {
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
            "kill-switch Sędziego Cieni"
        );
        Ok(kill_gap(&report))
    }
}

/// Czego kill-switch nie potwierdził (`None` — wszystko zatrzymane i zapisane w audycie).
pub fn kill_gap(report: &watchdog_contract::KillReport) -> Option<String> {
    let mut gaps = Vec::new();
    if !report.audited {
        gaps.push(
            "Sędzia Cieni nie potwierdził zatrzymania — uprawnienia agentek wygasną same,              a zapis w dzienniku audytu nie powstał"
                .to_owned(),
        );
    }
    if !report.jobs_failed.is_empty() {
        gaps.push(format!(
            "nie udało się zatrzymać procesów: {}",
            report.jobs_failed.len()
        ));
    }
    (!gaps.is_empty()).then(|| gaps.join("; "))
}

// Środowisko ścieżek i katalog Brokera w procesie żyją w `app-broker` (wybór Brokera dla
// `AppOptions`); reeksport dla dotychczasowych użytkowników.
pub use app_broker::inproc::{dev_dir, path_env, path_env_for};
