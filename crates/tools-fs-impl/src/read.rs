//! Narzędzia odczytu: lista, odczyt, atrybuty, wyszukiwanie. Wynik z nazwami lub treścią
//! plików jest niezaufany (taint sesji przez Brokera, delimitacja w prompcie przez runtime).

use std::collections::VecDeque;
use std::path::Path;

use platform_contract::DirEntry;
use safety_broker_contract::{Capability, TaintSource};
use tools_common_contract::{
    ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, parse_args, report_untrusted, text,
};
use tools_fs_contract::{
    Entry, ListArgs, ListOutput, PathArgs, ReadArgs, ReadOutput, SearchArgs, SearchHit,
    SearchOutput, StatOutput, glob_match,
};

use crate::core::{Core, Step, platform_failure};

fn entry(e: &DirEntry) -> Entry {
    Entry {
        path: e.path.to_string_lossy().into_owned(),
        is_dir: e.is_dir,
        size: e.size,
    }
}

fn to_value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or_default()
}

impl Core {
    async fn read_access(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        raw: &str,
        tree: bool,
    ) -> Step<(String, Vec<tools_common_contract::Authorization>)> {
        let path = self.resolve(raw, ctx, m).await?;
        let scope = self.scope(&path, tree)?;
        let action = format!("{} „{path}”", m.title.to_lowercase());
        let auths = self
            .authorize(
                ctx,
                m,
                vec![(Capability::FsRead(scope), self.facts(m, ctx))],
                &action,
            )
            .await?;
        Ok((path, auths))
    }

    /// `fs_list`.
    pub(crate) async fn list(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ListArgs = parse_args(args)?;
        let (path, auths) = self.read_access(ctx, m, &a.path, true).await?;
        let limit = a
            .limit
            .unwrap_or(self.config.list_max_entries)
            .min(self.config.list_max_entries) as usize;
        let depth = a.depth.unwrap_or(0).min(3);
        let mut entries = Vec::new();
        let mut truncated = false;
        let mut queue = VecDeque::from([(Core::path_buf(&path), 0u8)]);
        while let Some((dir, d)) = queue.pop_front() {
            let listed = match self.fs.list_dir(&dir) {
                Ok(l) => l,
                Err(e) if d == 0 => {
                    let out = platform_failure(&e, &format!("lista „{path}”"));
                    return Ok(self.done(ctx, m, &auths, out).await);
                }
                Err(_) => continue,
            };
            for e in listed {
                if self.denied(&e.path.to_string_lossy()) {
                    continue;
                }
                if entries.len() >= limit {
                    truncated = true;
                    break;
                }
                if e.is_dir && d < depth {
                    queue.push_back((e.path.clone(), d + 1));
                }
                entries.push(entry(&e));
            }
        }
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let lines: Vec<String> = entries
            .iter()
            .map(|e| {
                if e.is_dir {
                    format!("[katalog] {}", e.path)
                } else {
                    format!("{} ({} B)", e.path, e.size)
                }
            })
            .collect();
        let more = if truncated {
            "\n… (obcięto limitem)"
        } else {
            ""
        };
        let body = format!(
            "Zawartość „{path}” ({} wpisów):\n{}{more}",
            entries.len(),
            lines.join("\n")
        );
        let data = to_value(&ListOutput {
            path,
            entries,
            truncated,
        });
        let mut out = ToolOutcome::ok(body, data).untrusted(TaintSource::File);
        out.truncated = truncated;
        Ok(self.done(ctx, m, &auths, out).await)
    }

    /// `fs_read`.
    pub(crate) async fn read(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: ReadArgs = parse_args(args)?;
        let (path, auths) = self.read_access(ctx, m, &a.path, false).await?;
        let data = match self.fs.read(Path::new(&path)) {
            Ok(d) => d,
            Err(e) => {
                let out = platform_failure(&e, &format!("odczyt „{path}”"));
                return Ok(self.done(ctx, m, &auths, out).await);
            }
        };
        let total = data.len() as u64;
        let offset = a.offset.unwrap_or(0).min(total);
        let max = u64::from(
            a.max_bytes
                .unwrap_or(self.config.read_max_bytes)
                .min(self.config.read_max_bytes),
        );
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(data.len());
        let end = usize::try_from(offset.saturating_add(max))
            .unwrap_or(usize::MAX)
            .min(data.len());
        let slice = &data[start..end];
        let truncated = (end as u64) < total;
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let (content, binary) = match text::decode_text(slice) {
            Some(t) => (text::redact_secrets(&t), false),
            None => (String::new(), true),
        };
        let body = if binary {
            format!("Plik „{path}” ({total} B) jest binarny — treść pominięta.")
        } else {
            let range = if truncated || offset > 0 {
                format!(", bajty {start}–{end} z {total}")
            } else {
                String::new()
            };
            format!("Plik „{path}” ({total} B{range}):\n{content}")
        };
        let data = to_value(&ReadOutput {
            path,
            content,
            bytes_total: total,
            offset,
            truncated,
            binary,
        });
        let mut out = ToolOutcome::ok(body, data).untrusted(TaintSource::File);
        out.truncated = truncated;
        Ok(self.done(ctx, m, &auths, out).await)
    }

    /// `fs_stat`.
    pub(crate) async fn stat(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: PathArgs = parse_args(args)?;
        let (path, auths) = self.read_access(ctx, m, &a.path, false).await?;
        let p = Core::path_buf(&path);
        let kind = self.kind_of(&p);
        let exists = kind.is_some();
        let (is_dir, size) = kind.unwrap_or((false, 0));
        let body = match (exists, is_dir) {
            (false, _) => format!("„{path}” nie istnieje."),
            (true, true) => format!("„{path}” to katalog."),
            (true, false) => format!("„{path}” to plik, {size} B."),
        };
        let data = to_value(&StatOutput {
            path,
            exists,
            is_dir,
            size,
        });
        Ok(self.done(ctx, m, &auths, ToolOutcome::ok(body, data)).await)
    }

    fn content_line(&self, path: &Path, size: u64, needle: &str) -> Option<String> {
        if size > self.config.search_file_max_bytes {
            return None;
        }
        let data = self.fs.read(path).ok()?;
        let content = text::decode_text(&data)?;
        let lower = needle.to_lowercase();
        let line = content
            .lines()
            .find(|l| l.to_lowercase().contains(&lower))?;
        let (short, _) = text::truncate_chars(line.trim(), 200);
        Some(text::redact_secrets(&short))
    }

    /// `fs_search`.
    pub(crate) async fn search(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: SearchArgs = parse_args(args)?;
        if a.pattern.trim().is_empty() || a.content.as_deref().is_some_and(|c| c.trim().is_empty())
        {
            return Ok(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Pusty wzorzec albo pusty tekst do szukania.",
            ));
        }
        let (root, auths) = self.read_access(ctx, m, &a.root, true).await?;
        let max_results = a
            .max_results
            .unwrap_or(self.config.search_max_results)
            .min(self.config.search_max_results) as usize;
        let max_depth = a
            .max_depth
            .unwrap_or(self.config.search_max_depth)
            .min(self.config.search_max_depth);
        let (mut hits, mut visited, mut truncated) = (Vec::new(), 0u32, false);
        let mut queue = VecDeque::from([(Core::path_buf(&root), 0u8)]);
        'walk: while let Some((dir, d)) = queue.pop_front() {
            let Ok(listed) = self.fs.list_dir(&dir) else {
                continue;
            };
            for e in listed {
                let p = e.path.to_string_lossy().into_owned();
                if self.denied(&p) {
                    continue;
                }
                visited += 1;
                if visited > self.config.search_max_visited || hits.len() >= max_results {
                    truncated = true;
                    break 'walk;
                }
                if e.is_dir {
                    if d < max_depth {
                        queue.push_back((e.path.clone(), d + 1));
                    } else {
                        truncated = true;
                    }
                    continue;
                }
                let name = e
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !glob_match(&a.pattern, &name) {
                    continue;
                }
                match &a.content {
                    None => hits.push(SearchHit {
                        path: p,
                        line: None,
                    }),
                    Some(needle) => {
                        if let Some(line) = self.content_line(&e.path, e.size, needle) {
                            hits.push(SearchHit {
                                path: p,
                                line: Some(line),
                            });
                        }
                    }
                }
            }
        }
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let lines: Vec<String> = hits
            .iter()
            .map(|h| match &h.line {
                Some(l) => format!("{}: {l}", h.path),
                None => h.path.clone(),
            })
            .collect();
        let body = format!(
            "Wyniki w „{root}” ({}{}):\n{}",
            hits.len(),
            if truncated { ", obcięte limitem" } else { "" },
            lines.join("\n")
        );
        let data = to_value(&SearchOutput {
            root,
            hits,
            visited,
            truncated,
        });
        let mut out = ToolOutcome::ok(body, data).untrusted(TaintSource::File);
        out.truncated = truncated;
        Ok(self.done(ctx, m, &auths, out).await)
    }
}
