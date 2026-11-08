//! `fs_mkdir`: pusty katalog przez znacznik zapisany i usunięty w jednym kroku dziennika cofania.

use risk_classifier_contract::Destructiveness;
use safety_broker_contract::Capability;
use tools_common_contract::{ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, parse_args};
use tools_fs_contract::PathArgs;

use crate::core::{Core, Step, shown};
use crate::write::out_value;

/// Nazwa pliku-znacznika do tworzenia pustego katalogu przez `FsPort` (zapis tworzy brakujące
/// katalogi, cofnięcie je usuwa); znacznik jest usuwany w tym samym kroku dziennika.
pub const MKDIR_MARKER: &str = ".alfa-mkdir.tmp";

impl Core {
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
                    format!(
                        "Katalog „{}” już istnieje.",
                        shown(&path, ctx.workdir.as_deref())
                    ),
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
                    format!(
                        "Utworzono katalog „{}”{note}. Krok można cofnąć.",
                        shown(&path, ctx.workdir.as_deref())
                    ),
                    out_value(vec![path.clone()], 0, Some(id), exists),
                )
            })
            .await;
        Ok(out)
    }
}
