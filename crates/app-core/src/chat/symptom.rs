//! Symptomy dostawców dla Diagnosty: podsłuch strumienia dostawcy tury (`app_health::symptom_tap`;
//! błąd HTTP → `diagnostics.symptom` bez treści rozmowy).

use crate::core::AppCore;
use crate::ports::BrainChoice;

/// Opakowuje dostawcę wybranego dla tury podsłuchem symptomów (wybór Routera bez zmian).
pub(crate) fn tap(core: &AppCore, choice: BrainChoice) -> BrainChoice {
    app_health::symptom_tap(choice, &core.inner.bus)
}
