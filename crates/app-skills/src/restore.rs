//! Odtworzenie definicji z biblioteki Kreatora. Obsada rozmów pozostaje osobną decyzją.

use agent_builder_contract::AgentManifest;
use personas_contract::{Catalog, Personas};

/// Najpierw sprawdza cały katalog, potem publikuje brakujące definicje. Istniejące zgodne
/// wpisy są zachowane; konflikt nie nadpisuje persony ani roli z innego źródła.
pub(crate) async fn restore_catalog(
    personas: &dyn Personas,
    library: Vec<AgentManifest>,
) -> Result<(), String> {
    let mut catalog = Catalog::builtin();
    for role in personas.roles().into_iter().filter(|role| !role.builtin) {
        catalog.add_role(role).map_err(|e| e.to_string())?;
    }
    for persona in personas.personas().into_iter().filter(|p| !p.builtin) {
        catalog.add_persona(persona).map_err(|e| e.to_string())?;
    }
    let mut roles = Vec::new();
    let mut agents = Vec::new();
    for manifest in library {
        let role = manifest.role;
        match catalog.role(&role.id) {
            Some(existing) if existing == &role && !role.builtin => {}
            Some(_) => return Err(format!("Konflikt zapisanej roli „{}”.", role.id)),
            None => {
                catalog.add_role(role.clone()).map_err(|e| e.to_string())?;
                roles.push(role);
            }
        }
        let persona = manifest.persona;
        match catalog.persona(&persona.id) {
            Some(existing) if existing == &persona && !persona.builtin => {}
            Some(_) => return Err(format!("Konflikt zapisanej persony „{}”.", persona.id)),
            None => {
                catalog
                    .add_persona(persona.clone())
                    .map_err(|e| e.to_string())?;
                agents.push(persona);
            }
        }
    }
    for role in roles {
        personas.add_role(role).await.map_err(|e| e.to_string())?;
    }
    for persona in agents {
        personas
            .add_persona(persona)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_builder_contract::{BuilderCore, BuilderPolicy, contract_tests as ct};
    use personas_fake::FakePersonas;

    fn manifest() -> AgentManifest {
        let core = BuilderCore::new(
            BuilderPolicy::with_ceiling(risk_classifier_contract::AutonomyLevel::L3),
            ct::sample_tools(),
            Catalog::builtin(),
            voice_tts_contract::v0_chains(),
        );
        core.build(&agent_builder_contract::from_description(ct::DESCRIPTION).draft)
            .expect("manifest z próbki kontraktu")
            .manifest
    }

    #[tokio::test]
    async fn restores_definitions_once_without_assigning_roles() {
        let personas = FakePersonas::new();
        let m = manifest();
        restore_catalog(&personas, vec![m.clone()]).await.unwrap();
        let after = personas.export();
        assert_eq!(after.personas, vec![m.persona.clone()]);
        assert_eq!(after.roles, vec![m.role.clone()]);
        assert!(after.casts.is_empty());
        assert!(personas.set_cast_calls().is_empty());
        let events = personas.events().len();
        restore_catalog(&personas, vec![m]).await.unwrap();
        assert_eq!(personas.export(), after);
        assert_eq!(personas.events().len(), events, "bez ponownej publikacji");
    }

    #[tokio::test]
    async fn rejects_entire_batch_before_publish_when_a_later_definition_conflicts() {
        let personas = FakePersonas::new();
        let before = personas.export();
        let m = manifest();
        let mut conflicting = m.clone();
        conflicting.role.prompt = "Inny opis zatwierdzonej roli.".into();
        assert!(
            restore_catalog(&personas, vec![m, conflicting])
                .await
                .is_err()
        );
        assert_eq!(personas.export(), before);
        assert!(personas.events().is_empty());
    }

    #[tokio::test]
    async fn cannot_replace_an_existing_role_or_restore_a_builtin_persona() {
        let personas = FakePersonas::new();
        let m = manifest();
        restore_catalog(&personas, vec![m.clone()]).await.unwrap();
        let before = personas.export();
        let mut changed = m.clone();
        changed.role.read_only = !changed.role.read_only;
        assert!(restore_catalog(&personas, vec![changed]).await.is_err());
        assert_eq!(personas.export(), before);
        let mut builtin = m;
        builtin.persona = personas_contract::builtin_personas().remove(0);
        assert!(restore_catalog(&personas, vec![builtin]).await.is_err());
        assert_eq!(personas.export(), before);
    }
}
