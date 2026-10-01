//! Rejestr narzędzi przebiegu: manifesty → `ToolSpec` dla modelu, filtr ról (uprawnienia idą
//! za rolą: grupy narzędzi, „tylko odczyt”), lista narzędzi z `RunSpec`, koperta uprawnień v1
//! ([`RunGrant`]) i wbudowane narzędzie delegacji (rola z grupą `delegate` + obsada w opcjach).

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_runtime_contract::{
    DELEGATE_GROUP, DELEGATE_TOOL, DelegateArgs, RunGrant, RunOptions, RunSpec,
};
use providers_contract::ToolSpec;
use risk_classifier_contract::Reversibility;
use tools_common_contract::{Tool, ToolManifest, schema_of};

/// Narzędzia dostępne w przebiegu.
#[derive(Clone, Default)]
pub(crate) struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
    delegate: Option<ToolManifest>,
    /// Koperta przebiegu: narzędzia z `spec.tools` w kopercie v1 — przed filtrem ról agentki
    /// (sufit dla delegacji i Krytyczki: potomek ≤ rodzic).
    envelope: Vec<Arc<dyn Tool>>,
}

/// Czy koperta (jeśli jest) dopuszcza narzędzie.
fn granted(grant: Option<&RunGrant>, m: &ToolManifest) -> bool {
    grant.is_none_or(|g| g.permits(&m.name, &m.capabilities, m.mutating))
}

/// Manifest wbudowanego narzędzia delegacji (opis z listą ról obsady).
fn delegate_manifest(options: &RunOptions) -> Option<ToolManifest> {
    let crew = options.crew.as_ref()?;
    let roles: Vec<String> = crew
        .cast
        .assignments
        .iter()
        .flat_map(|(p, roles)| {
            roles.iter().map(move |r| {
                let name = crew.persona(p).map_or(p.as_str(), |x| x.name.as_str());
                format!("{r} ({name})")
            })
        })
        .collect();
    Some(ToolManifest {
        name: DELEGATE_TOOL.to_owned(),
        id: "agent-runtime.delegate".to_owned(),
        title: "Delegowanie zadania".to_owned(),
        description: format!(
            "Przekazuje samodzielne podzadanie innej roli z obsady (ta sama sesja; jej uprawnienia \
             są nie większe niż Twoje). Zwraca jej wynik. Role w obsadzie: {}.",
            roles.join(", ")
        ),
        input_schema: schema_of::<DelegateArgs>(),
        output_schema: serde_json::json!({"type": "object"}),
        reversible: Reversibility::Yes,
        capabilities: Vec::new(),
        groups: vec![DELEGATE_GROUP.to_owned()],
        mutating: true,
        untrusted_output: None,
    })
}

impl ToolRegistry {
    /// Przecięcie: wszystkie narzędzia × `spec.tools` × grupy ról (rola tylko do odczytu nie
    /// dostaje narzędzi zmieniających stan) × koperta v1. Nieznana nazwa w `spec.tools` = błąd.
    /// Delegacja: obsada w opcjach, rola z grupą `delegate`, głębokość < limit, koperta ją dopuszcza.
    pub(crate) fn for_run(
        all: &[Arc<dyn Tool>],
        spec: &RunSpec,
        options: &RunOptions,
        max_depth: u32,
    ) -> Result<Self, String> {
        let by_name: BTreeMap<String, Arc<dyn Tool>> = all
            .iter()
            .map(|t| (t.manifest().name.clone(), t.clone()))
            .collect();
        let groups: Vec<String> = spec.roles.iter().flat_map(|r| r.tools.clone()).collect();
        let read_only = !spec.roles.is_empty() && spec.roles.iter().all(|r| r.read_only);
        let grant = options.grant.as_ref();
        let mut reg = Self::default();
        for name in &spec.tools {
            if name == DELEGATE_TOOL {
                continue;
            }
            let tool = by_name
                .get(name)
                .ok_or_else(|| format!("nieznane narzędzie `{name}`"))?;
            let m = tool.manifest();
            if !granted(grant, m) || reg.envelope.iter().any(|t| t.manifest().name == *name) {
                continue;
            }
            reg.envelope.push(tool.clone());
            let allowed = spec.roles.is_empty() || m.allowed_for(&groups, read_only);
            if allowed && !reg.tools.contains_key(name) {
                reg.order.push(name.clone());
                reg.tools.insert(name.clone(), tool.clone());
            }
        }
        let may_delegate = groups.iter().any(|g| g == DELEGATE_GROUP)
            && !read_only
            && options.depth < max_depth
            && grant.is_none_or(|g| g.tools.contains(DELEGATE_TOOL));
        if may_delegate {
            reg.delegate = delegate_manifest(options);
            if reg.delegate.is_some() {
                reg.order.push(DELEGATE_TOOL.to_owned());
            }
        }
        Ok(reg)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Manifest narzędzia (także delegacji).
    pub(crate) fn manifest(&self, name: &str) -> Option<&ToolManifest> {
        if name == DELEGATE_TOOL {
            return self.delegate.as_ref();
        }
        self.tools.get(name).map(|t| t.manifest())
    }

    /// Czy to dostępne narzędzie delegacji.
    pub(crate) fn is_delegate(&self, name: &str) -> bool {
        name == DELEGATE_TOOL && self.delegate.is_some()
    }

    /// Czy wywołanie może iść równolegle (znane, tylko odczyt, nie delegacja).
    pub(crate) fn is_parallel_read(&self, name: &str) -> bool {
        self.tools.get(name).is_some_and(|t| !t.manifest().mutating)
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.order.clone()
    }

    /// Koperta przebiegu (bez delegacji) — sufit przebiegów potomnych.
    pub(crate) fn envelope(&self) -> impl Iterator<Item = &Arc<dyn Tool>> {
        self.envelope.iter()
    }

    /// Definicje dla modelu; w fazie weryfikacji tylko narzędzia niezmieniające stanu.
    pub(crate) fn specs(&self, read_only: bool) -> Vec<ToolSpec> {
        self.order
            .iter()
            .filter_map(|n| self.manifest(n))
            .filter(|m| !read_only || !m.mutating)
            .map(ToolManifest::to_spec)
            .collect()
    }
}
