//! Nazwy: tytuł domyślny, klucz wyszukiwania tytułu, bezpieczna nazwa katalogu sesji (Windows).

use lib_sqlstore::fold_pl;

use crate::ids::SessionId;

/// Tytuł sesji, gdy użytkownik nie podał żadnego.
pub const DEFAULT_TITLE: &str = "Nowa rozmowa";

/// Maksymalna długość nazwy katalogu sesji (znaki), z zapasem na `\out\…` w ścieżce.
const MAX_DIR_CHARS: usize = 64;

/// Nazwy zarezerwowane przez Windows (bez względu na rozszerzenie i wielkość liter).
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Klucz wyszukiwania tytułu: złożone diakrytyki ([`fold_pl`]) + małe litery.
pub fn title_key(title: &str) -> String {
    fold_pl(title).to_lowercase()
}

/// Bezpieczna nazwa katalogu sesji `<root>\Sesje\<nazwa>` z tytułu.
///
/// Zastępuje znaki zabronione w Windows (`<>:"/\|?*`, sterujące) podkreśleniem, zwija białe znaki,
/// obcina kropki/spacje na końcu, omija nazwy zarezerwowane (`CON`, `COM1`…), skraca do 64 znaków.
/// Pusty wynik → `sesja-<id>`. Polskie litery zostają (NTFS obsługuje Unicode).
pub fn session_dir_name(title: &str, id: &SessionId) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for c in title.trim().chars() {
        let c = if !c.is_whitespace() && (c.is_control() || "<>:\"/\\|?*".contains(c)) {
            '_'
        } else {
            c
        };
        if c.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    let mut out: String = out.chars().take(MAX_DIR_CHARS).collect();
    while out.ends_with(['.', ' ']) {
        out.pop();
    }
    let stem = out.split('.').next().unwrap_or_default().to_owned();
    if RESERVED.contains(&stem.trim_end().to_uppercase().as_str()) {
        out.insert(stem.len(), '_');
    }
    if out.is_empty() {
        let short: String = id.as_str().chars().take(8).collect();
        return format!("sesja-{short}");
    }
    out
}

/// Pierwsza wolna nazwa: `base`, `base (2)`, `base (3)`… (porównanie bez względu na wielkość liter,
/// jak w NTFS).
pub fn unique_dir_name(base: &str, taken: &[String]) -> String {
    let is_taken = |name: &str| {
        taken
            .iter()
            .any(|t| t.to_lowercase() == name.to_lowercase())
    };
    if !is_taken(base) {
        return base.to_owned();
    }
    (2_u32..)
        .map(|n| format!("{base} ({n})"))
        .find(|candidate| !is_taken(candidate))
        .unwrap_or_else(|| base.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> SessionId {
        SessionId::new("0192abcd-ef01-7000-8000-000000000000")
    }

    #[test]
    fn sanitizes_windows_names() {
        assert_eq!(
            session_dir_name("Raport: Q3/2026?", &id()),
            "Raport_ Q3_2026_"
        );
        assert_eq!(session_dir_name("  Żółta   łódź.. ", &id()), "Żółta łódź");
        assert_eq!(session_dir_name("con", &id()), "con_");
        assert_eq!(session_dir_name("COM1.txt", &id()), "COM1_.txt");
        assert_eq!(session_dir_name("a\tb", &id()), "a b");
        assert_eq!(session_dir_name(" ... ", &id()), "sesja-0192abcd");
        assert_eq!(
            session_dir_name(&"x".repeat(200), &id()).chars().count(),
            64
        );
    }

    #[test]
    fn unique_names_get_suffix() {
        let taken = vec!["Raport".to_owned(), "raport (2)".to_owned()];
        assert_eq!(unique_dir_name("Raport", &taken), "Raport (3)");
        assert_eq!(unique_dir_name("Inne", &taken), "Inne");
    }

    #[test]
    fn title_key_folds() {
        assert_eq!(title_key("Żółć ŁĄKA"), "zolc laka");
    }
}
