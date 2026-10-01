//! Traity modułu: katalog sesji, historia (drzewo append-only), dostęp do bazy sesji.

use std::sync::Arc;

use lib_sqlstore::Db;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SessionError;
use crate::ids::{SessionId, TurnId};
use crate::import::PortableSession;
use crate::session::{NewSession, SessionMeta, SessionPatch, SessionQuery, SessionSummary};
use crate::turn::{HeardPrefix, NewTurn, Turn};

/// Wynik ostatecznego usunięcia sesji (crypto-shredding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteReport {
    /// Czy klucz sesji był w sejfie i został usunięty.
    pub key_deleted: bool,
    /// Liczba usuniętych plików bazy (główny + `-wal`/`-shm`/`-journal`).
    pub files_removed: usize,
}

/// Warianty rodzeństwa tury (`‹ 2/3 ›` w UI): tury o tym samym rodzicu, rosnąco po `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Siblings {
    /// Wszystkie warianty (łącznie z turą, o którą pytano).
    pub turns: Vec<TurnId>,
    /// Pozycja tury w `turns` (od 0; w UI wyświetlana jako `index + 1`).
    pub index: usize,
}

/// Katalog sesji (baza `index.db`): tworzenie, metadane, lista, kosz, usuwanie.
pub trait SessionCatalog: Send + Sync {
    /// Tworzy sesję (osobna baza i klucz).
    fn create_session(&self, new: NewSession) -> Result<SessionMeta, SessionError>;
    /// Metadane sesji.
    fn session(&self, id: &SessionId) -> Result<SessionMeta, SessionError>;
    /// Zmienia metadane (tytuł, przypięcie, archiwum, projekt, tagi…).
    fn update_session(
        &self,
        id: &SessionId,
        patch: SessionPatch,
    ) -> Result<SessionMeta, SessionError>;
    /// Oznacza sesję jako skażoną (flaga tylko rośnie; idempotentne).
    fn mark_tainted(&self, id: &SessionId) -> Result<SessionMeta, SessionError>;
    /// Lista sesji wg zapytania (przypięte na górze, deterministyczny porządek).
    fn list_sessions(&self, query: &SessionQuery) -> Result<Vec<SessionSummary>, SessionError>;
    /// Ustawia kropkę aktywności (stan ulotny).
    fn set_activity(&self, id: &SessionId, active: bool) -> Result<(), SessionError>;
    /// Zeruje licznik nieprzeczytanych.
    fn mark_read(&self, id: &SessionId) -> Result<(), SessionError>;
    /// Przenosi do kosza logicznego (cofalne; okno cofnięcia liczy wywołujący).
    fn trash_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError>;
    /// Przywraca z kosza.
    fn restore_session(&self, id: &SessionId) -> Result<SessionMeta, SessionError>;
    /// Usuwa ostatecznie: klucz z sejfu, plik bazy z `-wal`/`-shm`, wpis katalogu.
    fn delete_session(&self, id: &SessionId) -> Result<DeleteReport, SessionError>;

    /// Import (`transfer`): tworzy sesję z przenośnego zapisu, zachowując identyfikator, metadane,
    /// znaczniki czasu, całe drzewo tur, aktywny liść i szkic (osobna baza i **nowy** klucz na tej
    /// maszynie). **Atomowo**: sesja powstaje w całości albo wcale (wpis katalogu na końcu).
    /// Walidacja: [`crate::PortableSession::validate`]; istniejąca sesja → [`SessionError::AlreadyExists`].
    ///
    /// Domyślnie: operacja nieobsługiwana (implementacje `-impl`/`-fake` ją nadpisują).
    fn adopt_session(&self, session: PortableSession) -> Result<SessionMeta, SessionError> {
        Err(unsupported("adopt_session", &session.meta.id))
    }
}

fn unsupported(op: &str, id: &SessionId) -> SessionError {
    SessionError::invalid(format!(
        "{op} nieobsługiwane przez tę implementację (sesja {id})"
    ))
}

/// Historia rozmowy: drzewo tur, **wyłącznie dopisywanie** (brak operacji zmiany treści tury).
pub trait SessionHistory: Send + Sync {
    /// Dopisuje turę na końcu linii: `parent` musi być liściem (inaczej [`SessionError::NotALeaf`]);
    /// `None` tylko dla pierwszej tury sesji (inaczej [`SessionError::RootExists`]). Tura trafia do
    /// gałęzi rodzica i staje się aktywnym liściem.
    fn append_turn(
        &self,
        id: &SessionId,
        parent: Option<TurnId>,
        turn: NewTurn,
    ) -> Result<Turn, SessionError>;
    /// „Edytuj”/„ponów”: dopisuje wariant tury `sibling_of` (ten sam rodzic) w **nowej gałęzi**.
    /// Wcześniejsze tury pozostają nietknięte. Nowa tura staje się aktywnym liściem.
    fn fork_from(
        &self,
        id: &SessionId,
        sibling_of: TurnId,
        turn: NewTurn,
    ) -> Result<Turn, SessionError>;
    /// Jedna tura.
    fn turn(&self, id: &SessionId, turn: TurnId) -> Result<Turn, SessionError>;
    /// Liniowa historia od korzenia do `leaf` włącznie (z turami ukrytymi — flaga `hidden`).
    fn branch_projection(&self, id: &SessionId, leaf: TurnId) -> Result<Vec<Turn>, SessionError>;
    /// Warianty rodzeństwa tury.
    fn siblings(&self, id: &SessionId, turn: TurnId) -> Result<Siblings, SessionError>;
    /// Liść osiągany z `from` przez zawsze najnowsze dziecko (przełączenie wariantu `‹ 2/3 ›`).
    fn latest_leaf(&self, id: &SessionId, from: TurnId) -> Result<TurnId, SessionError>;
    /// Ustawia aktywny (wyświetlany) liść.
    fn set_active_leaf(&self, id: &SessionId, leaf: TurnId) -> Result<(), SessionError>;
    /// Aktywny liść (`None` dla pustej sesji).
    fn active_leaf(&self, id: &SessionId) -> Result<Option<TurnId>, SessionError>;
    /// Dopisuje fakt „usłyszany prefiks” do tury asystentki (raz; potem niezmienny).
    fn record_heard_prefix(
        &self,
        id: &SessionId,
        turn: TurnId,
        prefix: HeardPrefix,
    ) -> Result<Turn, SessionError>;
    /// Ukrywa/pokazuje turę w widoku (treść i audyt zostają).
    fn set_hidden(&self, id: &SessionId, turn: TurnId, hidden: bool) -> Result<(), SessionError>;
    /// Zapisuje szkic composera (pusty tekst usuwa szkic).
    fn save_draft(&self, id: &SessionId, text: &str) -> Result<(), SessionError>;
    /// Szkic composera.
    fn draft(&self, id: &SessionId) -> Result<Option<String>, SessionError>;
    /// Liczba tur w sesji (wszystkie gałęzie).
    fn turn_count(&self, id: &SessionId) -> Result<u64, SessionError>;

    /// Całe drzewo (wszystkie gałęzie) rosnąco po `id` — eksport przenośny (`transfer`).
    ///
    /// Domyślnie: tury `1..=turn_count` (identyfikatory są kolejnymi liczbami od 1, bez luk);
    /// implementacje mogą to zrobić jednym zapytaniem.
    fn all_turns(&self, id: &SessionId) -> Result<Vec<Turn>, SessionError> {
        (1..=self.turn_count(id)?)
            .map(|n| self.turn(id, TurnId(n)))
            .collect()
    }

    /// Import (`transfer`): dopisuje partię pełnych tur z **zachowanymi** identyfikatorami, rodzicami,
    /// gałęziami, czasem, usłyszanym prefiksem i flagą `hidden`. Tury muszą spełniać reguły
    /// [`crate::TreeCursor`] (kolejne `id`, gałęzie jak przy `append_turn`/`fork_from`); istniejące
    /// tury nie są zmieniane (append-only). Wszystko albo nic: błąd dowolnej tury = brak zmian.
    /// Aktywny liść się nie zmienia (ustawia go wywołujący). Zwraca liczbę dopisanych tur.
    ///
    /// Domyślnie: operacja nieobsługiwana (implementacje `-impl`/`-fake` ją nadpisują).
    fn import_turns(&self, id: &SessionId, turns: &[Turn]) -> Result<u64, SessionError> {
        if turns.is_empty() {
            return Ok(0);
        }
        Err(unsupported("import_turns", id))
    }
}

/// Pełny kontrakt sesji (katalog + historia).
pub trait Sessions: SessionCatalog + SessionHistory {}

impl<T: SessionCatalog + SessionHistory> Sessions for T {}

/// Dostęp innych modułów (`search`, `memory`, `artifacts`) do szyfrowanej bazy sesji.
///
/// Wszyscy dostają **tę samą** instancję [`Db`] (jedno połączenie na plik). Moduł tworzy własne
/// tabele przez `lib_sqlstore::migrate` we własnej przestrzeni nazw. Usunięcie sesji kasuje także
/// ich dane (jeden plik, jeden klucz).
pub trait SessionDbProvider: Send + Sync {
    /// Otwarta baza sesji (otwiera przy pierwszym użyciu).
    fn session_db(&self, id: &SessionId) -> Result<Arc<Db>, SessionError>;
    /// Wszystkie istniejące sesje (także w koszu i archiwum) — dla wyszukiwania „wszędzie” w UI.
    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError>;
}
