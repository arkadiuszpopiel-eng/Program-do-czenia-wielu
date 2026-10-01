//! Izolowany katalog roboczy mostu (§8.5): `git worktree add --detach` dla repozytoriów,
//! kopia (bez dowiązań symbolicznych) dla pozostałych katalogów. Most nigdy nie pracuje
//! bezpośrednio w katalogu użytkownika.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use agent_backends_contract::{
    BackendError, PreparedWorkdir, SessionRef, TaskId, WorkdirKind, WorkdirMode, WorkdirSpec,
    Workspace,
};
use async_trait::async_trait;

use crate::process::cli_env;

/// Domyślny limit rozmiaru kopii (bajty).
pub const DEFAULT_MAX_COPY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Worktree/kopie w katalogu `root` (np. `%LOCALAPPDATA%\Alfa\worktrees`).
#[derive(Debug, Clone)]
pub struct GitWorkspace {
    root: PathBuf,
    git: PathBuf,
    max_copy_bytes: u64,
}

fn ws_err(context: &str, e: impl std::fmt::Display) -> BackendError {
    BackendError::Workspace(format!("{context}: {e}"))
}

impl GitWorkspace {
    /// Katalog bazowy (tworzony przy pierwszym użyciu); `git` z PATH.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            git: PathBuf::from("git"),
            max_copy_bytes: DEFAULT_MAX_COPY_BYTES,
        }
    }

    /// Zmienia limit rozmiaru kopii.
    #[must_use]
    pub fn with_max_copy_bytes(mut self, bytes: u64) -> Self {
        self.max_copy_bytes = bytes;
        self
    }

    async fn git(&self, cwd: &Path, args: &[&str]) -> Result<String, BackendError> {
        let out = tokio::process::Command::new(&self.git)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(cli_env())
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| ws_err("nie udało się uruchomić git", e))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(ws_err(
                "git zwrócił błąd",
                String::from_utf8_lossy(&out.stderr)
                    .chars()
                    .take(300)
                    .collect::<String>(),
            ))
        }
    }

    async fn is_git_repo(&self, dir: &Path) -> bool {
        self.git(dir, &["rev-parse", "--is-inside-work-tree"])
            .await
            .is_ok_and(|o| o == "true")
    }

    fn root_canonical(&self) -> Result<PathBuf, BackendError> {
        std::fs::create_dir_all(&self.root).map_err(|e| ws_err("katalog roboczy", e))?;
        self.root
            .canonicalize()
            .map_err(|e| ws_err("katalog roboczy", e))
    }
}

fn copy_tree(from: &Path, to: &Path, skip: &Path, budget: &mut u64) -> Result<(), BackendError> {
    std::fs::create_dir_all(to).map_err(|e| ws_err("kopia", e))?;
    for entry in std::fs::read_dir(from).map_err(|e| ws_err("kopia", e))? {
        let entry = entry.map_err(|e| ws_err("kopia", e))?;
        let path = entry.path();
        if path == skip {
            continue;
        }
        let kind = entry.file_type().map_err(|e| ws_err("kopia", e))?;
        let target = to.join(entry.file_name());
        if kind.is_symlink() {
            // Dowiązania pomijamy: mogłyby wskazywać poza katalog (np. na poświadczenia).
            continue;
        }
        if kind.is_dir() {
            copy_tree(&path, &target, skip, budget)?;
        } else if kind.is_file() {
            let len = entry.metadata().map_err(|e| ws_err("kopia", e))?.len();
            *budget = budget
                .checked_sub(len)
                .ok_or_else(|| BackendError::Workspace("katalog za duży do skopiowania".into()))?;
            std::fs::copy(&path, &target).map_err(|e| ws_err("kopia", e))?;
        }
    }
    Ok(())
}

#[async_trait]
impl Workspace for GitWorkspace {
    async fn prepare(
        &self,
        task: &TaskId,
        spec: &WorkdirSpec,
    ) -> Result<PreparedWorkdir, BackendError> {
        let source = spec
            .source
            .canonicalize()
            .map_err(|e| ws_err("katalog źródłowy", e))?;
        if !source.is_dir() {
            return Err(BackendError::Workspace("źródło nie jest katalogiem".into()));
        }
        let root = self.root_canonical()?;
        if source.starts_with(&root) {
            return Err(BackendError::Workspace(
                "źródło leży w katalogu roboczym mostów".into(),
            ));
        }
        let name: String = task
            .0
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let dest = root.join(name);
        if dest.exists() {
            return Err(BackendError::Workspace(
                "katalog zadania już istnieje".into(),
            ));
        }
        if spec.mode == WorkdirMode::Worktree && self.is_git_repo(&source).await {
            let dest_s = dest.to_string_lossy().into_owned();
            self.git(&source, &["worktree", "add", "--detach", &dest_s, "HEAD"])
                .await?;
            return Ok(PreparedWorkdir {
                path: dest,
                source,
                kind: WorkdirKind::GitWorktree,
            });
        }
        let (from, to, skip) = (source.clone(), dest.clone(), root.clone());
        let mut budget = self.max_copy_bytes;
        let copied = tokio::task::spawn_blocking(move || copy_tree(&from, &to, &skip, &mut budget))
            .await
            .map_err(|e| ws_err("kopia", e))?;
        if let Err(e) = copied {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(e);
        }
        Ok(PreparedWorkdir {
            path: dest,
            source,
            kind: WorkdirKind::Copy,
        })
    }

    async fn reuse(&self, session: &SessionRef) -> Result<PreparedWorkdir, BackendError> {
        let root = self.root_canonical()?;
        let path = session
            .workdir
            .canonicalize()
            .map_err(|e| ws_err("katalog sesji", e))?;
        if !path.starts_with(&root) || path == root {
            return Err(BackendError::Workspace(
                "katalog wznawianej sesji leży poza katalogiem roboczym mostów".into(),
            ));
        }
        Ok(PreparedWorkdir {
            source: path.clone(),
            path,
            kind: WorkdirKind::Resumed,
        })
    }

    async fn release(&self, prepared: &PreparedWorkdir, keep: bool) -> Result<(), BackendError> {
        if keep || prepared.kind == WorkdirKind::Resumed {
            return Ok(());
        }
        if prepared.kind == WorkdirKind::GitWorktree {
            let path = prepared.path.to_string_lossy().into_owned();
            return self
                .git(&prepared.source, &["worktree", "remove", "--force", &path])
                .await
                .map(|_| ());
        }
        std::fs::remove_dir_all(&prepared.path).map_err(|e| ws_err("usuwanie kopii", e))
    }
}
