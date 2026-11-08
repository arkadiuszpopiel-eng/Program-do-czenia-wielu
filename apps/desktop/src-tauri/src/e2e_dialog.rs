//! Odpowiedzi okien wyboru pliku i katalogu w budowie testowej (cecha `e2e`): test prawdziwej
//! aplikacji kolejkuje odpowiedź (`e2e_answer_dialog`), a następne okno „Otwórz”/„Zapisz jako”/
//! wyboru katalogu bierze ją zamiast pokazać okno systemowe. Okno systemowe wymaga pierwszego
//! planu, a testy nie mogą wchodzić w sesję właściciela (31.10). Poza budową testową kolejki nie
//! ma: [`take`] zawsze oddaje `None` i dialogi działają zwyczajnie.

use std::path::PathBuf;

#[cfg(feature = "e2e")]
static QUEUE: std::sync::Mutex<std::collections::VecDeque<Option<PathBuf>>> =
    std::sync::Mutex::new(std::collections::VecDeque::new());

/// Zakolejkowana odpowiedź następnego okna: `Some(None)` — test „anulował”, `None` — brak
/// odpowiedzi (okno systemowe).
pub(crate) fn take() -> Option<Option<PathBuf>> {
    #[cfg(feature = "e2e")]
    {
        QUEUE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
    }
    #[cfg(not(feature = "e2e"))]
    {
        None
    }
}

/// `e2e_answer_dialog` — wyłącznie budowa testowa: kolejkuje odpowiedź następnego okna wyboru
/// (`path` — wybrany plik albo katalog, `null` — anulowanie). Wynik: liczba odpowiedzi w kolejce.
/// Poza budową testową — odmowa.
#[tauri::command]
pub async fn e2e_answer_dialog(path: Option<String>) -> Result<usize, String> {
    #[cfg(feature = "e2e")]
    {
        let mut queue = QUEUE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.push_back(path.map(PathBuf::from));
        Ok(queue.len())
    }
    #[cfg(not(feature = "e2e"))]
    {
        let _ = path;
        Err("Polecenie tylko dla budowy testowej.".into())
    }
}
