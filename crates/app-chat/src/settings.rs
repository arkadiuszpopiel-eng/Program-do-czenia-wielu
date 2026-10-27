//! Ustawienia przebiegów agentek: budżety, limit czekania na zgodę i samoweryfikacja (Ustawienia →
//! Agentki) oraz styl odpowiedzi i temperatura (Ustawienia → Ogólne) — jedno źródło dla rozmowy
//! z agentką i zadań w tle (`CoreHost::agent_settings`). Raport 32: przebieg pomijał styl.

use app_agents::{AgentSettings, keys};

use crate::engine::ChatEngine;
use crate::style;

impl ChatEngine {
    pub(crate) async fn continuation_policy(&self) -> agent_runtime_contract::ContinuationPolicy {
        let settings = self.agent_settings().await;
        agent_runtime_contract::ContinuationPolicy {
            auto_steps: self.config_bool("agents.auto_extend_steps", false).await,
            auto_time: self.config_bool("agents.auto_extend_time", false).await,
            auto_approval: self.config_bool("agents.auto_extend_approval", false).await,
            step_chunk: settings.max_steps.clamp(1, 500),
            time_chunk_ms: u64::from(settings.max_minutes.max(1)) * 60_000,
        }
    }
    /// Ustawienia agentek (Ustawienia → Agentki) ze stylem i temperaturą (Ustawienia → Ogólne).
    pub async fn agent_settings(&self) -> AgentSettings {
        let d = AgentSettings::default();
        let num = |v: Option<serde_json::Value>, default: u64| {
            v.and_then(|v| v.as_f64())
                .filter(|n| n.is_finite() && *n >= 0.0)
                .map_or(default, |n| n.round() as u64)
        };
        let u32_of = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        let text = |v: Option<serde_json::Value>| {
            v.and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        AgentSettings {
            max_steps: u32_of(num(
                self.config_value(keys::MAX_STEPS).await,
                d.max_steps.into(),
            )),
            max_minutes: u32_of(num(
                self.config_value(keys::MAX_MINUTES).await,
                d.max_minutes.into(),
            )),
            max_cost_grosze: num(self.config_value(keys::MAX_COST_PLN).await, 0) * 100,
            approval_timeout_s: match self.inner.approval_timeout {
                Some(limit) => u32_of(limit.as_secs().max(1)),
                None => u32_of(num(
                    self.config_value(keys::APPROVAL_TIMEOUT_S).await,
                    d.approval_timeout_s.into(),
                )),
            },
            verify: self.config_bool(keys::VERIFY, d.verify).await,
            reply_style: style::style_line(&text(self.config_value(style::STYLE_KEY).await)),
            temperature: style::temperature(&text(self.config_value(style::TEMPERATURE_KEY).await)),
        }
    }
}
