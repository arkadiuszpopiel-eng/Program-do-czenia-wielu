//! Księga cen modeli: wbudowany `providers-catalog/ceny/cennik.toml` (sprawdzony na stronach dostawców)
//! i plik użytkownika `cennik.toml` w konfiguracji Alfy, który go nadpisuje (aktualizacje zatwierdzone
//! przez właściciela). Bez księgi profil dostawcy z konta miał pusty cennik i każdy koszt wychodził
//! 0,00 zł (zgłoszenie właściciela 2026-10-20). Brak ceny modelu = koszt nieznany, nie 0.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use providers_contract::{Pricing, PricingTable};
use serde::{Deserialize, Serialize};

/// Wbudowany cennik (część wydania).
pub const BUILTIN_PRICES: &str = include_str!("../../../providers-catalog/ceny/cennik.toml");
/// Najwięcej pozycji w jednym cenniku (ochrona przed przypadkowo wklejonym dużym tekstem).
const MAX_ENTRIES: usize = 2_000;
/// Najwyższa wiarygodna cena (USD za 1 mln tokenów) — wyżej to raczej pomyłka jednostek.
const MAX_PRICE: f64 = 10_000.0;

/// Cena jednego modelu (USD za 1 mln tokenów).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceEntry {
    /// Dostawca z katalogu (`openai`, `anthropic`, `openrouter`…).
    pub provider: String,
    /// Identyfikator modelu w API.
    pub model: String,
    /// Wejście.
    pub input: f64,
    /// Wyjście.
    pub output: f64,
    /// Odczyt z cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    /// Zapis do cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    /// Strona, z której pochodzi cena.
    #[serde(default)]
    pub source: String,
    /// Uwagi (progi długiego kontekstu, zapowiedziane zmiany).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl PriceEntry {
    fn pricing(&self) -> Pricing {
        Pricing {
            input_per_mtok_usd: self.input,
            output_per_mtok_usd: self.output,
            cache_read_per_mtok_usd: self.cache_read,
            cache_write_per_mtok_usd: self.cache_write,
        }
    }

    fn key(&self) -> (String, String) {
        (self.provider.clone(), self.model.clone())
    }
}

/// Cennik: data sprawdzenia i pozycje.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceBook {
    /// Dzień sprawdzenia (`RRRR-MM-DD`).
    #[serde(default)]
    pub checked: String,
    /// Pozycje.
    #[serde(default, rename = "price")]
    pub prices: Vec<PriceEntry>,
}

/// Zmiana ceny względem bieżącego cennika (podgląd przed zapisem).
#[derive(Debug, Clone, PartialEq)]
pub struct PriceChange {
    /// Pozycja po zmianie.
    pub entry: PriceEntry,
    /// Cena przed zmianą (`None` — nowy model).
    pub before: Option<PriceEntry>,
}

impl PriceBook {
    /// Wbudowany cennik wydania.
    pub fn builtin() -> Self {
        Self::parse(BUILTIN_PRICES).unwrap_or_default()
    }

    /// Cennik z tekstu TOML — z kontrolą: liczby skończone i nieujemne, identyfikatory niepuste,
    /// rozsądne wartości (fail-closed: błędny tekst niczego nie zmienia).
    pub fn parse(text: &str) -> Result<Self, String> {
        let book: Self = toml::from_str(text).map_err(|e| format!("cennik: {e}"))?;
        if book.prices.len() > MAX_ENTRIES {
            return Err(format!("cennik: więcej niż {MAX_ENTRIES} pozycji"));
        }
        for p in &book.prices {
            let who = format!("{} / {}", p.provider, p.model);
            if p.provider.trim().is_empty() || p.model.trim().is_empty() {
                return Err("cennik: pozycja bez dostawcy albo modelu".into());
            }
            let values = [Some(p.input), Some(p.output), p.cache_read, p.cache_write];
            if values
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=MAX_PRICE).contains(&v))
            {
                return Err(format!(
                    "cennik: {who} — cena spoza zakresu 0–{MAX_PRICE} USD za 1 mln tokenów"
                ));
            }
        }
        Ok(book)
    }

    /// Ten cennik z pozycjami `user` w miejsce tych samych modeli (i nowymi modelami).
    pub fn overlaid(&self, user: &Self) -> Self {
        let mut map: BTreeMap<(String, String), PriceEntry> =
            self.prices.iter().map(|p| (p.key(), p.clone())).collect();
        for p in &user.prices {
            map.insert(p.key(), p.clone());
        }
        let checked = if user.checked > self.checked {
            user.checked.clone()
        } else {
            self.checked.clone()
        };
        Self {
            checked,
            prices: map.into_values().collect(),
        }
    }

    /// Cennik profilu dostawcy.
    pub fn table_for(&self, provider: &str) -> PricingTable {
        self.prices
            .iter()
            .filter(|p| p.provider == provider)
            .map(|p| (p.model.clone(), p.pricing()))
            .collect()
    }

    /// Pozycje `newer`, które zmieniają ten cennik (nowe modele albo inne ceny).
    pub fn changes(&self, newer: &Self) -> Vec<PriceChange> {
        let now: BTreeMap<(String, String), &PriceEntry> =
            self.prices.iter().map(|p| (p.key(), p)).collect();
        newer
            .prices
            .iter()
            .filter_map(|p| {
                let before = now.get(&p.key()).copied();
                let same = before.is_some_and(|b| b.pricing() == p.pricing());
                (!same).then(|| PriceChange {
                    entry: p.clone(),
                    before: before.cloned(),
                })
            })
            .collect()
    }

    /// Tekst TOML.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string(self).map_err(|e| format!("cennik: {e}"))
    }
}

/// Plik cennika użytkownika nałożony na cennik wbudowany; wczytywany ponownie po zmianie na dysku.
pub struct PriceFile {
    path: PathBuf,
    builtin: PriceBook,
    cache: Mutex<(Option<SystemTime>, PriceBook)>,
}

impl PriceFile {
    /// Plik użytkownika w `path` (może nie istnieć).
    pub fn new(path: PathBuf) -> Self {
        let builtin = PriceBook::builtin();
        Self {
            path,
            cache: Mutex::new((None, builtin.clone())),
            builtin,
        }
    }

    /// Ścieżka pliku użytkownika.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Cennik wbudowany.
    pub fn builtin(&self) -> &PriceBook {
        &self.builtin
    }

    /// Pozycje z pliku użytkownika (pusty, gdy brak pliku albo jest błędny).
    pub fn user(&self) -> PriceBook {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| PriceBook::parse(&text).ok())
            .unwrap_or_default()
    }

    /// Cennik obowiązujący: wbudowany z nałożonym plikiem użytkownika. Błędny plik użytkownika
    /// jest pomijany (zostaje cennik wbudowany) — zapis przez [`PriceFile::apply`] go sprawdza.
    pub fn current(&self) -> PriceBook {
        let modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.0 != modified {
            *cache = (modified, self.builtin.overlaid(&self.user()));
        }
        cache.1.clone()
    }

    /// Nakłada sprawdzone pozycje `update` na plik użytkownika i zapisuje go (zapis atomowy przez
    /// plik tymczasowy). Zwraca zmiany względem cennika obowiązującego przed zapisem.
    pub fn apply(&self, update: &PriceBook) -> Result<Vec<PriceChange>, String> {
        let changes = self.current().changes(update);
        let merged = self.user().overlaid(update);
        let text = merged.to_toml()?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = self.path.with_extension("toml.nowy");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| format!("{}: {e}", self.path.display()))?;
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATE: &str = r#"
checked = "2026-11-01"

[[price]]
provider = "openai"
model = "gpt-6.1-sol"
input = 1.5
output = 9.0
source = "https://developers.openai.com/api/docs/pricing"

[[price]]
provider = "openai"
model = "gpt-nowy"
input = 0.5
output = 1.0
"#;

    #[test]
    fn builtin_book_parses_and_covers_owner_models() {
        let book = PriceBook::builtin();
        assert!(book.prices.len() > 80, "{}", book.prices.len());
        assert!(!book.checked.is_empty());
        for (provider, model) in [
            ("anthropic", "claude-haiku-5-5"),
            ("openai", "gpt-6.1-sol"),
            ("openrouter", "openai/gpt-6-luna"),
        ] {
            assert!(
                book.table_for(provider).contains_key(model),
                "{provider}/{model}"
            );
        }
        assert!(
            book.prices.iter().all(|p| p.source.starts_with("https://")),
            "każda cena ma źródło"
        );
    }

    #[test]
    fn invalid_text_is_rejected() {
        assert!(PriceBook::parse("[[price]]\nprovider = \"x\"").is_err());
        let negative = "[[price]]\nprovider = \"a\"\nmodel = \"m\"\ninput = -1.0\noutput = 1.0";
        assert!(PriceBook::parse(negative).is_err());
        let huge = "[[price]]\nprovider = \"a\"\nmodel = \"m\"\ninput = 1e9\noutput = 1.0";
        assert!(PriceBook::parse(huge).is_err());
        let empty = "[[price]]\nprovider = \" \"\nmodel = \"m\"\ninput = 1.0\noutput = 1.0";
        assert!(PriceBook::parse(empty).is_err());
        assert!(PriceBook::parse("nieznane = 1").is_err());
    }

    #[test]
    fn user_file_overrides_builtin_and_apply_reports_changes() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let file = PriceFile::new(dir.path().join("cennik.toml"));
        let before = file.current();
        let old = before.table_for("openai")["gpt-6.1-sol"];
        let update = PriceBook::parse(UPDATE).unwrap_or_else(|e| panic!("{e}"));
        let changes = file.apply(&update).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(changes.len(), 2);
        assert!(
            changes
                .iter()
                .any(|c| c.entry.model == "gpt-nowy" && c.before.is_none())
        );
        let after = file.current();
        let table = after.table_for("openai");
        assert!((table["gpt-6.1-sol"].input_per_mtok_usd - 1.5).abs() < 1e-9);
        assert!(table.contains_key("gpt-nowy"));
        assert_ne!(table["gpt-6.1-sol"], old);
        assert_eq!(after.checked, "2026-11-01");
        // Ten sam tekst drugi raz niczego nie zmienia.
        assert!(file.apply(&update).unwrap_or_default().is_empty());
        // Wbudowane ceny innych modeli zostają.
        assert!(table.contains_key("gpt-5-mini"));
    }

    #[test]
    fn broken_user_file_falls_back_to_builtin() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("cennik.toml");
        std::fs::write(&path, "to nie jest toml [[").unwrap_or_else(|e| panic!("{e}"));
        let file = PriceFile::new(path);
        assert_eq!(file.current(), PriceBook::builtin());
    }
}
