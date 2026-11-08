//! Pozyskanie obrazu: zrzut z maskowaniem (Broker `gui.control`) albo plik (deny-lista przed
//! Brokerem, `fs.read(plik)`, wymiary i format z nagłówka przed odczytem całości).

use std::path::Path;

use lib_media::{ByteSource, FileSource, Limits, MediaKind, probe};
use platform_contract::CaptureTarget;
use safety_broker_contract::{ApprovalId, Capability, TaintSource};
use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, paths,
};
use tools_screen_contract::{MaskOut, ScreenToolsConfig, to_output, to_request};
use tools_vision_contract::ImageSpec;
use tools_window_contract::gui::{self, Step};

use crate::Core;

/// Obraz gotowy do OCR albo dla modelu.
pub(crate) struct Acquired {
    pub(crate) bytes: Vec<u8>,
    pub(crate) media_type: String,
    pub(crate) source: &'static str,
    pub(crate) path: Option<String>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale: f64,
    pub(crate) origin: (i32, i32),
    pub(crate) masked: Vec<MaskOut>,
    pub(crate) black_frame: bool,
    pub(crate) taint: TaintSource,
    pub(crate) approval: Option<ApprovalId>,
}

pub(crate) fn invalid(text: impl std::fmt::Display) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(
        ToolErrorKind::InvalidArgs,
        format!("Niepoprawne argumenty: {text}."),
    ))
}

fn failed(kind: ToolErrorKind, text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

impl Core {
    /// Obraz ze źródła; `max_bytes` — limit obrazu (zrzut PNG albo plik).
    pub(crate) async fn acquire(
        &self,
        spec: ImageSpec,
        m: &ToolManifest,
        ctx: &ToolCtx,
        action: &str,
        max_bytes: u64,
    ) -> Step<Acquired> {
        match spec {
            ImageSpec::Screen(args) => self.screen(&args, m, ctx, action, max_bytes).await,
            ImageSpec::File(raw) => self.file(&raw, m, ctx, action, max_bytes).await,
        }
    }

    async fn screen(
        &self,
        args: &tools_screen_contract::CaptureArgs,
        m: &ToolManifest,
        ctx: &ToolCtx,
        action: &str,
        max_bytes: u64,
    ) -> Step<Acquired> {
        let config = ScreenToolsConfig {
            masked_apps: self.config.masked_apps.clone(),
            ..ScreenToolsConfig::default()
        };
        let request = to_request(args, &config).map_err(invalid)?;
        let cap = match request.target {
            CaptureTarget::Window { window } => {
                let desktop = self.desktop.clone();
                let target =
                    gui::blocking(move || gui::target_window(&*desktop, window, "zrzut okna"))
                        .await??;
                gui::app_capability(&target, action)?
            }
            CaptureTarget::Monitor { .. } | CaptureTarget::Region { .. } => {
                gui::desktop_capability()?
            }
        };
        let auth = gui::authorize(&self.gate, ctx, m, &cap, true, action).await?;
        if ctx.cancel.is_cancelled() {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(ToolOutcome::cancelled(action)));
        }
        let port = self.capture.clone();
        let shot = gui::blocking(move || port.capture(&request)).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let shot = shot?.map_err(|e| Box::new(gui::gui_outcome(&e, action)))?;
        if shot.png.len() as u64 > max_bytes {
            return Err(failed(
                ToolErrorKind::Io,
                format!(
                    "Nie wykonano: {action} — zrzut ma {} B, więcej niż limit {max_bytes} B. Wybierz mniejszy obszar.",
                    shot.png.len()
                ),
            ));
        }
        let out = to_output(&shot);
        Ok(Acquired {
            bytes: shot.png,
            media_type: "image/png".into(),
            source: "screen",
            path: None,
            width: out.width,
            height: out.height,
            scale: out.scale,
            origin: (out.source_x, out.source_y),
            masked: out.masked,
            black_frame: out.black_frame,
            taint: TaintSource::Screen,
            approval: auth.approval,
        })
    }

    /// Ścieżka od modelu → ścieżka sprawdzona (postać, deny-lista także po dowiązaniach).
    fn resolve(&self, raw: &str, ctx: &ToolCtx, action: &str) -> Step<String> {
        let path = paths::resolve_path(raw, ctx.workdir.as_deref(), &self.env)
            .map_err(|e| invalid(format!("ścieżka: {e}")))?;
        let denied = paths::protected_with_links(&path, |p| {
            self.deny.is_denied_path(p, &self.env) || paths::has_credential_segment(p)
        });
        if denied {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                &format!("{action} „{path}”"),
            )));
        }
        Ok(path)
    }

    async fn file(
        &self,
        raw: &str,
        m: &ToolManifest,
        ctx: &ToolCtx,
        action: &str,
        max_bytes: u64,
    ) -> Step<Acquired> {
        let path = self.resolve(raw, ctx, action)?;
        let scope =
            paths::exact_scope(&path, &self.env).map_err(|e| invalid(format!("ścieżka: {e}")))?;
        let cap = Capability::FsRead(scope);
        let auth = gui::authorize(&self.gate, ctx, m, &cap, true, action).await?;
        let files = self.files.clone();
        let (p, limit, max_pixels) = (path.clone(), max_bytes, self.config.max_pixels);
        let read =
            gui::blocking(move || read_image(&*files, Path::new(&p), limit, max_pixels)).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let (bytes, info) = read?.map_err(|(kind, why)| {
            failed(kind, format!("Nie wykonano: {action} „{path}” — {why}."))
        })?;
        Ok(Acquired {
            bytes,
            media_type: info.mime,
            source: "file",
            path: Some(path),
            width: info.width.unwrap_or(0),
            height: info.height.unwrap_or(0),
            scale: 1.0,
            origin: (0, 0),
            masked: Vec::new(),
            black_frame: false,
            taint: TaintSource::File,
            approval: auth.approval,
        })
    }
}

/// Plik obrazu: rozmiar, nagłówek (format, wymiary, liczba pikseli) i dopiero potem całość.
fn read_image(
    files: &dyn lib_media::RangeRead,
    path: &Path,
    max_bytes: u64,
    max_pixels: u64,
) -> Result<(Vec<u8>, lib_media::MediaInfo), (ToolErrorKind, String)> {
    let io = |e: platform_contract::PlatformError| {
        let kind = match e {
            platform_contract::PlatformError::NotFound(_) => ToolErrorKind::NotFound,
            platform_contract::PlatformError::InvalidPath(_) => ToolErrorKind::InvalidArgs,
            _ => ToolErrorKind::Io,
        };
        (kind, e.to_string())
    };
    let mut src = FileSource::open(files, path).map_err(io)?;
    let size = src.size();
    if size > max_bytes {
        return Err((
            ToolErrorKind::InvalidArgs,
            format!("plik ma {size} B — więcej niż limit {max_bytes} B"),
        ));
    }
    let info = probe(&mut src, &Limits::default()).map_err(|e| {
        (
            ToolErrorKind::InvalidArgs,
            format!("to nie jest obsługiwany obraz ({e})"),
        )
    })?;
    if info.kind != MediaKind::Image {
        return Err((
            ToolErrorKind::InvalidArgs,
            format!("plik to {} ({}), nie obraz", info.format, info.mime),
        ));
    }
    match info.pixels() {
        Some(px) if px <= max_pixels => {}
        Some(px) => {
            return Err((
                ToolErrorKind::InvalidArgs,
                format!(
                    "obraz ma {px} pikseli — więcej niż limit {max_pixels} (ochrona przed bombą dekompresyjną)"
                ),
            ));
        }
        None => {
            return Err((
                ToolErrorKind::Unsupported,
                format!("nie da się ustalić wymiarów obrazu {}", info.format),
            ));
        }
    }
    let bytes = files.read_all(path, max_bytes).map_err(io)?;
    Ok((bytes, info))
}
