//! Wspólne kroki narzędzi systemowych: zgoda Brokera (`decide` → zatwierdzenie → `verify`),
//! wywołania portu na wątku blokującym, mapowanie błędów portu, zdarzenia, tekst wyniku.

use std::sync::{Arc, Mutex};

use core_bus_contract::{EventBus, Level};
use platform_apps_contract::{SysError, SysPort};
use platform_contract::{DesktopPort, HardwarePort, PowerPort};
use safety_broker_contract::{Capability, DeclaredFacts, TaintSource};
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome,
    action_request, base_facts, report_untrusted, text, tool_event,
};
use tools_system_contract::{SystemToolsConfig, sysinfo_capability};

use crate::undo::UndoLog;

/// Wynik pośredni (`Err` = gotowy wynik dla modelu).
pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

pub(crate) fn fail(kind: ToolErrorKind, text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

pub(crate) fn deny(reason: DenialReason, action: &str) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::denied(reason, action))
}

/// Błąd portu → wynik dla modelu.
pub(crate) fn sys_failure(e: &SysError, action: &str) -> Box<ToolOutcome> {
    let msg = |hint: &str| format!("Nie wykonano: {action} — {e}.{hint}");
    match e {
        SysError::Protected(_) => {
            let mut o = ToolOutcome::denied(DenialReason::Policy, action);
            o.text = format!("Odmowa: {action} — {e}. Nie ponawiaj; to cel chroniony.");
            Box::new(o)
        }
        SysError::NotFound(_) => fail(ToolErrorKind::NotFound, msg(" Odśwież listę.")),
        SysError::Changed(_) => fail(
            ToolErrorKind::NotFound,
            msg(" Proces się zmienił (PID użyty ponownie) — odczytaj listę ponownie."),
        ),
        SysError::PermissionDenied(_) => fail(
            ToolErrorKind::Io,
            msg(
                " Operacja wymaga uprawnień administratora (podniesienie przez UAC — poza tą wersją).",
            ),
        ),
        SysError::Invalid(_) => fail(ToolErrorKind::InvalidArgs, msg("")),
        SysError::Unsupported(_) => fail(ToolErrorKind::Unsupported, msg("")),
        SysError::Timeout(_) => fail(ToolErrorKind::Timeout, msg("")),
        SysError::Io(_) => fail(ToolErrorKind::Io, msg("")),
    }
}

pub(crate) struct Core {
    pub(crate) sys: Arc<dyn SysPort>,
    pub(crate) power: Option<Arc<dyn PowerPort>>,
    pub(crate) desktop: Option<Arc<dyn DesktopPort>>,
    pub(crate) hardware: Option<Arc<dyn HardwarePort>>,
    pub(crate) gate: BrokerGate,
    pub(crate) config: SystemToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
    pub(crate) undo: Mutex<UndoLog>,
}

impl Core {
    pub(crate) async fn emit(&self, name: &str, payload: serde_json::Value, ctx: &ToolCtx) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(tool_event(name, Level::Info, payload, ctx))
                .await;
        }
    }

    /// Zgoda na jedną zdolność z weryfikacją tokenu dla tego użycia.
    pub(crate) async fn authorize(
        &self,
        ctx: &ToolCtx,
        cap: Capability,
        facts: DeclaredFacts,
        action: &str,
    ) -> Step<Authorization> {
        let auth = self
            .gate
            .authorize(action_request(ctx, cap.clone(), facts), ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        if let Err(e) = self.gate.verify(&auth, &cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(e.into_outcome(action)));
        }
        Ok(auth)
    }

    /// Zgoda na odczyt systemu (`gui.control(system-info.exe)`, dane prywatne).
    pub(crate) async fn authorize_read(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        action: &str,
    ) -> Step<Authorization> {
        let cap = sysinfo_capability()
            .map_err(|e| fail(ToolErrorKind::Internal, format!("Zdolność odczytu: {e}.")))?;
        let mut facts = base_facts(m, ctx);
        facts.touches_private_data = true;
        self.authorize(ctx, cap, facts, action).await
    }

    pub(crate) async fn release(&self, auth: &Authorization) {
        self.gate.release(std::slice::from_ref(auth)).await;
    }

    /// Wywołanie portu na wątku blokującym.
    pub(crate) async fn port<T, F>(&self, action: &str, work: F) -> Step<T>
    where
        T: Send + 'static,
        F: FnOnce(&dyn SysPort) -> Result<T, SysError> + Send + 'static,
    {
        let sys = self.sys.clone();
        let joined = tokio::task::spawn_blocking(move || work(sys.as_ref())).await;
        match joined {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(sys_failure(&e, action)),
            Err(e) => Err(fail(
                ToolErrorKind::Internal,
                format!("Wątek portu systemu: {e}."),
            )),
        }
    }

    /// Wynik odczytu: tekst (podsumowanie + dane), treść niezaufana, taint zgłoszony Brokerowi.
    pub(crate) async fn untrusted_ok<T: serde::Serialize>(
        &self,
        ctx: &ToolCtx,
        auth: &Authorization,
        summary: String,
        data: &T,
    ) -> ToolOutcome {
        self.release(auth).await;
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let value = serde_json::to_value(data).unwrap_or_default();
        let mut out =
            ToolOutcome::ok(self.render(&summary, &value), value).untrusted(TaintSource::File);
        out.approval = auth.approval;
        out
    }

    /// Podsumowanie + JSON danych, obcięte limitem wyniku.
    pub(crate) fn render(&self, summary: &str, data: &serde_json::Value) -> String {
        let body = serde_json::to_string(data).unwrap_or_default();
        let (text, _) =
            text::truncate_chars(&format!("{summary}\n{body}"), self.config.output_max_chars);
        text
    }
}
