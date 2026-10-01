//! Trwały magazyn checkpointów: plik JSON na przebieg w katalogu sesji, zapis atomowy
//! (plik tymczasowy + `rename`), odporny na urwany zapis (stary checkpoint zostaje).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use agent_runtime_contract::{Checkpoint, CheckpointError, CheckpointStore, RunId};

/// Magazyn katalogowy (`<sesja>/runs/<run>.json`).
#[derive(Debug, Clone)]
pub struct DirCheckpointStore {
    dir: PathBuf,
}

fn err(e: impl std::fmt::Display) -> CheckpointError {
    CheckpointError(e.to_string())
}

fn valid_id(run: &RunId) -> Result<&str, CheckpointError> {
    let id = run.as_str();
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(id)
    } else {
        Err(err(format!("niepoprawny identyfikator przebiegu `{id}`")))
    }
}

impl DirCheckpointStore {
    /// Otwiera (tworzy) katalog.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, CheckpointError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(err)?;
        Ok(Self { dir })
    }

    /// Katalog magazynu.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, run: &RunId) -> Result<PathBuf, CheckpointError> {
        Ok(self.dir.join(format!("{}.json", valid_id(run)?)))
    }
}

impl CheckpointStore for DirCheckpointStore {
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        let path = self.path(&checkpoint.run)?;
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_vec(checkpoint).map_err(err)?;
        let mut f = fs::File::create(&tmp).map_err(err)?;
        f.write_all(&data).map_err(err)?;
        f.sync_all().map_err(err)?;
        drop(f);
        fs::rename(&tmp, &path).map_err(err)
    }

    fn latest(&self, run: &RunId) -> Result<Option<Checkpoint>, CheckpointError> {
        let path = self.path(run)?;
        match fs::read(&path) {
            Ok(data) => serde_json::from_slice(&data).map(Some).map_err(err),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(err(e)),
        }
    }

    fn runs(&self) -> Result<Vec<RunId>, CheckpointError> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(err)? {
            let name = entry
                .map_err(err)?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if let Some(id) = name.strip_suffix(".json") {
                out.push(RunId::new(id));
            }
        }
        out.sort();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_runtime_contract::contract_tests::sample_spec;

    #[test]
    fn round_trip_and_rejects_bad_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let store = DirCheckpointStore::open(tmp.path().join("runs")).unwrap();
        let mut cp = Checkpoint::initial(RunId::new("r-1"), sample_spec("m", &["fs_read"]));
        cp.seq = 3;
        store.save(&cp).unwrap();
        cp.seq = 4;
        store.save(&cp).unwrap();
        assert_eq!(store.latest(&RunId::new("r-1")).unwrap(), Some(cp));
        assert_eq!(store.latest(&RunId::new("brak")).unwrap(), None);
        assert_eq!(store.runs().unwrap(), vec![RunId::new("r-1")]);
        assert!(store.latest(&RunId::new("../x")).is_err());
        assert!(store.dir().ends_with("runs"));
        std::fs::write(store.dir().join("zly.json"), b"{").unwrap();
        assert!(store.latest(&RunId::new("zly")).is_err());
    }
}
