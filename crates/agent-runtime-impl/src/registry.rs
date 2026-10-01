//! Rejestr narzędzi przebiegu: manifesty → `ToolSpec` dla modelu, filtr ról (uprawnienia idą
//! za rolą: grupy narzędzi, „tylko odczyt”) i listy narzędzi z `RunSpec`.

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_runtime_contract::RunSpec;
use providers_contract::ToolSpec;
use tools_common_contract::Tool;

/// Narzędzia dostępne w przebiegu.
#[derive(Clone, Default)]
pub(crate) struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    /// Przecięcie: wszystkie narzędzia × `spec.tools` × grupy ról (rola tylko do odczytu nie
    /// dostaje narzędzi zmieniających stan). Nieznana nazwa w `spec.tools` = błąd.
    pub(crate) fn for_spec(all: &[Arc<dyn Tool>], spec: &RunSpec) -> Result<Self, String> {
        let by_name: BTreeMap<String, Arc<dyn Tool>> = all
            .iter()
            .map(|t| (t.manifest().name.clone(), t.clone()))
            .collect();
        let groups: Vec<String> = spec.roles.iter().flat_map(|r| r.tools.clone()).collect();
        let read_only = !spec.roles.is_empty() && spec.roles.iter().all(|r| r.read_only);
        let mut reg = Self::default();
        for name in &spec.tools {
            let tool = by_name
                .get(name)
                .ok_or_else(|| format!("nieznane narzędzie `{name}`"))?;
            let m = tool.manifest();
            let allowed = spec.roles.is_empty() || m.allowed_for(&groups, read_only);
            if allowed && !reg.tools.contains_key(name) {
                reg.order.push(name.clone());
                reg.tools.insert(name.clone(), tool.clone());
            }
        }
        Ok(reg)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.order.clone()
    }

    /// Definicje dla modelu; w fazie weryfikacji tylko narzędzia niezmieniające stanu.
    pub(crate) fn specs(&self, read_only: bool) -> Vec<ToolSpec> {
        self.order
            .iter()
            .filter_map(|n| self.tools.get(n))
            .filter(|t| !read_only || !t.manifest().mutating)
            .map(|t| t.manifest().to_spec())
            .collect()
    }
}
