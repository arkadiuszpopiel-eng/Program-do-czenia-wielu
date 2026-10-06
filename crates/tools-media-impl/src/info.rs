//! `media_info`: nagłówki pliku (`lib-media`) po zgodzie `fs.read(plik)`.

use lib_media::{MediaInfo, MediaKind};
use tools_common_contract::{ToolCtx, ToolManifest, ToolOutcome, parse_args};
use tools_media_contract::{EVENT_INFO, InfoArgs, InfoOutput};

use crate::core::{Core, Step, invalid};

/// Opis po polsku dla modelu.
pub(crate) fn summary(info: &MediaInfo) -> String {
    let kind = match info.kind {
        MediaKind::Image => "obraz",
        MediaKind::Audio => "dźwięk",
        MediaKind::Video => "wideo",
    };
    let mut parts = vec![format!("{kind} {} ({})", info.format, info.mime)];
    if let (Some(w), Some(h)) = (info.width, info.height) {
        parts.push(format!("{w}×{h} px"));
    }
    if let Some(ms) = info.duration_ms {
        parts.push(format!(
            "{}:{:02}.{:03}",
            ms / 60_000,
            ms / 1000 % 60,
            ms % 1000
        ));
    }
    if let Some(rate) = info.sample_rate {
        parts.push(format!("{rate} Hz"));
    }
    if let Some(ch) = info.channels.filter(|_| info.kind != MediaKind::Image) {
        parts.push(format!("kanały: {ch}"));
    }
    if !info.codecs.is_empty() {
        parts.push(format!("kodeki: {}", info.codecs.join(", ")));
    }
    if let Some(f) = info.frames.filter(|f| *f > 1) {
        parts.push(format!("klatki: {f}"));
    }
    parts.push(format!("{} B", info.size_bytes));
    let mut text = parts.join(", ");
    if info.partial {
        text.push_str(" (nagłówek rozpoznany częściowo)");
    }
    text
}

impl Core {
    /// `media_info`.
    pub(crate) async fn info(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: InfoArgs = parse_args(args)?;
        if a.path.trim().is_empty() {
            return Err(invalid("pusta `path`"));
        }
        let action = "odczyt informacji o pliku";
        let path = self.resolve(&a.path, ctx, action)?;
        let caps = vec![self.capability(&path, false)?];
        let auths = self.authorize(ctx, m, caps, action).await?;
        let info = self.probe(&path, action).await;
        self.gate.release(&auths).await;
        let info = info?;
        let payload = serde_json::json!({
            "tool": m.id, "kind": info.kind, "format": info.format, "partial": info.partial,
        });
        self.emit(EVENT_INFO, payload, ctx).await;
        let text = format!("Plik „{path}”: {}.", summary(&info));
        let data = InfoOutput { path, info };
        let mut out = ToolOutcome::ok(text, serde_json::to_value(&data).unwrap_or_default());
        out.approval = auths.iter().find_map(|a| a.approval);
        Ok(out)
    }
}
