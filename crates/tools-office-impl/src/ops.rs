//! `office_read` i `office_edit`.

use std::path::PathBuf;

use platform_apps_contract::{CellValue, OfficeApp, OfficeFile, app_for_file, check_session};
use safety_broker_contract::{Capability, TaintSource};
use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, UndoRef, UndoService,
    parse_args, report_untrusted, text,
};
use tools_office_contract::{
    EVENT_EDIT, EVENT_READ, EditArgs, EditOutput, ReadArgs, ReadOutput, SheetOut, same_app,
    to_edits, to_query, version_path,
};
use undo_journal_contract::StepCtx;

use crate::core::{Core, Step, blocking, invalid, office_failure};

fn app_name(app: OfficeApp) -> &'static str {
    match app {
        OfficeApp::Word => "word",
        OfficeApp::Excel => "excel",
    }
}

fn cell_json(c: &CellValue) -> serde_json::Value {
    match c {
        CellValue::Text(t) => serde_json::Value::String(text::redact_secrets(t)),
        other => serde_json::to_value(other).unwrap_or_default(),
    }
}

impl Core {
    async fn open_file(&self, path: &str, app: OfficeApp, action: &str) -> Step<OfficeFile> {
        let bytes = self.read_file(path, action).await?;
        let office = self.office.clone();
        let p = PathBuf::from(path);
        let zone = blocking(move || office.zone_of(&p)).await?;
        let file =
            OfficeFile::new(path, bytes, zone).map_err(|e| Box::new(office_failure(&e, action)))?;
        if file.app != app {
            return Err(invalid("rodzaj dokumentu"));
        }
        Ok(file)
    }

    /// `office_read`.
    pub(crate) async fn read(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ReadArgs = parse_args(args)?;
        let action = "odczyt dokumentu";
        let path = self.resolve(&a.path, ctx, action)?;
        let app = app_for_file(&path).ok_or_else(|| invalid("nieobsługiwany format pliku"))?;
        let query = to_query(
            &a,
            app,
            self.config.text_max_chars,
            self.config.table_max_cells,
        )
        .map_err(invalid)?;
        let caps = vec![
            Capability::FsRead(self.scope(&path)?),
            Self::app_capability(app)?,
        ];
        let auths = self.authorize(ctx, m, caps, action).await?;
        let result = async {
            let file = self.open_file(&path, app, action).await?;
            let office = self.office.clone();
            let content = blocking(move || office.read(&file, &query))
                .await?
                .and_then(|c| check_session(&c.session).map(|()| c))
                .map_err(|e| Box::new(office_failure(&e, action)))?;
            Ok::<_, Box<ToolOutcome>>(content)
        }
        .await;
        self.gate.release(&auths).await;
        let content = result?;
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let out = ReadOutput {
            path: path.clone(),
            app: app_name(app).into(),
            text: content.text.as_deref().map(text::redact_secrets),
            tables: content
                .tables
                .iter()
                .map(|t| {
                    t.iter()
                        .map(|r| r.iter().map(|c| text::redact_secrets(c)).collect())
                        .collect()
                })
                .collect(),
            cells: content
                .cells
                .iter()
                .map(|r| r.iter().map(cell_json).collect())
                .collect(),
            sheets: content
                .sheets
                .iter()
                .map(|s| SheetOut {
                    name: s.name.clone(),
                    used_range: s.used_range.clone(),
                })
                .collect(),
            truncated: content.truncated,
            protected_view: content.session.protected_view,
            macros_present: content.session.macros_present,
        };
        let payload =
            serde_json::json!({"tool": m.id, "app": out.app, "protected_view": out.protected_view});
        self.emit(EVENT_READ, payload, ctx).await;
        let mut body = format!("Dokument „{path}” ({})", out.app);
        if out.protected_view {
            body.push_str(" — plik z Internetu, otwarty w Protected View (tylko odczyt)");
        }
        if out.macros_present {
            body.push_str(" — zawiera makra (wyłączone, nie uruchomiono)");
        }
        body.push_str(":\n");
        if let Some(t) = &out.text {
            body.push_str(t);
        }
        for (i, t) in out.tables.iter().enumerate() {
            body.push_str(&format!("\nTabela {}:\n", i + 1));
            for r in t {
                body.push_str(&r.join(" | "));
                body.push('\n');
            }
        }
        for r in &out.cells {
            let cells: Vec<String> = r.iter().map(|v| v.to_string()).collect();
            body.push_str(&cells.join("\t"));
            body.push('\n');
        }
        for s in &out.sheets {
            body.push_str(&format!("Arkusz „{}”: {}\n", s.name, s.used_range));
        }
        let (body, cut) = text::truncate_chars(&body, self.config.output_max_chars);
        let mut outcome = ToolOutcome::ok(body, serde_json::to_value(&out).unwrap_or_default())
            .untrusted(TaintSource::File);
        outcome.truncated = cut || out.truncated;
        outcome.approval = auths.iter().find_map(|a| a.approval);
        Ok(outcome)
    }

    fn output_path(
        &self,
        a: &EditArgs,
        original: &str,
        ctx: &ToolCtx,
        action: &str,
    ) -> Step<String> {
        let out = match &a.output {
            Some(raw) => self.resolve(raw, ctx, action)?,
            None => version_path(original, |p| self.exists(p))
                .ok_or_else(|| invalid("brak wolnej nazwy nowej wersji — podaj `output`"))?,
        };
        if out.to_lowercase().replace('/', "\\") == original.to_lowercase().replace('/', "\\") {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::Policy,
                "zapis edycji w miejscu oryginału (oryginał zostaje bez zmian — wybierz inną ścieżkę)",
            )));
        }
        if !same_app(original, &out) {
            return Err(invalid("nowa wersja musi mieć format tej samej aplikacji"));
        }
        Ok(out)
    }

    /// `office_edit`.
    pub(crate) async fn edit(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: EditArgs = parse_args(args)?;
        let action = "edycja dokumentu";
        let original = self.resolve(&a.path, ctx, action)?;
        let app = app_for_file(&original).ok_or_else(|| invalid("nieobsługiwany format pliku"))?;
        let edits = to_edits(&a).map_err(invalid)?;
        if edits.iter().any(|e| e.app() != app) {
            return Err(invalid("edycja nie pasuje do rodzaju dokumentu"));
        }
        let output = self.output_path(&a, &original, ctx, action)?;
        let caps = vec![
            Capability::FsRead(self.scope(&original)?),
            Capability::FsWrite(self.scope(&output)?),
            Self::app_capability(app)?,
        ];
        let auths = self.authorize(ctx, m, caps, action).await?;
        let result = self
            .edit_with(ctx, m, app, &original, &output, edits, action)
            .await;
        self.gate.release(&auths).await;
        let mut out = result?;
        out.approval = auths.iter().find_map(|a| a.approval);
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    async fn edit_with(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        app: OfficeApp,
        original: &str,
        output: &str,
        edits: Vec<platform_apps_contract::OfficeEdit>,
        action: &str,
    ) -> Step<ToolOutcome> {
        let file = self.open_file(original, app, action).await?;
        if file.zone.is_untrusted() {
            return Ok(office_failure(
                &platform_apps_contract::OfficeError::ProtectedView,
                action,
            ));
        }
        if ctx.cancel.is_cancelled() {
            return Ok(ToolOutcome::cancelled(action));
        }
        let office = self.office.clone();
        let edited = blocking(move || office.edit(&file, &edits))
            .await?
            .and_then(|e| check_session(&e.session).map(|()| e))
            .map_err(|e| Box::new(office_failure(&e, action)))?;
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
            .map_err(|e| {
                Box::new(ToolOutcome::failed(
                    ToolErrorKind::Internal,
                    format!("Nie wykonano: {action} — dziennik cofania: {e}."),
                ))
            })?;
        let journal = self.journal.clone();
        let (p, bytes) = (PathBuf::from(output), edited.bytes.clone());
        let written = blocking(move || {
            journal.write(step, &p, &bytes)?;
            journal.commit_step(step)
        })
        .await?;
        let summary = match written {
            Ok(s) => s,
            Err(e) => {
                let _ = self.journal.abort_step(step);
                return Ok(ToolOutcome::failed(
                    ToolErrorKind::Io,
                    format!("Nie wykonano: zapis nowej wersji „{output}” — {e}."),
                ));
            }
        };
        let data = EditOutput {
            original: original.to_owned(),
            output: output.to_owned(),
            applied: edited.applied,
            replacements: edited.replacements,
            bytes: edited.bytes.len() as u64,
            undo_step: Some(step.0),
        };
        let payload =
            serde_json::json!({"tool": m.id, "applied": data.applied, "undo_step": step.0});
        self.emit(EVENT_EDIT, payload, ctx).await;
        let mut out = ToolOutcome::ok(
            format!(
                "Zapisałam nową wersję „{output}” ({} zmian, zamian tekstu: {}). Oryginał „{original}” bez zmian; krok można cofnąć.",
                data.applied, data.replacements
            ),
            serde_json::to_value(&data).unwrap_or_default(),
        );
        out.undo = Some(UndoRef {
            service: UndoService::Journal,
            id: step.0,
            text: summary.text,
        });
        Ok(out)
    }
}
