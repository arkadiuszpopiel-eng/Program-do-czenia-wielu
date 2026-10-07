//! Głosy v0 bez kluczy (ADR 0011): dwie bazowe mówczynie Pocket TTS PL × wysokość/tempo → cztery
//! odrębne głosy; zapas Piper `pl_PL` z tymi samymi proporcjami. Reguła: głos (także zapasowy)
//! jednej agentki nie może brzmieć jak głos innej.

use std::collections::HashMap;

use personas_contract::PersonaId;

use crate::{TtsEngine, TtsError, VoicePreset, VoiceRef};

/// Mówczynie bazowe Pocket TTS PL (nazwy głosów modelu społeczności; wybór ostateczny: casting, ADR 11).
pub const POCKET_BASE: [&str; 2] = ["pl-f1", "pl-f2"];
/// Głos zapasowy Piper (kobiecy, `pl_PL`).
pub const PIPER_BASE: &str = "pl_PL-gosia-medium";

fn voice(persona: PersonaId, engine: TtsEngine, base: &str, pitch: f32, rate: f32) -> VoiceRef {
    VoiceRef {
        persona,
        engine,
        preset: VoicePreset {
            base_speaker: base.into(),
            pitch,
            rate,
        },
        reference: None,
    }
}

/// Łańcuchy v0 (biblie głosu: Alfa — ciepły środek; Beta — wyżej, miękko, umiarkowanie;
/// Gama — niżej, wolniej; Delta — jaśniej, żwawo).
pub fn v0_chains() -> Vec<(PersonaId, Vec<VoiceRef>)> {
    let p = |id: PersonaId, base: &str, pitch: f32, rate: f32, piper_pitch: f32| {
        (
            id.clone(),
            vec![
                voice(id.clone(), TtsEngine::Pocket, base, pitch, rate),
                voice(id, TtsEngine::Piper, PIPER_BASE, piper_pitch, rate),
            ],
        )
    };
    vec![
        p(PersonaId::alfa(), POCKET_BASE[0], 1.00, 1.00, 1.00),
        p(PersonaId::beta(), POCKET_BASE[1], 1.06, 0.97, 1.08),
        p(PersonaId::gama(), POCKET_BASE[0], 0.92, 0.92, 0.90),
        p(PersonaId::delta(), POCKET_BASE[1], 1.12, 1.08, 1.14),
    ]
}

/// Sprawdza łańcuchy: zakresy presetów i odrębność brzmień między agentkami.
pub fn validate_chains(chains: &[(PersonaId, Vec<VoiceRef>)]) -> Result<(), TtsError> {
    let mut owners: HashMap<String, &PersonaId> = HashMap::new();
    for (persona, chain) in chains {
        if chain.is_empty() {
            return Err(TtsError::NoVoice(persona.to_string()));
        }
        for v in chain {
            let p = &v.preset;
            if !(0.7..=1.4).contains(&p.pitch) || !(0.6..=1.6).contains(&p.rate) {
                return Err(TtsError::InvalidConfig(format!(
                    "{persona}: preset poza zakresem {p:?}"
                )));
            }
            if &v.persona != persona {
                return Err(TtsError::InvalidConfig(format!(
                    "{persona}: głos przypisany do {}",
                    v.persona
                )));
            }
            if let Some(other) = owners.insert(v.timbre(), persona)
                && other != persona
            {
                return Err(TtsError::InvalidConfig(format!(
                    "{persona} i {other} mają to samo brzmienie ({})",
                    v.timbre()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v0_has_four_distinct_voices_from_two_bases() {
        let chains = v0_chains();
        assert_eq!(chains.len(), 4);
        assert!(validate_chains(&chains).is_ok());
        let bases: std::collections::BTreeSet<&str> = chains
            .iter()
            .map(|(_, c)| c[0].preset.base_speaker.as_str())
            .collect();
        assert_eq!(bases.len(), 2);
    }

    #[test]
    fn rejects_shared_timbre_and_bad_presets() {
        let mut chains = v0_chains();
        let mut stolen = chains[0].1[0].clone();
        stolen.persona = chains[1].0.clone();
        chains[1].1.push(stolen);
        assert!(matches!(
            validate_chains(&chains),
            Err(TtsError::InvalidConfig(_))
        ));
        let mut bad = v0_chains();
        bad[2].1[0].preset.pitch = 2.0;
        assert!(validate_chains(&bad).is_err());
        let mut wrong_owner = v0_chains();
        wrong_owner[3].1[0].persona = PersonaId::alfa();
        assert!(validate_chains(&wrong_owner).is_err());
        let empty = vec![(PersonaId::alfa(), vec![])];
        assert_eq!(
            validate_chains(&empty),
            Err(TtsError::NoVoice("alfa".into()))
        );
    }
}
