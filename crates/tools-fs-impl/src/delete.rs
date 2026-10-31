//! Usuwanie: do Kosza (odwracalne) i trwałe (zawsze potwierdzenie właściciela). Każda mutacja
//! = krok dziennika cofania, jak w `write.rs`.

use risk_classifier_contract::Destructiveness;
use safety_broker_contract::Capability;
use tools_common_contract::{
    ToolCtx, ToolIntent, ToolManifest, ToolOutcome, ToolStatus, parse_args,
};
use tools_fs_contract::{INTENT_CONFIRM_DELETE_PERMANENT, PathArgs};

use crate::core::{Core, Step, shown};
use crate::write::out_value;

impl Core {
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
                        format!(
                            "Przeniesiono „{}” do Kosza. Krok można cofnąć.",
                            shown(&path, ctx.workdir.as_deref())
                        ),
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
                        format!(
                            "Usunięto trwale „{}” (po zatwierdzeniu właściciela).",
                            shown(&path, ctx.workdir.as_deref())
                        ),
                        out_value(vec![path.clone()], 0, Some(id), false),
                    )
                },
            )
            .await)
    }
}
