//! Zrzuty ekranu i okien (F6, PLAN §7.2, §8.7; THREAT_MODEL S26): tylko na żądanie, zawsze
//! z maskowaniem okien Alfy/Brokera ([`crate::TargetGuard`]), aplikacji z deny-listy zrzutów
//! (menedżery haseł, okna poświadczeń, aplikacje dostawców) i pól haseł (UIA `IsPassword`);
//! okno, którego pól haseł nie dało się sprawdzić (UIA zawiesiło się), jest maskowane w całości
//! (fail-closed). Skalowanie do limitu, PNG. Plan maskowania liczy [`mask_plan`] — ta sama logika
//! w atrapie i implementacji.

use serde::{Deserialize, Serialize};

use crate::desktop::{DesktopWindow, WindowState};
use crate::gui::{GuiError, SENSITIVE_APPS, ScreenRect, image_file_name};
use crate::image::{MASK_COLOR, RgbaImage, encode_png_with};
use crate::window::WindowId;

/// Aplikacje zawsze maskowane na zrzutach (menedżery haseł, okna poświadczeń Windows) — ta
/// sama lista co cele chronione przed UIA i wejściem ([`SENSITIVE_APPS`], fala 5, PT-25).
pub const DEFAULT_MASKED_APPS: [&str; 8] = SENSITIVE_APPS;
/// Domyślny limit dłuższego boku zrzutu (px) — wygodny dla modeli wizyjnych.
pub const DEFAULT_CAPTURE_MAX_SIDE: u32 = 1_568;
/// Zakres limitu boku.
pub const CAPTURE_SIDE_RANGE: (u32, u32) = (64, 4_096);

/// Co przechwycić.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum CaptureTarget {
    /// Jedno okno (`PrintWindow` — także zasłonięte).
    Window {
        /// Okno.
        window: WindowId,
    },
    /// Cały monitor.
    Monitor {
        /// Indeks monitora.
        index: u32,
    },
    /// Obszar ekranu wirtualnego.
    Region {
        /// Prostokąt.
        rect: ScreenRect,
    },
}

/// Żądanie zrzutu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureRequest {
    /// Cel.
    pub target: CaptureTarget,
    /// Limit szerokości wyniku (px).
    pub max_width: u32,
    /// Limit wysokości wyniku (px).
    pub max_height: u32,
    /// Dodatkowe aplikacje maskowane (np. aplikacje dostawców planów); [`DEFAULT_MASKED_APPS`]
    /// obowiązują zawsze.
    pub masked_apps: Vec<String>,
}

impl CaptureRequest {
    /// Żądanie z domyślnym limitem rozmiaru.
    pub fn new(target: CaptureTarget) -> Self {
        Self {
            target,
            max_width: DEFAULT_CAPTURE_MAX_SIDE,
            max_height: DEFAULT_CAPTURE_MAX_SIDE,
            masked_apps: Vec::new(),
        }
    }

    /// Walidacja limitów i obszaru.
    pub fn validate(&self) -> Result<(), GuiError> {
        let (lo, hi) = CAPTURE_SIDE_RANGE;
        if !(lo..=hi).contains(&self.max_width) || !(lo..=hi).contains(&self.max_height) {
            return Err(GuiError::Policy(format!(
                "limit boku zrzutu poza {lo}–{hi} px"
            )));
        }
        if let CaptureTarget::Region { rect } = self.target
            && (rect.is_empty() || rect.width() > 16_384 || rect.height() > 16_384)
        {
            return Err(GuiError::Policy(
                "pusty albo zbyt duży obszar zrzutu".into(),
            ));
        }
        Ok(())
    }

    /// Czy aplikacja (obraz procesu) jest maskowana.
    pub fn is_masked_app(&self, image: &str) -> bool {
        let name = image_file_name(image);
        DEFAULT_MASKED_APPS
            .iter()
            .copied()
            .chain(self.masked_apps.iter().map(String::as_str))
            .any(|a| image_file_name(a) == name)
    }
}

/// Dlaczego obszar zamaskowano.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskReason {
    /// Okno Alfy/Brokera albo nieznanego procesu.
    ProtectedWindow,
    /// Aplikacja z deny-listy zrzutów.
    MaskedApp,
    /// Pole hasła (`IsPassword`).
    PasswordField,
    /// Pól haseł okna nie dało się sprawdzić (UIA) — okno w całości.
    Unverified,
}

/// Zamaskowany obszar (współrzędne ekranu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskedArea {
    /// Prostokąt.
    pub rect: ScreenRect,
    /// Powód.
    pub reason: MaskReason,
}

/// Wynik zrzutu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screenshot {
    /// PNG.
    pub png: Vec<u8>,
    /// Szerokość obrazu.
    pub width: u32,
    /// Wysokość obrazu.
    pub height: u32,
    /// Przechwycony obszar ekranu (przed skalowaniem).
    pub source: ScreenRect,
    /// Zamaskowane obszary.
    pub masked: Vec<MaskedArea>,
    /// Cała klatka czarna (okno chronione przed przechwyceniem, DRM) — wykrycie, §7.3.
    pub black_frame: bool,
}

/// Port zrzutów.
pub trait ScreenCapturePort: Send + Sync {
    /// Zrzut z maskowaniem (odmowa dla okna chronionego i aplikacji maskowanej jako celu).
    fn capture(&self, request: &CaptureRequest) -> Result<Screenshot, GuiError>;
}

/// Okna do zamaskowania w obszarze `source`: chronione i z deny-listy (dla zrzutu jednego okna —
/// tylko ono, bo `PrintWindow` nie rysuje innych) oraz pola haseł.
pub fn mask_plan(
    source: &ScreenRect,
    windows: &[DesktopWindow],
    request: &CaptureRequest,
    password_rects: &[ScreenRect],
) -> Vec<MaskedArea> {
    let only = match request.target {
        CaptureTarget::Window { window } => Some(window),
        _ => None,
    };
    let mut out: Vec<MaskedArea> = windows
        .iter()
        .filter(|w| w.state != WindowState::Minimized && only.is_none_or(|id| id == w.id))
        .filter_map(|w| {
            // Aplikacja z deny-listy zrzutów (menedżer haseł jest też chroniony) — powód
            // dokładniejszy niż „okno chronione”.
            let reason = if request.is_masked_app(&w.image) {
                MaskReason::MaskedApp
            } else if w.protected {
                MaskReason::ProtectedWindow
            } else {
                return None;
            };
            source
                .intersect(&w.rect)
                .map(|rect| MaskedArea { rect, reason })
        })
        .collect();
    out.extend(
        password_rects
            .iter()
            .filter_map(|r| source.intersect(r))
            .map(|rect| MaskedArea {
                rect,
                reason: MaskReason::PasswordField,
            }),
    );
    out
}

/// Maskuje (współrzędne ekranu → obrazu), wykrywa czarną klatkę (przed maskowaniem) i skaluje.
pub fn mask_and_scale(
    mut raw: RgbaImage,
    source: &ScreenRect,
    masks: &[MaskedArea],
    request: &CaptureRequest,
) -> (RgbaImage, bool) {
    let black = raw.is_black();
    for m in masks {
        let local = ScreenRect {
            left: m.rect.left - source.left,
            top: m.rect.top - source.top,
            right: m.rect.right - source.left,
            bottom: m.rect.bottom - source.top,
        };
        raw.fill_rect(&local, MASK_COLOR);
    }
    (
        raw.scale_to_fit(request.max_width, request.max_height),
        black,
    )
}

/// Składa wynik: maskowanie, skalowanie, PNG z podanym kompresorem zlib.
pub fn finish_capture(
    raw: RgbaImage,
    source: ScreenRect,
    masked: Vec<MaskedArea>,
    request: &CaptureRequest,
    zlib: &dyn Fn(&[u8]) -> Vec<u8>,
) -> Screenshot {
    let (image, black_frame) = mask_and_scale(raw, &source, &masked, request);
    Screenshot {
        png: encode_png_with(&image, zlib),
        width: image.width,
        height: image.height,
        source,
        masked,
        black_frame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::zlib_stored;

    fn window(id: u64, image: &str, rect: ScreenRect, protected: bool) -> DesktopWindow {
        DesktopWindow {
            id: WindowId(id),
            title: String::new(),
            class_name: String::new(),
            pid: 1,
            image: image.into(),
            rect,
            monitor: 0,
            dpi: 96,
            state: WindowState::Normal,
            focused: false,
            z_order: 0,
            elevated: false,
            protected,
        }
    }

    #[test]
    fn plan_masks_protected_masked_and_passwords() {
        let screen = ScreenRect::from_xywh(0, 0, 100, 100);
        let ws = vec![
            window(
                1,
                "alfa-broker-ui.exe",
                ScreenRect::from_xywh(10, 10, 20, 20),
                true,
            ),
            window(
                2,
                r"C:\KeePass\KeePass.exe",
                ScreenRect::from_xywh(50, 50, 80, 80),
                false,
            ),
            window(
                3,
                "notepad.exe",
                ScreenRect::from_xywh(0, 0, 100, 100),
                false,
            ),
            window(4, "claude.exe", ScreenRect::from_xywh(0, 90, 10, 10), false),
        ];
        let mut req = CaptureRequest::new(CaptureTarget::Monitor { index: 0 });
        req.masked_apps = vec!["Claude.exe".into()];
        let pw = [ScreenRect::from_xywh(95, 0, 50, 5)];
        let plan = mask_plan(&screen, &ws, &req, &pw);
        let reasons: Vec<MaskReason> = plan.iter().map(|m| m.reason).collect();
        assert_eq!(
            reasons,
            vec![
                MaskReason::ProtectedWindow,
                MaskReason::MaskedApp,
                MaskReason::MaskedApp,
                MaskReason::PasswordField
            ]
        );
        assert_eq!(plan[1].rect, ScreenRect::from_xywh(50, 50, 50, 50));
        assert_eq!(plan[3].rect, ScreenRect::from_xywh(95, 0, 5, 5));
        let one = CaptureRequest::new(CaptureTarget::Window {
            window: WindowId(3),
        });
        assert!(mask_plan(&screen, &ws, &one, &[]).is_empty());
        let mut minimized = ws[0].clone();
        minimized.state = WindowState::Minimized;
        assert!(mask_plan(&screen, &[minimized], &req, &[]).is_empty());
    }

    #[test]
    fn finish_masks_before_scaling() {
        let raw = RgbaImage::filled(100, 50, [250, 250, 250, 255]).unwrap();
        let src = ScreenRect::from_xywh(1000, 0, 100, 50);
        let masks = vec![MaskedArea {
            rect: ScreenRect::from_xywh(1000, 0, 50, 50),
            reason: MaskReason::PasswordField,
        }];
        let mut req = CaptureRequest::new(CaptureTarget::Region { rect: src });
        req.max_width = 64;
        assert!(req.validate().is_ok());
        let (img, black) = mask_and_scale(raw.clone(), &src, &masks, &req);
        assert!(!black);
        assert_eq!((img.width, img.height), (64, 32));
        assert_eq!(img.pixel(0, 0), Some(MASK_COLOR));
        assert_eq!(img.pixel(63, 0), Some([250, 250, 250, 255]));
        let shot = finish_capture(raw, src, masks, &req, &zlib_stored);
        assert_eq!(shot.width, 64);
        assert!(shot.png.starts_with(&[0x89, b'P']));
        let black = RgbaImage::filled(4, 4, [0, 0, 0, 255]).unwrap();
        assert!(finish_capture(black, src, vec![], &req, &zlib_stored).black_frame);
    }

    #[test]
    fn request_validation() {
        let mut r = CaptureRequest::new(CaptureTarget::Monitor { index: 0 });
        assert!(r.validate().is_ok());
        r.max_width = 10;
        assert!(r.validate().is_err());
        let r = CaptureRequest::new(CaptureTarget::Region {
            rect: ScreenRect::default(),
        });
        assert!(r.validate().is_err());
        assert!(r.is_masked_app(r"C:\x\CredentialUIBroker.exe"));
        assert!(!r.is_masked_app("notepad.exe"));
    }
}
