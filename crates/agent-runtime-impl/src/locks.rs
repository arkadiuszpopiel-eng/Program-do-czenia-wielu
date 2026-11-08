//! Zasoby wyłączne wywołań narzędzi (PLAN §9.3): narzędzie zmieniające stan dostaje na czas
//! wywołania dzierżawy plików, których dotyka (ścieżki z argumentów względem katalogu
//! roboczego), katalogu polecenia powłoki albo ekranu+wejścia (`gui.control`). Dzierżawy brane
//! w porządku kanonicznym (bez cykli oczekiwania), z limitem czekania i anulowaniem; już trzymane
//! przez zadanie schedulera (`AlreadyHeld`) — bez nowej dzierżawy.

use std::collections::BTreeSet;
use std::time::Duration;

use scheduler_contract::{
    Holder as LockHolder, Lease, LeaseRequest, OnTimeout, Priority, Resource, SchedError,
};
use tools_common_contract::ToolManifest;

use crate::engine::Engine;

/// Klucze argumentów wskazujące pliki, których dotyka narzędzie.
const PATH_KEYS: [&str; 6] = ["path", "from", "to", "dest", "target", "root"];

fn absolute(path: &str, workdir: Option<&str>) -> String {
    let p = path.trim();
    let rooted = p.starts_with('/') || p.starts_with('\\') || p.chars().nth(1) == Some(':');
    match workdir {
        Some(dir) if !rooted => format!("{}/{}", dir.trim_end_matches(['/', '\\']), p),
        _ => p.to_owned(),
    }
}

/// Zasoby wywołania (puste dla narzędzi tylko do odczytu).
pub fn resources_for(
    manifest: &ToolManifest,
    args: &serde_json::Value,
    workdir: Option<&str>,
) -> BTreeSet<Resource> {
    let mut out = BTreeSet::new();
    if !manifest.mutating {
        return out;
    }
    let caps = &manifest.capabilities;
    if caps.iter().any(|c| c == "gui.control") {
        out.insert(Resource::ScreenInput);
    }
    if caps.iter().any(|c| c == "fs.write") {
        for key in PATH_KEYS {
            if let Some(p) = args.get(key).and_then(serde_json::Value::as_str) {
                out.insert(Resource::file(&absolute(p, workdir)));
            }
        }
    }
    if caps.iter().any(|c| c == "shell.exec") {
        let cwd = args.get("cwd").and_then(serde_json::Value::as_str);
        if let Some(dir) = cwd
            .map(|c| absolute(c, workdir))
            .or(workdir.map(str::to_owned))
        {
            out.insert(Resource::file(&dir));
        }
    }
    out
}

/// Dlaczego nie dostano zasobu (dla modelu).
pub(crate) fn lock_failure(resource: &Resource, err: &SchedError) -> String {
    match err {
        SchedError::Timeout { .. } => format!(
            "zasób {resource} jest zajęty przez inną agentkę zbyt długo — zrób najpierw coś innego albo spróbuj później"
        ),
        SchedError::Cancelled => "anulowano w trakcie czekania na zasób".to_owned(),
        other => format!("zasób {resource} niedostępny: {other}"),
    }
}

impl Engine {
    /// Posiadaczka dzierżaw przebiegu.
    fn lock_holder(&self) -> LockHolder {
        self.hooks
            .lease_holder
            .clone()
            .unwrap_or_else(|| LockHolder::System(format!("agent-run:{}", self.handle.run)))
    }

    /// Bierze dzierżawy wywołania; `Err(powód)` = nie wykonuj.
    pub(crate) async fn acquire(
        &self,
        manifest: &ToolManifest,
        args: &serde_json::Value,
    ) -> Result<Vec<Lease>, String> {
        let Some(locks) = self.shared.ext.locks.clone() else {
            return Ok(Vec::new());
        };
        let wanted = resources_for(manifest, args, self.cp.spec.workdir.as_deref());
        let wait = Duration::from_millis(self.shared.config.lease_wait_ms);
        let mut leases = Vec::with_capacity(wanted.len());
        for resource in wanted {
            let req =
                LeaseRequest::new(resource.clone(), self.lock_holder(), Priority::Normal, wait)
                    .on_timeout(OnTimeout::Fail);
            let got = tokio::select! {
                r = locks.acquire(req) => r,
                () = self.handle.cancel.cancelled() => Err(SchedError::Cancelled),
            };
            match got {
                Ok(lease) => leases.push(lease),
                Err(SchedError::AlreadyHeld { .. }) => {}
                Err(e) => return Err(lock_failure(&resource, &e)),
            }
        }
        Ok(leases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(caps: &[&str], mutating: bool) -> ToolManifest {
        ToolManifest {
            name: "t".into(),
            id: "t".into(),
            title: "t".into(),
            description: String::new(),
            input_schema: json!({}),
            output_schema: json!({}),
            reversible: risk_classifier_contract::Reversibility::Yes,
            capabilities: caps.iter().map(|c| (*c).to_owned()).collect(),
            groups: vec![],
            mutating,
            untrusted_output: None,
        }
    }

    #[test]
    fn resources_from_args() {
        let w = manifest(&["fs.write"], true);
        let r = resources_for(
            &w,
            &json!({"from": "a.txt", "to": "C:\\X\\b.txt"}),
            Some("/d"),
        );
        assert_eq!(
            r,
            BTreeSet::from([Resource::file("/d/a.txt"), Resource::file("c:/x/b.txt")])
        );
        assert!(
            resources_for(&manifest(&["fs.read"], false), &json!({"path": "a"}), None).is_empty()
        );
        let g = manifest(&["gui.control"], true);
        assert_eq!(
            resources_for(&g, &json!({}), None),
            BTreeSet::from([Resource::ScreenInput])
        );
        let s = manifest(&["shell.exec"], true);
        assert_eq!(
            resources_for(&s, &json!({"command": "ls"}), Some("/w")),
            BTreeSet::from([Resource::file("/w")])
        );
        assert!(lock_failure(&Resource::ScreenInput, &SchedError::Cancelled).contains("anulowano"));
    }
}
