//! Bramka budżetu nad `cost-meter`: bieżące wydatki miesiąca (całość, tło, dostawca), kurs,
//! konfiguracja limitów → `cost_meter_contract::evaluate` (bez I/O: agregaty w pamięci).

use std::sync::Arc;

use cost_meter_contract::{
    BudgetDecision, CostClock, CostMeter, Month, Spent, TotalsQuery, evaluate, usd_to_pln,
};
use providers_contract::ProviderId;
use router_contract::BudgetGate;

/// Bramka budżetu z licznika kosztów.
pub struct CostMeterGate {
    meter: Arc<dyn CostMeter>,
    clock: Arc<dyn CostClock>,
}

impl CostMeterGate {
    /// Bramka nad licznikiem i jego zegarem (granice miesięcy lokalnie).
    pub fn new(meter: Arc<dyn CostMeter>, clock: Arc<dyn CostClock>) -> Self {
        Self { meter, clock }
    }
}

impl BudgetGate for CostMeterGate {
    fn check(
        &self,
        provider: &ProviderId,
        estimate_micro_usd: u64,
        background: bool,
    ) -> BudgetDecision {
        let month = Month::of(self.clock.today());
        let account = accounts_hub_contract::ProviderId::new(provider.as_str()).ok();
        let spent = Spent {
            month_micro_pln: self.meter.totals(&TotalsQuery::Month { month }).micro_pln,
            background_micro_pln: self
                .meter
                .totals(&TotalsQuery::Background { month })
                .micro_pln,
            provider_micro_pln: account.as_ref().map_or(0, |p| {
                self.meter
                    .totals(&TotalsQuery::Provider {
                        provider: p.clone(),
                        month,
                    })
                    .micro_pln
            }),
        };
        let estimate = usd_to_pln(estimate_micro_usd, self.meter.current_rate().rate_e4);
        evaluate(
            &self.meter.budget(),
            spent,
            estimate,
            background,
            account.as_ref(),
        )
    }
}
