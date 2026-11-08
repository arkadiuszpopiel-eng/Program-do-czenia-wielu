//! Atrapa `RegistryPort`: drzewo kluczy w pamięci, ta sama deny-lista i redakcja co
//! implementacja (`check_key` przed dostępem, `guard_listing`, `RegValue::guarded`). Licznik
//! „surowych” dostępów do kluczy z sekretami jest dowodem, że strażnik działa przed odczytem.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use platform_apps_contract::{
    RegData, RegKey, RegListing, RegValue, RegistryError, RegistryPort, check_key, guard_listing,
};

#[derive(Debug, Default)]
struct State {
    /// Klucz (postać tekstowa małymi literami) → (postać oryginalna, wartości).
    keys: BTreeMap<String, (String, Vec<RegValue>)>,
    raw_secret_reads: u32,
    reads: u32,
}

/// Atrapa rejestru.
#[derive(Debug, Default)]
pub struct FakeRegistry {
    state: Mutex<State>,
}

impl FakeRegistry {
    /// Pusty rejestr.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Ustawia wartość (tworzy klucz i przodków). Niepoprawny klucz jest pomijany.
    pub fn set(&self, key: &str, name: &str, data: RegData) {
        let Ok(k) = RegKey::parse(key) else {
            return;
        };
        let mut s = self.lock();
        let mut path = k.hive().short().to_owned();
        s.keys
            .entry(path.to_lowercase())
            .or_insert_with(|| (path.clone(), Vec::new()));
        for seg in k.segments() {
            path = format!("{path}\\{seg}");
            s.keys
                .entry(path.to_lowercase())
                .or_insert_with(|| (path.clone(), Vec::new()));
        }
        if let Some((_, values)) = s.keys.get_mut(&path.to_lowercase()) {
            values.retain(|v| !v.name.eq_ignore_ascii_case(name));
            values.push(RegValue {
                name: name.to_owned(),
                data,
            });
        }
    }

    /// Ile razy dane klucza z sekretami zostały odczytane z magazynu (musi być 0).
    pub fn raw_secret_reads(&self) -> u32 {
        self.lock().raw_secret_reads
    }

    /// Ile odczytów dotarło do magazynu.
    pub fn reads(&self) -> u32 {
        self.lock().reads
    }

    /// Surowy dostęp do magazynu (po strażniku).
    fn raw(&self, key: &RegKey) -> Result<(Vec<String>, Vec<RegValue>), RegistryError> {
        let mut s = self.lock();
        s.reads += 1;
        if key.is_secret() {
            s.raw_secret_reads += 1;
        }
        let id = key.to_string().to_lowercase();
        let (_, values) = s
            .keys
            .get(&id)
            .ok_or_else(|| RegistryError::NotFound(key.to_string()))?;
        let prefix = format!("{id}\\");
        let subkeys = s
            .keys
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix) && !k[prefix.len()..].contains('\\'))
            .filter_map(|(_, (orig, _))| orig.rsplit('\\').next().map(str::to_owned))
            .collect();
        Ok((subkeys, values.clone()))
    }
}

impl RegistryPort for FakeRegistry {
    fn list(&self, key: &RegKey, max_entries: usize) -> Result<RegListing, RegistryError> {
        check_key(key)?;
        let (subkeys, values) = self.raw(key)?;
        let listing = RegListing {
            key: key.to_string(),
            subkeys,
            values,
            hidden_subkeys: 0,
            truncated: false,
        };
        Ok(guard_listing(key, listing, max_entries))
    }

    fn read_value(&self, key: &RegKey, name: &str) -> Result<RegValue, RegistryError> {
        check_key(key)?;
        if name.chars().any(char::is_control) || name.chars().count() > 16_383 {
            return Err(RegistryError::InvalidKey(name.chars().take(64).collect()));
        }
        let (_, values) = self.raw(key)?;
        values
            .into_iter()
            .find(|v| v.name.eq_ignore_ascii_case(name))
            .map(RegValue::guarded)
            .ok_or_else(|| RegistryError::NotFound(format!("{key} → {name}")))
    }
}
