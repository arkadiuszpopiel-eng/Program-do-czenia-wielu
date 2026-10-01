//! Komendy `costs_*`: podsumowanie (grosze) i limit miesięczny PLN (wyłączalny, PLAN §14.6).

use chrono::{Datelike, Local};
use cost_meter_contract::{
    BudgetOrigin, CostMeter, LimitMode, MonthlyLimit, TotalsQuery, grosze_to_micro_pln,
};
use sessions_contract::{SessionHistory, SessionId};

use crate::core::AppCore;
use crate::dto::{ContextUsage, CostLimitView, CostSummary, FxView, Money};
use crate::error::AppError;
use crate::ids;
use crate::settings::keys;

/// Okno kontekstu, gdy model go nie podaje.
const DEFAULT_CONTEXT: u64 = 200_000;

impl AppCore {
    fn context_used(&self, session: &SessionId) -> u64 {
        let sessions = &self.inner.sessions;
        let Ok(Some(leaf)) = sessions.active_leaf(session) else {
            return 0;
        };
        sessions
            .branch_projection(session, leaf)
            .map(|path| {
                path.iter()
                    .map(|t| t.content.text.chars().count() as u64)
                    .sum::<u64>()
                    / 4
            })
            .unwrap_or(0)
    }

    /// Podsumowanie kosztów (sesja / dzień / miesiąc w groszach, limit, kontekst, kurs).
    pub(crate) async fn cost_summary(&self, session: Option<&SessionId>) -> CostSummary {
        let costs = &self.inner.costs;
        let today = Local::now().date_naive();
        let total = |q: TotalsQuery| Money::from_micro_pln(costs.totals(&q).micro_pln);
        let budget = costs.budget();
        let rate = costs.current_rate();
        let (used, max) = match session {
            Some(s) => (
                self.context_used(s),
                self.rt()
                    .context_window
                    .get(s)
                    .copied()
                    .unwrap_or(DEFAULT_CONTEXT),
            ),
            None => (0, DEFAULT_CONTEXT),
        };
        CostSummary {
            session: session.map_or(Money::pln(0), |s| {
                total(TotalsQuery::Session { session: s.clone() })
            }),
            day: total(TotalsQuery::Day { day: today }),
            month: total(TotalsQuery::Month {
                month: cost_meter_contract::Month {
                    year: today.year(),
                    month: today.month(),
                },
            }),
            limit: CostLimitView {
                enabled: budget.monthly.mode == LimitMode::Enforced,
                monthly: Money::from_micro_pln(budget.monthly.amount_micro_pln),
            },
            context: ContextUsage {
                used_tokens: used,
                max_tokens: max,
                compacted: false,
            },
            fx: FxView {
                usd_pln: f64::from(rate.rate_e4) / 10_000.0,
                date: rate
                    .effective_date
                    .map_or_else(|| today.to_string(), |d| d.to_string()),
                stale: rate.stale,
            },
        }
    }

    /// `costs_summary`.
    pub async fn costs_summary(&self, session_id: Option<String>) -> Result<CostSummary, AppError> {
        let id = match session_id {
            Some(s) => Some(ids::session(&s)?),
            None => None,
        };
        Ok(self.cost_summary(id.as_ref()).await)
    }

    /// `costs_set_monthly_limit`: wyłączony limit = tylko wskaźnik i alerty.
    pub async fn costs_set_monthly_limit(
        &self,
        enabled: bool,
        monthly: Money,
    ) -> Result<(), AppError> {
        if monthly.currency != crate::dto::Currency::Pln || monthly.minor < 0 {
            return Err(AppError::invalid("Limit podaje się w groszach (PLN, ≥ 0)."));
        }
        let grosze = u64::try_from(monthly.minor).unwrap_or(0);
        let mut budget = self.inner.costs.budget();
        budget.monthly = MonthlyLimit {
            amount_micro_pln: grosze_to_micro_pln(grosze),
            mode: if enabled {
                LimitMode::Enforced
            } else {
                LimitMode::AlertOnly
            },
        };
        self.inner
            .costs
            .set_budget(budget, BudgetOrigin::User)
            .await?;
        self.config_set(keys::COST_LIMIT_ENABLED, Some(enabled.into()), false)
            .await?;
        self.config_set(keys::COST_LIMIT_GROSZE, Some(grosze.into()), false)
            .await?;
        let session = self
            .config_str(keys::ACTIVE_SESSION)
            .await
            .and_then(|s| ids::session(&s).ok());
        let costs = self.cost_summary(session.as_ref()).await;
        self.emit(crate::dto::AlfaEvent::CostsChanged {
            session_id: session.map(|s| s.to_string()).unwrap_or_default(),
            costs,
        });
        Ok(())
    }
}
