//! Hub kont — operacje na kluczu istniejącego konta: raport testu, wymiana klucza, wyłączanie,
//! model wpisany ręcznie. (Dodawanie i lista: `accounts.rs`.)

use accounts_hub_contract::{AccountId, AccountsHub, SecretString, TestSummary};

use super::accounts::{NOT_A_KEY, account_dto};
use crate::core::AppCore;
use crate::dto::{Account, EnvImportKind, EnvImportResult, SecretInput, TestReport};
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

    /// `accounts_rotate`: wymiana klucza (najpierw test u dostawcy). Zapis tylko dla klucza rozpoznanego
    /// albo niesprawdzalnego z góry (`accepts_new_key`, także przy `ok: false`: brak środków, „unsupported”);
    /// inaczej konto i stary klucz zostają. Przypisania, limit i koszty zostają zawsze.
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
        let (hub, url) = (&self.inner.hub, current.base_url.as_deref());
        let twin = hub.find_by_secret(&current.provider, url, &current.enterprise, &secret);
        if let Some(existing) = twin.filter(|existing| *existing != id) {
            let label = hub
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

    /// `accounts_set_enterprise`: ustawienia dla firm istniejącego konta (organizacja i projekt
    /// OpenAI albo wersja API i wdrożenia Azure OpenAI; adres zasobu Azure bez zmian).
    pub async fn accounts_set_enterprise(
        &self,
        account_id: String,
        enterprise: crate::dto::EnterpriseSettings,
    ) -> Result<Account, AppError> {
        let account = app_models::accounts::enterprise::set_enterprise(
            self.inner.hub.as_ref(),
            &account_id,
            enterprise,
        )
        .await?;
        self.announce_account(&account.id).await;
        Ok(account_dto(&account))
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

    /// `accounts_import_env`: import kluczy ze zmiennych środowiskowych wymienionych w katalogu
    /// dostawców (np. `OPENAI_API_KEY`). Wyłącznie na życzenie użytkownika; znaleziony klucz
    /// trafia do sejfu, a raport zawiera tylko nazwy zmiennych.
    pub async fn accounts_import_env(&self) -> Result<Vec<EnvImportResult>, AppError> {
        self.import_env_from(&accounts_hub_impl::ProcessEnv).await
    }

    /// Import z podanego źródła zmiennych (testy podają własne).
    pub async fn import_env_from(
        &self,
        env: &dyn accounts_hub_contract::EnvSource,
    ) -> Result<Vec<EnvImportResult>, AppError> {
        use accounts_hub_contract::EnvImportOutcome as O;
        let report = self.inner.hub.import_from_env(env).await?;
        let mut out = Vec::with_capacity(report.len());
        for item in report {
            let (result, account) = match item.outcome {
                O::Imported { account } => (EnvImportKind::Imported, Some(account)),
                O::AlreadyPresent { account } => (EnvImportKind::AlreadyPresent, Some(account)),
                O::NotSet => (EnvImportKind::NotSet, None),
            };
            if let (EnvImportKind::Imported, Some(id)) = (result, &account) {
                self.announce_account(id).await;
            }
            let provider_name = self
                .inner
                .hub
                .provider(&item.provider)
                .map_or_else(|| item.provider.to_string(), |p| p.display_name.clone());
            out.push(EnvImportResult {
                provider: item.provider.to_string(),
                provider_name,
                var: item.var,
                result,
                account_id: account.map(|a| a.to_string()),
            });
        }
        Ok(out)
    }
}
