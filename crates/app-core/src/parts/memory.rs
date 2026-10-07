//! Pamięć F7 w kompozycji: `MemoryModule` (bazy zakresów w `%LOCALAPPDATA%\Alfa\memory`, klucze
//! w sejfie), Strażniczka pamięci (model lokalny przez lokalny rdzeń Routera, budżet tła,
//! licznik bezczynności „nigdy bezczynny" do czasu portu platformy) i `MemoryApp` dla komend.

use std::sync::Arc;

use app_memory::guardian::{self, GuardianDeps, UnknownIdle, keys};
use app_memory::{MemoryApp, MemoryAppParts, RoleAccess};
use core_bus_contract::EventBus;
use core_config_contract::{ConfigKey, ConfigStore, Scope};
use memory_contract::{MemoryService, PrivacyOracle};
use providers_contract::{HealthState, ModelProvider};
use router_impl::RoutedProvider;
use sessions_contract::SessionCatalog;

use super::{Built, Kernel, need};
use crate::compose::{HealthSlot, started};
use crate::error::AppError;
use crate::options::AppPaths;
use app_modules::secrets::StoreKeyVault;

pub(super) async fn setting(kernel: &Kernel, key: &str) -> Option<serde_json::Value> {
    let key = ConfigKey::new(key).ok()?;
    kernel.config.get(&key, &Scope::Global).await.ok().flatten()
}

impl Built {
    /// Moduł pamięci F7 (zakres sesji w bazach sesji, reszta w bazach zakresów).
    pub(super) async fn build_memory(
        &mut self,
        paths: &AppPaths,
        kernel: &Kernel,
        bus: &Arc<dyn EventBus>,
        slot: Option<&HealthSlot>,
    ) -> Result<(), AppError> {
        let sessions = need(&self.sessions, "sessions")?;
        let search = need(&self.search, "search")?;
        let vault = Arc::new(StoreKeyVault::new(kernel.secrets.clone()));
        let (module, privacy) = app_memory::memory_module(paths.memory(), vault, sessions, search)?;
        self.memory = Some(started(module, bus, slot).await?);
        self.privacy = Some(privacy);
        Ok(())
    }

    /// Strażniczka pamięci (po Routerze — lokalny rdzeń jako model porządkowania).
    pub(super) async fn build_consolidation(
        &mut self,
        kernel: &Kernel,
        bus: &Arc<dyn EventBus>,
        slot: Option<&HealthSlot>,
    ) -> Result<(), AppError> {
        let memory: Arc<dyn MemoryService> = need(&self.memory, "memory")?.service();
        let privacy = self
            .privacy
            .clone()
            .ok_or_else(|| AppError::internal("brak prywatności pamięci"))?;
        let device = kernel
            .device
            .clone()
            .ok_or_else(|| AppError::internal("brak profilu urządzenia"))?;
        let model = self.extra.routers.as_ref().map(|r| {
            let routed =
                RoutedProvider::new(r.local.clone(), router_contract::TaskClass::Summarize)
                    .background(true);
            (
                Arc::new(routed) as Arc<dyn ModelProvider>,
                router_contract::AUTO_MODEL.to_owned(),
            )
        });
        let enabled = setting(kernel, keys::CONSOLIDATION).await;
        let extract = setting(kernel, keys::AUTO_EXTRACT).await;
        let window = setting(kernel, keys::WINDOW).await;
        let module = guardian::module(GuardianDeps {
            memory,
            privacy,
            device,
            idle: match &kernel.signals {
                Some(port) => Arc::new(super::signals::PortIdle(port.clone())),
                None => Arc::new(UnknownIdle),
            },
            meter: need(&self.costs, "cost-meter")?,
            model,
            bus: bus.clone(),
            config: guardian::config(
                enabled.and_then(|v| v.as_bool()).unwrap_or(true),
                extract.as_ref().and_then(|v| v.as_str()),
                window.as_ref().and_then(|v| v.as_str()),
            ),
            interval: memory_consolidation_impl::DEFAULT_INTERVAL,
        })?;
        self.consolidation = Some(started(module, bus, slot).await?);
        Ok(())
    }

    /// Pamięć aplikacji (Inspektor, narzędzia, kontekst czatu).
    pub(super) async fn memory_app(&self, kernel: &Kernel) -> Result<Arc<MemoryApp>, AppError> {
        let sessions = need(&self.sessions, "sessions")?;
        let catalog: Arc<dyn SessionCatalog> = sessions;
        let personas = need(&self.personas, "personas")?;
        let privacy: Arc<dyn PrivacyOracle> = self
            .privacy
            .clone()
            .ok_or_else(|| AppError::internal("brak prywatności pamięci"))?;
        let local = self.extra.routers.as_ref().map(|r| r.local.clone());
        let window = setting(kernel, keys::WINDOW).await;
        let enabled = setting(kernel, keys::CONSOLIDATION).await;
        Ok(Arc::new(MemoryApp::new(MemoryAppParts {
            service: need(&self.memory, "memory")?.service(),
            privacy,
            access: Arc::new(RoleAccess::new(personas, catalog.clone())),
            catalog,
            events: kernel.events.clone(),
            guardian: self.consolidation.clone(),
            model_available: Arc::new(move || {
                local.as_ref().is_some_and(|core| {
                    core.all_registered()
                        .iter()
                        .any(|(_, r)| r.provider.health().state != HealthState::Unconfigured)
                })
            }),
            enabled: enabled.and_then(|v| v.as_bool()).unwrap_or(true),
            window: window
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| guardian::DEFAULT_WINDOW.to_owned()),
        })))
    }
}
