//! Limity kont z kreatora (`accounts-hub`, np. 100 zł/mies.) jako limity dostawców budżetu.
//! Przegląd 2026-10-22: wiersz konta pokazywał limit, ale nic go nie przepisywało do
//! `BudgetConfig::providers`, więc nie było ani alertu, ani blokady.

use std::collections::BTreeMap;

use accounts_hub_contract::{Account, AccountState, CostLimit, ProviderId};

use crate::api::{CostError, CostMeter};
use crate::budget::BudgetOrigin;

/// Limity dostawców z kont: włączone konta jednego dostawcy sumowane (budżet liczy wydatki per
/// dostawca). Dostawca dostaje limit tylko wtedy, gdy każde jego konto ma limit; blokuje tylko,
/// gdy blokują wszystkie (konto z samym ostrzeżeniem zostawia dostawcę przy ostrzeżeniach).
pub fn provider_limits<'a>(
    accounts: impl IntoIterator<Item = &'a Account>,
) -> BTreeMap<ProviderId, CostLimit> {
    sum_limits(
        accounts
            .into_iter()
            .filter(|a| a.state != AccountState::Disabled)
            .map(|a| (a.provider.clone(), a.cost_limit)),
    )
}

fn sum_limits(
    accounts: impl IntoIterator<Item = (ProviderId, Option<CostLimit>)>,
) -> BTreeMap<ProviderId, CostLimit> {
    let mut sums: BTreeMap<ProviderId, Option<CostLimit>> = BTreeMap::new();
    for (provider, limit) in accounts {
        let entry = sums.entry(provider).or_insert(Some(CostLimit {
            enabled: true,
            monthly_limit_grosze: 0,
        }));
        *entry = match (*entry, limit) {
            (Some(sum), Some(limit)) => Some(CostLimit {
                enabled: sum.enabled && limit.enabled,
                monthly_limit_grosze: sum
                    .monthly_limit_grosze
                    .saturating_add(limit.monthly_limit_grosze),
            }),
            _ => None,
        };
    }
    sums.into_iter()
        .filter_map(|(provider, limit)| limit.map(|l| (provider, l)))
        .collect()
}

/// Przepisuje limity kont do budżetu (zmiana tylko, gdy są inne niż obecne).
pub async fn apply_account_limits(
    meter: &dyn CostMeter,
    accounts: &[Account],
) -> Result<(), CostError> {
    let mut budget = meter.budget();
    let providers = provider_limits(accounts);
    if budget.providers == providers {
        return Ok(());
    }
    budget.providers = providers;
    meter.set_budget(budget, BudgetOrigin::User).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(enabled: bool, zl: u64) -> Option<CostLimit> {
        Some(CostLimit {
            enabled,
            monthly_limit_grosze: zl * 100,
        })
    }

    #[test]
    fn account_limits_become_provider_limits() {
        let p = |s: &str| ProviderId::new(s).unwrap();
        let out = sum_limits([
            (p("anthropic"), limit(true, 100)),
            (p("anthropic"), limit(true, 50)),
            (p("openai"), limit(true, 100)),
            (p("openai"), limit(false, 20)),
            (p("deepseek"), limit(true, 10)),
            (p("deepseek"), None),
        ]);
        assert_eq!(out[&p("anthropic")], limit(true, 150).unwrap());
        // Konto z samym ostrzeżeniem — dostawca bez blokady, suma do alertów.
        assert_eq!(out[&p("openai")], limit(false, 120).unwrap());
        // Konto bez limitu — dostawca bez limitu.
        assert!(!out.contains_key(&p("deepseek")));
    }
}
