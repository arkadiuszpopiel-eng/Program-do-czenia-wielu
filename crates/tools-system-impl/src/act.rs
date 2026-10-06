//! Akcje: zakończenie procesu (strażnik celów + `gui.control(<obraz>)` + tożsamość), sterowanie
//! usługą (`system.admin(service_control)`), zapis zmiennej użytkownika (`system.admin(setx …)`)
//! z krokiem cofania.

use platform_apps_contract::{
    ProcessIdentity, ServiceCommand, is_critical_process, is_protected_entry,
};
use platform_contract::image_file_name;
use risk_classifier_contract::{Destructiveness, KernelRule};
use safety_broker_contract::{AdminOp, AppSelector, Capability, ServiceAction};
use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, base_facts, parse_args, text,
};
use tools_system_contract::{
    EVENT_ENV_SET, EVENT_PROCESS_KILLED, EVENT_SERVICE, EnvSetArgs, EnvSetOut, KillOut,
    ProcessKillArgs, ServiceActionArg, ServiceControlArgs, policy_refusal,
};

use crate::core::{Core, Step, deny, fail};
use crate::read::service_out;
use crate::undo::EnvStep;

/// Napis polecenia `system.admin` dla zapisu zmiennej (karta Brokera, reguły powłoki Jądra);
/// wartość zredagowana — sekrety nie trafiają do audytu.
pub(crate) fn env_command(name: &str, value: Option<&str>) -> String {
    match value {
        Some(v) => format!("setx {name} \"{}\"", text::redact_secrets(v)),
        None => format!(r"reg delete HKCU\Environment /v {name} /f"),
    }
}

impl Core {
    /// `system_process_kill`.
    pub(crate) async fn kill(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let raw = args.clone();
        let a: ProcessKillArgs = parse_args(args)?;
        let action = format!("zakończenie procesu {} (PID {})", a.name, a.pid);
        if let Some(why) = policy_refusal(&m.name, &raw) {
            return Err(deny(DenialReason::Policy, &why));
        }
        let pid = a.pid;
        let (d, all) = self
            .port(&action, move |s| Ok((s.process(pid)?, s.processes()?)))
            .await?;
        if image_file_name(&d.entry.image) != image_file_name(&a.name) {
            return Err(fail(
                ToolErrorKind::NotFound,
                format!(
                    "Nie wykonano: {action} — PID {pid} to teraz „{}”. Odczytaj listę procesów ponownie.",
                    d.entry.image
                ),
            ));
        }
        if is_critical_process(&d.entry.image) {
            return Err(deny(DenialReason::Policy, &action));
        }
        let guard = self.sys.guard();
        let path_protected = d
            .path
            .as_deref()
            .is_some_and(|p| guard.is_protected(d.entry.pid, p));
        if path_protected || is_protected_entry(guard, &d.entry, &all) {
            return Err(deny(
                DenialReason::KernelBlock {
                    rule: KernelRule::GuiControlOfKernelProcess,
                },
                &action,
            ));
        }
        if d.entry.own != Some(true) || d.elevated == Some(true) {
            let mut o = ToolOutcome::denied(DenialReason::Policy, &action);
            o.text = format!(
                "Odmowa: {action} — proces innego użytkownika, systemu albo administratora. \
                 Agentka kończy wyłącznie Twoje procesy."
            );
            return Err(Box::new(o));
        }
        let app = AppSelector::parse(&d.entry.image).map_err(|_| {
            deny(
                DenialReason::KernelBlock {
                    rule: KernelRule::GuiControlOfKernelProcess,
                },
                &action,
            )
        })?;
        let mut facts = base_facts(m, ctx);
        facts.destructive = Destructiveness::Permanent;
        let auth = self
            .authorize(ctx, Capability::GuiControl(app), facts, &action)
            .await?;
        if ctx.cancel.is_cancelled() {
            self.release(&auth).await;
            return Ok(ToolOutcome::cancelled(&action));
        }
        let id = ProcessIdentity::of(&d);
        let done = self.port(&action, move |s| s.terminate(&id)).await;
        self.release(&auth).await;
        done?;
        let image = image_file_name(&d.entry.image);
        self.emit(
            EVENT_PROCESS_KILLED,
            serde_json::json!({"pid": pid, "image": image}),
            ctx,
        )
        .await;
        let out = KillOut { pid, name: image };
        let mut o = ToolOutcome::ok(
            format!("Zakończono proces {} (PID {pid}).", out.name),
            serde_json::to_value(&out).unwrap_or_default(),
        );
        o.approval = auth.approval;
        Ok(o)
    }

    /// `system_service_control`.
    pub(crate) async fn service_control(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let raw = args.clone();
        let a: ServiceControlArgs = parse_args(args)?;
        let verb = match a.action {
            ServiceActionArg::Start => "start",
            ServiceActionArg::Stop => "zatrzymanie",
            ServiceActionArg::Restart => "restart",
        };
        let action = format!("{verb} usługi {}", a.name);
        if let Some(why) = policy_refusal(&m.name, &raw) {
            return Err(deny(DenialReason::Policy, &why));
        }
        let all = self.port(&action, |s| s.services()).await?;
        let svc = all
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(&a.name))
            .ok_or_else(|| {
                fail(
                    ToolErrorKind::NotFound,
                    format!(
                        "Nie wykonano: {action} — nie ma takiej usługi. Sprawdź `system_services`."
                    ),
                )
            })?;
        let name = svc.name.clone();
        let (op, cmd) = match a.action {
            ServiceActionArg::Start => (ServiceAction::Start, ServiceCommand::Start),
            ServiceActionArg::Stop => (ServiceAction::Stop, ServiceCommand::Stop),
            ServiceActionArg::Restart => (ServiceAction::Restart, ServiceCommand::Restart),
        };
        let cap = Capability::SystemAdmin(AdminOp::ServiceControl {
            service: name.clone(),
            action: op,
        });
        let auth = self
            .authorize(ctx, cap, base_facts(m, ctx), &action)
            .await?;
        if ctx.cancel.is_cancelled() {
            self.release(&auth).await;
            return Ok(ToolOutcome::cancelled(&action));
        }
        let timeout = self.config.service_timeout_ms;
        let target = name.clone();
        let done = self
            .port(&action, move |s| s.control_service(&target, cmd, timeout))
            .await;
        self.release(&auth).await;
        let entry = done?;
        let out = service_out(&entry);
        self.emit(
            EVENT_SERVICE,
            serde_json::json!({"service": name, "action": verb, "state": out.state}),
            ctx,
        )
        .await;
        let mut o = ToolOutcome::ok(
            format!("Usługa {name}: {verb} — stan {}.", out.state),
            serde_json::to_value(&out).unwrap_or_default(),
        );
        o.approval = auth.approval;
        Ok(o)
    }

    /// `system_env_set`.
    pub(crate) async fn env_set(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let raw = args.clone();
        let a: EnvSetArgs = parse_args(args)?;
        let action = match &a.value {
            Some(_) => format!("ustawienie zmiennej użytkownika {}", a.name),
            None => format!("usunięcie zmiennej użytkownika {}", a.name),
        };
        if let Some(why) = policy_refusal(&m.name, &raw) {
            return Err(deny(DenialReason::Policy, &why));
        }
        let cap = Capability::SystemAdmin(AdminOp::Other {
            command: env_command(&a.name, a.value.as_deref()),
        });
        let mut facts = base_facts(m, ctx);
        facts.command = Some(env_command(&a.name, a.value.as_deref()));
        let auth = self.authorize(ctx, cap, facts, &action).await?;
        if ctx.cancel.is_cancelled() {
            self.release(&auth).await;
            return Ok(ToolOutcome::cancelled(&action));
        }
        let (name, value) = (a.name.clone(), a.value.clone());
        let done = self
            .port(&action, move |s| s.set_user_env(&name, value.as_deref()))
            .await;
        self.release(&auth).await;
        let previous = done?;
        let previous_set = previous.is_some();
        let undo_id = self.undo.lock().map_or(0, |mut log| {
            log.push(
                EnvStep {
                    name: a.name.clone(),
                    previous,
                    written: a.value.clone(),
                },
                self.config.max_undo,
            )
        });
        self.emit(
            EVENT_ENV_SET,
            serde_json::json!({"name": a.name, "deleted": a.value.is_none()}),
            ctx,
        )
        .await;
        let out = EnvSetOut {
            name: a.name.clone(),
            deleted: a.value.is_none(),
            previous_set,
            undo_id,
        };
        let mut o = ToolOutcome::ok(
            format!(
                "{} zmienną użytkownika {} (krok cofania #{undo_id}). Nowe programy zobaczą zmianę po uruchomieniu.",
                if out.deleted {
                    "Usunięto"
                } else {
                    "Ustawiono"
                },
                out.name
            ),
            serde_json::to_value(&out).unwrap_or_default(),
        );
        o.approval = auth.approval;
        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_command_redacts_secrets() {
        assert_eq!(env_command("EDITOR", Some("code")), "setx EDITOR \"code\"");
        let c = env_command("X", Some("token=ghp_abcdefghijklmnopqrstuvwxyz0123"));
        assert!(!c.contains("ghp_abc"), "{c}");
        assert!(env_command("X", None).starts_with("reg delete"));
    }
}
