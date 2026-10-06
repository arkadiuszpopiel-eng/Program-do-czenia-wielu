//! `media_play`: WAV (albo inny format przekonwertowany do WAV przez ffmpeg) → mono
//! w częstotliwości obsługiwanej przez mikser → `AudioPlayer` (kolejka mówienia, w tle).

use std::path::{Path, PathBuf};

use tools_common_contract::{ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, parse_args};
use tools_media_contract::{
    AudioClip, ConvertJob, EVENT_PLAY, PlayArgs, PlayError, PlayOutput, TargetFormat,
};
use voice_audio_contract::wav::decode_wav;
use voice_audio_contract::{Resampler, SUPPORTED_RATES, downmix};

use crate::core::{Core, Step, blocking, invalid, platform_failure};

/// Klip mono w częstotliwości z listy miksera (inne → 48 kHz).
pub(crate) fn to_clip(
    pcm: &[f32],
    channels: u16,
    rate: u32,
    agent: &str,
    label: &str,
) -> AudioClip {
    let mono = downmix(pcm, channels.max(1));
    let (samples, sample_rate) = if SUPPORTED_RATES.contains(&rate) {
        (mono, rate)
    } else {
        let mut r = Resampler::new(rate, 48_000);
        let mut out = Vec::with_capacity(mono.len() * 2);
        r.process(&mono, &mut out);
        r.flush(&mut out);
        (out, 48_000)
    };
    AudioClip {
        samples,
        sample_rate,
        channels: 1,
        agent: agent.to_owned(),
        label: label.to_owned(),
    }
}

fn play_failure(e: &PlayError, action: &str) -> ToolOutcome {
    let kind = match e {
        PlayError::Unavailable(_) => ToolErrorKind::Unsupported,
        PlayError::Format(_) => ToolErrorKind::InvalidArgs,
    };
    ToolOutcome::failed(kind, format!("Nie wykonano: {action} — {e}."))
}

impl Core {
    /// Bajty WAV: plik WAV wprost albo konwersja (fragment do limitu czasu odtwarzania).
    async fn wav_bytes(&self, path: &str, ctx: &ToolCtx, action: &str) -> Step<(Vec<u8>, bool)> {
        let info = self.probe(path, action).await?;
        if info
            .duration_ms
            .is_some_and(|d| d > self.config.max_play_ms)
        {
            return Err(invalid(format!(
                "nagranie trwa ponad {} s — wytnij fragment (media_convert z `duration_s`)",
                self.config.max_play_ms / 1000
            )));
        }
        if info.format == "wav" && info.codecs.iter().any(|c| c.starts_with("pcm_")) {
            let files = self.files.clone();
            let (p, max) = (PathBuf::from(path), self.config.max_play_bytes);
            let bytes = blocking(move || files.read_all(&p, max))
                .await?
                .map_err(|e| Box::new(platform_failure(&e, action)))?;
            return Ok((bytes, false));
        }
        if info.kind == lib_media::MediaKind::Image {
            return Err(invalid(format!("{} to obraz, nie dźwięk", info.format)));
        }
        let job = ConvertJob {
            input: PathBuf::from(path),
            input_format: info.format,
            target: TargetFormat::Wav,
            start_ms: None,
            duration_ms: Some(self.config.max_play_ms),
            max_side: None,
            max_output_bytes: self.config.max_play_bytes,
        };
        let bytes = self.transcode(job, ctx, action).await?;
        Ok((bytes, true))
    }

    /// `media_play`.
    pub(crate) async fn play(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: PlayArgs = parse_args(args)?;
        if a.path.trim().is_empty() {
            return Err(invalid("pusta `path`"));
        }
        let action = "odtwarzanie dźwięku";
        let path = self.resolve(&a.path, ctx, action)?;
        let caps = vec![self.capability(&path, false)?];
        let auths = self.authorize(ctx, m, caps, action).await?;
        let wav = self.wav_bytes(&path, ctx, action).await;
        self.gate.release(&auths).await;
        let (bytes, converted) = wav?;
        let (pcm, format) =
            decode_wav(&bytes).map_err(|e| invalid(format!("nieczytelny WAV ({e})")))?;
        let agent = ctx.holder.agent.as_ref().map_or("alfa", |a| a.as_str());
        let label = Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let clip = to_clip(&pcm, format.channels, format.sample_rate, agent, &label);
        if clip.duration_ms() > self.config.max_play_ms {
            return Err(invalid("nagranie dłuższe niż limit odtwarzania"));
        }
        if ctx.cancel.is_cancelled() {
            return Ok(ToolOutcome::cancelled(action));
        }
        let ticket = self
            .player
            .play(clip, ctx.cancel.clone())
            .await
            .map_err(|e| Box::new(play_failure(&e, action)))?;
        let payload = serde_json::json!({
            "tool": m.id, "playback": ticket.id, "duration_ms": ticket.duration_ms, "queued": ticket.queued,
        });
        self.emit(EVENT_PLAY, payload, ctx).await;
        let wait = if ticket.queued {
            " Głośnik jest zajęty — zagra po bieżącej wypowiedzi."
        } else {
            ""
        };
        let data = PlayOutput {
            path: path.clone(),
            playback: ticket.id,
            duration_ms: ticket.duration_ms,
            queued: ticket.queued,
            converted,
        };
        let mut out = ToolOutcome::ok(
            format!(
                "Odtwarzam „{label}” ({} s) w tle.{wait} Mowa użytkownika je ścisza i zatrzymuje.",
                ticket.duration_ms.div_ceil(1000)
            ),
            serde_json::to_value(&data).unwrap_or_default(),
        );
        out.approval = auths.iter().find_map(|x| x.approval);
        Ok(out)
    }
}
