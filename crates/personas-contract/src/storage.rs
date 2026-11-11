//! Port trwałej obsady; adapter aplikacji przechowuje ją w szyfrowanej bazie sesji.

use std::collections::BTreeMap;

use core_bus_contract::SessionId;

use crate::Cast;

/// Zapis właścicielskiej obsady, bez uprawnień i bez uruchamiania mikrofonu.
pub trait CastStore: Send + Sync {
    /// Ostatnie obsady istniejących sesji, także w archiwum i cofalnym koszu.
    /// Sesje ostatecznie usunięte nie mogą zostać zwrócone.
    fn load(&self) -> Result<BTreeMap<SessionId, Cast>, String>;

    /// Trwały zapis jednej sesji przed potwierdzeniem zmiany i publikacją zdarzeń.
    fn save(&self, session: &SessionId, cast: &Cast) -> Result<(), String>;
}
