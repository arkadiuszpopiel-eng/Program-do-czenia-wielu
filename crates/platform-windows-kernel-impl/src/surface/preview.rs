//! Podgląd okna Sędziego Cieni w pliku BMP: ten sam układ (`layout`) i to samo rysowanie
//! (`paint`) co w prawdziwym oknie, tylko w bitmapie w pamięci — do przeglądu wyglądu w testach
//! bez pokazywania okna na ekranie właściciela.

#![allow(unsafe_code)]

use platform_contract::{PlatformError, SurfaceView};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GdiFlush, SelectObject,
};

use super::layout::measure;
use super::button::ButtonState;
use super::paint::Look;
use super::state::kind;
use super::theme::palette_of;

/// Bitmapa 32-bitowa (BGRA, wiersze od góry) jako plik BMP.
fn bmp(w: i32, h: i32, pixels: &[u8]) -> Vec<u8> {
    let size = u32::try_from(pixels.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(pixels.len() + 54);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(54 + size).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&(-h).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(pixels);
    out
}

/// Rysuje kartę jak okno (motyw `dark`, skala `dpi`, pomoc rozwinięta albo nie; fokus na
/// przycisku bezpiecznym) i zwraca plik BMP obszaru klienta.
pub(super) fn render(
    view: &SurfaceView,
    dark: bool,
    dpi: u32,
    help_open: bool,
    max_height: i32,
) -> Result<Vec<u8>, PlatformError> {
    view.validate()?;
    let mut look = Look::new(dpi, palette_of(dark));
    look.tone = view.tone;
    // SAFETY: kontekst w pamięci i sekcja DIB tylko tej funkcji, zwalniane przed powrotem;
    // piksele czytane po `GdiFlush`, dopóki sekcja żyje.
    unsafe {
        let mem = CreateCompatibleDC(None);
        let m = measure(mem, &look.fonts, view, help_open, max_height);
        let (w, h) = m.client;
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).unwrap_or(40),
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(dib) = CreateDIBSection(
            Some(mem),
            &raw const info,
            DIB_RGB_COLORS,
            &raw mut bits,
            None,
            0,
        ) else {
            let _ = DeleteDC(mem);
            return Err(PlatformError::Io("podgląd: brak bitmapy".into()));
        };
        let old = SelectObject(mem, dib.into());
        look.paint(mem, view, &m);
        for (id, r) in m.controls() {
            let label = view
                .buttons
                .iter()
                .find(|b| b.id == id)
                .map_or("", |b| b.label.as_str());
            let st = ButtonState {
                pressed: false,
                focused: id == view.initial_focus,
                hover: false,
                kind: kind(view, help_open, id),
            };
            look.button(mem, r, label, &st);
        }
        let _ = GdiFlush();
        let len = usize::try_from(w * h * 4).unwrap_or(0);
        let pixels = std::slice::from_raw_parts(bits.cast::<u8>(), len).to_vec();
        SelectObject(mem, old);
        let _ = DeleteObject(dib.into());
        let _ = DeleteDC(mem);
        Ok(bmp(w, h, &pixels))
    }
}
