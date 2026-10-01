//! Zrzuty wirtualnego pulpitu: okna rysowane kolorami (od dołu kolejności Z), pola haseł na
//! czerwono, okna chronione przed przechwyceniem na czarno; maskowanie i skalowanie wspólne
//! z implementacją Windows (`mask_plan`, `mask_and_scale` z kontraktu), PNG bez kompresji.

use platform_contract::{
    CaptureRequest, CaptureTarget, GuiError, MaskReason, MaskedArea, RgbaImage, ScreenCapturePort,
    ScreenRect, Screenshot, WindowState, encode_png, mask_and_scale, mask_plan,
};

use super::{FakeDesktop, State, Win};

/// Kolor tła pulpitu.
pub const DESKTOP_COLOR: [u8; 4] = [0, 0, 64, 255];
/// Kolor pola hasła na surowym obrazie (po maskowaniu nie może zostać ani jeden piksel).
pub const PASSWORD_COLOR: [u8; 4] = [255, 0, 0, 255];

fn draw(img: &mut RgbaImage, source: &ScreenRect, w: &Win) {
    let local = |r: &ScreenRect| ScreenRect {
        left: r.left - source.left,
        top: r.top - source.top,
        right: r.right - source.left,
        bottom: r.bottom - source.top,
    };
    let color = if w.capture_blocked {
        [0, 0, 0, 255]
    } else {
        w.color
    };
    img.fill_rect(&local(&w.info.rect), color);
    if w.capture_blocked {
        return;
    }
    for e in w
        .elements
        .iter()
        .filter(|e| e.node.is_password && !e.node.offscreen)
    {
        if let Some(r) = e.node.rect.intersect(&w.info.rect) {
            img.fill_rect(&local(&r), PASSWORD_COLOR);
        }
    }
}

fn render(s: &State, source: &ScreenRect, only: Option<&Win>) -> Result<RgbaImage, GuiError> {
    let width = u32::try_from(source.width()).unwrap_or(0);
    let height = u32::try_from(source.height()).unwrap_or(0);
    let mut img = RgbaImage::filled(width, height, DESKTOP_COLOR)
        .ok_or_else(|| GuiError::Policy("pusty albo zbyt duży obszar zrzutu".into()))?;
    match only {
        Some(w) => draw(&mut img, source, w),
        None => {
            for w in s.windows.iter().rev() {
                if w.info.state != WindowState::Minimized {
                    draw(&mut img, source, w);
                }
            }
        }
    }
    Ok(img)
}

impl FakeDesktop {
    fn capture_source<'a>(
        &self,
        s: &'a State,
        req: &CaptureRequest,
    ) -> Result<(ScreenRect, Option<&'a Win>), GuiError> {
        let screen = s.monitors.iter().fold(ScreenRect::default(), |acc, m| {
            if acc.is_empty() {
                m.rect
            } else {
                ScreenRect {
                    left: acc.left.min(m.rect.left),
                    top: acc.top.min(m.rect.top),
                    right: acc.right.max(m.rect.right),
                    bottom: acc.bottom.max(m.rect.bottom),
                }
            }
        });
        match req.target {
            CaptureTarget::Window { window } => {
                let w = s.win(window)?;
                self.guard.check(w.info.pid, &w.info.image, "zrzut okna")?;
                if req.is_masked_app(&w.info.image) {
                    return Err(GuiError::Policy("aplikacja na deny-liście zrzutów".into()));
                }
                if w.info.state == WindowState::Minimized {
                    return Err(GuiError::Policy(
                        "okno zminimalizowane — najpierw je przywróć".into(),
                    ));
                }
                Ok((w.info.rect, Some(w)))
            }
            CaptureTarget::Monitor { index } => s
                .monitors
                .iter()
                .find(|m| m.index == index)
                .map(|m| (m.rect, None))
                .ok_or_else(|| GuiError::ElementNotFound(format!("monitor {index}"))),
            CaptureTarget::Region { rect } => rect
                .intersect(&screen)
                .map(|r| (r, None))
                .ok_or_else(|| GuiError::Policy("obszar poza ekranem".into())),
        }
    }
}

impl ScreenCapturePort for FakeDesktop {
    fn capture(&self, request: &CaptureRequest) -> Result<Screenshot, GuiError> {
        request.validate()?;
        let mut s = self.lock();
        let (source, only) = self.capture_source(&s, request)?;
        let raw = render(&s, &source, only)?;
        let windows: Vec<_> = s.windows.iter().map(|w| w.info.clone()).collect();
        let candidates: Vec<&Win> = s
            .windows
            .iter()
            .filter(|w| only.is_none_or(|o| o.info.id == w.info.id))
            .filter(|w| !w.info.protected && w.info.state != WindowState::Minimized)
            .filter(|w| w.info.rect.intersect(&source).is_some())
            .collect();
        let mut passwords = Vec::new();
        let mut unverified = Vec::new();
        for w in candidates {
            if s.uia_hang || s.password_check_fails.contains(&w.info.id) {
                if let Some(rect) = source.intersect(&w.info.rect) {
                    unverified.push(MaskedArea {
                        rect,
                        reason: MaskReason::Unverified,
                    });
                }
                continue;
            }
            passwords.extend(
                w.elements
                    .iter()
                    .filter(|e| e.node.is_password && !e.node.offscreen)
                    .map(|e| e.node.rect),
            );
        }
        let mut masked = mask_plan(&source, &windows, request, &passwords);
        masked.extend(unverified);
        let (image, black_frame) = mask_and_scale(raw, &source, &masked, request);
        let shot = Screenshot {
            png: encode_png(&image),
            width: image.width,
            height: image.height,
            source,
            masked,
            black_frame,
        };
        s.last_capture = Some(image);
        Ok(shot)
    }
}
