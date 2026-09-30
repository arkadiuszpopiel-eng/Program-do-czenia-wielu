//! Walidacja manifestu i wspólne błędy.

use crate::manifest::ModuleManifest;

/// Błędy parsowania i walidacji manifestu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Błąd składni TOML lub brak wymaganego pola.
    #[error("błąd składni manifestu: {0}")]
    Syntax(String),
    /// `id` nie jest w kebab-case.
    #[error("id modułu `{0}` nie jest w kebab-case")]
    InvalidId(String),
    /// Referencja kontraktu w złym formacie.
    #[error("referencja kontraktu `{0}` nie ma formatu `<nazwa>-contract@<major>`")]
    InvalidContractRef(String),
    /// Zdolność w złym formacie.
    #[error("zdolność `{0}` nie ma formatu `<nazwa>` lub `<nazwa>(<zakres>)`")]
    InvalidCapability(String),
    /// Moduł wymaga kontraktu, który sam dostarcza.
    #[error("moduł dostarcza i jednocześnie wymaga `{0}`")]
    SelfDependency(String),
    /// Budżet poza zakresem.
    #[error("budżet nieprawidłowy: {0}")]
    InvalidBudget(String),
    /// Health-check poza zakresem.
    #[error("health-check nieprawidłowy: {0}")]
    InvalidHealth(String),
    /// Zduplikowana pozycja w liście.
    #[error("duplikat `{0}` w polu `{1}`")]
    Duplicate(String, &'static str),
}

/// Czy tekst jest w kebab-case: `[a-z0-9]+(-[a-z0-9]+)*`.
pub fn is_kebab_case(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn check_unique<T: ToString>(items: &[T], field: &'static str) -> Result<(), ManifestError> {
    let mut seen = std::collections::BTreeSet::new();
    for item in items {
        let text = item.to_string();
        if !seen.insert(text.clone()) {
            return Err(ManifestError::Duplicate(text, field));
        }
    }
    Ok(())
}

/// Reguły semantyczne (poza tym, co wymusza serde): duplikaty, self-dependency, zakresy.
pub fn validate(manifest: &ModuleManifest) -> Result<(), ManifestError> {
    check_unique(&manifest.provides, "provides")?;
    check_unique(&manifest.requires, "requires")?;
    check_unique(&manifest.capabilities, "capabilities")?;
    if let Some(dup) = manifest
        .provides
        .iter()
        .find(|p| manifest.requires.contains(p))
    {
        return Err(ManifestError::SelfDependency(dup.to_string()));
    }
    if manifest.budget.ram_mb == 0 {
        return Err(ManifestError::InvalidBudget("ram_mb musi być > 0".into()));
    }
    if manifest.budget.cpu_pct > 100 {
        return Err(ManifestError::InvalidBudget(
            "cpu_pct musi być ≤ 100".into(),
        ));
    }
    if manifest.health.interval_s == 0 {
        return Err(ManifestError::InvalidHealth(
            "interval_s musi być > 0".into(),
        ));
    }
    Ok(())
}
