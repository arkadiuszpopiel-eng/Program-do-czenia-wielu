//! Stabilny `MachineId`: SHA-256 z separacją domeny nad identyfikatorem systemu (Windows
//! `MachineGuid`, Linux `/etc/machine-id`), a gdy go brak — nad losowym UUID zapisanym raz
//! w pliku stanu. Wynik (32 hex) nie pozwala odtworzyć identyfikatora systemu.

use std::fs;
use std::path::Path;

use device_profile_contract::{DeviceProfileError, MachineId};
use sha2::{Digest, Sha256};

/// Separacja domeny skrótu (zmiana = nowe identyfikatory wszystkich maszyn; nie zmieniać).
const DOMAIN: &[u8] = b"alfa/device-profile/machine-id/v1\0";
/// Nazwa pliku z identyfikatorem zapasowym w katalogu stanu.
pub const MACHINE_ID_FILE: &str = "machine-id";

/// Identyfikator z ziarna systemowego (wielkość liter i białe znaki bez znaczenia).
pub fn derive_machine_id(seed: &str) -> Result<MachineId, DeviceProfileError> {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update(seed.trim().to_lowercase().as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    MachineId::parse(&hex)
}

fn storage(e: &std::io::Error, path: &Path) -> DeviceProfileError {
    DeviceProfileError::Storage(format!("{}: {e}", path.display()))
}

/// Identyfikator maszyny: z ziarna systemowego, a bez niego z pliku `state_dir/machine-id`
/// (tworzonego przy pierwszym uruchomieniu z losowego UUID).
pub fn resolve_machine_id(
    seed: Option<&str>,
    state_dir: &Path,
) -> Result<MachineId, DeviceProfileError> {
    if let Some(seed) = seed.filter(|s| !s.trim().is_empty()) {
        return derive_machine_id(seed);
    }
    let file = state_dir.join(MACHINE_ID_FILE);
    if let Ok(text) = fs::read_to_string(&file)
        && let Ok(id) = MachineId::parse(text.trim())
    {
        return Ok(id);
    }
    let id = derive_machine_id(&uuid::Uuid::new_v4().to_string())?;
    fs::create_dir_all(state_dir).map_err(|e| storage(&e, state_dir))?;
    let tmp = state_dir.join(format!(".{MACHINE_ID_FILE}.{}.tmp", std::process::id()));
    fs::write(&tmp, id.as_str()).map_err(|e| storage(&e, &tmp))?;
    fs::rename(&tmp, &file).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        storage(&e, &file)
    })?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_ids_are_stable_normalized_and_opaque() {
        let guid = "3F2504E0-4F89-11D3-9A0C-0305E82C3301";
        let a = derive_machine_id(guid).unwrap();
        let b = derive_machine_id(&format!("  {}\n", guid.to_lowercase())).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_str().len(), 32);
        assert!(!a.as_str().contains("3f2504e0"));
        assert_ne!(a, derive_machine_id("inny").unwrap());
        // Wartość referencyjna: zmiana algorytmu zmieniłaby identyfikatory istniejących maszyn.
        assert_eq!(
            derive_machine_id("ABC").unwrap().as_str(),
            "43812e872f830bd95a43f4a7b2f26f67"
        );
    }

    #[test]
    fn fallback_file_is_created_once_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("stan");
        let first = resolve_machine_id(None, &state).unwrap();
        let second = resolve_machine_id(Some("  "), &state).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            fs::read_to_string(state.join(MACHINE_ID_FILE)).unwrap(),
            first.as_str()
        );
        fs::write(state.join(MACHINE_ID_FILE), "zepsuty").unwrap();
        let regenerated = resolve_machine_id(None, &state).unwrap();
        assert_ne!(regenerated, first);
        let seeded = resolve_machine_id(Some("guid"), &state).unwrap();
        assert_eq!(seeded, derive_machine_id("guid").unwrap());
    }
}
