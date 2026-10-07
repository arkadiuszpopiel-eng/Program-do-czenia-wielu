//! Odczyty: procesy, szczegóły procesu, usługi, Dziennik zdarzeń, zmienne, stan systemu. Każdy
//! przez `gui.control(system-info.exe)`; wynik niezaufany, sekrety ukryte/zredagowane.

use platform_apps_contract::{
    EnvScope, EventLevel, EventLogName, EventQuery, ProcessEntry, is_protected_entry,
    is_secret_env_name,
};
use platform_contract::{AudioDirection, PowerSource};
use tools_common_contract::{ToolCtx, ToolManifest, ToolOutcome, parse_args, text};
use tools_system_contract::{
    AudioOut, DisplayOut, EnvArgs, EnvListOut, EnvOut, EnvScopeArg, EventOut, EventsArgs,
    EventsOut, LevelArg, LogArg, PowerOut, ProcDetailsOut, ProcOut, ProcessInfoArgs, ProcessesArgs,
    ProcessesOut, ServiceFilter, ServiceOut, ServicesArgs, ServicesOut, StatusArgs, StatusOut,
};

use crate::core::{Core, Step};

/// Najdłuższa wartość zmiennej w wyniku (znaki).
const MAX_ENV_VALUE_OUT: usize = 2_000;

fn proc_out(e: &ProcessEntry, protected: bool) -> ProcOut {
    ProcOut {
        pid: e.pid,
        parent_pid: e.parent_pid,
        name: text::redact_secrets(&e.image),
        own: e.own,
        protected,
    }
}

pub(crate) fn service_out(s: &platform_apps_contract::ServiceEntry) -> ServiceOut {
    ServiceOut {
        name: s.name.clone(),
        display_name: text::redact_secrets(&s.display_name),
        state: serde_json::to_value(s.state)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        pid: s.pid,
    }
}

fn contains(hay: &str, needle: Option<&String>) -> bool {
    needle.is_none_or(|n| hay.to_lowercase().contains(&n.to_lowercase()))
}

impl Core {
    /// `system_processes`.
    pub(crate) async fn processes(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ProcessesArgs = parse_args(args)?;
        let action = "lista procesów";
        let auth = self.authorize_read(ctx, m, action).await?;
        let all = match self.port(action, |s| s.processes()).await {
            Ok(v) => v,
            Err(e) => {
                self.release(&auth).await;
                return Err(e);
            }
        };
        let guard = self.sys.guard().clone();
        let matching: Vec<ProcOut> = all
            .iter()
            .filter(|p| contains(&p.image, a.name_contains.as_ref()))
            .map(|p| proc_out(p, is_protected_entry(&guard, p, &all)))
            .collect();
        let limit = a.limit.unwrap_or(self.config.max_list).clamp(1, 500) as usize;
        let total = u32::try_from(matching.len()).unwrap_or(u32::MAX);
        let out = ProcessesOut {
            truncated: matching.len() > limit,
            processes: matching.into_iter().take(limit).collect(),
            total,
        };
        let summary = format!("Procesy: {} z {total}.", out.processes.len());
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }

    /// `system_process_info`.
    pub(crate) async fn process_info(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ProcessInfoArgs = parse_args(args)?;
        let action = format!("szczegóły procesu {}", a.pid);
        let auth = self.authorize_read(ctx, m, &action).await?;
        let pid = a.pid;
        let read = self
            .port(&action, move |s| Ok((s.process(pid)?, s.processes()?)))
            .await;
        let (d, all) = match read {
            Ok(v) => v,
            Err(e) => {
                self.release(&auth).await;
                return Err(e);
            }
        };
        let guard = self.sys.guard();
        let protected = is_protected_entry(guard, &d.entry, &all)
            || d.path
                .as_deref()
                .is_some_and(|p| guard.is_protected(d.entry.pid, p));
        let out = ProcDetailsOut {
            process: proc_out(&d.entry, protected),
            path: if protected {
                None
            } else {
                d.path.as_deref().map(text::redact_secrets)
            },
            elevated: d.elevated,
            session_id: d.entry.session_id,
            started_ms: d.started_ms,
            threads: d.entry.threads,
            memory_kb: d.memory_kb,
        };
        let summary = format!(
            "Proces {} (PID {}){}.",
            out.process.name,
            out.process.pid,
            if protected { " — chroniony" } else { "" }
        );
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }

    /// `system_services`.
    pub(crate) async fn services(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ServicesArgs = parse_args(args)?;
        let action = "lista usług";
        let auth = self.authorize_read(ctx, m, action).await?;
        let all = match self.port(action, |s| s.services()).await {
            Ok(v) => v,
            Err(e) => {
                self.release(&auth).await;
                return Err(e);
            }
        };
        let matching: Vec<ServiceOut> = all
            .iter()
            .filter(|s| {
                contains(&s.name, a.name_contains.as_ref())
                    || contains(&s.display_name, a.name_contains.as_ref())
            })
            .filter(|s| match a.state {
                None => true,
                Some(ServiceFilter::Running) => {
                    s.state == platform_apps_contract::ServiceState::Running
                }
                Some(ServiceFilter::Stopped) => {
                    s.state == platform_apps_contract::ServiceState::Stopped
                }
            })
            .map(service_out)
            .collect();
        let limit = a.limit.unwrap_or(self.config.max_list).clamp(1, 500) as usize;
        let total = u32::try_from(matching.len()).unwrap_or(u32::MAX);
        let out = ServicesOut {
            truncated: matching.len() > limit,
            services: matching.into_iter().take(limit).collect(),
            total,
        };
        let summary = format!("Usługi: {} z {total}.", out.services.len());
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }

    /// `system_events`.
    pub(crate) async fn events(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: EventsArgs = parse_args(args)?;
        let log = match a.log {
            LogArg::Application => EventLogName::Application,
            LogArg::System => EventLogName::System,
        };
        let min_level = match a.level.unwrap_or(LevelArg::Warning) {
            LevelArg::Critical => EventLevel::Critical,
            LevelArg::Error => EventLevel::Error,
            LevelArg::Warning => EventLevel::Warning,
            LevelArg::Information => EventLevel::Information,
        };
        let max = a.max.unwrap_or(self.config.max_events).clamp(1, 200);
        let query = EventQuery {
            log,
            min_level: Some(min_level),
            provider: a.provider.clone(),
            since_ms: Some(u64::from(a.since_hours.unwrap_or(24).clamp(1, 720)) * 3_600_000),
            max,
        };
        let action = format!("odczyt dziennika {}", log.channel());
        let auth = self.authorize_read(ctx, m, &action).await?;
        let records = match self.port(&action, move |s| s.events(&query)).await {
            Ok(v) => v,
            Err(e) => {
                self.release(&auth).await;
                return Err(e);
            }
        };
        let max_chars = self.config.max_message_chars;
        let out = EventsOut {
            truncated: records.len() >= max as usize,
            events: records
                .iter()
                .map(|r| EventOut {
                    time_ms: r.time_ms,
                    level: format!("{:?}", r.level).to_lowercase(),
                    provider: text::redact_secrets(&r.provider),
                    event_id: r.event_id,
                    message: text::truncate_chars(&text::redact_secrets(&r.message), max_chars).0,
                })
                .collect(),
        };
        let summary = format!("Zdarzenia ({}): {}.", log.channel(), out.events.len());
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }

    /// `system_env`.
    pub(crate) async fn env(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: EnvArgs = parse_args(args)?;
        let (scope, label) = match a.scope.unwrap_or(EnvScopeArg::User) {
            EnvScopeArg::Process => (EnvScope::Process, "process"),
            EnvScopeArg::User => (EnvScope::User, "user"),
            EnvScopeArg::Machine => (EnvScope::Machine, "machine"),
        };
        let action = "odczyt zmiennych środowiskowych";
        let auth = self.authorize_read(ctx, m, action).await?;
        let vars = match self.port(action, move |s| s.env(scope)).await {
            Ok(v) => v,
            Err(e) => {
                self.release(&auth).await;
                return Err(e);
            }
        };
        let out = EnvListOut {
            scope: label.to_owned(),
            vars: vars
                .into_iter()
                .filter(|v| contains(&v.name, a.name_contains.as_ref()))
                .map(|v| {
                    let hidden = v.value.is_none() || is_secret_env_name(&v.name);
                    EnvOut {
                        value: v.value.filter(|_| !hidden).map(|x| {
                            text::truncate_chars(&text::redact_secrets(&x), MAX_ENV_VALUE_OUT).0
                        }),
                        name: v.name,
                        hidden,
                    }
                })
                .collect(),
        };
        let summary = format!("Zmienne ({label}): {}.", out.vars.len());
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }

    /// `system_status`.
    pub(crate) async fn status(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let _: StatusArgs = parse_args(args)?;
        let action = "stan systemu";
        let auth = self.authorize_read(ctx, m, action).await?;
        let (power, desktop, hardware) = (
            self.power.clone(),
            self.desktop.clone(),
            self.hardware.clone(),
        );
        let joined = tokio::task::spawn_blocking(move || {
            let mut out = StatusOut {
                power: None,
                displays: Vec::new(),
                audio: Vec::new(),
                unavailable: Vec::new(),
            };
            match power.map(|p| p.power()) {
                Some(Ok(p)) => {
                    out.power = Some(PowerOut {
                        source: match p.source {
                            PowerSource::Ac => "ac",
                            PowerSource::Battery => "battery",
                            PowerSource::Unknown => "unknown",
                        }
                        .into(),
                        battery_present: p.battery_present,
                        battery_percent: p.battery_percent,
                        saver: p.saver,
                    });
                }
                _ => out.unavailable.push("zasilanie".into()),
            }
            match desktop.map(|d| d.monitors()) {
                Some(Ok(ms)) => {
                    out.displays = ms
                        .iter()
                        .map(|m| DisplayOut {
                            index: m.index,
                            width: m.rect.width(),
                            height: m.rect.height(),
                            dpi: m.dpi,
                            primary: m.primary,
                        })
                        .collect();
                }
                _ => out.unavailable.push("monitory".into()),
            }
            match hardware.map(|h| h.audio_endpoints()) {
                Some(Ok(eps)) => {
                    out.audio = eps
                        .iter()
                        .map(|e| AudioOut {
                            name: text::redact_secrets(&e.name),
                            direction: match e.direction {
                                AudioDirection::Capture => "capture",
                                AudioDirection::Render => "render",
                            }
                            .into(),
                        })
                        .collect();
                }
                _ => out.unavailable.push("audio".into()),
            }
            out
        })
        .await;
        let out = match joined {
            Ok(o) => o,
            Err(e) => {
                self.release(&auth).await;
                return Err(crate::core::fail(
                    tools_common_contract::ToolErrorKind::Internal,
                    format!("Wątek stanu systemu: {e}."),
                ));
            }
        };
        let summary = format!(
            "Stan systemu: {} monitor(y), {} urządzeń audio{}.",
            out.displays.len(),
            out.audio.len(),
            if out.unavailable.is_empty() {
                String::new()
            } else {
                format!("; niedostępne: {}", out.unavailable.join(", "))
            }
        );
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }
}
