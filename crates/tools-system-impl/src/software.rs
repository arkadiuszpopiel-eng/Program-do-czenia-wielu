//! `system_software`: wykrywanie programów (PATH + `--version` przez `CliProbe` — czyste
//! środowisko, limit czasu) i przeglądarek po znanych ścieżkach; wynik w pamięci kilka minut.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use accounts_hub_contract::CliProbe;
use tools_common_contract::{ToolCtx, ToolManifest, ToolOutcome, parse_args};
use tools_system_contract::{
    BROWSER_CATALOG, CLI_CATALOG, COMMON, SoftwareArgs, SoftwareItem, SoftwareKind, SoftwareOut,
    SoftwareProbe, describe, parse_version,
};

use crate::core::{Core, Step};

/// Jak długo trzymać wynik wykrycia (programy instaluje się rzadko; `refresh` wymusza nowe).
const CACHE_FOR: Duration = Duration::from_secs(10 * 60);

/// Wykrywanie nad `CliProbe` (ten sam mechanizm co wykrycie mostów CLI).
pub struct CliSoftwareProbe {
    cli: Arc<dyn CliProbe>,
    /// Katalogi instalacji przeglądarek (`%ProgramFiles%`, `%ProgramFiles(x86)%`, `%LOCALAPPDATA%`).
    roots: Vec<PathBuf>,
}

impl CliSoftwareProbe {
    /// Wykrywanie z katalogami przeglądarek ze zmiennych środowiska procesu.
    pub fn new(cli: Arc<dyn CliProbe>) -> Self {
        let roots = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(|k| std::env::var_os(k).map(PathBuf::from))
            .collect();
        Self { cli, roots }
    }

    /// Wykrywanie z podanymi katalogami przeglądarek (testy).
    pub fn with_roots(cli: Arc<dyn CliProbe>, roots: Vec<PathBuf>) -> Self {
        Self { cli, roots }
    }
}

impl SoftwareProbe for CliSoftwareProbe {
    fn scan(&self) -> Vec<SoftwareItem> {
        // Programy sprawdzane równolegle (każdy to krótki proces z limitem czasu).
        let mut items: Vec<SoftwareItem> = std::thread::scope(|scope| {
            let handles: Vec<_> = CLI_CATALOG
                .iter()
                .map(|(id, name, kind, program, args)| {
                    let cli = &self.cli;
                    scope.spawn(move || {
                        let path = cli.locate(program)?;
                        let version = if args.is_empty() {
                            None
                        } else {
                            cli.run_output(&path, args)
                                .and_then(|o| parse_version(&o.text))
                        };
                        Some(SoftwareItem {
                            id: (*id).to_owned(),
                            name: (*name).to_owned(),
                            kind: *kind,
                            version,
                            command: (*program).to_owned(),
                        })
                    })
                })
                .collect();
            handles
                .into_iter()
                .filter_map(|h| h.join().ok().flatten())
                .collect()
        });
        for (id, name, paths) in BROWSER_CATALOG {
            let found = self
                .roots
                .iter()
                .flat_map(|root| paths.iter().map(move |p| root.join(p)))
                .find(|p| p.is_file());
            if let Some(path) = found {
                items.push(SoftwareItem {
                    id: (*id).to_owned(),
                    name: (*name).to_owned(),
                    kind: SoftwareKind::Browser,
                    version: None,
                    command: path.display().to_string(),
                });
            }
        }
        items.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
        items
    }
}

/// Popularne narzędzia, których nie wykryto.
pub(crate) fn missing(items: &[SoftwareItem]) -> Vec<String> {
    COMMON
        .iter()
        .filter(|c| {
            !items
                .iter()
                .any(|i| i.id == **c || (**c == "python" && i.id == "py"))
        })
        .map(|c| (*c).to_owned())
        .collect()
}

impl Core {
    /// Wynik wykrycia z pamięci albo nowe wykrycie (`refresh`, pusty albo stary wynik).
    pub(crate) async fn software_items(&self, refresh: bool) -> Option<Vec<SoftwareItem>> {
        let probe = self.software.clone()?;
        if !refresh
            && let Ok(cache) = self.software_cache.lock()
            && let Some((at, items)) = cache.as_ref()
            && at.elapsed() < CACHE_FOR
        {
            return Some(items.clone());
        }
        let items = tokio::task::spawn_blocking(move || probe.scan())
            .await
            .ok()?;
        if let Ok(mut cache) = self.software_cache.lock() {
            *cache = Some((Instant::now(), items.clone()));
        }
        Some(items)
    }

    /// `system_software`.
    pub(crate) async fn software(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: SoftwareArgs = parse_args(args)?;
        let action = "lista zainstalowanych programów";
        let auth = self.authorize_read(ctx, m, action).await?;
        let Some(all) = self.software_items(a.refresh.unwrap_or(false)).await else {
            self.release(&auth).await;
            return Err(crate::core::fail(
                tools_common_contract::ToolErrorKind::Unsupported,
                "Wykrywanie programów jest niedostępne w tej konfiguracji.".into(),
            ));
        };
        let missing = missing(&all);
        let items: Vec<SoftwareItem> = all
            .into_iter()
            .filter(|i| a.kind.is_none_or(|k| i.kind == k))
            .collect();
        let summary = describe(&items, if a.kind.is_none() { &missing } else { &[] });
        let out = SoftwareOut { items, missing };
        Ok(self.untrusted_ok(ctx, &auth, summary, &out).await)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use accounts_hub_contract::CliOutput;

    use super::*;

    struct Probe;

    impl CliProbe for Probe {
        fn locate(&self, program: &str) -> Option<PathBuf> {
            ["node", "git", "py", "winget"].contains(&program).then(|| {
                std::env::temp_dir()
                    .join("alfa-software-test-bin")
                    .join(format!("{program}.exe"))
            })
        }
        fn version_output(&self, _path: &Path) -> Option<String> {
            None
        }
        fn run_output(&self, path: &Path, _args: &[&str]) -> Option<CliOutput> {
            let text = match path.file_stem()?.to_str()? {
                "node" => "v20.11.1",
                "git" => "git version 2.45.1.windows.1",
                "py" => "Python 3.12.4",
                _ => "v1.9.25200",
            };
            Some(CliOutput {
                success: true,
                text: text.into(),
            })
        }
    }

    #[test]
    fn scan_finds_tools_versions_and_browsers() {
        let dir = std::env::temp_dir().join(format!("alfa-soft-{}", std::process::id()));
        // Pełna ścieżka względna katalogu Windows: na innych hostach `\` jest częścią nazwy.
        let edge = dir.join(r"Microsoft\Edge\Application\msedge.exe");
        std::fs::create_dir_all(edge.parent().unwrap()).unwrap();
        std::fs::write(&edge, b"").unwrap();
        let probe = CliSoftwareProbe::with_roots(Arc::new(Probe), vec![dir.clone()]);
        let items = probe.scan();
        let get = |id: &str| items.iter().find(|i| i.id == id);
        assert_eq!(get("node").unwrap().version.as_deref(), Some("20.11.1"));
        assert_eq!(get("git").unwrap().version.as_deref(), Some("2.45.1"));
        assert_eq!(get("py").unwrap().version.as_deref(), Some("3.12.4"));
        assert_eq!(get("winget").unwrap().version.as_deref(), Some("1.9.25200"));
        assert_eq!(get("edge").unwrap().kind, SoftwareKind::Browser);
        assert!(get("chrome").is_none() && get("docker").is_none());
        let miss = missing(&items);
        assert!(miss.contains(&"docker".to_owned()) && !miss.contains(&"python".to_owned()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
