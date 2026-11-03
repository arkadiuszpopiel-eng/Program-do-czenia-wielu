//! Narzędzia zmieniające stan: zapis, przeniesienie, kopia, zmiana nazwy, Kosz, trwałe
//! usunięcie (zawsze potwierdzenie właściciela), katalog. Każda mutacja = krok dziennika
//! cofania (brak wpisu = brak operacji).

use risk_classifier_contract::Destructiveness;
use safety_broker_contract::Capability;
use tools_common_contract::{
    Authorization, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, parse_args, text,
};
use tools_fs_contract::{FromToArgs, MutationOutput, RenameArgs, WriteArgs, WriteMode};
use undo_journal_contract::UndoError;

use crate::core::{Core, Step, platform_failure, shown};

pub(crate) fn out_value(
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

/// Liczba znaczników redakcji w treści.
fn redactions(data: &[u8]) -> usize {
    String::from_utf8_lossy(data)
        .matches(text::REDACTED)
        .count()
}

impl Core {
    pub(crate) async fn mutate_access(
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
        self.dir_access(ctx, m, caps, action, None).await
    }

    /// Jak [`Self::mutate_access`]; `dir` — usuwany albo przenoszony katalog: zawartość z `.git`
    /// czy `.vscode` to dla Sędziego zmiana ścieżki chronionej (pierwsza zdolność).
    pub(crate) async fn dir_access(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        caps: Vec<(
            Capability,
            risk_classifier_contract::Reversibility,
            Destructiveness,
        )>,
        action: &str,
        dir: Option<&str>,
    ) -> Step<Vec<Authorization>> {
        let inside = dir.is_some_and(|d| self.holds_protected(d, ctx.workdir.as_deref()));
        let caps = caps
            .into_iter()
            .enumerate()
            .map(|(i, (cap, reversible, destructive))| {
                let mut f = self.facts(m, ctx);
                f.reversible = reversible;
                f.destructive = destructive;
                f.contains_protected = inside && i == 0;
                (cap, f)
            })
            .collect();
        self.authorize(ctx, m, caps, action).await
    }

    pub(crate) async fn journaled(
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
        // Stara treść jest potrzebna przy dopisywaniu i przy treści ze znacznikiem redakcji —
        // i tylko gdy wynik zmieści się w limicie; w pozostałych przypadkach wystarczy wiedzieć,
        // czy plik istnieje (zakres pusty).
        let limit = self.config.write_max_bytes;
        let marked = a.content.contains(text::REDACTED);
        let wanted = match a.mode {
            WriteMode::Append => limit,
            _ if marked => limit,
            _ => 0,
        };
        let existing = match self.fs.read_range(&p, 0, wanted) {
            Ok(d) => Some(d),
            Err(platform_contract::PlatformError::NotFound(_)) => None,
            Err(e) => {
                return Ok(self
                    .done(ctx, m, &auths, platform_failure(&e, &action))
                    .await);
            }
        };
        let had = existing.as_ref().map_or(0, |(d, _)| redactions(d));
        // Dopisanie do niepustego pliku nie pokazuje modelowi treści — nie odblokowuje nadpisania.
        let learned = a.mode != WriteMode::Append || existing.as_ref().is_none_or(|(_, t)| *t == 0);
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
            // Limit dotyczy pliku po zapisie, nie tylko dopisywanej treści — inaczej `append`
            // pozwalał rosnąć plikowi bez końca (każde dopisanie zapisuje całość z kopią w dzienniku).
            (WriteMode::Append, Some((_, total)))
                if total.saturating_add(a.content.len() as u64) > limit =>
            {
                let out = ToolOutcome::failed(
                    ToolErrorKind::InvalidArgs,
                    format!(
                        "Nie wykonano: {action} — plik po dopisaniu przekroczyłby limit zapisu."
                    ),
                );
                return Ok(self.done(ctx, m, &auths, out).await);
            }
            (WriteMode::Overwrite, Some((_, total)))
                if total > 0 && !self.seen.knows(ctx, &path) =>
            {
                let out = crate::seen::unread(&action);
                return Ok(self.done(ctx, m, &auths, out).await);
            }
            (WriteMode::Append, Some((mut old, _))) => {
                old.extend_from_slice(a.content.as_bytes());
                old
            }
            _ => a.content.into_bytes(),
        };
        // `fs_read` pokazuje modelowi `[ZREDAGOWANO]` w miejscu wartości podobnych do sekretów
        // (także zwykłego kodu, np. `secret: SecretName`). Przepisanie pliku z odczytu wstawiałoby
        // znacznik zamiast prawdziwej treści i psuło źródła — nowy znacznik oznacza odmowę.
        if marked && redactions(&data) > had {
            let out = ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!(
                    "Nie wykonano: {action} — treść zawiera znacznik {} z odczytu pliku. Odczyt ukrył \
                     w tym miejscu wartość (możliwy sekret), więc zapis wstawiłby znacznik zamiast \
                     niej. Nie przepisuj tych linii: zmień inny plik, dopisz zmiany trybem `append` \
                     albo poproś właściciela o tę zmianę.",
                    text::REDACTED
                ),
            );
            return Ok(self.done(ctx, m, &auths, out).await);
        }
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
                        format!(
                            "Zapisano „{}” ({bytes} B). Krok można cofnąć.",
                            shown(&path, ctx.workdir.as_deref())
                        ),
                        out_value(vec![path.clone()], bytes, Some(id), true),
                    )
                },
            )
            .await;
        if out.is_ok() && learned {
            self.seen.note(ctx, &path);
        }
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
        let auths = self.dir_access(ctx, m, caps, &action, Some(&from)).await?;
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
                        format!(
                            "Przeniesiono „{}” → „{}”. Krok można cofnąć.",
                            shown(&from, ctx.workdir.as_deref()),
                            shown(&to, ctx.workdir.as_deref())
                        ),
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
        // Tylko cel od modelu — `fs_rename` składa cel bezwzględny z katalogu źródła.
        if let Some(out) = crate::near::trash_target(&a.to) {
            return Err(out);
        }
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
        if let Some(out) = crate::near::trash_target(&a.to) {
            return Err(out);
        }
        let from = self.resolve(&a.from, ctx, m).await?;
        let to = self.resolve(&a.to, ctx, m).await?;
        let action = format!("kopia „{from}” → „{to}”");
        let mut read_facts = self.facts(m, ctx);
        read_facts.reversible = risk_classifier_contract::Reversibility::Yes;
        // Kopia katalogu z `.git` czy `.vscode` tworzy ścieżki chronione w celu.
        let mut write_facts = self.facts(m, ctx);
        write_facts.contains_protected = self.holds_protected(&from, ctx.workdir.as_deref());
        let caps = vec![
            (Capability::FsRead(self.scope(&from, false)?), read_facts),
            (Capability::FsWrite(self.scope(&to, false)?), write_facts),
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
                        format!(
                            "Skopiowano „{}” → „{}”. Krok można cofnąć.",
                            shown(&from, ctx.workdir.as_deref()),
                            shown(&to, ctx.workdir.as_deref())
                        ),
                        out_value(vec![from.clone(), to.clone()], 0, Some(id), true),
                    )
                },
            )
            .await)
    }
}
