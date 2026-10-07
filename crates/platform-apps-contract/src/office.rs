//! Port Office (Word/Excel przez automatyzację COM, PLAN §7.2 „Office COM — P1/F6”).
//!
//! Niezmienniki (testowane na atrapie, egzekwowane w implementacji i w narzędziach):
//! - port pracuje na **kopii roboczej** z bajtów pliku — nigdy nie dostaje ścieżki oryginału do
//!   zapisu; wynik edycji to nowe bajty (narzędzie zapisuje je jako nową wersję przez dziennik
//!   cofania);
//! - makra/VBA **zawsze wyłączone**: `AutomationSecurity = msoAutomationSecurityForceDisable` (3)
//!   ustawione i odczytane z powrotem przed otwarciem dokumentu; każdy wynik niesie
//!   [`OfficeSession`], a [`check_session`] odrzuca inny stan (fail-closed);
//! - plik z Internetu (Mark-of-the-Web, strefa 3/4) otwierany wyłącznie w Protected View, tylko do
//!   odczytu; edycja → [`OfficeError::ProtectedView`];
//! - formuły Excela tylko z listy dozwolonej ([`crate::check_formula`]): bez DDE, odwołań zewnętrznych,
//!   funkcji sieciowych (`WEBSERVICE`, `IMAGE`, `STOCKHISTORY`…) i wykonujących kod (`CALL`, `PY`);
//!   tekst zaczynający się od `=`/`+`/`-`/`@` jest wpisywany jako tekst ([`crate::safe_cell_text`]);
//! - treść dokumentów jest niezaufana (taint `File` w narzędziach).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::edit::{OfficeEdit, OfficeEdited};

/// `msoAutomationSecurityForceDisable` — makra wyłączone bez pytania.
pub const AUTOMATION_SECURITY_FORCE_DISABLE: i32 = 3;
/// Największy obsługiwany plik (bajty).
pub const MAX_OFFICE_BYTES: usize = 64 * 1024 * 1024;
/// Najwięcej komórek w odczycie lub zapisie zakresu.
pub const MAX_RANGE_CELLS: usize = 10_000;
/// Najwięcej edycji w jednym wywołaniu.
pub const MAX_EDITS: usize = 200;
/// Najdłuższy tekst wstawiany jedną edycją (znaki).
pub const MAX_EDIT_CHARS: usize = 200_000;
/// Najdłuższa formuła (znaki).
pub const MAX_FORMULA_CHARS: usize = 2_000;
/// Limit pojedynczej operacji Office (ms).
pub const OFFICE_CALL_TIMEOUT_MS: u64 = 60_000;

/// Aplikacja Office.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfficeApp {
    /// Word (`Word.Application`).
    Word,
    /// Excel (`Excel.Application`).
    Excel,
}

impl OfficeApp {
    /// Plik wykonywalny (selektor `gui.control`).
    pub fn exe(self) -> &'static str {
        match self {
            Self::Word => "winword.exe",
            Self::Excel => "excel.exe",
        }
    }

    /// ProgID serwera COM.
    pub fn prog_id(self) -> &'static str {
        match self {
            Self::Word => "Word.Application",
            Self::Excel => "Excel.Application",
        }
    }
}

/// Aplikacja dla pliku po rozszerzeniu (szablony `.dot*`/`.xlt*`, dodatki `.xlam` i nieznane
/// → `None`).
pub fn app_for_file(name: &str) -> Option<OfficeApp> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "docx" | "docm" | "doc" | "rtf" | "odt" => Some(OfficeApp::Word),
        "xlsx" | "xlsm" | "xls" | "xlsb" | "ods" | "csv" => Some(OfficeApp::Excel),
        _ => None,
    }
}

/// Strefa pochodzenia pliku (Mark-of-the-Web, `Zone.Identifier`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileZone {
    /// Bez znacznika (komputer lokalny).
    #[default]
    Local,
    /// Intranet (1).
    Intranet,
    /// Zaufane witryny (2).
    Trusted,
    /// Internet (3).
    Internet,
    /// Witryny z ograniczeniami (4).
    Restricted,
}

impl FileZone {
    /// Strefa z `ZoneId` (nieznana wartość → `Restricted`, fail-closed).
    pub fn from_zone_id(id: u32) -> Self {
        match id {
            0 => Self::Local,
            1 => Self::Intranet,
            2 => Self::Trusted,
            3 => Self::Internet,
            _ => Self::Restricted,
        }
    }

    /// Strefa z treści strumienia `Zone.Identifier` (`[ZoneTransfer]\nZoneId=3`); brak `ZoneId`
    /// → `Internet` (znacznik jest, wartość nieczytelna — fail-closed).
    pub fn from_zone_identifier(text: &str) -> Self {
        text.lines()
            .filter_map(|l| l.trim().strip_prefix("ZoneId="))
            .find_map(|v| v.trim().parse::<u32>().ok())
            .map_or(Self::Internet, Self::from_zone_id)
    }

    /// Czy plik wymaga Protected View (Internet albo ograniczone).
    pub fn is_untrusted(self) -> bool {
        matches!(self, Self::Internet | Self::Restricted)
    }
}

/// Błąd portu Office.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OfficeError {
    /// Brak zainstalowanego Office (albo aplikacji) na tej maszynie.
    #[error("Office niedostępny: {0}")]
    NotInstalled(String),
    /// Plik z Internetu — tylko odczyt w Protected View, edycja zablokowana.
    #[error(
        "plik pochodzi z Internetu (Protected View) — edycja zablokowana; właściciel może go odblokować we Właściwościach pliku"
    )]
    ProtectedView,
    /// Przekroczony limit czasu (zawieszony Office).
    #[error("limit czasu {ms} ms: {op}")]
    Timeout {
        /// Operacja.
        op: String,
        /// Limit.
        ms: u64,
    },
    /// Zasady (formuła spoza listy, limit, makra nie wyłączone).
    #[error("zasady: {0}")]
    Policy(String),
    /// Dokument nieczytelny (uszkodzony, chroniony hasłem, brak arkusza, zły zakres).
    #[error("dokument: {0}")]
    Document(String),
    /// Nieobsługiwane (platforma, format, edycja innej aplikacji).
    #[error("nieobsługiwane: {0}")]
    Unsupported(String),
}

/// Plik do przetworzenia (kopia robocza w pamięci).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficeFile {
    /// Aplikacja.
    pub app: OfficeApp,
    /// Nazwa pliku (bez katalogu) — rozszerzenie wyznacza format zapisu.
    pub file_name: String,
    /// Zawartość oryginału.
    pub bytes: Vec<u8>,
    /// Strefa pochodzenia oryginału.
    pub zone: FileZone,
}

impl OfficeFile {
    /// Plik z walidacją nazwy, formatu i rozmiaru.
    pub fn new(file_name: &str, bytes: Vec<u8>, zone: FileZone) -> Result<Self, OfficeError> {
        // Separator Windows i POSIX niezależnie od platformy budowania.
        let name = file_name
            .rsplit(['\\', '/'])
            .next()
            .map(str::trim)
            .filter(|n| {
                !n.is_empty()
                    && !n.starts_with("~$")
                    && !n.contains([':', '<', '>', '"', '|', '?', '*'])
                    && !n.chars().any(char::is_control)
            })
            .ok_or_else(|| OfficeError::Document(format!("niepoprawna nazwa `{file_name}`")))?;
        let app = app_for_file(name)
            .ok_or_else(|| OfficeError::Unsupported(format!("format pliku `{name}`")))?;
        if bytes.len() > MAX_OFFICE_BYTES {
            return Err(OfficeError::Policy("plik większy niż 64 MiB".into()));
        }
        Ok(Self {
            app,
            file_name: name.to_owned(),
            bytes,
            zone,
        })
    }
}

/// Stan sesji automatyzacji (dowód wyłączenia makr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficeSession {
    /// `Application.AutomationSecurity` odczytane po ustawieniu.
    pub automation_security: i32,
    /// Dokument otwarty w Protected View.
    pub protected_view: bool,
    /// Dokument zawiera projekt VBA (makra nie zostały uruchomione).
    pub macros_present: bool,
    /// Użyto instancji Office już działającej u użytkownika (niczego w niej nie ukrywamy).
    pub shared_instance: bool,
}

/// Fail-closed: wynik bez dowodu wyłączenia makr jest odrzucany.
pub fn check_session(session: &OfficeSession) -> Result<(), OfficeError> {
    if session.automation_security != AUTOMATION_SECURITY_FORCE_DISABLE {
        return Err(OfficeError::Policy(format!(
            "makra nie zostały wyłączone (AutomationSecurity = {})",
            session.automation_security
        )));
    }
    Ok(())
}

/// Co odczytać.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OfficeQuery {
    /// Tekst dokumentu (Excel: zawartość używanych zakresów jako TSV).
    Text {
        /// Limit znaków.
        max_chars: usize,
    },
    /// Tabele dokumentu Word.
    Tables {
        /// Limit komórek łącznie.
        max_cells: usize,
    },
    /// Zakres komórek Excela.
    Range {
        /// Arkusz (`None` = pierwszy).
        sheet: Option<String>,
        /// Zakres A1 (`B2:D10`).
        range: String,
        /// Zwracać formuły zamiast wartości.
        formulas: bool,
    },
    /// Arkusze z używanymi zakresami.
    Sheets,
}

/// Wartość komórki.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CellValue {
    /// Pusta.
    Empty,
    /// Liczba (także data jako liczba seryjna).
    Number(f64),
    /// Prawda/fałsz.
    Bool(bool),
    /// Tekst albo formuła (przy `formulas = true`).
    Text(String),
    /// Błąd (`#DIV/0!`).
    Error {
        /// Kod błędu.
        error: String,
    },
}

/// Arkusz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetInfo {
    /// Nazwa.
    pub name: String,
    /// Używany zakres (`A1:F20`; pusty arkusz = `A1`).
    pub used_range: String,
}

/// Wynik odczytu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfficeContent {
    /// Tekst (zapytanie `Text`).
    pub text: Option<String>,
    /// Tabele Worda (wiersze × komórki).
    pub tables: Vec<Vec<Vec<String>>>,
    /// Komórki zakresu (wiersze × kolumny).
    pub cells: Vec<Vec<CellValue>>,
    /// Arkusze.
    pub sheets: Vec<SheetInfo>,
    /// Obcięto limitem.
    pub truncated: bool,
    /// Stan sesji.
    pub session: OfficeSession,
}

/// Port Office. Implementacja Windows: COM late binding (`IDispatch`) na wątku STA z limitem
/// czasu; atrapa: dokument w pamięci.
pub trait OfficePort: Send + Sync {
    /// Czy aplikacja jest dostępna.
    fn available(&self, app: OfficeApp) -> bool;
    /// Strefa pochodzenia pliku (Mark-of-the-Web); błąd odczytu znacznika → `Internet`.
    fn zone_of(&self, path: &Path) -> FileZone;
    /// Odczyt (Protected View dla plików z Internetu).
    fn read(&self, file: &OfficeFile, query: &OfficeQuery) -> Result<OfficeContent, OfficeError>;
    /// Edycje kopii roboczej → nowe bajty (plik z Internetu → [`OfficeError::ProtectedView`]).
    fn edit(&self, file: &OfficeFile, edits: &[OfficeEdit]) -> Result<OfficeEdited, OfficeError>;
}
