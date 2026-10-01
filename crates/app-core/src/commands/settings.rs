//! Komendy `settings_*`: drzewo §15, wartości, zapis z walidacją JSON Schema (`core-config`),
//! reset do domyślnej, nadpisania skrótów (reguła AltGr z §8.6).

use std::collections::BTreeMap;

use crate::core::AppCore;
use crate::dto::{SettingScope, SettingValue, SettingsPageDef};
use crate::error::AppError;
use crate::settings::keys;

/// Litery, których nie wolno łączyć z `Ctrl+Alt(+Shift)` (polski AltGr, PLAN §8.6).
const ALTGR_LETTERS: [char; 9] = ['a', 'c', 'e', 'l', 'n', 'o', 's', 'x', 'z'];

/// Kombinacja zarezerwowana dla kill-switcha.
pub const KILL_SWITCH_CHORD: &str = "Ctrl+Shift+F12";

/// Sprawdza kombinację skrótu: reguła AltGr i zarezerwowany kill-switch.
pub fn validate_chord(chord: &str) -> Result<(), AppError> {
    let parts: Vec<String> = chord.split('+').map(|p| p.trim().to_lowercase()).collect();
    let has = |m: &str| parts.iter().any(|p| p == m);
    if has("ctrl")
        && has("alt")
        && let Some(key) = parts.last()
        && key.chars().count() == 1
        && key.chars().all(|c| ALTGR_LETTERS.contains(&c))
    {
        return Err(AppError::forbidden(format!(
            "Skrót „{chord}” koliduje z polskim AltGr (Ctrl+Alt+{}).",
            key.to_uppercase()
        )));
    }
    if chord.eq_ignore_ascii_case(KILL_SWITCH_CHORD) {
        return Err(AppError::forbidden(
            "Ctrl+Shift+F12 jest zarezerwowany dla STOP WSZYSTKIEGO.",
        ));
    }
    Ok(())
}

impl AppCore {
    /// `settings_schema`.
    pub async fn settings_schema(&self) -> Result<Vec<SettingsPageDef>, AppError> {
        Ok(self.inner.settings.pages().to_vec())
    }

    /// `settings_values`: wartości wynikowe (warstwy: domyślna < wspólna < maszyna).
    pub async fn settings_values(&self) -> Result<BTreeMap<String, SettingValue>, AppError> {
        let mut out = BTreeMap::new();
        for def in self.inner.settings.defs() {
            let value = self
                .config_value(&def.key)
                .await
                .as_ref()
                .and_then(SettingValue::from_json)
                .unwrap_or_else(|| def.default.clone());
            out.insert(def.key.clone(), value);
        }
        Ok(out)
    }

    /// `settings_set`.
    pub async fn settings_set(&self, key: String, value: SettingValue) -> Result<(), AppError> {
        let def = self
            .inner
            .settings
            .def(&key)
            .ok_or_else(|| AppError::invalid(format!("Nieznane ustawienie „{key}”.")))?;
        let machine = def.scope == SettingScope::Machine;
        self.config_set(&key, Some(value.to_json()), machine).await
    }

    /// `settings_reset`: usuwa nadpisania i zwraca wartość domyślną.
    pub async fn settings_reset(&self, key: String) -> Result<SettingValue, AppError> {
        let def = self
            .inner
            .settings
            .def(&key)
            .cloned()
            .ok_or_else(|| AppError::invalid(format!("Nieznane ustawienie „{key}”.")))?;
        self.config_set(&key, None, true).await?;
        self.config_set(&key, None, false).await?;
        Ok(def.default)
    }

    /// `settings_set_shortcut`: `None` = domyślny, `""` = wyłączony.
    pub async fn settings_set_shortcut(
        &self,
        action_id: String,
        chord: Option<String>,
    ) -> Result<(), AppError> {
        if action_id.trim().is_empty() || action_id.len() > 64 {
            return Err(AppError::invalid(
                "Nieprawidłowy identyfikator akcji skrótu.",
            ));
        }
        let mut overrides = self.shortcut_overrides().await;
        match chord {
            None => {
                overrides.remove(&action_id);
            }
            Some(chord) => {
                if !chord.is_empty() {
                    validate_chord(&chord)?;
                }
                overrides.insert(action_id, chord);
            }
        }
        let text = serde_json::to_string(&overrides).map_err(AppError::internal)?;
        self.config_set(keys::SHORTCUTS, Some(text.into()), false)
            .await
    }
}
