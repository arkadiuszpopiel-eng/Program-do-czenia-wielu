//! Kreator „Dodaj dostawcę / konto / klucz” jako czysta maszyna stanów (bez UI i bez I/O).
//! Kolejność: dostawca → klucz → test połączenia → wykrycie modeli → przypisania → limit → potwierdzenie.
//! Niezmiennik: `finish` jest możliwe tylko po udanym teście aktualnie wpisanego klucza.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use compliance_contract::ProviderApiStatus;

use crate::account::{AccountSource, Assignments, ConnectionReport, CostLimit, NewAccount};
use crate::catalog::{AuthKind, ModelInfo, ProviderCatalogEntry, validate_base_url};
use crate::ids::ProviderId;
use crate::probe::ModelListError;
use crate::secret::SecretString;

/// Krok kreatora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WizardStep {
    /// Wybór dostawcy.
    ChooseProvider,
    /// Wklejenie klucza (i endpointu, jeśli katalog go nie zna).
    EnterKey,
    /// Test połączenia.
    TestConnection,
    /// Wykrycie modeli.
    DiscoverModels,
    /// Przypisanie do klas zadań, agentek i głosu.
    Assign,
    /// Limit kosztów.
    CostLimit,
    /// Podsumowanie przed zapisem.
    Confirm,
}

/// Ostrzeżenia pokazywane w kreatorze.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "warning", rename_all = "snake_case")]
pub enum WizardWarning {
    /// Dostawca ma status „szary” w katalogu.
    GreyProvider,
    /// Wpis katalogu niezweryfikowany.
    UnverifiedProvider,
    /// Nie udało się wykryć modeli — identyfikator wpisze użytkownik.
    ModelsNotDiscovered {
        /// Powód.
        reason: String,
    },
}

/// Błędy kreatora.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum WizardError {
    /// Operacja w złym kroku.
    #[error("operacja niedozwolona w kroku {actual:?} (oczekiwano {expected:?})")]
    WrongStep {
        /// Oczekiwany krok.
        expected: WizardStep,
        /// Bieżący krok.
        actual: WizardStep,
    },
    /// Dostawca zabroniony w rejestrze/katalogu.
    #[error("dostawca `{0}` jest zabroniony")]
    ForbiddenProvider(ProviderId),
    /// Uwierzytelnienie przez logowanie w CLI obsługuje krok „mosty CLI” (F4), nie ten kreator.
    #[error("dostawca `{0}` wymaga logowania w oficjalnym CLI (krok „mosty CLI”)")]
    CliLoginRequired(ProviderId),
    /// Klucz pusty lub w złym formacie.
    #[error("klucz jest pusty albo zawiera niedozwolone znaki")]
    InvalidKey,
    /// Brak lub zły endpoint.
    #[error("{0}")]
    InvalidBaseUrl(String),
    /// Test klucza nie przeszedł.
    #[error("test połączenia nie przeszedł — popraw klucz")]
    TestNotPassed,
}

/// Wynik kreatora gotowy do zapisu przez hub.
#[derive(Debug, Clone)]
pub struct WizardOutcome {
    /// Dane konta (z kluczem).
    pub account: NewAccount,
    /// Wykryte modele.
    pub models: Vec<ModelInfo>,
    /// Raport udanego testu.
    pub test: ConnectionReport,
}

/// Stan kreatora. `Debug` nie ujawnia klucza (`SecretString` jest zredagowany).
#[derive(Debug, Clone)]
pub struct Wizard {
    step: WizardStep,
    provider: Option<ProviderCatalogEntry>,
    label: String,
    secret: Option<SecretString>,
    base_url: Option<String>,
    last_test: Option<ConnectionReport>,
    models: Vec<ModelInfo>,
    assignments: Assignments,
    cost_limit: Option<CostLimit>,
    warnings: Vec<WizardWarning>,
}

impl Default for Wizard {
    fn default() -> Self {
        Self::new()
    }
}

impl Wizard {
    /// Nowy kreator w kroku wyboru dostawcy.
    pub fn new() -> Self {
        Self {
            step: WizardStep::ChooseProvider,
            provider: None,
            label: String::new(),
            secret: None,
            base_url: None,
            last_test: None,
            models: Vec::new(),
            assignments: Assignments::default(),
            cost_limit: None,
            warnings: Vec::new(),
        }
    }

    /// Bieżący krok.
    pub fn step(&self) -> WizardStep {
        self.step
    }

    /// Wybrany dostawca.
    pub fn provider(&self) -> Option<&ProviderCatalogEntry> {
        self.provider.as_ref()
    }

    /// Klucz do testu (tylko dla huba).
    pub fn secret(&self) -> Option<&SecretString> {
        self.secret.as_ref()
    }

    /// Endpoint: podany przez użytkownika albo z katalogu.
    pub fn base_url(&self) -> Option<&str> {
        self.base_url
            .as_deref()
            .or_else(|| self.provider.as_ref().and_then(|p| p.base_url.as_deref()))
    }

    /// Ostatni raport testu.
    pub fn last_test(&self) -> Option<&ConnectionReport> {
        self.last_test.as_ref()
    }

    /// Wykryte modele.
    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    /// Ostrzeżenia.
    pub fn warnings(&self) -> &[WizardWarning] {
        &self.warnings
    }

    fn require_step(&self, expected: WizardStep) -> Result<(), WizardError> {
        if self.step == expected {
            Ok(())
        } else {
            Err(WizardError::WrongStep {
                expected,
                actual: self.step,
            })
        }
    }

    /// Wybór dostawcy. Zabroniony → błąd; szary/niezweryfikowany → ostrzeżenie.
    pub fn choose_provider(&mut self, entry: ProviderCatalogEntry) -> Result<(), WizardError> {
        self.require_step(WizardStep::ChooseProvider)?;
        if entry.compliance_status == ProviderApiStatus::Forbidden {
            return Err(WizardError::ForbiddenProvider(entry.id));
        }
        if entry.auth == AuthKind::OauthCli {
            return Err(WizardError::CliLoginRequired(entry.id));
        }
        self.warnings = match entry.compliance_status {
            ProviderApiStatus::Gray => vec![WizardWarning::GreyProvider],
            ProviderApiStatus::Unverified => vec![WizardWarning::UnverifiedProvider],
            ProviderApiStatus::Green | ProviderApiStatus::Forbidden => Vec::new(),
        };
        self.label = entry.display_name.clone();
        self.provider = Some(entry);
        self.step = WizardStep::EnterKey;
        Ok(())
    }

    /// Klucz, etykieta i (opcjonalnie) endpoint. Każda zmiana klucza unieważnia wcześniejszy test.
    pub fn enter_key(
        &mut self,
        secret: SecretString,
        label: &str,
        base_url: Option<String>,
    ) -> Result<(), WizardError> {
        self.require_step(WizardStep::EnterKey)?;
        let provider = self.provider.as_ref().ok_or(WizardError::WrongStep {
            expected: WizardStep::ChooseProvider,
            actual: self.step,
        })?;
        let needs_key = provider.auth == AuthKind::ApiKey;
        if needs_key && !secret.is_plausible_key() {
            return Err(WizardError::InvalidKey);
        }
        let base_url = base_url.filter(|u| !u.trim().is_empty());
        match (&base_url, &provider.base_url) {
            (Some(url), _) => validate_base_url(url).map_err(WizardError::InvalidBaseUrl)?,
            (None, None) => {
                return Err(WizardError::InvalidBaseUrl(
                    "katalog nie zna endpointu tego dostawcy — podaj go".into(),
                ));
            }
            (None, Some(_)) => {}
        }
        if !label.trim().is_empty() {
            label.trim().clone_into(&mut self.label);
        }
        self.secret = Some(secret);
        self.base_url = base_url;
        self.last_test = None;
        self.step = WizardStep::TestConnection;
        Ok(())
    }

    /// Zapisuje wynik testu: udany → wykrywanie modeli, nieudany → powrót do klucza.
    pub fn record_test(&mut self, report: ConnectionReport) -> Result<(), WizardError> {
        self.require_step(WizardStep::TestConnection)?;
        self.step = if report.outcome.key_works() {
            WizardStep::DiscoverModels
        } else {
            WizardStep::EnterKey
        };
        self.last_test = Some(report);
        Ok(())
    }

    /// Zapisuje wynik wykrywania modeli (błąd nie blokuje — ostrzeżenie i ręczny model).
    pub fn record_models(
        &mut self,
        result: Result<Vec<ModelInfo>, ModelListError>,
    ) -> Result<(), WizardError> {
        self.require_step(WizardStep::DiscoverModels)?;
        match result {
            Ok(models) => self.models = models,
            Err(e) => {
                self.models.clear();
                self.warnings.push(WizardWarning::ModelsNotDiscovered {
                    reason: e.to_string(),
                });
            }
        }
        self.step = WizardStep::Assign;
        Ok(())
    }

    /// Przypisania.
    pub fn assign(&mut self, assignments: Assignments) -> Result<(), WizardError> {
        self.require_step(WizardStep::Assign)?;
        self.assignments = assignments;
        self.step = WizardStep::CostLimit;
        Ok(())
    }

    /// Limit kosztów (`None` = bez limitu dla tego dostawcy).
    pub fn set_cost_limit(&mut self, limit: Option<CostLimit>) -> Result<(), WizardError> {
        self.require_step(WizardStep::CostLimit)?;
        self.cost_limit = limit;
        self.step = WizardStep::Confirm;
        Ok(())
    }

    /// Krok wstecz. Powrót do klucza albo dostawcy unieważnia test.
    pub fn back(&mut self) {
        self.step = match self.step {
            WizardStep::ChooseProvider | WizardStep::EnterKey => {
                *self = Self::new();
                WizardStep::ChooseProvider
            }
            WizardStep::TestConnection | WizardStep::DiscoverModels => {
                self.last_test = None;
                WizardStep::EnterKey
            }
            WizardStep::Assign => WizardStep::DiscoverModels,
            WizardStep::CostLimit => WizardStep::Assign,
            WizardStep::Confirm => WizardStep::CostLimit,
        };
    }

    /// Kończy kreator: wymaga kroku potwierdzenia i udanego testu bieżącego klucza.
    pub fn finish(self) -> Result<WizardOutcome, WizardError> {
        self.require_step(WizardStep::Confirm)?;
        let test = self
            .last_test
            .filter(|t| t.outcome.key_works())
            .ok_or(WizardError::TestNotPassed)?;
        let provider = self.provider.ok_or(WizardError::TestNotPassed)?;
        let secret = self.secret.ok_or(WizardError::InvalidKey)?;
        Ok(WizardOutcome {
            account: NewAccount {
                provider: provider.id,
                label: self.label,
                secret,
                base_url: self.base_url,
                assignments: self.assignments,
                cost_limit: self.cost_limit,
                source: AccountSource::Wizard,
            },
            models: self.models,
            test,
        })
    }
}
