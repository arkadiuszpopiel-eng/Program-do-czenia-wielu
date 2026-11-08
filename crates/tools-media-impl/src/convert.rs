//! `media_convert`: źródło (nagłówek, rodzaj zgodny z formatem) → `Transcoder` (anulowanie
//! przebiegu przerywa proces) → **nowy** plik przez dziennik cofania (nigdy nadpisanie).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, UndoRef, UndoService,
    parse_args,
};
use tools_media_contract::{
    ConvertArgs, ConvertJob, ConvertOutput, EVENT_CONVERT, input_demuxer, output_path,
};
use undo_journal_contract::StepCtx;

use crate::core::{Core, Step, blocking, convert_failure, invalid};

fn same_path(a: &str, b: &str) -> bool {
    a.replace('/', "\\")
        .eq_ignore_ascii_case(&b.replace('/', "\\"))
}

impl Core {
    fn new_file(&self, a: &ConvertArgs, src: &str, ctx: &ToolCtx, action: &str) -> Step<String> {
        let out = match &a.output {
            Some(raw) => self.resolve(raw, ctx, action)?,
            None => output_path(src, a.format.ext(), |p| self.fs.exists(Path::new(p)))
                .ok_or_else(|| invalid("brak wolnej nazwy nowego pliku — podaj `output`"))?,
        };
        if same_path(&out, src) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::Policy,
                "zapis wyniku w miejscu źródła (źródło zostaje bez zmian — wybierz inną nazwę)",
            )));
        }
        if self.fs.exists(Path::new(&out)) {
            return Err(Box::new(ToolOutcome::failed(
                ToolErrorKind::AlreadyExists,
                format!(
                    "Nie wykonano: {action} — „{out}” już istnieje (nie nadpisuję). Wybierz inną nazwę `output`."
                ),
            )));
        }
        Ok(out)
    }

    /// Konwersja na wątku blokującym; anulowanie przebiegu ustawia flagę procesu.
    pub(crate) async fn transcode(
        &self,
        job: ConvertJob,
        ctx: &ToolCtx,
        action: &str,
    ) -> Step<Vec<u8>> {
        let flag = Arc::new(AtomicBool::new(false));
        let (watch_flag, token) = (flag.clone(), ctx.cancel.clone());
        let watcher = tokio::spawn(async move {
            token.cancelled().await;
            watch_flag.store(true, Ordering::SeqCst);
        });
        let t = self.transcoder.clone();
        let r = blocking(move || t.convert(&job, flag)).await;
        watcher.abort();
        r?.map_err(|e| Box::new(convert_failure(&e, action)))
    }

    /// `media_convert`.
    pub(crate) async fn convert(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ConvertArgs = parse_args(args)?;
        let (start_ms, duration_ms) = a.validate().map_err(invalid)?;
        let action = "konwersja pliku";
        let src = self.resolve(&a.path, ctx, action)?;
        // Bez ffmpeg nie pytamy właściciela o zgodę na coś, czego i tak nie da się zrobić.
        let t = self.transcoder.clone();
        blocking(move || t.available())
            .await?
            .map_err(|e| Box::new(convert_failure(&e, action)))?;
        let dst = self.new_file(&a, &src, ctx, action)?;
        let caps = vec![self.capability(&src, false)?, self.capability(&dst, true)?];
        let auths = self.authorize(ctx, m, caps, action).await?;
        let job = |input_format: String| ConvertJob {
            input: PathBuf::from(&src),
            input_format,
            target: a.format,
            start_ms,
            duration_ms,
            max_side: a.max_side,
            max_output_bytes: self.config.max_output_bytes,
        };
        let result = async {
            let info = self.probe(&src, action).await?;
            if !a.format.accepts(info.kind) || input_demuxer(&info.format).is_none() {
                return Err(invalid(format!(
                    "nie da się przekonwertować {} ({:?}) do {}",
                    info.format,
                    info.kind,
                    a.format.ext()
                )));
            }
            let bytes = self.transcode(job(info.format), ctx, action).await?;
            if ctx.cancel.is_cancelled() {
                return Err(Box::new(ToolOutcome::cancelled(action)));
            }
            let len = bytes.len() as u64;
            let (step, summary) = self.write_new(ctx, m, &dst, &bytes, action).await?;
            Ok((step, summary, len))
        }
        .await;
        self.gate.release(&auths).await;
        let (step, summary, len) = result?;
        let data = ConvertOutput {
            original: src.clone(),
            output: dst.clone(),
            format: a.format,
            bytes: len,
            undo_step: Some(step),
        };
        let payload = serde_json::json!({"tool": m.id, "format": a.format, "undo_step": step});
        self.emit(EVENT_CONVERT, payload, ctx).await;
        let mut out = ToolOutcome::ok(
            format!(
                "Zapisałam nowy plik „{dst}” ({}). Źródło „{src}” bez zmian; krok można cofnąć.",
                a.format.ext()
            ),
            serde_json::to_value(&data).unwrap_or_default(),
        );
        out.undo = Some(UndoRef {
            service: UndoService::Journal,
            id: step,
            text: summary,
        });
        out.approval = auths.iter().find_map(|x| x.approval);
        Ok(out)
    }

    /// Nowy plik przez dziennik cofania (pre-image: brak pliku → „Cofnij” go usuwa).
    async fn write_new(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        dst: &str,
        bytes: &[u8],
        action: &str,
    ) -> Step<(u64, String)> {
        if self.fs.exists(Path::new(dst)) {
            return Err(Box::new(ToolOutcome::failed(
                ToolErrorKind::AlreadyExists,
                format!(
                    "Nie wykonano: {action} — „{dst}” pojawił się w trakcie konwersji (nie nadpisuję)."
                ),
            )));
        }
        let internal = |e: undo_journal_contract::UndoError| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::Internal,
                format!("Nie wykonano: {action} — dziennik cofania: {e}."),
            ))
        };
        let step = self
            .journal
            .begin_step(StepCtx {
                session: ctx.holder.session.clone(),
                agent: ctx.holder.agent.clone(),
                run: ctx.run.clone(),
                turn: None,
                label: ctx.undo_label(m),
                allow_irreversible: false,
            })
            .map_err(internal)?;
        let journal = self.journal.clone();
        let (p, data) = (PathBuf::from(dst), bytes.to_vec());
        let written = blocking(move || {
            journal.write(step, &p, &data)?;
            journal.commit_step(step)
        })
        .await?;
        match written {
            Ok(summary) => Ok((step.0, summary.text)),
            Err(e) => {
                let _ = self.journal.abort_step(step);
                Err(Box::new(ToolOutcome::failed(
                    ToolErrorKind::Io,
                    format!("Nie wykonano: zapis „{dst}” — {e}."),
                )))
            }
        }
    }
}
