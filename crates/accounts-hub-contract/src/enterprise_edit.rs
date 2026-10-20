//! Zmiana ustawień firmowych istniejącego konta (C2.12, wersja 0.0.65) — reguły wspólne dla
//! `accounts-hub-impl` i `accounts-hub-fake`.
//!
//! Kolejność w hubie: [`crate::EnterpriseSettings::checked`] (dostawca i adres konta), potem
//! odmowa, gdy ten sam klucz w tym samym projekcie OpenAI ma już inne konto
//! ([`crate::AccountsHub::find_by_secret`]), na końcu [`apply_enterprise`] na bieżącym wpisie
//! konta, zapis i zdarzenie [`EVENT_ENTERPRISE_CHANGED`] z ładunkiem [`enterprise_payload`].

use std::collections::BTreeSet;

use crate::account::Account;
use crate::api::AccountsError;
use crate::catalog::ModelInfo;
use crate::enterprise::{AZURE_OPENAI, EnterpriseSettings, mask_id};
use crate::ids::ModelId;

/// Zdarzenie: zmieniono ustawienia firmowe konta (identyfikatory tylko skrócone).
pub const EVENT_ENTERPRISE_CHANGED: &str = "accounts.enterprise.changed";

/// Nakłada **sprawdzone** ustawienia na konto. Konto Azure OpenAI dostaje listę modeli z wdrożeń
/// (to samo, co zwraca lista modeli zasobu: model Alfy, nazwa wdrożenia jako nazwa wyświetlana;
/// znane możliwości modelu zostają), a kolejki tracą modele, których wdrożeń już nie ma.
pub fn apply_enterprise(
    account: &mut Account,
    settings: EnterpriseSettings,
) -> Result<(), AccountsError> {
    if account.provider.as_str() == AZURE_OPENAI {
        let mut models = Vec::with_capacity(settings.deployments.len());
        for deployment in &settings.deployments {
            let id = ModelId::new(deployment.alias())?;
            let known = account.models.iter().find(|m| m.id == id);
            models.push(ModelInfo {
                display_name: Some(deployment.name.clone()),
                capabilities: known.and_then(|m| m.capabilities),
                context_window: known.and_then(|m| m.context_window),
                id,
            });
        }
        let present: BTreeSet<ModelId> = models.iter().map(|m| m.id.clone()).collect();
        for queue in account.queues.values_mut() {
            queue.retain(|m| present.contains(m));
        }
        account.queues.retain(|_, queue| !queue.is_empty());
        account.models = models;
    }
    account.enterprise = settings;
    Ok(())
}

/// Ładunek zdarzenia [`EVENT_ENTERPRISE_CHANGED`]: organizacja i projekt skrócone ([`mask_id`]),
/// z wdrożeń tylko liczba.
pub fn enterprise_payload(account: &Account) -> serde_json::Value {
    let e = &account.enterprise;
    serde_json::json!({
        "account": account.id,
        "provider": account.provider,
        "organization": e.organization.as_deref().map(mask_id),
        "project": e.project.as_deref().map(mask_id),
        "api_version": e.api_version,
        "deployments": e.deployments.len(),
    })
}

/// Odmowa: ten sam klucz w tym samym projekcie OpenAI to już konto `label` (osobny projekt =
/// osobny budżet, ten sam projekt = duplikat).
pub fn duplicate_in_project(label: &str) -> AccountsError {
    AccountsError::InvalidInput(format!(
        "ten klucz w tym projekcie OpenAI jest już kontem „{label}” — wybierz inny projekt"
    ))
}
