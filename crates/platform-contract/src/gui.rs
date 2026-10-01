//! Wspólne typy computer use (PLAN §7, §8.2; THREAT_MODEL S11, S26): prostokąt ekranu, błąd
//! operacji GUI i strażnik celów — okna procesów Alfy, Brokera, Broker-UI i helpera nigdy nie są
//! celem wejścia syntetycznego, akcji UIA, zmiany okna ani odczytu (zrzut je maskuje).
//!
//! Strażnik działa w porcie, **tuż przed każdą akcją**, niezależnie od tokenu `gui.control`
//! z Brokera (obrona w głąb: Broker odmawia `gui.control(alfa*.exe)`, port i tak sprawdza PID
//! i obraz procesu okna docelowego w chwili wysłania, a UIPI blokuje wejście do Broker-UI).

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Prostokąt w pikselach fizycznych ekranu wirtualnego (lewa/górna włącznie, prawa/dolna wyłącznie).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenRect {
    /// Lewa krawędź.
    pub left: i32,
    /// Górna krawędź.
    pub top: i32,
    /// Prawa krawędź (wyłącznie).
    pub right: i32,
    /// Dolna krawędź (wyłącznie).
    pub bottom: i32,
}

impl ScreenRect {
    /// Nowy prostokąt z położenia i rozmiaru.
    pub fn from_xywh(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x.saturating_add(width.max(0)),
            bottom: y.saturating_add(height.max(0)),
        }
    }

    /// Szerokość (≥ 0).
    pub fn width(&self) -> i32 {
        self.right.saturating_sub(self.left).max(0)
    }

    /// Wysokość (≥ 0).
    pub fn height(&self) -> i32 {
        self.bottom.saturating_sub(self.top).max(0)
    }

    /// Czy pusty.
    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    /// Czy punkt leży wewnątrz.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// Część wspólna (albo `None`, gdy rozłączne).
    pub fn intersect(&self, other: &ScreenRect) -> Option<ScreenRect> {
        let r = ScreenRect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (!r.is_empty()).then_some(r)
    }

    /// Środek (punkt kliknięcia elementu).
    pub fn center(&self) -> (i32, i32) {
        (
            self.left.saturating_add(self.width() / 2),
            self.top.saturating_add(self.height() / 2),
        )
    }
}

/// Błąd operacji GUI (UIA, wejście syntetyczne, okna, zrzuty).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiError {
    /// Cel należy do procesu chronionego (Alfa, Broker, Broker-UI, helper) albo procesu
    /// o nieznanym obrazie (fail-closed).
    #[error("cel chroniony: {0}")]
    ProtectedTarget(String),
    /// Okno docelowe zmieniło się przed wysłaniem (fokus/okno pod kursorem inne niż cel).
    #[error("cel zmienił się przed akcją: oczekiwano okna {expected}, jest {actual:?}")]
    TargetChanged {
        /// Oczekiwane okno.
        expected: u64,
        /// Faktyczne okno (brak = pulpit/nic).
        actual: Option<u64>,
    },
    /// Użytkownik dotknął myszy lub klawiatury — fizyczne wejście ma pierwszeństwo (PLAN §7.4).
    #[error("przerwano: użytkownik użył myszy lub klawiatury (wysłano {sent} paczek zdarzeń)")]
    UserInterrupted {
        /// Ile paczek zdarzeń wysłano przed przerwaniem.
        sent: u32,
    },
    /// Użytkownik właśnie pracuje (świeże fizyczne wejście) — wejście nie jest przejmowane.
    #[error("użytkownik właśnie używa myszy lub klawiatury — nie przejmuję sterowania")]
    UserActive,
    /// Przekroczony limit czasu (UIA potrafi wisieć na zawieszonej aplikacji).
    #[error("limit czasu {ms} ms: {op}")]
    Timeout {
        /// Operacja.
        op: String,
        /// Limit (ms).
        ms: u64,
    },
    /// Zasady (skrót systemowy, pole hasła, limit długości, zły argument).
    #[error("zasady: {0}")]
    Policy(String),
    /// Element nie istnieje (zniknął albo nieaktualne odwołanie).
    #[error("nie znaleziono elementu: {0}")]
    ElementNotFound(String),
    /// Element nie obsługuje wymaganego wzorca UIA albo jest wyłączony.
    #[error("element nie obsługuje akcji: {0}")]
    PatternUnsupported(String),
    /// Okno procesu podniesionego (administratora) — UIPI blokuje wejście bez helpera `uiAccess`.
    #[error("okno administratora: sterowanie wymaga helpera uiAccess (UIPI)")]
    Elevated,
    /// Anulowano (kill-switch, „stop”, anulowanie przebiegu).
    #[error("anulowano")]
    Cancelled,
    /// Błąd platformy.
    #[error("{0}")]
    Platform(PlatformError),
}

impl From<PlatformError> for GuiError {
    fn from(value: PlatformError) -> Self {
        Self::Platform(value)
    }
}

/// Obrazy procesów chronionych zawsze (suma list Brokera i `platform-windows`; porównanie bez
/// wielkości liter po nazwie pliku).
pub const PROTECTED_IMAGES: [&str; 9] = [
    "alfa.exe",
    "alfa-core.exe",
    "alfa-desktop.exe",
    "alfa-broker.exe",
    "alfa-broker-ui.exe",
    "alfa-watchdog.exe",
    "alfa-updater.exe",
    "alfa-uiaccess-helper.exe",
    "alfa-mcp-proxy.exe",
];

/// Nazwa pliku z pełnej ścieżki (`C:\a\b.EXE` → `b.exe`), małymi literami, bez końcowych kropek i spacji.
pub fn image_file_name(image: &str) -> String {
    image
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(['.', ' '])
        .to_lowercase()
}

/// Alias 8.3 (`ALFA-B~1.EXE`) pasujący do długiej nazwy: ten sam początek (≤ 6 znaków przed `~`)
/// i rozszerzenie — traktowany jak trafienie (fail-closed).
fn short_name_matches(name: &str, protected: &str) -> bool {
    let Some((stem, rest)) = name.split_once('~') else {
        return false;
    };
    let ext = |n: &str| n.rsplit_once('.').map(|(_, e)| e.to_owned());
    !stem.is_empty()
        && rest.chars().next().is_some_and(|c| c.is_ascii_digit())
        && ext(name) == ext(protected)
        && protected
            .replace([' ', '.'], "")
            .starts_with(&stem.replace(['.', ' '], ""))
}

/// Strażnik celów GUI: procesy chronione po PID, nazwie obrazu i katalogu instalacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetGuard {
    /// Nazwy plików wykonywalnych (małymi literami).
    pub images: Vec<String>,
    /// Dodatkowe PID-y (np. Broker-UI uruchomiony przez usługę); bieżący proces — zawsze.
    pub pids: Vec<u32>,
    /// Katalogi (prefiksy ścieżek, małymi literami, `\`), z których każdy obraz jest chroniony
    /// (np. `%LOCALAPPDATA%\Alfa`).
    pub image_dirs: Vec<String>,
}

impl Default for TargetGuard {
    fn default() -> Self {
        Self::baseline()
    }
}

impl TargetGuard {
    /// Lista bazowa ([`PROTECTED_IMAGES`] + bieżący proces).
    pub fn baseline() -> Self {
        Self {
            images: PROTECTED_IMAGES.map(String::from).to_vec(),
            pids: Vec::new(),
            image_dirs: Vec::new(),
        }
    }

    /// Dodaje chronione PID-y (builder).
    #[must_use]
    pub fn with_pids(mut self, pids: impl IntoIterator<Item = u32>) -> Self {
        self.pids.extend(pids);
        self
    }

    /// Dodaje chronione obrazy (builder; lista bazowa zostaje).
    #[must_use]
    pub fn with_images<S: AsRef<str>>(mut self, images: impl IntoIterator<Item = S>) -> Self {
        self.images
            .extend(images.into_iter().map(|i| image_file_name(i.as_ref())));
        self
    }

    /// Dodaje chronione katalogi instalacji (builder).
    #[must_use]
    pub fn with_image_dirs<S: AsRef<str>>(mut self, dirs: impl IntoIterator<Item = S>) -> Self {
        self.image_dirs.extend(dirs.into_iter().map(|d| {
            let d = d.as_ref().replace('/', "\\").to_lowercase();
            if d.ends_with('\\') {
                d
            } else {
                format!("{d}\\")
            }
        }));
        self
    }

    /// Czy proces jest chroniony. Pusty/nieznany obraz = chroniony (fail-closed).
    pub fn is_protected(&self, pid: u32, image: &str) -> bool {
        let name = image_file_name(image);
        let path = image.replace('/', "\\").to_lowercase();
        pid == std::process::id()
            || self.pids.contains(&pid)
            || name.is_empty()
            || PROTECTED_IMAGES
                .iter()
                .copied()
                .chain(self.images.iter().map(String::as_str))
                .any(|p| p == name || short_name_matches(&name, p))
            || self.image_dirs.iter().any(|d| path.starts_with(d.as_str()))
    }

    /// Sprawdzenie przed akcją: `Err(ProtectedTarget)` dla procesu chronionego.
    pub fn check(&self, pid: u32, image: &str, what: &str) -> Result<(), GuiError> {
        if self.is_protected(pid, image) {
            let shown = if image.is_empty() {
                "nieznany proces".to_owned()
            } else {
                image_file_name(image)
            };
            return Err(GuiError::ProtectedTarget(format!(
                "{what}: okno procesu {shown} (PID {pid}) — Alfa, Broker albo proces nieznany; \
                 agentka nie steruje nim ani go nie odczytuje"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_geometry() {
        let r = ScreenRect::from_xywh(10, 20, 100, 50);
        assert_eq!((r.width(), r.height()), (100, 50));
        assert!(r.contains(10, 20) && !r.contains(110, 20) && !r.contains(9, 20));
        assert_eq!(r.center(), (60, 45));
        let o = ScreenRect::from_xywh(100, 60, 50, 50);
        assert_eq!(
            r.intersect(&o),
            Some(ScreenRect::from_xywh(100, 60, 10, 10))
        );
        assert_eq!(r.intersect(&ScreenRect::from_xywh(500, 500, 1, 1)), None);
        assert!(ScreenRect::from_xywh(0, 0, -5, 3).is_empty());
    }

    #[test]
    fn guard_is_fail_closed() {
        let g = TargetGuard::baseline()
            .with_pids([4242])
            .with_images(["KeePass.exe"])
            .with_image_dirs([r"C:\Users\ala\AppData\Local\Alfa"]);
        assert!(g.is_protected(std::process::id(), "notepad.exe"));
        assert!(g.is_protected(4242, "notepad.exe"));
        assert!(g.is_protected(7, r"C:\Program Files\Alfa\ALFA-BROKER-UI.EXE"));
        assert!(g.is_protected(7, "alfa-desktop.exe. "));
        assert!(g.is_protected(7, "ALFA-B~1.EXE"), "alias 8.3");
        assert!(g.is_protected(7, "keepass.exe"));
        assert!(g.is_protected(7, r"c:\users\ala\appdata\local\alfa\v2\x.exe"));
        assert!(g.is_protected(7, ""), "nieznany obraz = chroniony");
        assert!(!g.is_protected(7, r"C:\Windows\notepad.exe"));
        assert!(!g.is_protected(7, "NOTEP~1.EXE"));
        let err = g.check(7, "alfa.exe", "klik").unwrap_err();
        assert!(matches!(err, GuiError::ProtectedTarget(m) if m.contains("alfa.exe")));
        assert!(g.check(7, "notepad.exe", "klik").is_ok());
        assert!(
            matches!(g.check(9, "", "x"), Err(GuiError::ProtectedTarget(m)) if m.contains("nieznany"))
        );
        assert_eq!(image_file_name(r"C:\A\B.EXE"), "b.exe");
    }

    #[test]
    fn errors_have_polish_messages() {
        let e: GuiError = PlatformError::Io("x".into()).into();
        assert!(e.to_string().contains("x"));
        for e in [
            GuiError::UserInterrupted { sent: 2 },
            GuiError::UserActive,
            GuiError::Elevated,
            GuiError::Cancelled,
            GuiError::Timeout {
                op: "drzewo".into(),
                ms: 5,
            },
            GuiError::TargetChanged {
                expected: 1,
                actual: None,
            },
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
