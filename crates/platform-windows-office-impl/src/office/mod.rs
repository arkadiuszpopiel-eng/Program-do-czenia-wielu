//! `OfficePort` dla Windows: automatyzacja COM (late binding `IDispatch`) na dedykowanym wątku
//! STA z limitem czasu każdego wywołania. Przebieg operacji:
//!
//! 1. bajty pliku → **kopia robocza** w prywatnym katalogu Alfy (`work_dir\<unikat>\<nazwa>`);
//!    plik z Internetu dostaje z powrotem znacznik `Zone.Identifier` (Office widzi go jako
//!    niezaufany);
//! 2. `CoCreateInstance(Word/Excel.Application)` → `AutomationSecurity = 3` (ustawione **i
//!    odczytane** przed otwarciem — inaczej odmowa), `DisplayAlerts` wyłączone, bez aktualizacji
//!    łączy; Excel: tryb obliczeń ręczny przed otwarciem (formuły sieciowe w pliku nie liczą się);
//! 3. odczyt (plik z Internetu: `ProtectedViewWindows.Open`) albo edycje → `Save` kopii →
//!    bajty nowej wersji; oryginał nie jest nigdy otwierany;
//! 4. zamknięcie bez zapisu, `Quit` własnej instancji (instancja użytkownika: przywrócenie ustawień),
//!    usunięcie katalogu kopii.

#[cfg(windows)]
mod disp;
#[cfg(windows)]
mod excel;
#[cfg(windows)]
mod sta;
#[cfg(windows)]
mod word;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use platform_apps_contract::check_session;
use platform_apps_contract::{
    FileZone, OFFICE_CALL_TIMEOUT_MS, OfficeApp, OfficeContent, OfficeEdit, OfficeEdited,
    OfficeError, OfficeFile, OfficePort, OfficeQuery, check_edits,
};

/// Konfiguracja (`[platform.office]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficeConfig {
    /// Prywatny katalog kopii roboczych (`%LOCALAPPDATA%\Alfa\office-work`).
    pub work_dir: PathBuf,
    /// Limit jednej operacji (ms).
    pub call_timeout_ms: u64,
    /// Ile porzuconych, wiszących wątków STA naraz (potem odmowa).
    pub max_hung_threads: usize,
}

impl OfficeConfig {
    /// Konfiguracja z katalogiem kopii roboczych.
    pub fn new(work_dir: impl Into<PathBuf>) -> Self {
        Self {
            work_dir: work_dir.into(),
            call_timeout_ms: OFFICE_CALL_TIMEOUT_MS,
            max_hung_threads: 2,
        }
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Kopia robocza (katalog usuwany w `Drop`).
#[derive(Debug)]
pub struct WorkCopy {
    dir: PathBuf,
    path: PathBuf,
}

/// Treść strumienia `Zone.Identifier` dla pliku z Internetu.
pub const ZONE_INTERNET: &str = "[ZoneTransfer]\r\nZoneId=3\r\n";

impl WorkCopy {
    /// Zapisuje bajty do nowego katalogu kopii; plik niezaufany dostaje znacznik MOTW.
    pub fn create(work_dir: &Path, file: &OfficeFile) -> Result<Self, OfficeError> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = work_dir.join(format!("w{nanos:x}-{n}-{}", std::process::id()));
        let io = |e: std::io::Error| OfficeError::Document(format!("kopia robocza: {e}"));
        std::fs::create_dir_all(&dir).map_err(io)?;
        let copy = Self {
            path: dir.join(&file.file_name),
            dir,
        };
        std::fs::write(&copy.path, &file.bytes).map_err(io)?;
        if file.zone.is_untrusted() && cfg!(windows) {
            std::fs::write(zone_stream(&copy.path), ZONE_INTERNET).map_err(io)?;
        }
        Ok(copy)
    }

    /// Ścieżka kopii.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Zawartość kopii po zapisie.
    pub fn read_back(&self) -> Result<Vec<u8>, OfficeError> {
        std::fs::read(&self.path).map_err(|e| OfficeError::Document(format!("odczyt kopii: {e}")))
    }
}

impl Drop for WorkCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Ścieżka strumienia ADS `Zone.Identifier` (NTFS).
pub fn zone_stream(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(":Zone.Identifier");
    PathBuf::from(s)
}

/// Strefa pliku: brak strumienia → lokalny; strumień → `ZoneId`; inny błąd → Internet
/// (fail-closed). Poza Windows (brak ADS) → lokalny.
pub fn zone_of_path(path: &Path) -> FileZone {
    if !cfg!(windows) {
        return FileZone::Local;
    }
    match std::fs::read_to_string(zone_stream(path)) {
        Ok(text) => FileZone::from_zone_identifier(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileZone::Local,
        Err(_) => FileZone::Internet,
    }
}

/// Office przez COM.
#[derive(Debug)]
pub struct WinOffice {
    config: OfficeConfig,
    #[cfg(windows)]
    host: sta::StaHost,
}

impl WinOffice {
    /// Port z konfiguracji (wątek STA startuje leniwie).
    pub fn new(config: OfficeConfig) -> Self {
        Self {
            #[cfg(windows)]
            host: sta::StaHost::new(config.call_timeout_ms, config.max_hung_threads),
            config,
        }
    }

    /// Konfiguracja.
    pub fn config(&self) -> &OfficeConfig {
        &self.config
    }
}

#[cfg(windows)]
impl OfficePort for WinOffice {
    fn available(&self, app: OfficeApp) -> bool {
        sta::registered(app)
    }

    fn zone_of(&self, path: &Path) -> FileZone {
        zone_of_path(path)
    }

    fn read(&self, file: &OfficeFile, query: &OfficeQuery) -> Result<OfficeContent, OfficeError> {
        let copy = WorkCopy::create(&self.config.work_dir, file)?;
        let (path, app, zone, query) = (
            copy.path().to_path_buf(),
            file.app,
            file.zone,
            query.clone(),
        );
        let content = self.host.call("odczyt dokumentu", move |ctx| match app {
            OfficeApp::Word => word::read(ctx, &path, zone, &query),
            OfficeApp::Excel => excel::read(ctx, &path, zone, &query),
        })?;
        check_session(&content.session)?;
        Ok(content)
    }

    fn edit(&self, file: &OfficeFile, edits: &[OfficeEdit]) -> Result<OfficeEdited, OfficeError> {
        check_edits(file, edits)?;
        let copy = WorkCopy::create(&self.config.work_dir, file)?;
        let (path, app, edits) = (copy.path().to_path_buf(), file.app, edits.to_vec());
        let (applied, replacements, session) =
            self.host.call("edycja dokumentu", move |ctx| match app {
                OfficeApp::Word => word::edit(ctx, &path, &edits),
                OfficeApp::Excel => excel::edit(ctx, &path, &edits),
            })?;
        check_session(&session)?;
        Ok(OfficeEdited {
            bytes: copy.read_back()?,
            applied,
            replacements,
            session,
        })
    }
}

#[cfg(not(windows))]
impl OfficePort for WinOffice {
    fn available(&self, _app: OfficeApp) -> bool {
        false
    }

    fn zone_of(&self, path: &Path) -> FileZone {
        zone_of_path(path)
    }

    fn read(&self, _file: &OfficeFile, _query: &OfficeQuery) -> Result<OfficeContent, OfficeError> {
        Err(OfficeError::NotInstalled(
            "automatyzacja Office wymaga Windows".into(),
        ))
    }

    fn edit(&self, file: &OfficeFile, edits: &[OfficeEdit]) -> Result<OfficeEdited, OfficeError> {
        check_edits(file, edits)?;
        Err(OfficeError::NotInstalled(
            "automatyzacja Office wymaga Windows".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_copy_is_private_and_removed() {
        let base = std::env::temp_dir().join(format!("alfa-office-test-{}", std::process::id()));
        let file = OfficeFile::new("Raport.docx", b"abc".to_vec(), FileZone::Internet)
            .unwrap_or_else(|e| panic!("{e}"));
        let dir;
        {
            let copy = WorkCopy::create(&base, &file).unwrap_or_else(|e| panic!("{e}"));
            assert!(copy.path().starts_with(&base));
            assert!(copy.path().ends_with("Raport.docx"));
            assert_eq!(copy.read_back().unwrap_or_default(), b"abc");
            dir = copy
                .path()
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            assert!(dir.exists());
        }
        assert!(!dir.exists(), "katalog kopii usunięty");
        let _ = std::fs::remove_dir_all(&base);
        assert!(zone_stream(Path::new("C:/a.docx")).ends_with("a.docx:Zone.Identifier"));
        if !cfg!(windows) {
            assert_eq!(zone_of_path(Path::new("/nie/ma")), FileZone::Local);
        }
    }

    #[test]
    fn off_windows_port_refuses() {
        let office = WinOffice::new(OfficeConfig::new(std::env::temp_dir()));
        assert_eq!(office.config().call_timeout_ms, OFFICE_CALL_TIMEOUT_MS);
        if cfg!(windows) {
            return;
        }
        let file =
            OfficeFile::new("a.docx", vec![], FileZone::Local).unwrap_or_else(|e| panic!("{e}"));
        assert!(!office.available(OfficeApp::Word));
        assert!(
            office
                .read(&file, &OfficeQuery::Text { max_chars: 1 })
                .is_err()
        );
        let mut web = file.clone();
        web.zone = FileZone::Internet;
        let edit = [platform_apps_contract::OfficeEdit::InsertText {
            position: platform_apps_contract::TextPosition::End,
            text: "x".into(),
        }];
        assert_eq!(office.edit(&web, &edit), Err(OfficeError::ProtectedView));
    }
}
