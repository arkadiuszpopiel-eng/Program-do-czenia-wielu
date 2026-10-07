//! `net_download`: plik do kwarantanny sesji — `fs.write(kwarantanna)` i `net.egress(host)`
//! z Brokera, katalog bez dowiązań i poza deny-listą, nazwa oczyszczona, zapis strumieniowy
//! (bufor ≤ 256 KiB na wątku blokującym), SHA-256 w locie, limit rozmiaru (deklarowany i
//! rzeczywisty), przerwanie = usunięcie pliku częściowego, MOTW przy zatwierdzeniu.

use std::path::{Path, PathBuf};
use std::time::Duration;

use platform_apps_contract::{
    DownloadSink, disposition_file_name, is_executable_name, sanitize_file_name,
};
use safety_broker_contract::{Capability, TaintSource};
use sha2::{Digest, Sha256};
use tokio::time::Instant;
use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, base_facts, parse_args, paths,
    report_untrusted,
};
use tools_net_contract::{DownloadArgs, DownloadOut, EVENT_DOWNLOAD, HttpMethod, QUARANTINE_DIR};

use crate::core::{Core, Step, fail, refuse};
use crate::fetch::checked_target;

/// Bufor zapisu przed przekazaniem do wątku blokującego.
const FLUSH_BYTES: usize = 256 * 1024;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Nazwa katalogu sesji w kwarantannie zapasowej (identyfikator jako bezpieczna nazwa).
fn session_dir(session: &str) -> String {
    sanitize_file_name(session)
}

/// Nazwa z argumentu, nagłówka albo ostatniego segmentu ścieżki adresu.
fn file_name(arg: Option<&str>, disposition: Option<&str>, final_url: &str) -> String {
    let from_url = || {
        lib_netguard::check_url(final_url).ok().and_then(|t| {
            t.url()
                .path_segments()
                .and_then(|mut s| s.next_back().map(str::to_owned))
        })
    };
    let raw = arg
        .map(str::to_owned)
        .or_else(|| disposition.and_then(disposition_file_name))
        .or_else(from_url)
        .unwrap_or_default();
    sanitize_file_name(&raw)
}

async fn flush(
    sink: Box<dyn DownloadSink>,
    buf: Vec<u8>,
    action: &str,
) -> Step<Box<dyn DownloadSink>> {
    let joined = tokio::task::spawn_blocking(move || {
        let mut sink = sink;
        sink.write(&buf).map(|()| sink)
    })
    .await;
    match joined {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(fail(
            ToolErrorKind::Io,
            format!("Nie wykonano: {action} — {e}."),
        )),
        Err(e) => Err(fail(ToolErrorKind::Internal, format!("Wątek zapisu: {e}."))),
    }
}

impl Core {
    /// Katalog kwarantanny sesji: `<katalog roboczy>\Kwarantanna` albo `<korzeń>\<sesja>`.
    fn quarantine_dir(&self, ctx: &ToolCtx, action: &str) -> Step<PathBuf> {
        let dir = match (&ctx.workdir, &self.quarantine_root) {
            (Some(w), _) => Path::new(w).join(QUARANTINE_DIR),
            (None, Some(root)) => root.join(session_dir(ctx.holder.session.as_str())),
            (None, None) => {
                return Err(fail(
                    ToolErrorKind::NotFound,
                    format!(
                        "Nie wykonano: {action} — sesja nie ma katalogu roboczego (wybierz go w pasku sesji)."
                    ),
                ));
            }
        };
        let text = dir.to_string_lossy().into_owned();
        let protected =
            |p: &str| self.deny.is_denied_path(p, &self.env) || paths::has_credential_segment(p);
        if !dir.is_absolute() || paths::protected_with_links(&text, protected) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                action,
            )));
        }
        Ok(dir)
    }

    /// `net_download`.
    pub(crate) async fn download(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: DownloadArgs = parse_args(args)?;
        let action = format!("pobranie pliku {}", a.url);
        let target = checked_target(&a.url, &action)?;
        let dir = self.quarantine_dir(ctx, &action)?;
        let dir_text = dir.to_string_lossy().into_owned();
        let scope = paths::tree_scope(&dir_text, &self.env)
            .map_err(|e| refuse(DenialReason::Policy, &action, &e.to_string()))?;
        let fs_cap = Capability::FsWrite(scope);
        // Nowy plik w kwarantannie da się usunąć — zapis odwracalny (skutki sieciowe osobno).
        let mut fs_facts = base_facts(m, ctx);
        fs_facts.reversible = risk_classifier_contract::Reversibility::Yes;
        let fs_auth = self.authorize(ctx, &fs_cap, fs_facts, &action).await?;
        let deadline = Instant::now() + Duration::from_millis(self.config.download_timeout_ms);
        let opened = match self
            .open(ctx, m, target, HttpMethod::Get, deadline, &action)
            .await
        {
            Ok(o) => o,
            Err(e) => {
                self.gate.release(std::slice::from_ref(&fs_auth)).await;
                return Err(e);
            }
        };
        let result = self
            .store(
                ctx,
                &a,
                &dir,
                (&fs_cap, &fs_auth),
                opened,
                deadline,
                &action,
            )
            .await;
        self.gate.release(std::slice::from_ref(&fs_auth)).await;
        let (out, approval) = result?;
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        self.emit(
            EVENT_DOWNLOAD,
            serde_json::json!({"path": out.path, "bytes": out.bytes, "sha256": out.sha256}),
            ctx,
        )
        .await;
        let text = format!(
            "Pobrano do kwarantanny: {} ({} B, SHA-256 {}){}. Plik pochodzi z Internetu — niezaufany.",
            out.path,
            out.bytes,
            out.sha256,
            if out.executable {
                "; plik wykonywalny — nie uruchamiaj"
            } else {
                ""
            }
        );
        let mut o = ToolOutcome::ok(text, serde_json::to_value(&out).unwrap_or_default())
            .untrusted(TaintSource::Web);
        o.approval = approval.or(fs_auth.approval);
        Ok(o)
    }

    #[allow(clippy::too_many_arguments)]
    async fn store(
        &self,
        ctx: &ToolCtx,
        a: &DownloadArgs,
        dir: &Path,
        (fs_cap, fs_auth): (&Capability, &tools_common_contract::Authorization),
        mut opened: crate::core::Opened,
        deadline: Instant,
        action: &str,
    ) -> Step<(DownloadOut, Option<safety_broker_contract::ApprovalId>)> {
        let approval = opened.grants.iter().find_map(|g| g.auth.approval);
        let r = self
            .store_inner(
                ctx,
                a,
                dir,
                (fs_cap, fs_auth),
                &mut opened,
                deadline,
                action,
            )
            .await;
        self.release(&opened.grants).await;
        r.map(|out| (out, approval))
    }

    #[allow(clippy::too_many_arguments)]
    async fn store_inner(
        &self,
        ctx: &ToolCtx,
        a: &DownloadArgs,
        dir: &Path,
        (fs_cap, fs_auth): (&Capability, &tools_common_contract::Authorization),
        opened: &mut crate::core::Opened,
        deadline: Instant,
        action: &str,
    ) -> Step<DownloadOut> {
        let resp = &mut opened.response;
        if !(200..300).contains(&resp.status) {
            return Err(fail(
                ToolErrorKind::Io,
                format!(
                    "Nie wykonano: {action} — serwer zwrócił HTTP {}.",
                    resp.status
                ),
            ));
        }
        let max = self.config.max_download_bytes;
        if resp.content_length.is_some_and(|n| n > max) {
            return Err(refuse(
                DenialReason::Policy,
                action,
                &format!("plik większy niż limit {max} B"),
            ));
        }
        let name = file_name(
            a.file_name.as_deref(),
            resp.content_disposition.as_deref(),
            &opened.final_url,
        );
        let exact = paths::exact_scope(&dir.join(&name).to_string_lossy(), &self.env)
            .map_err(|e| refuse(DenialReason::Policy, action, &e.to_string()))?;
        if let Err(e) = self
            .gate
            .verify(fs_auth, &Capability::FsWrite(exact), &ctx.holder)
        {
            return Err(Box::new(e.into_outcome(action)));
        }
        self.gate
            .verify(fs_auth, fs_cap, &ctx.holder)
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        let (store, d, n) = (self.downloads.clone(), dir.to_path_buf(), name.clone());
        let begun = tokio::task::spawn_blocking(move || store.begin(&d, &n)).await;
        let mut sink = match begun {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(refuse(DenialReason::Policy, action, &e.to_string())),
            Err(e) => return Err(fail(ToolErrorKind::Internal, format!("Wątek zapisu: {e}."))),
        };
        let mut hasher = Sha256::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut total: u64 = 0;
        while let Some(chunk) = self
            .next_chunk(ctx, &mut resp.body, deadline, action)
            .await?
        {
            total = total.saturating_add(chunk.len() as u64);
            if total > max {
                // Porzucony zapis usuwa plik częściowy (`Drop` ujścia).
                return Err(refuse(
                    DenialReason::Policy,
                    action,
                    &format!("plik większy niż limit {max} B — przerwano i usunięto część"),
                ));
            }
            hasher.update(&chunk);
            buf.extend_from_slice(&chunk);
            if buf.len() >= FLUSH_BYTES {
                sink = flush(sink, std::mem::take(&mut buf), action).await?;
            }
        }
        if !buf.is_empty() {
            sink = flush(sink, buf, action).await?;
        }
        let url = opened.final_url.clone();
        let committed = tokio::task::spawn_blocking(move || sink.commit(&url)).await;
        let path = match committed {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                return Err(fail(
                    ToolErrorKind::Io,
                    format!("Nie wykonano: {action} — {e}."),
                ));
            }
            Err(e) => return Err(fail(ToolErrorKind::Internal, format!("Wątek zapisu: {e}."))),
        };
        Ok(DownloadOut {
            path: path.to_string_lossy().into_owned(),
            bytes: total,
            sha256: hex(&hasher.finalize()),
            content_type: resp.content_type.clone(),
            final_url: opened.final_url.clone(),
            executable: is_executable_name(&name),
        })
    }
}
