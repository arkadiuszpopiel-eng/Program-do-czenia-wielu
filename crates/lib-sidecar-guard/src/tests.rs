use super::*;

fn seen(image: &str, path: &str, created: u64) -> Seen {
    Seen {
        image: image.into(),
        path: path.into(),
        created,
    }
}

#[test]
fn registry_file_is_parsed_leniently_with_and_without_fingerprint() {
    let rows = parse(
        "12\tLlama-Server.exe\nśmieci\n0x\tx.exe\n7\t\n34\twhisper-server.exe\tC:\\A\\Whisper-Server.exe\t133\n\
         35\tx.exe\t\t9\n36\ty.exe\tc:\\y.exe\tnie-liczba\n",
    );
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[0],
        (
            12,
            Entry {
                image: "llama-server.exe".into(),
                seen: None
            }
        )
    );
    assert_eq!(
        rows[1].1.seen,
        Some(seen("whisper-server.exe", "c:\\a\\whisper-server.exe", 133))
    );
    assert_eq!(rows[2].1.seen, None, "pusta ścieżka — bez odcisku");
    assert_eq!(rows[3].1.seen, None, "zły czas — bez odcisku");
    assert_eq!(
        image_name(Path::new("C:/a/b/Llama-Server.EXE")),
        "llama-server.exe"
    );
    assert_eq!(image_name(Path::new("")), "");
    // Zapis i odczyt dają to samo.
    let entry = &rows[1].1;
    assert_eq!(parse(&line(34, entry)), vec![(34, entry.clone())]);
}

#[test]
fn identity_needs_same_image_and_fingerprint_when_known() {
    let known = Entry {
        image: "llama-server.exe".into(),
        seen: Some(seen("llama-server.exe", "c:\\alfa\\llama-server.exe", 100)),
    };
    assert!(known.matches(&seen("llama-server.exe", "c:\\alfa\\llama-server.exe", 100)));
    assert!(
        !known.matches(&seen("llama-server.exe", "c:\\alfa\\llama-server.exe", 101)),
        "ten sam numer i plik, ale inny proces (inny czas utworzenia)"
    );
    assert!(!known.matches(&seen("llama-server.exe", "d:\\inny\\llama-server.exe", 100)));
    assert!(!known.matches(&seen("cmd.exe", "c:\\alfa\\llama-server.exe", 100)));
    let unknown = Entry {
        image: "cmd.exe".into(),
        seen: None,
    };
    assert!(unknown.matches(&seen("cmd.exe", "c:\\x\\cmd.exe", 5)));
    assert!(!unknown.matches(&seen("ping.exe", "c:\\x\\ping.exe", 5)));
}

#[test]
fn snapshot_rows_are_parsed() {
    let map = os::parse_snapshot(
        "41\tCMD.EXE\tC:\\Windows\\System32\\cmd.exe\t133000000000000000\r\nzłe\n42\tx.exe\t\tbad\n",
    );
    assert_eq!(map.len(), 1);
    assert_eq!(
        map[&41],
        seen(
            "cmd.exe",
            "c:\\windows\\system32\\cmd.exe",
            133_000_000_000_000_000
        )
    );
}

/// Jeden test na cały przebieg: rejestr jest wspólny dla procesu testowego.
#[cfg(windows)]
#[test]
fn children_are_killed_on_exit_and_orphans_on_next_start_only_when_identity_matches() {
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    fn sleeper() -> Child {
        Command::new("cmd")
            .args(["/C", "ping -n 90 127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cmd: {e}"))
    }
    fn exited(child: &mut Child) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }
    fn wait_fingerprint(pid: u32) -> Option<Seen> {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(s) = fingerprint_of(pid) {
                return Some(s);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("katalog: {e}"));
    let file = dir.path().join("sidecars.pids");
    assert_eq!(init(file.clone()), 0, "pusty rejestr — nic do sprzątania");

    // Zgłoszenie: odcisk (pełna ścieżka i czas utworzenia) trafia do rejestru na dysku.
    let mut mine = sleeper();
    let mut other = sleeper();
    register(mine.id(), Path::new("C:/Windows/System32/cmd.exe"));
    assert_eq!(registered(), 1);
    let print = wait_fingerprint(mine.id()).unwrap_or_else(|| panic!("brak odcisku procesu"));
    assert!(print.path.ends_with("\\cmd.exe"), "{print:?}");
    assert!(print.created > 0);
    assert!(std::fs::read_to_string(&file).is_ok_and(|t| t.contains(&print.path)));

    // Wyjście z programu: zgłoszony proces jest kończony, niezgłoszony zostaje.
    assert_eq!(kill_all(), 1);
    assert!(exited(&mut mine), "zgłoszony proces zakończony");
    assert!(
        matches!(other.try_wait(), Ok(None)),
        "obcy proces nietknięty"
    );
    assert_eq!(registered(), 0);

    // Awaria: rejestr na dysku. Osierocony proces z pasującym odciskiem — zakończony. Ten sam
    // plik pod innym numerem, ale z innym czasem utworzenia (numer użyty ponownie) — nietknięty;
    // inna nazwa pliku — nietknięta.
    let mut orphan = sleeper();
    let orphan_print = os::snapshot(&[orphan.id()])
        .and_then(|mut s| s.remove(&orphan.id()))
        .unwrap_or_else(|| panic!("odcisk sieroty"));
    let other_print = os::snapshot(&[other.id()])
        .and_then(|mut s| s.remove(&other.id()))
        .unwrap_or_else(|| panic!("odcisk innego"));
    let text = format!(
        "{}\tcmd.exe\t{}\t{}\n{}\tcmd.exe\t{}\t{}\n{}\tllama-server.exe\n",
        orphan.id(),
        orphan_print.path,
        orphan_print.created,
        other.id(),
        other_print.path,
        other_print.created + 1,
        other.id(),
    );
    std::fs::write(&file, text).unwrap_or_else(|e| panic!("zapis: {e}"));
    assert_eq!(init(file.clone()), 1);
    assert!(
        exited(&mut orphan),
        "osierocony proces zakończony przy starcie"
    );
    assert!(
        matches!(other.try_wait(), Ok(None)),
        "numer użyty ponownie albo inny obraz — nietknięty"
    );

    // Proces zakończony przez moduł jest wykreślany i nie jest ruszany przy wyjściu.
    register(other.id(), Path::new("cmd.exe"));
    unregister(other.id());
    assert_eq!(kill_all(), 0);
    assert!(matches!(other.try_wait(), Ok(None)));
    let _ = other.kill();
    let _ = other.wait();
}
