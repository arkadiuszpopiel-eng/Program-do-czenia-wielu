//! Rejestr procesów pobocznych Alfy (`llama-server`, `whisper-server`, silniki głosu, Smart Turn).
//!
//! Moduł uruchamiający proces woła [`register`], po jego zakończeniu [`unregister`]. Powłoka woła
//! [`kill_all`] przy wyjściu z programu (wyjście przez `exit` nie niszczy obiektów procesów, więc
//! `kill_on_drop` nie działa) oraz [`init`] przy starcie — rejestr poprzedniego uruchomienia leży
//! na dysku i pozwala zakończyć procesy osierocone przez awarię. Proces jest kończony tylko wtedy,
//! gdy pod zapisanym numerem nadal działa plik o zapisanej nazwie (numery procesów są używane
//! ponownie).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

#[derive(Default)]
struct State {
    /// Plik rejestru (`None` — bez zapisu na dysk, np. w testach modułów).
    file: Option<PathBuf>,
    /// Numer procesu → nazwa pliku obrazu (małymi literami).
    children: BTreeMap<u32, String>,
}

fn state() -> MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn image_name(program: &Path) -> String {
    program
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn persist(state: &State) {
    let Some(file) = &state.file else { return };
    let text: String = state
        .children
        .iter()
        .map(|(pid, image)| format!("{pid}\t{image}\n"))
        .collect();
    if let Err(error) = std::fs::write(file, text) {
        tracing::warn!(plik = %file.display(), %error, "rejestr procesów pobocznych: zapis nieudany");
    }
}

/// Wpisy rejestru z pliku (`numer<TAB>nazwa`); błędne wiersze są pomijane.
fn parse(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|line| {
            let (pid, image) = line.split_once('\t')?;
            let image = image.trim().to_lowercase();
            (!image.is_empty()).then_some((pid.trim().parse().ok()?, image))
        })
        .collect()
}

/// Ustawia plik rejestru i kończy procesy osierocone przez poprzednie uruchomienie. Zwraca liczbę
/// zakończonych procesów.
pub fn init(file: PathBuf) -> usize {
    let previous = std::fs::read_to_string(&file)
        .map(|text| parse(&text))
        .unwrap_or_default();
    let mut killed = 0;
    for (pid, image) in previous {
        if os::alive_as(pid, &image) && os::kill(pid) {
            tracing::warn!(pid, obraz = %image, "zakończono proces poboczny osierocony przez poprzednie uruchomienie");
            killed += 1;
        }
    }
    let mut st = state();
    st.file = Some(file);
    persist(&st);
    killed
}

/// Zgłasza uruchomiony proces poboczny.
pub fn register(pid: u32, program: &Path) {
    let image = image_name(program);
    if pid == 0 || image.is_empty() {
        return;
    }
    let mut st = state();
    st.children.insert(pid, image);
    persist(&st);
}

/// Wykreśla proces, który się zakończył albo został zabity przez swój moduł.
pub fn unregister(pid: u32) {
    let mut st = state();
    if st.children.remove(&pid).is_some() {
        persist(&st);
    }
}

/// Kończy wszystkie zgłoszone procesy, które jeszcze działają (z ich procesami potomnymi).
/// Zwraca liczbę zakończonych.
pub fn kill_all() -> usize {
    let children = std::mem::take(&mut state().children);
    let mut killed = 0;
    for (pid, image) in &children {
        if os::alive_as(*pid, image) && os::kill(*pid) {
            killed += 1;
        }
    }
    persist(&state());
    killed
}

/// Liczba zgłoszonych procesów (diagnostyka, testy).
pub fn registered() -> usize {
    state().children.len()
}

#[cfg(windows)]
mod os {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    /// Bez okna konsoli dla narzędzi systemowych.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn tool(name: &str) -> Command {
        let mut cmd = Command::new(name);
        cmd.stdin(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    /// Czy pod numerem `pid` działa proces o nazwie pliku `image` (małymi literami).
    pub fn alive_as(pid: u32, image: &str) -> bool {
        let filter = format!("PID eq {pid}");
        tool("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .is_ok_and(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .to_lowercase()
                    .contains(&format!("\"{image}\""))
            })
    }

    /// Kończy proces z potomkami.
    pub fn kill(pid: u32) -> bool {
        tool("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(not(windows))]
mod os {
    /// Poza Windows rejestr tylko prowadzi listę (procesy kończy `kill_on_drop` modułów).
    pub fn alive_as(_pid: u32, _image: &str) -> bool {
        false
    }

    pub fn kill(_pid: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_file_is_parsed_leniently() {
        let rows = parse("12\tLlama-Server.exe\nśmieci\n0x\tx.exe\n7\t\n34\twhisper-server.exe\n");
        assert_eq!(
            rows,
            vec![
                (12, "llama-server.exe".to_owned()),
                (34, "whisper-server.exe".to_owned())
            ]
        );
        assert_eq!(
            image_name(Path::new("C:/a/b/Llama-Server.EXE")),
            "llama-server.exe"
        );
        assert_eq!(image_name(Path::new("")), "");
    }

    /// Jeden test na cały przebieg: rejestr jest wspólny dla procesu testowego.
    #[cfg(windows)]
    #[test]
    fn children_are_killed_on_exit_and_orphans_on_next_start() {
        use std::process::{Child, Command, Stdio};
        use std::time::{Duration, Instant};

        fn sleeper() -> Child {
            Command::new("cmd")
                .args(["/C", "ping -n 60 127.0.0.1"])
                .stdout(Stdio::null())
                .spawn()
                .unwrap_or_else(|e| panic!("cmd: {e}"))
        }
        fn exited(child: &mut Child) -> bool {
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            false
        }

        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("katalog: {e}"));
        let file = dir.path().join("sidecars.pids");
        assert_eq!(init(file.clone()), 0, "pusty rejestr — nic do sprzątania");

        // Wyjście z programu: zgłoszony proces jest kończony, niezgłoszony zostaje.
        let mut mine = sleeper();
        let mut other = sleeper();
        register(mine.id(), Path::new("C:/Windows/System32/cmd.exe"));
        assert_eq!(registered(), 1);
        assert!(std::fs::read_to_string(&file).is_ok_and(|t| t.contains("cmd.exe")));
        assert_eq!(kill_all(), 1);
        assert!(exited(&mut mine), "zgłoszony proces zakończony");
        assert!(
            matches!(other.try_wait(), Ok(None)),
            "obcy proces nietknięty"
        );
        assert_eq!(registered(), 0);

        // Awaria: rejestr został na dysku — następny start kończy osierocony proces, ale nie rusza
        // numeru, pod którym działa już coś innego niż zapisano.
        let mut orphan = sleeper();
        let text = format!(
            "{}\tcmd.exe\n{}\tllama-server.exe\n",
            orphan.id(),
            other.id()
        );
        std::fs::write(&file, text).unwrap_or_else(|e| panic!("zapis: {e}"));
        assert_eq!(init(file.clone()), 1);
        assert!(
            exited(&mut orphan),
            "osierocony proces zakończony przy starcie"
        );
        assert!(
            matches!(other.try_wait(), Ok(None)),
            "inny obraz pod tym numerem — nietknięty"
        );

        // Proces zakończony przez moduł jest wykreślany i nie jest ruszany przy wyjściu.
        register(other.id(), Path::new("cmd.exe"));
        unregister(other.id());
        assert_eq!(kill_all(), 0);
        assert!(matches!(other.try_wait(), Ok(None)));
        let _ = other.kill();
        let _ = other.wait();
    }
}
