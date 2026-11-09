//! Adapter trwałej obsady wiązany po odtworzeniu katalogu własnych person i ról.

use std::sync::Arc;

use core_bus_contract::SessionId;
use personas_contract::{CastChange, CastStore, ChangeOrigin, PersonasError, PersonasState};

use crate::PersonasModule;

impl PersonasModule {
    /// Odtwarza istniejące sesje bez ponownej publikacji poleceń i wiąże zapis.
    /// Błąd odczytu lub walidacji nie zmienia stanu ani nie wiąże magazynu.
    pub fn bind_store(&self, store: Arc<dyn CastStore>) -> Result<(), PersonasError> {
        let mut state = self.state();
        let mut slot = self.store.write().unwrap_or_else(|p| p.into_inner());
        if slot.is_some() {
            return Err(PersonasError::Storage("magazyn obsady już związany".into()));
        }
        let mut next = state.clone();
        for (session, cast) in store.load().map_err(PersonasError::Storage)? {
            next.set_cast(&session, cast, ChangeOrigin::Import)?;
        }
        *state = next;
        *slot = Some(store);
        Ok(())
    }

    /// Baza sesji została usunięta; usuwa również ulotną kopię obsady.
    pub fn forget_session(&self, session: &SessionId) {
        self.state().forget_session(session);
    }

    /// Walidacja, zapis, dopiero potem stan widoczny innym wywołaniom.
    pub(super) fn transaction<T>(
        &self,
        f: impl FnOnce(&mut PersonasState) -> Result<(T, Vec<CastChange>), PersonasError>,
    ) -> Result<T, PersonasError> {
        let mut state = self.state();
        let mut next = state.clone();
        let (result, changes) = f(&mut next)?;
        if let Some(store) = self
            .store
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            for change in changes {
                store
                    .save(&change.session, &change.after)
                    .map_err(PersonasError::Storage)?;
            }
        }
        *state = next;
        Ok(result)
    }
}
