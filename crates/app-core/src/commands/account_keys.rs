//! Hub kont — operacje na kluczu istniejącego konta: raport testu, wymiana klucza, wyłączanie,
//! model wpisany ręcznie. (Dodawanie i lista: `accounts.rs`.)

use accounts_hub_contract::{AccountId, AccountsHub, SecretString, TestSummary};

use super::accounts::{NOT_A_KEY, account_dto};
use crate::core::AppCore;
use crate::dto::{Account, SecretInput, TestReport};
use crate::error::AppError;

impl AppCore {
    /// Raport dla interfejsu: opis po ludzku, a szczegóły dostawcy osobno (`detail`), żeby „401”
    /// nie udawało diagnozy.
    pub(super) fn report(&self, id: &AccountId, summary: &TestSummary) -> TestReport {
        use accounts_hub_contract::TestOutcome as O;
        let error = match &summary.outcome {
            O::Ok | O::RateLimited | O::Restricted { .. } => None,
            O::InvalidKey => Some("Dostawca odrzucił ten klucz.".to_owned()),
            O::NoCredit { .. } => Some(
                "Klucz jest poprawny, ale konto u dostawcy nie ma środków albo aktywnych płatności."
                    .to_owned(),
            ),
            O::Refused { .. } => Some(
                "Dostawca odmówił dostępu z powodu innego niż sam klucz (kraj, wyłączone API albo zapora)."
                    .to_owned(),
            ),
            O::Timeout => Some("Dostawca nie odpowiedział w wyznaczonym czasie.".to_owned()),
            O::Network { message } | O::Unsupported { message } => Some(message.clone()),
        };
        let detail = summary.detail.as_ref();
        TestReport {
            ok: summary.outcome.key_works(),
            latency_ms: summary.latency_ms,
            models: self
                .account(id)
                .map(|a| account_dto(&a).models)
                .unwrap_or_default(),
            error,
            reason: (summary.outcome != O::Ok).then(|| summary.outcome.code().to_owned()),
            status: detail.and_then(|d| d.status),
            provider_code: detail.and_then(|d| d.code.clone()),
            detail: detail.map(|d| d.message.clone()),
        }
    }

    /// `accounts_rotate`: wymiana klucza istniejącego konta. Nowy klucz jest najpierw sprawdzany
    /// u dostawcy i trafia do Credential Managera tylko wtedy, gdy działa — przy nieudanym teście
    /// (`ok: false`) konto i stary klucz zostają bez zmian. Przypisania, limit i koszty zostają.
    pub async fn accounts_rotate(
        &self,
        account_id: String,
        secret: SecretInput,
    ) -> Result<TestReport, AppError> {
        let id = super::accounts::account_id(&account_id)?;
        let current = self.account(&id)?;
        let secret = SecretString::from_input(secret.expose());
        if !secret.is_plausible_key() {
            return Err(AppError::invalid(NOT_A_KEY));
        }
        let twin =
            self.inner
                .hub
                .find_by_secret(&current.provider, current.base_url.as_deref(), &secret);
        if let Some(existing) = twin.filter(|existing| *existing != id) {
            let label = self
                .inner
                .hub
                .account(&existing)
                .map_or_else(|| existing.to_string(), |a| a.label);
            return Err(AppError::invalid(format!(
                "Ten klucz jest już używany przez konto „{label}”."
            )));
        }
        let summary = self.inner.hub.try_rotate(&id, secret).await?;
        self.announce_account(&id).await;
        Ok(self.report(&id, &summary))
    }

    /// `accounts_set_disabled`: wyłączone konto zostaje na liście z kluczem, ale router go nie
    /// używa; włączenie przywraca stan z ostatniego testu.
    pub async fn accounts_set_disabled(
        &self,
        account_id: String,
        disabled: bool,
    ) -> Result<Account, AppError> {
        let id = super::accounts::account_id(&account_id)?;
        self.inner.hub.set_disabled(&id, disabled).await?;
        self.announce_account(&id).await;
        Ok(account_dto(&self.account(&id)?))
    }

    /// `accounts_set_model`: model wpisany ręcznie — dla klucza bez prawa do listy modeli. Bez
    /// modelu Router nie ma czego wywołać, a kreator obiecywałby działające konto.
    pub async fn accounts_set_model(
        &self,
        account_id: String,
        model: String,
    ) -> Result<Account, AppError> {
        let id = super::accounts::account_id(&account_id)?;
        let model = accounts_hub_contract::ModelId::new(model.trim())
            .map_err(|_| AppError::invalid("Podaj nazwę modelu, np. gpt-4o-mini."))?;
        self.inner.hub.set_manual_model(&id, model).await?;
        self.announce_account(&id).await;
        Ok(account_dto(&self.account(&id)?))
    }

    /// `accounts_set_queue`: kolejka modeli konta dla klasy zadań (kolejność prób Routera,
    /// najwyżej 10). Pusta lista przywraca wybór automatyczny. Model, który do klasy się nie
    /// nadaje (np. synteza mowy w rozmowie), jest odrzucany z wyjaśnieniem.
    pub async fn accounts_set_queue(
        &self,
        account_id: String,
        task_class: String,
        models: Vec<String>,
    ) -> Result<Account, AppError> {
        let id = super::accounts::account_id(&account_id)?;
        let class = super::accounts::task_from_name(&task_class)?;
        let models = models
            .iter()
            .map(|m| accounts_hub_contract::ModelId::new(m.trim()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                AppError::invalid("Nazwa modelu w kolejce jest pusta albo niepoprawna.")
            })?;
        self.inner.hub.set_model_queue(&id, class, models).await?;
        self.announce_account(&id).await;
        Ok(account_dto(&self.account(&id)?))
    }

    /// `accounts_suggest_queue`: propozycja kolejki z modeli konta dla klasy zadań i strategii
    /// (`quality`, `balanced`, `economy`). Niczego nie zapisuje.
    pub async fn accounts_suggest_queue(
        &self,
        account_id: String,
        task_class: String,
        strategy: String,
    ) -> Result<Vec<String>, AppError> {
        use accounts_hub_contract::QueueStrategy;
        let id = super::accounts::account_id(&account_id)?;
        let class = super::accounts::task_from_name(&task_class)?;
        let strategy = match strategy.as_str() {
            "quality" => QueueStrategy::Quality,
            "balanced" => QueueStrategy::Balanced,
            "economy" => QueueStrategy::Economy,
            other => return Err(AppError::invalid(format!("Nieznana strategia „{other}”."))),
        };
        let account = self.account(&id)?;
        let ids = account.models.iter().map(|m| m.id.as_str());
        Ok(accounts_hub_contract::suggest_queue(ids, class, strategy)
            .into_iter()
            .map(str::to_owned)
            .collect())
    }
}
