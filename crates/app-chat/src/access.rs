//! Uprawnienia narzędzi per persona, nakładane przed każdym uruchomieniem.

use crate::engine::ChatEngine;
use personas_contract::Role;

const FULL_GROUPS: &[&str] = &[
    "fs",
    "worktree",
    "shell",
    "engine",
    "cli-bridge",
    "gui.control",
    "office",
    "browser",
    "web",
    "plugin",
    "system",
    "net",
    "vision",
    "media",
    "mcp",
    "memory",
    "delegate",
];

fn apply_profile(mut roles: Vec<Role>, mode: &str, groups: &str, read_only: bool) -> Vec<Role> {
    // Pusty wektor ról w runtime oznacza brak filtra, więc profil musi pozostawić rolę
    // ograniczającą także przy zadaniu persony nieobsadzonej w tej sesji.
    if roles.is_empty() {
        roles = personas_contract::builtin_roles()
            .into_iter()
            .filter(|r| r.id.as_str() == "thinker")
            .collect();
    }
    let selected: Option<Vec<String>> = match mode {
        "full" | "read" => Some(FULL_GROUPS.iter().map(|g| (*g).to_owned()).collect()),
        "custom" => Some(
            groups
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        "off" => Some(Vec::new()),
        "roles" => None,
        _ => Some(Vec::new()),
    };
    if let Some(groups) = selected {
        for role in &mut roles {
            role.tools.clone_from(&groups);
            role.read_only = mode == "read" || mode == "off" || (mode == "custom" && read_only);
        }
    }
    roles
}

impl ChatEngine {
    /// Jedna ścieżka przygotowania nowych zadań czatu i harmonogramu.
    pub async fn prepare_agent_access(
        &self,
        spec: &mut agent_runtime_contract::RunSpec,
        options: &mut agent_runtime_contract::RunOptions,
    ) {
        spec.roles = self
            .agent_access(
                &spec.session,
                spec.agent.as_str(),
                std::mem::take(&mut spec.roles),
            )
            .await;
        self.apply_crew_access(&spec.session, options).await;
    }
    /// Dyrygentka zachowuje orkiestrację czatu i Pipeline; może zlecać wykonanie sobie.
    pub(crate) async fn orchestrates(&self, agent: &str, roles: &[Role]) -> bool {
        roles
            .iter()
            .any(|r| r.tools.iter().any(|g| g == "delegate"))
            && self.can_orchestrate(agent).await
    }
    /// Delegacja jest narzędziem zmieniającym stan, niedostępnym w profilach odczytu/wyłączenia.
    pub(crate) async fn can_orchestrate(&self, agent: &str) -> bool {
        self.config_value(&format!("agents.{agent}.access"))
            .await
            .and_then(|v| v.as_str().map(str::to_owned))
            .is_none_or(|mode| mode == "full" || mode == "roles")
    }
    /// Profile są przypinane także do potomków runtime i niezależnej Krytyczki.
    pub async fn apply_crew_access(
        &self,
        session: &sessions_contract::SessionId,
        options: &mut agent_runtime_contract::RunOptions,
    ) {
        options.continuation = self.continuation_policy().await;
        use sessions_contract::SessionCatalog;
        options.request_privacy = match self.inner.sessions.session(session).map(|s| s.privacy) {
            Ok(sessions_contract::PrivacyTag::Normal) => providers_contract::PrivacyTag::Normal,
            _ => providers_contract::PrivacyTag::Private,
        };
        if let Some(crew) = options.crew.as_mut() {
            for persona in &crew.personas {
                let roles = self
                    .agent_access(session, persona.id.as_str(), crew.roles.clone())
                    .await;
                crew.persona_roles.insert(persona.id.clone(), roles);
            }
        }
    }
    /// Profil wybrany przez właściciela; brak nadpisania Alfy oznacza pełny dostęp.
    pub async fn agent_access(
        &self,
        session: &sessions_contract::SessionId,
        agent: &str,
        roles: Vec<Role>,
    ) -> Vec<Role> {
        let prefix = format!("agents.{agent}");
        let mode = self
            .config_value(&format!("{prefix}.access"))
            .await
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| if agent == "alfa" { "full" } else { "roles" }.into());
        let groups = self
            .config_value(&format!("{prefix}.groups"))
            .await
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let read = self
            .config_bool(&format!("{prefix}.read_only"), false)
            .await;
        if (mode == "full" || mode == "read")
            && let Some(stack) = &self.inner.agents
        {
            let mut available = std::collections::BTreeSet::from(["delegate".to_owned()]);
            for tool in stack.tools.all() {
                available.extend(tool.manifest().groups.clone());
            }
            return apply_profile(
                roles,
                "custom",
                &available.into_iter().collect::<Vec<_>>().join(","),
                mode == "read",
            );
        }
        if mode == "roles" {
            // Rola zadania może zmienić instrukcję, lecz nie poszerza przydziału narzędzi.
            let assigned = self.cast_roles(session, &personas_contract::PersonaId::new(agent));
            let groups = assigned
                .iter()
                .flat_map(|r| r.tools.iter().cloned())
                .collect::<Vec<_>>()
                .join(",");
            let read_only = assigned.is_empty() || assigned.iter().all(|r| r.read_only);
            return apply_profile(roles, "custom", &groups, read_only);
        }
        apply_profile(roles, &mode, &groups, read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use personas_contract::builtin_roles;

    #[test]
    fn full_profile_expands_groups_and_off_never_becomes_unrestricted() {
        let roles = builtin_roles();
        let conductor = vec![roles[0].clone()];
        let full = apply_profile(conductor.clone(), "full", "", false);
        assert!(full.iter().any(|r| r.tools.iter().any(|g| g == "fs")));
        assert!(full.iter().any(|r| r.tools.iter().any(|g| g == "shell")));
        let off = apply_profile(conductor, "off", "", false);
        assert!(!off.is_empty());
        assert!(off.iter().all(|r| r.tools.is_empty() && r.read_only));
    }

    #[test]
    fn custom_profile_replaces_every_role_and_read_mode_cannot_mutate() {
        let roles = builtin_roles();
        let custom = apply_profile(roles.clone(), "custom", "fs.read, net.read", true);
        assert!(
            custom
                .iter()
                .all(|r| r.tools == ["fs.read", "net.read"] && r.read_only)
        );
        assert!(
            apply_profile(roles.clone(), "read", "", false)
                .iter()
                .all(|r| r.read_only)
        );
        assert_eq!(apply_profile(roles.clone(), "roles", "shell", false), roles);
    }

    #[test]
    fn empty_roles_and_invalid_profile_never_disable_the_tool_filter() {
        for mode in ["off", "roles", "invalid"] {
            let roles = apply_profile(Vec::new(), mode, "", false);
            assert!(!roles.is_empty());
            assert!(roles.iter().all(|r| r.tools.is_empty()));
        }
    }
}
