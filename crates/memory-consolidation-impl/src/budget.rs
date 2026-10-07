//! Budżet tła przez `cost-meter` (`background = true`): szacunek mikro-USD → mikro-PLN bieżącym
//! kursem, decyzja `check_budget`, rejestracja faktycznego zużycia (koszt podany przez dostawcę
//! albo „nieznany” — nigdy liczony jako 0, poza modelem lokalnym).

use std::sync::Arc;

use accounts_hub_contract::{ModelId, ProviderId};
use async_trait::async_trait;
use core_bus_contract::AgentId;
use cost_meter_contract::{BudgetDecision, CostInput, CostMeter, Pricing, Usage, usd_to_pln};
use memory_consolidation_contract::{
    BackgroundBudget, BudgetVerdict, ConsolidationError, ConsolidatorModel, LlmUsage,
};

/// Persona w roli Strażniczki pamięci w obsadzie „Standard” (PLAN §9.2) — przypisanie kosztu.
pub const GUARDIAN_PERSONA: &str = "beta";

/// Budżet tła na liczniku kosztów.
pub struct CostMeterBudget {
    meter: Arc<dyn CostMeter>,
}

impl CostMeterBudget {
    /// Nowy adapter.
    pub fn new(meter: Arc<dyn CostMeter>) -> Self {
        Self { meter }
    }
}

fn err(e: impl std::fmt::Display) -> ConsolidationError {
    ConsolidationError::new(e.to_string())
}

#[async_trait]
impl BackgroundBudget for CostMeterBudget {
    async fn check(
        &self,
        model: &ConsolidatorModel,
        estimate_micro_usd: Option<u64>,
    ) -> BudgetVerdict {
        let pln =
            estimate_micro_usd.map_or(0, |usd| usd_to_pln(usd, self.meter.current_rate().rate_e4));
        if estimate_micro_usd.is_none() && !model.local {
            return BudgetVerdict::Deny {
                reason: "nieznany koszt modelu chmurowego w tle".into(),
            };
        }
        let provider = ProviderId::new(model.provider.clone()).ok();
        match self.meter.check_budget(pln, true, provider.as_ref()).await {
            BudgetDecision::Allow | BudgetDecision::Warn { .. } => BudgetVerdict::Allow,
            BudgetDecision::Block { notice } => BudgetVerdict::Deny {
                reason: format!("limit tła: {notice:?}"),
            },
        }
    }

    async fn record(&self, usage: &LlmUsage) -> Result<(), ConsolidationError> {
        let input = CostInput {
            session: None,
            agent: Some(AgentId::new(GUARDIAN_PERSONA)),
            provider: ProviderId::new(usage.provider.clone()).map_err(err)?,
            account: None,
            model: ModelId::new(usage.model.clone()).map_err(err)?,
            usage: Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            pricing: match usage.cost_micro_usd {
                Some(micro_usd) => Pricing::Reported { micro_usd },
                None => Pricing::Unknown,
            },
            background: true,
        };
        self.meter.record(input).await.map(|_| ()).map_err(err)
    }
}
