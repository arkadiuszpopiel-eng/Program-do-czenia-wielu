//! Zestaw ewaluacyjny narzędzi F3 (`evals/F3/tools/`, ACCEPTANCE F3-07): zadania plikowe
//! i powłoki dla agentki z opisem, stanem początkowym katalogu i sprawdzeniem stanu końcowego.
//! Runner ([`run_task`]) przechodzi każde zadanie przez `agent-runtime` na dowolnym
//! `ModelProvider` (CI: skryptowana atrapa, u użytkownika — model lokalny llama.cpp) z tymi
//! samymi narzędziami i Brokerem co aplikacja.

mod run;

pub use run::{EvalEnv, EvalOptions, TaskResult, Totals, run_task, summarize, to_markdown};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Minimalna liczba zadań w zestawie.
pub const MIN_TASKS: usize = 30;

/// Rodzaj zadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Tylko narzędzia plikowe.
    Fs,
    /// Polecenia powłoki (wymaga `ExecPort` — Windows).
    Shell,
}

/// Stan początkowy katalogu zadania (ścieżki względne).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// Pliki: ścieżka → treść.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// Puste katalogi.
    #[serde(default)]
    pub dirs: Vec<String>,
}

/// Sprawdzenie treści pliku.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCheck {
    /// Dokładna treść (po obcięciu białych znaków na końcach).
    #[serde(default)]
    pub equals: Option<String>,
    /// Fragmenty, które muszą wystąpić.
    #[serde(default)]
    pub contains: Vec<String>,
    /// Fragmenty, których nie może być.
    #[serde(default)]
    pub not_contains: Vec<String>,
}

/// Oczekiwany stan końcowy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// Pliki, które muszą istnieć (z opcjonalnym sprawdzeniem treści).
    #[serde(default)]
    pub files: BTreeMap<String, FileCheck>,
    /// Ścieżki, których nie może być.
    #[serde(default)]
    pub absent: Vec<String>,
    /// Katalogi, które muszą istnieć.
    #[serde(default)]
    pub dirs: Vec<String>,
    /// Pliki ze stanu początkowego, które nie mogą się zmienić.
    #[serde(default)]
    pub unchanged: Vec<String>,
    /// Fragmenty odpowiedzi końcowej agentki (bez rozróżniania wielkości liter).
    #[serde(default)]
    pub answer_contains: Vec<String>,
}

/// Wywołanie narzędzia w skrypcie atrapy (CI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptedCall {
    /// Narzędzie (`fs_write`).
    pub tool: String,
    /// Argumenty (ścieżki względne do katalogu zadania).
    pub args: serde_json::Value,
}

/// Zadanie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalTask {
    /// Identyfikator (`fs-01-…`).
    pub id: String,
    /// Rodzaj.
    pub kind: TaskKind,
    /// Polecenie dla agentki (po polsku).
    pub goal: String,
    /// Stan początkowy.
    #[serde(default)]
    pub setup: Setup,
    /// Stan oczekiwany.
    pub expect: Expect,
    /// Skrypt atrapy modelu (zadania przechodzone na CI).
    #[serde(default)]
    pub ci_script: Vec<ScriptedCall>,
    /// Odpowiedź końcowa atrapy (CI).
    #[serde(default)]
    pub ci_answer: Option<String>,
}

/// Zestaw zadań.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSet {
    /// Wersja formatu.
    pub version: u32,
    /// Opis.
    pub description: String,
    /// Zadania.
    pub tasks: Vec<EvalTask>,
}

fn bad_rel(path: &str) -> bool {
    path.is_empty()
        || path.starts_with(['/', '\\'])
        || path.contains(':')
        || path.split(['/', '\\']).any(|s| s == ".." || s.is_empty())
}

impl TaskSet {
    /// Parsuje i waliduje zestaw (`tools` — nazwy narzędzi dostępnych agentce).
    pub fn parse(json: &str, tools: &[String]) -> Result<Self, String> {
        let set: Self = serde_json::from_str(json).map_err(|e| format!("format zestawu: {e}"))?;
        set.validate(tools)?;
        Ok(set)
    }

    /// Walidacja: wersja, ≥ 30 zadań, unikalne id, ścieżki względne bez `..`, znane narzędzia.
    pub fn validate(&self, tools: &[String]) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("nieobsługiwana wersja {}", self.version));
        }
        if self.tasks.len() < MIN_TASKS {
            return Err(format!("za mało zadań: {} < {MIN_TASKS}", self.tasks.len()));
        }
        let mut ids = BTreeSet::new();
        for t in &self.tasks {
            let fail = |why: &str| Err(format!("zadanie `{}`: {why}", t.id));
            if !ids.insert(t.id.as_str()) {
                return fail("powtórzony identyfikator");
            }
            if t.goal.trim().len() < 10 {
                return fail("brak polecenia");
            }
            let e = &t.expect;
            let checks = e.files.len()
                + e.absent.len()
                + e.dirs.len()
                + e.unchanged.len()
                + e.answer_contains.len();
            if checks == 0 {
                return fail("brak sprawdzeń stanu końcowego");
            }
            let paths = t
                .setup
                .files
                .keys()
                .chain(&t.setup.dirs)
                .chain(e.files.keys())
                .chain(&e.absent)
                .chain(&e.dirs)
                .chain(&e.unchanged);
            if let Some(p) = paths.into_iter().find(|p| bad_rel(p)) {
                return fail(&format!(
                    "ścieżka „{p}” nie jest względna albo zawiera `..`"
                ));
            }
            if let Some(c) = t.ci_script.iter().find(|c| !tools.contains(&c.tool)) {
                return fail(&format!("nieznane narzędzie `{}` w skrypcie", c.tool));
            }
        }
        Ok(())
    }

    /// Zadania ze skryptem atrapy (przechodzone na CI).
    pub fn scripted(&self) -> Vec<&EvalTask> {
        self.tasks
            .iter()
            .filter(|t| !t.ci_script.is_empty())
            .collect()
    }
}

/// Tworzy stan początkowy w `dir`.
pub fn prepare(task: &EvalTask, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for d in &task.setup.dirs {
        std::fs::create_dir_all(dir.join(d))?;
    }
    for (p, content) in &task.setup.files {
        let path = dir.join(p);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

/// Tekst pliku do porównań: UTF-8 albo UTF-16 LE z BOM (Windows PowerShell 5 `Out-File`),
/// bez BOM, końce linii `\r\n` → `\n`.
pub fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => {
            let units: Vec<u16> = rest
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()?
        }
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8(rest.to_vec()).ok()?,
        _ => String::from_utf8(bytes).ok()?,
    };
    Some(text.replace("\r\n", "\n"))
}

/// Sprawdza stan końcowy; zwraca listę niespełnionych warunków (pusta = zaliczone).
pub fn check(task: &EvalTask, dir: &Path, answer: &str) -> Vec<String> {
    let e = &task.expect;
    let mut failures = Vec::new();
    for (p, c) in &e.files {
        let Some(text) = read_text(&dir.join(p)) else {
            failures.push(format!("brak pliku {p}"));
            continue;
        };
        if let Some(want) = &c.equals
            && text.trim() != want.trim()
        {
            failures.push(format!("{p}: treść „{}” ≠ „{want}”", text.trim()));
        }
        for frag in &c.contains {
            if !text.contains(frag.as_str()) {
                failures.push(format!("{p}: brak „{frag}”"));
            }
        }
        for frag in &c.not_contains {
            if text.contains(frag.as_str()) {
                failures.push(format!("{p}: zawiera „{frag}”"));
            }
        }
    }
    for p in &e.absent {
        if dir.join(p).exists() {
            failures.push(format!("{p} nadal istnieje"));
        }
    }
    for p in &e.dirs {
        if !dir.join(p).is_dir() {
            failures.push(format!("brak katalogu {p}"));
        }
    }
    for p in &e.unchanged {
        let now = std::fs::read_to_string(dir.join(p)).ok();
        if now.as_deref() != task.setup.files.get(p).map(String::as_str) {
            failures.push(format!("{p} zmieniony albo usunięty"));
        }
    }
    let lower = answer.to_lowercase();
    for frag in &e.answer_contains {
        if !lower.contains(&frag.to_lowercase()) {
            failures.push(format!("odpowiedź bez „{frag}”"));
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str) -> EvalTask {
        EvalTask {
            id: id.into(),
            kind: TaskKind::Fs,
            goal: "Utwórz plik notatka.txt z tekstem X.".into(),
            setup: Setup {
                files: BTreeMap::from([("a.txt".into(), "A".into())]),
                dirs: vec!["Pobrane".into()],
            },
            expect: Expect {
                files: BTreeMap::from([(
                    "notatka.txt".into(),
                    FileCheck {
                        equals: Some("X".into()),
                        ..FileCheck::default()
                    },
                )]),
                unchanged: vec!["a.txt".into()],
                answer_contains: vec!["gotowe".into()],
                ..Expect::default()
            },
            ci_script: vec![ScriptedCall {
                tool: "fs_write".into(),
                args: serde_json::json!({"path": "notatka.txt", "content": "X"}),
            }],
            ci_answer: None,
        }
    }

    #[test]
    fn validation_and_checks() {
        let tools = vec!["fs_write".to_owned()];
        let mut set = TaskSet {
            version: 1,
            description: "t".into(),
            tasks: (0..MIN_TASKS).map(|i| task(&format!("t{i}"))).collect(),
        };
        assert!(set.validate(&tools).is_ok());
        assert_eq!(set.scripted().len(), MIN_TASKS);
        set.tasks[1].id = "t0".into();
        assert!(set.validate(&tools).unwrap_err().contains("powtórzony"));
        set.tasks[1].id = "t1".into();
        set.tasks[2].setup.dirs = vec!["../x".into()];
        assert!(set.validate(&tools).is_err());
        set.tasks[2].setup.dirs.clear();
        set.tasks[3].ci_script[0].tool = "rm".into();
        assert!(set.validate(&[]).is_err());
        assert!(TaskSet::parse("{}", &tools).is_err());

        let dir = tempfile::tempdir().unwrap();
        let t = task("x");
        prepare(&t, dir.path()).unwrap();
        assert_eq!(check(&t, dir.path(), "Gotowe").len(), 1);
        std::fs::write(dir.path().join("notatka.txt"), "X\n").unwrap();
        assert!(check(&t, dir.path(), "Gotowe.").is_empty());
        std::fs::write(dir.path().join("notatka.txt"), b"\xEF\xBB\xBFX\r\n").unwrap();
        assert!(check(&t, dir.path(), "Gotowe.").is_empty());
        let utf16: Vec<u8> = [0xFF, 0xFE, b'X', 0, b'\r', 0, b'\n', 0].to_vec();
        std::fs::write(dir.path().join("notatka.txt"), utf16).unwrap();
        assert!(check(&t, dir.path(), "Gotowe.").is_empty());
        std::fs::write(dir.path().join("a.txt"), "zmiana").unwrap();
        let f = check(&t, dir.path(), "nie");
        assert_eq!(f.len(), 2, "{f:?}");
    }
}
