//! Cennik modeli dla interfejsu: obowiązujące ceny (wbudowane + plik użytkownika) i aktualizacja
//! z tekstu TOML — najpierw podgląd zmian, zapis dopiero na wyraźne „Zastosuj” właściciela
//! (obowiązek „Kontrola cenników” podaje gotowy tekst; agentka sama pliku nie zapisuje).

use std::path::Path;

use app_api::dto::{PriceChangeRow, PriceRow, PricingView};
use providers_api_impl::{PriceBook, PriceChange, PriceEntry, PriceFile};

/// Plik cennika użytkownika w katalogu konfiguracji.
pub const FILE: &str = "cennik.toml";

fn row(entry: &PriceEntry, user: &PriceBook) -> PriceRow {
    PriceRow {
        provider: entry.provider.clone(),
        model: entry.model.clone(),
        input: entry.input,
        output: entry.output,
        cache_read: entry.cache_read,
        cache_write: entry.cache_write,
        source: entry.source.clone(),
        note: entry.note.clone(),
        user: user
            .prices
            .iter()
            .any(|p| p.provider == entry.provider && p.model == entry.model),
    }
}

fn view(file: &PriceFile, changes: &[PriceChange], saved: bool) -> PricingView {
    let current = file.current();
    let user = file.user();
    PricingView {
        checked: current.checked.clone(),
        file: file.path().display().to_string(),
        rows: current.prices.iter().map(|p| row(p, &user)).collect(),
        changes: changes
            .iter()
            .map(|c| PriceChangeRow {
                after: row(&c.entry, &user),
                before: c.before.as_ref().map(|b| row(b, &user)),
            })
            .collect(),
        saved,
    }
}

/// Cennik obowiązujący.
pub fn get(path: &Path) -> PricingView {
    view(&PriceFile::new(path.to_path_buf()), &[], false)
}

/// Aktualizacja z tekstu: `save = false` — tylko podgląd zmian; `true` — zapis do pliku
/// użytkownika. Błędny tekst nie zmienia niczego.
pub fn apply(path: &Path, text: &str, save: bool) -> Result<PricingView, String> {
    let update = PriceBook::parse(text)?;
    if update.prices.is_empty() {
        return Err("cennik: tekst nie zawiera żadnej ceny ([[price]])".into());
    }
    let file = PriceFile::new(path.to_path_buf());
    if save {
        let changes = file.apply(&update)?;
        Ok(view(&file, &changes, true))
    } else {
        let changes = file.current().changes(&update);
        Ok(view(&file, &changes, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATE: &str = "checked = \"2026-11-02\"\n[[price]]\nprovider = \"openai\"\n\
        model = \"gpt-6-luna\"\ninput = 0.08\noutput = 0.4\nsource = \"https://developers.openai.com/api/docs/pricing\"\n";

    #[test]
    fn preview_does_not_save_and_apply_does() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join(FILE);
        let before = get(&path);
        assert!(before.rows.len() > 80);
        assert!(before.rows.iter().all(|r| !r.user));
        let preview = apply(&path, UPDATE, false).unwrap_or_else(|e| panic!("{e}"));
        assert!(!preview.saved);
        assert_eq!(preview.changes.len(), 1);
        assert!(preview.changes[0].before.is_some());
        assert!(!path.exists(), "podgląd niczego nie zapisuje");
        let saved = apply(&path, UPDATE, true).unwrap_or_else(|e| panic!("{e}"));
        assert!(saved.saved && path.exists());
        let luna = saved
            .rows
            .iter()
            .find(|r| r.provider == "openai" && r.model == "gpt-6-luna")
            .unwrap_or_else(|| panic!("brak gpt-6-luna"));
        assert!(luna.user && (luna.input - 0.08).abs() < 1e-9);
        assert_eq!(saved.checked, "2026-11-02");
    }

    #[test]
    fn invalid_or_empty_text_changes_nothing() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join(FILE);
        assert!(apply(&path, "checked = \"2026-11-02\"", true).is_err());
        assert!(apply(&path, "[[price]]\nprovider = \"a\"", true).is_err());
        assert!(!path.exists());
    }
}
