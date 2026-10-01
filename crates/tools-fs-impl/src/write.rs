//! Narzędzia zmieniające stan: zapis, przeniesienie, kopia, zmiana nazwy, Kosz, trwałe
//! usunięcie (zawsze potwierdzenie właściciela), katalog. Każda mutacja = krok dziennika
//! cofania (brak wpisu = brak operacji).

use risk_classifier_contract::Destructiveness;
use safety_broker_contract::Capability;
use tools_common_contract::{
    Authorization, ToolCtx, ToolErrorKind, ToolIntent, ToolManifest, ToolOutcome, ToolStatus,
    parse_args,
};
use tools_fs_contract::{
    FromToArgs, INTENT_CONFIRM_DELETE_PERMANENT, MutationOutput, PathArgs, RenameArgs, WriteArgs,
    WriteMode,
};
use undo_journal_contract::UndoError;

use crate::core::{Core, Step, platform_failure};

/// Nazwa pliku-znacznika do tworzenia pustego katalogu przez `FsPort` (zapis tworzy brakujące
/// katalogi, cofnięcie je usuwa); znacznik jest usuwany w tym samym kroku dziennika.
pub const MKDIR_MARKER: &str = ".alfa-mkdir.tmp";

fn out_value(
    paths: Vec<String>,
    bytes: u64,
    undo_step: Option<u64>,
    exists: bool,
) -> serde_json::Value {
    serde_json::to_value(MutationOutput {
        paths,
        bytes,
        undo_step,
        exists,
    })
    .unwrap_or_default()
}

impl Core {
    async fn mutate_access(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        caps: Vec<(
            Capability,
            risk_classifier_contract::Reversibility,
            Destructiveness,
        )>,
        action: &str,
    ) -> Step<Vec<Authorization>> {
        let caps = caps
            .into_iter()
            .map(|(cap, reversible, destructive)| {
                let mut f = self.facts(m, ctx);
                f.reversible = reversible;
                f.destructive = destructive;
                (cap, f)
            })
            .collect();
        self.authorize(ctx, m, caps, action).await
    }

    async fn journaled(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        auths: &[Authorization],
        irreversible: bool,
        op: impl FnOnce(undo_journal_contract::StepId) -> Result<(), UndoError>,
        ok: impl FnOnce(u64) -> (String, serde_json::Value),
    ) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return self
                .done(ctx, m, auths, ToolOutcome::cancelled(&m.title))
                .await;
        }
        let step = match self.begin(ctx, m, irreversible) {
            Ok(s) => s,
            Err(out) => return self.done(ctx, m, auths, *out).await,
        };
        match self.finish(step, op(step), m) {
            Ok(undo) => {
                let (text, data) = ok(undo.id);
                let mut out = ToolOutcome::ok(text, data);
                out.undo = Some(undo);
                self.done(ctx, m, auths, out).await
            }
            Err(out) => self.done(ctx, m, auths, *out).await,
        }
    }

    /// `fs_write`.
    pub(crate) async fn write(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: WriteArgs = parse_args(args)?;
        if a.content.len() as u64 > self.config.write_max_bytes {
            return Ok(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Treść przekracza limit zapisu.",
            ));
        }
        let path = self.resolve(&a.path, ctx, m).await?;
        let scope = self.scope(&path, false)?;
        let action = format!("zapis „{path}”");
        let rev = m.reversible;
        let auths = self
            .mutate_access(
                ctx,
                m,
                vec![(Capability::FsWrite(scope), rev, Destructiveness::None)],
                &action,
            )
            .await?;
        let p = Core::path_buf(&path);
        let existing = match self.fs.read(&p) {
            Ok(d) => Some(d),
            Err(platform_contract::PlatformError::NotFound(_)) => None,
            Err(e) => {
                return Ok(self
                    .done(ctx, m, &auths, platform_failure(&e, &action))
                    .await);
            }
        };
        let data = match (a.mode, existing) {
            (WriteMode::Create, Some(_)) => {
                let out = ToolOutcome::failed(
                    ToolErrorKind::AlreadyExists,
                    format!(
                        "Nie wykonano: {action} — plik istnieje (użyj trybu `overwrite` albo `append`)."
                    ),
                );
                return Ok(self.done(ctx, m, &auths, out).await);
            }
            (WriteMode::Append, Some(mut old)) => {
                old.extend_from_slice(a.content.as_bytes());
                old
            }
            _ => a.content.into_bytes(),
        };
        let bytes = data.len() as u64;
        let journal = self.journal.clone();
        let out = self
            .journaled(
                ctx,
                m,
                &auths,
                false,
                |step| journal.write(step, &p, &data),
                |id| {
                    (
                        format!("Zapisano „{path}” ({bytes} B). Krok można cofnąć."),
                        out_value(vec![path.clone()], bytes, Some(id), true),
                    )
                },
            )
            .await;
        Ok(out)
    }

    /// `fs_move` i `fs_rename` (przeniesienie w obrębie zakresu).
    pub(crate) async fn relocate(
        &self,
        from: &str,
        to: &str,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let from = self.resolve(from, ctx, m).await?;
        let to = self.resolve(to, ctx, m).await?;
        let action = format!("przeniesienie „{from}” → „{to}”");
        let caps = vec![
            (
                Capability::FsWrite(self.scope(&from, false)?),
                m.reversible,
                Destructiveness::None,
            ),
            (
                Capability::FsWrite(self.scope(&to, false)?),
                m.reversible,
                Destructiveness::None,
            ),
        ];
        let auths = self.mutate_access(ctx, m, caps, &action).await?;
        let (f, t) = (Core::path_buf(&from), Core::path_buf(&to));
        let journal = self.journal.clone();
        Ok(self
            .journaled(
                ctx,
                m,
                &auths,
                false,
                |step| journal.move_path(step, &f, &t),
                |id| {
                    (
                        format!("Przeniesiono „{from}” → „{to}”. Krok można cofnąć."),
                        out_value(vec![from.clone(), to.clone()], 0, Some(id), true),
                    )
                },
            )
            .await)
    }

    /// `fs_move`.
    pub(crate) async fn move_path(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: FromToArgs = parse_args(args)?;
        self.relocate(&a.from, &a.to, ctx, m).await
    }

    /// `fs_rename`.
    pub(crate) async fn rename(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: RenameArgs = parse_args(args)?;
        let name = a.new_name.trim();
        if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
            return Ok(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Nowa nazwa nie może zawierać separatorów ścieżki ani `:`.",
            ));
        }
        let from = self.resolve(&a.path, ctx, m).await?;
        let cut = from
            .trim_end_matches(['/', '\\'])
            .rfind(['/', '\\'])
            .unwrap_or(0);
        let (parent, sep) = from.split_at(cut);
        let sep = sep.chars().next().unwrap_or('/');
        let to = format!("{parent}{sep}{name}");
        self.relocate(&from, &to, ctx, m).await
    }

    /// `fs_copy`.
    pub(crate) async fn copy(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: FromToArgs = parse_args(args)?;
        let from = self.resolve(&a.from, ctx, m).await?;
        let to = self.resolve(&a.to, ctx, m).await?;
        let action = format!("kopia „{from}” → „{to}”");
        let mut read_facts = self.facts(m, ctx);
        read_facts.reversible = risk_classifier_contract::Reversibility::Yes;
        let caps = vec![
            (Capability::FsRead(self.scope(&from, false)?), read_facts),
            (
                Capability::FsWrite(self.scope(&to, false)?),
                self.facts(m, ctx),
            ),
        ];
        let auths = self.authorize(ctx, m, caps, &action).await?;
        let (f, t) = (Core::path_buf(&from), Core::path_buf(&to));
        let journal = self.journal.clone();
        Ok(self
            .journaled(
                ctx,
                m,
                &auths,
                false,
                |step| journal.copy(step, &f, &t),
                |id| {
                    (
                        format!("Skopiowano „{from}” → „{to}”. Krok można cofnąć."),
                        out_value(vec![from.clone(), to.clone()], 0, Some(id), true),
                    )
                },
            )
            .await)
    }

    /// `fs_delete` (do Kosza).
    pub(crate) async fn delete(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: PathArgs = parse_args(args)?;
        let path = self.resolve(&a.path, ctx, m).await?;
        let action = format!("usunięcie do Kosza „{path}”");
        let caps = vec![(
            Capability::FsWrite(self.scope(&path, false)?),
            m.reversible,
            Destructiveness::Recoverable,
        )];
        let auths = self.mutate_access(ctx, m, caps, &action).await?;
        let p = Core::path_buf(&path);
        let journal = self.journal.clone();
        Ok(self
            .journaled(
                ctx,
                m,
                &auths,
                false,
                |step| journal.delete(step, &p),
                |id| {
                    (
                        format!("Przeniesiono „{path}” do Kosza. Krok można cofnąć."),
                        out_value(vec![path.clone()], 0, Some(id), false),
                    )
                },
            )
            .await)
    }

    /// `fs_delete_permanent`: wykonane wyłącznie po zatwierdzeniu w Broker-UI; gdy poziom
    /// autonomii pozwoliłby bez pytania — intencja potwierdzenia dla UI (nic nie usunięto).
    pub(crate) async fn delete_permanent(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: PathArgs = parse_args(args)?;
        let path = self.resolve(&a.path, ctx, m).await?;
        let action = format!("trwałe usunięcie „{path}”");
        let caps = vec![(
            Capability::FsWrite(self.scope(&path, false)?),
            m.reversible,
            Destructiveness::Permanent,
        )];
        let auths = self.mutate_access(ctx, m, caps, &action).await?;
        if auths.iter().all(|a| a.approval.is_none()) {
            let mut out = ToolOutcome::ok(
                format!(
                    "Nie usunięto: {action} wymaga potwierdzenia właściciela. Pokazałam kartę potwierdzenia; rozważ usunięcie do Kosza (`fs_delete`)."
                ),
                out_value(vec![path.clone()], 0, None, true),
            );
            out.status = ToolStatus::NeedsConfirmation;
            out.intent = Some(ToolIntent {
                kind: INTENT_CONFIRM_DELETE_PERMANENT.to_owned(),
                title: format!("Usunąć trwale „{path}”?"),
                details: serde_json::json!({ "paths": [path] }),
            });
            return Ok(self.done(ctx, m, &auths, out).await);
        }
        let p = Core::path_buf(&path);
        let journal = self.journal.clone();
        Ok(self
            .journaled(
                ctx,
                m,
                &auths,
                true,
                |step| journal.delete_permanent(step, &p),
                |id| {
                    (
                        format!("Usunięto trwale „{path}” (po zatwierdzeniu właściciela)."),
                        out_value(vec![path.clone()], 0, Some(id), false),
                    )
                },
            )
            .await)
    }

    /// `fs_mkdir`: zapis i usunięcie znacznika w jednym kroku dziennika (`FsPort` tworzy
    /// brakujące katalogi przy zapisie i usuwa je przy cofnięciu).
    pub(crate) async fn mkdir(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: PathArgs = parse_args(args)?;
        let path = self.resolve(&a.path, ctx, m).await?;
        let action = format!("utworzenie katalogu „{path}”");
        let caps = vec![(
            Capability::FsWrite(self.scope(&path, true)?),
            m.reversible,
            Destructiveness::None,
        )];
        let auths = self.mutate_access(ctx, m, caps, &action).await?;
        let p = Core::path_buf(&path);
        if let Some((is_dir, _)) = self.kind_of(&p) {
            let out = if is_dir {
                ToolOutcome::ok(
                    format!("Katalog „{path}” już istnieje."),
                    out_value(vec![path.clone()], 0, None, true),
                )
            } else {
                ToolOutcome::failed(
                    ToolErrorKind::AlreadyExists,
                    format!("Nie wykonano: {action} — istnieje plik o tej nazwie."),
                )
            };
            return Ok(self.done(ctx, m, &auths, out).await);
        }
        let marker = p.join(MKDIR_MARKER);
        let journal = self.journal.clone();
        let fs = self.fs.clone();
        let op = |step| {
            journal.write(step, &marker, b"")?;
            journal.delete_permanent(step, &marker)
        };
        let out = self
            .journaled(ctx, m, &auths, false, op, |id| {
                let exists = fs.exists(&p);
                let note = if exists {
                    ""
                } else {
                    " (w tym systemie plików pusty katalog powstaje przy pierwszym zapisie)"
                };
                (
                    format!("Utworzono katalog „{path}”{note}. Krok można cofnąć."),
                    out_value(vec![path.clone()], 0, Some(id), exists),
                )
            })
            .await;
        Ok(out)
    }
}
