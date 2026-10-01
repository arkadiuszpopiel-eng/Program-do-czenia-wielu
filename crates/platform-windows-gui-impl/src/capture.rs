//! Zrzuty GDI (wybór: BitBlt ekranu z `CAPTUREBLT` + `PrintWindow(PW_RENDERFULLCONTENT)` dla
//! okna zamiast Windows.Graphics.Capture — synchronicznie, bez WinRT/D3D11, bez żółtej ramki
//! i asynchronicznej puli klatek; okna z `WDA_EXCLUDEFROMCAPTURE` i DRM wychodzą czarne, co
//! wykrywamy). Maskowanie wspólne z atrapą: okna chronione i z deny-listy, pola haseł z UIA;
//! okno, którego pól haseł nie dało się sprawdzić w budżecie czasu, maskowane w całości.

#![allow(unsafe_code)]

use std::time::{Duration, Instant};

use platform_contract::{
    CaptureRequest, CaptureTarget, DesktopWindow, GuiError, MaskReason, MaskedArea, PlatformError,
    RgbaImage, ScreenRect, Screenshot, WindowState, finish_capture, mask_plan,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
    CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HBITMAP, HDC,
    HGDIOBJ, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, PW_RENDERFULLCONTENT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use crate::win::{frame_and_window_rect, hwnd_of, last_error};
use crate::zlib::zlib_best;

/// Kontekst pamięci GDI z bitmapą (zwalniane w `Drop`).
struct MemCanvas {
    screen: HDC,
    mem: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    width: i32,
    height: i32,
}

impl MemCanvas {
    fn new(width: i32, height: i32) -> Result<Self, GuiError> {
        // SAFETY: DC ekranu i zgodny DC pamięci z bitmapą; wszystko zwalnia `Drop`.
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            if screen.is_invalid() || mem.is_invalid() || bitmap.is_invalid() {
                let err = last_error("CreateCompatibleBitmap");
                if !bitmap.is_invalid() {
                    let _ = DeleteObject(bitmap.into());
                }
                if !mem.is_invalid() {
                    let _ = DeleteDC(mem);
                }
                ReleaseDC(None, screen);
                return Err(err);
            }
            let old = SelectObject(mem, bitmap.into());
            Ok(Self {
                screen,
                mem,
                bitmap,
                old,
                width,
                height,
            })
        }
    }

    fn pixels(&self) -> Result<RgbaImage, GuiError> {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.width,
                biHeight: -self.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let w = u32::try_from(self.width).unwrap_or(0);
        let h = u32::try_from(self.height).unwrap_or(0);
        let mut buf = vec![0u8; w as usize * h as usize * 4];
        // SAFETY: bitmapa jest odznaczana na czas `GetDIBits` (wymóg API); bufor ma w×h×4 B.
        let lines = unsafe {
            SelectObject(self.mem, self.old);
            let n = GetDIBits(
                self.mem,
                self.bitmap,
                0,
                h,
                Some(buf.as_mut_ptr().cast()),
                &raw mut info,
                DIB_RGB_COLORS,
            );
            SelectObject(self.mem, self.bitmap.into());
            n
        };
        if lines <= 0 {
            return Err(last_error("GetDIBits"));
        }
        RgbaImage::from_bgra(w, h, buf).ok_or_else(|| GuiError::Policy("zbyt duży obraz".into()))
    }
}

impl Drop for MemCanvas {
    fn drop(&mut self) {
        // SAFETY: zwolnienie zasobów utworzonych w `new`, każdy raz.
        unsafe {
            SelectObject(self.mem, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.mem);
            ReleaseDC(None, self.screen);
        }
    }
}

fn virtual_screen() -> ScreenRect {
    // SAFETY: odczyt metryk systemu.
    unsafe {
        ScreenRect::from_xywh(
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

fn grab_screen(src: &ScreenRect) -> Result<RgbaImage, GuiError> {
    let canvas = MemCanvas::new(src.width(), src.height())?;
    // SAFETY: kopiowanie z DC ekranu do bitmapy o rozmiarze obszaru.
    unsafe {
        BitBlt(
            canvas.mem,
            0,
            0,
            src.width(),
            src.height(),
            Some(canvas.screen),
            src.left,
            src.top,
            SRCCOPY | CAPTUREBLT,
        )
    }
    .map_err(|e| crate::win::win_error("BitBlt", &e))?;
    canvas.pixels()
}

fn grab_window(window: &DesktopWindow) -> Result<(RgbaImage, ScreenRect), GuiError> {
    let hwnd = hwnd_of(window.id);
    let (frame, full) = frame_and_window_rect(hwnd);
    let canvas = MemCanvas::new(full.width(), full.height())?;
    // SAFETY: rysowanie okna (także zasłoniętego) do DC pamięci.
    let ok = unsafe { PrintWindow(hwnd, canvas.mem, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)) };
    if !ok.as_bool() {
        return Err(last_error("PrintWindow"));
    }
    let img = canvas.pixels()?;
    // Przycięcie do ramki DWM (bez niewidocznych krawędzi).
    let (dx, dy) = (frame.left - full.left, frame.top - full.top);
    let (w, h) = (frame.width().max(1), frame.height().max(1));
    let mut out = RgbaImage::filled(
        u32::try_from(w).unwrap_or(1),
        u32::try_from(h).unwrap_or(1),
        [0, 0, 0, 255],
    )
    .ok_or_else(|| GuiError::Policy("zbyt duże okno".into()))?;
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (
                u32::try_from(x + dx).unwrap_or(u32::MAX),
                u32::try_from(y + dy).unwrap_or(u32::MAX),
            );
            if let Some(p) = img.pixel(sx, sy) {
                let i = (y as usize * w as usize + x as usize) * 4;
                out.pixels[i..i + 4].copy_from_slice(&p);
            }
        }
    }
    Ok((out, frame))
}

/// Zrzut: `windows` = lista okien (kolejność Z), `passwords(okno)` = pola haseł z UIA.
pub(crate) fn capture(
    request: &CaptureRequest,
    windows: &[DesktopWindow],
    monitors: &[platform_contract::MonitorInfo],
    budget_ms: u64,
    mut passwords: impl FnMut(&DesktopWindow, u64) -> Result<Vec<ScreenRect>, GuiError>,
) -> Result<Screenshot, GuiError> {
    request.validate()?;
    let (raw, source, only) = match request.target {
        CaptureTarget::Window { window } => {
            let w = windows
                .iter()
                .find(|w| w.id == window)
                .ok_or_else(|| GuiError::ElementNotFound(format!("okno {}", window.0)))?;
            if w.protected {
                return Err(GuiError::ProtectedTarget(format!(
                    "zrzut okna procesu {}",
                    platform_contract::image_file_name(&w.image)
                )));
            }
            if request.is_masked_app(&w.image) {
                return Err(GuiError::Policy("aplikacja na deny-liście zrzutów".into()));
            }
            if w.state == WindowState::Minimized {
                return Err(GuiError::Policy(
                    "okno zminimalizowane — najpierw je przywróć".into(),
                ));
            }
            let (img, frame) = grab_window(w)?;
            (img, frame, Some(w.id))
        }
        CaptureTarget::Monitor { index } => {
            let m = monitors
                .iter()
                .find(|m| m.index == index)
                .ok_or_else(|| GuiError::ElementNotFound(format!("monitor {index}")))?;
            (grab_screen(&m.rect)?, m.rect, None)
        }
        CaptureTarget::Region { rect } => {
            let r = rect
                .intersect(&virtual_screen())
                .ok_or_else(|| GuiError::Policy("obszar poza ekranem".into()))?;
            (grab_screen(&r)?, r, None)
        }
    };
    let deadline = Instant::now() + Duration::from_millis(budget_ms);
    let mut rects = Vec::new();
    let mut unverified = Vec::new();
    for w in windows.iter().filter(|w| {
        only.is_none_or(|id| id == w.id)
            && !w.protected
            && !request.is_masked_app(&w.image)
            && w.state != WindowState::Minimized
    }) {
        let Some(visible) = source.intersect(&w.rect) else {
            continue;
        };
        let left = deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        let left = u64::try_from(left).unwrap_or(0);
        match (left > 0).then(|| passwords(w, left)) {
            Some(Ok(found)) => rects.extend(found),
            _ => unverified.push(MaskedArea {
                rect: visible,
                reason: MaskReason::Unverified,
            }),
        }
    }
    let mut masked = mask_plan(&source, windows, request, &rects);
    masked.extend(unverified);
    if raw.width == 0 {
        return Err(GuiError::Platform(PlatformError::Io("pusty obraz".into())));
    }
    Ok(finish_capture(raw, source, masked, request, &zlib_best))
}
