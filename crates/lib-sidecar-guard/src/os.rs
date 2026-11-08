//! Zapytania do systemu: odcisk procesów (nazwa, pełna ścieżka, czas utworzenia) i ich kończenie.
//! Bez `windows-rs` (zasada: Windows API tylko w `platform-windows-*`) — narzędzia systemowe.

#[cfg(any(windows, test))]
use std::collections::BTreeMap;

/// Proces zastany pod numerem: nazwa pliku i ścieżka (małymi literami), czas utworzenia (FILETIME).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// Nazwa pliku obrazu.
    pub image: String,
    /// Pełna ścieżka pliku obrazu.
    pub path: String,
    /// Czas utworzenia procesu (jednostki 100 ns od 1601 r., UTC).
    pub created: u64,
}

/// Wiersz odpowiedzi: `numer<TAB>nazwa<TAB>ścieżka<TAB>czas`.
#[cfg(any(windows, test))]
pub(crate) fn parse_snapshot(text: &str) -> BTreeMap<u32, Seen> {
    text.lines()
        .filter_map(|row| {
            let mut cols = row.trim_end_matches('\r').split('\t');
            let pid = cols.next()?.trim().parse().ok()?;
            let image = cols.next()?.trim().to_lowercase();
            let path = cols.next()?.trim().to_lowercase();
            let created = cols.next()?.trim().parse().ok()?;
            (!image.is_empty()).then_some((
                pid,
                Seen {
                    image,
                    path,
                    created,
                },
            ))
        })
        .collect()
}

#[cfg(windows)]
mod imp {
    use std::collections::BTreeMap;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    use super::{Seen, parse_snapshot};

    /// Bez okna konsoli dla narzędzi systemowych.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn tool(name: &str) -> Command {
        let mut cmd = Command::new(name);
        cmd.stdin(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    /// Odcisk procesów o podanych numerach jednym zapytaniem (Win32_Process); `None` — zapytanie
    /// nieudane (brak odpowiedzi systemu), pusta mapa — żaden z procesów nie działa.
    pub fn snapshot(pids: &[u32]) -> Option<BTreeMap<u32, Seen>> {
        if pids.is_empty() {
            return Some(BTreeMap::new());
        }
        let filter = pids
            .iter()
            .map(|p| format!("ProcessId={p}"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let script = format!(
            "$ErrorActionPreference='Stop'; Get-CimInstance Win32_Process -Filter '{filter}' | \
             ForEach-Object {{ \"$($_.ProcessId)`t$($_.Name)`t$($_.ExecutablePath)`t$($_.CreationDate.ToFileTimeUtc())\" }}"
        );
        let out = tool("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| parse_snapshot(&String::from_utf8_lossy(&out.stdout)))
    }

    /// Zapas, gdy zapytanie się nie udało: czy pod numerem działa plik o tej nazwie.
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
mod imp {
    use std::collections::BTreeMap;

    use super::Seen;

    /// Poza Windows rejestr tylko prowadzi listę (procesy kończy `kill_on_drop` modułów).
    pub fn snapshot(_pids: &[u32]) -> Option<BTreeMap<u32, Seen>> {
        Some(BTreeMap::new())
    }

    pub fn alive_as(_pid: u32, _image: &str) -> bool {
        false
    }

    pub fn kill(_pid: u32) -> bool {
        false
    }
}

pub(crate) use imp::{alive_as, kill, snapshot};
