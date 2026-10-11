//! Karty graficzne na żywo: wolna pamięć przed uruchomieniem silnika, który ma liczyć na karcie,
//! oraz podgląd dla strony „Urządzenia” (pamięć, obciążenie, temperatura, moc, procesy).
//! Źródło: `nvidia-smi` (instalowany ze sterownikiem NVIDIA). Bez niego pomiaru nie ma i decyzję
//! podejmuje wywołujący (dziś: próba na karcie, a po nieudanym starcie — silnik zapasowy).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Najdłuższe czekanie na odpowiedź `nvidia-smi`.
const TIMEOUT: Duration = Duration::from_secs(2);

/// Programy uruchamiane przez Alfę (nazwy plików bez rozszerzenia).
const ALFA_PROGRAMS: [&str; 7] = [
    "alfa-desktop",
    "llama-server",
    "whisper-server",
    "alfa-supertonic",
    "alfa-smart-turn",
    "piper",
    "pocket-tts",
];

/// Stan jednej karty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuCard {
    /// Numer karty w sterowniku.
    pub index: u32,
    /// Nazwa.
    pub name: String,
    /// Pamięć razem (MiB).
    pub total_mb: u32,
    /// Pamięć zajęta (MiB).
    pub used_mb: u32,
    /// Pamięć wolna (MiB).
    pub free_mb: u32,
    /// Obciążenie (%).
    pub load_pct: Option<u32>,
    /// Temperatura (°C).
    pub temp_c: Option<u32>,
    /// Pobór mocy (W, zaokrąglony).
    pub power_w: Option<u32>,
}

/// Proces korzystający z karty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuProcess {
    /// Identyfikator procesu.
    pub pid: u32,
    /// Nazwa programu (bez katalogu).
    pub name: String,
    /// Zajęta pamięć karty (MiB); system Windows często jej nie podaje.
    pub used_mb: Option<u32>,
    /// Program uruchomiony przez Alfę (silnik modelu, mowy albo sam program).
    pub alfa: bool,
}

fn run(args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("nvidia-smi");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let output = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Największa wolna pamięć spośród kart z wyjścia
/// `nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits` (MiB na linię).
pub fn parse_free_mb(output: &str) -> Option<u32> {
    output
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .max()
}

/// Wolna pamięć kart NVIDIA w MiB (największa z kart); `None`, gdy nie da się zmierzyć.
pub fn nvidia_free_mb() -> Option<u32> {
    parse_free_mb(&run(&[
        "--query-gpu=memory.free",
        "--format=csv,noheader,nounits",
    ])?)
}

/// Liczba całkowita z pola; ułamek jest zaokrąglany, „[N/A]” i śmieci dają `None`.
fn number(field: &str) -> Option<u32> {
    let value: f64 = field.trim().parse().ok()?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (value.is_finite() && value >= 0.0).then(|| value.round() as u32)
}

/// Karty z wyjścia zapytania o pola: numer, nazwa, pamięć razem / zajęta / wolna, obciążenie,
/// temperatura, moc. Linie bez liczby pamięci są pomijane.
pub fn parse_cards(output: &str) -> Vec<GpuCard> {
    output
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() < 8 {
                return None;
            }
            // Nazwa może zawierać przecinek — pola liczbowe liczymy od końca.
            let n = f.len();
            Some(GpuCard {
                index: number(f[0])?,
                name: f[1..n - 6].join(", "),
                total_mb: number(f[n - 6])?,
                used_mb: number(f[n - 5])?,
                free_mb: number(f[n - 4])?,
                load_pct: number(f[n - 3]),
                temp_c: number(f[n - 2]),
                power_w: number(f[n - 1]),
            })
        })
        .collect()
}

/// Procesy z wyjścia zapytania o pola: identyfikator, ścieżka programu, zajęta pamięć.
/// Powtórzenia tego samego procesu są łączone.
pub fn parse_processes(output: &str) -> Vec<GpuProcess> {
    let mut out: Vec<GpuProcess> = Vec::new();
    for line in output.lines() {
        let Some((pid, rest)) = line.split_once(',') else {
            continue;
        };
        let Some((path, used)) = rest.rsplit_once(',') else {
            continue;
        };
        let Some(pid) = number(pid) else {
            continue;
        };
        if out.iter().any(|p| p.pid == pid) {
            continue;
        }
        let file = path.trim().rsplit(['\\', '/']).next().unwrap_or_default();
        let stem = file
            .strip_suffix(".exe")
            .or_else(|| file.strip_suffix(".EXE"))
            .unwrap_or(file);
        if stem.is_empty() {
            continue;
        }
        out.push(GpuProcess {
            pid,
            name: file.to_owned(),
            used_mb: number(used),
            alfa: ALFA_PROGRAMS.iter().any(|p| stem.eq_ignore_ascii_case(p)),
        });
    }
    // Programy Alfy na górze, potem największe zużycie, potem nazwa — kolejność stała między odczytami.
    out.sort_by(|a, b| {
        b.alfa
            .cmp(&a.alfa)
            .then(b.used_mb.cmp(&a.used_mb))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.pid.cmp(&b.pid))
    });
    out
}

/// Karty NVIDIA i procesy, które z nich korzystają; `None`, gdy pomiaru nie da się zrobić
/// (brak `nvidia-smi` — inna karta albo brak sterownika).
pub fn nvidia_live() -> Option<(Vec<GpuCard>, Vec<GpuProcess>)> {
    let cards = parse_cards(&run(&[
        "--query-gpu=index,name,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu,power.draw",
        "--format=csv,noheader,nounits",
    ])?);
    if cards.is_empty() {
        return None;
    }
    let processes = run(&[
        "--query-compute-apps=pid,process_name,used_memory",
        "--format=csv,noheader,nounits",
    ])
    .map(|out| parse_processes(&out))
    .unwrap_or_default();
    Some((cards, processes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_card_with_most_free_memory() {
        assert_eq!(parse_free_mb("5368\n"), Some(5368));
        assert_eq!(parse_free_mb("  812 \r\n 11240\r\n"), Some(11240));
        assert_eq!(parse_free_mb(""), None);
        assert_eq!(parse_free_mb("[N/A]\n"), None);
        assert_eq!(parse_free_mb("brak karty\n4096\n"), Some(4096));
    }

    #[test]
    fn cards_are_read_with_missing_readings_and_commas_in_names() {
        let out = "0, NVIDIA GeForce RTX 4050 Laptop GPU, 6141, 549, 5372, 0, 48, 2.33\r\n\
                   1, Karta, z przecinkiem, 16384, 1024, 15360, [N/A], [N/A], [N/A]\n\
                   zepsuta linia\n2, Bez pamięci, [N/A], 1, 2, 3, 4, 5\n";
        let cards = parse_cards(out);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].name, "NVIDIA GeForce RTX 4050 Laptop GPU");
        assert_eq!(
            (cards[0].total_mb, cards[0].used_mb, cards[0].free_mb),
            (6141, 549, 5372)
        );
        assert_eq!(
            (cards[0].load_pct, cards[0].temp_c, cards[0].power_w),
            (Some(0), Some(48), Some(2))
        );
        assert_eq!(cards[1].name, "Karta, z przecinkiem");
        assert_eq!(
            (cards[1].load_pct, cards[1].temp_c, cards[1].power_w),
            (None, None, None)
        );
    }

    #[test]
    fn processes_are_named_marked_and_ordered() {
        let out = "17000, C:\\Program Files\\NVIDIA Corporation\\NVIDIA app\\CEF\\NVIDIA Overlay.exe, [N/A]\n\
                   4242, C:\\Users\\x\\AppData\\Local\\Alfa\\sidecars\\llama-cuda\\llama-server.exe, 2610\n\
                   12696, C:\\Windows\\explorer.exe, 120\n\
                   4242, C:\\Users\\x\\AppData\\Local\\Alfa\\sidecars\\llama-cuda\\llama-server.exe, 2610\n\
                   śmieci\n9, , 5\n";
        let p = parse_processes(out);
        assert_eq!(p.len(), 3);
        assert_eq!(
            (p[0].name.as_str(), p[0].alfa, p[0].used_mb),
            ("llama-server.exe", true, Some(2610))
        );
        assert_eq!(
            (p[1].name.as_str(), p[1].alfa, p[1].used_mb),
            ("explorer.exe", false, Some(120))
        );
        assert_eq!(
            (p[2].name.as_str(), p[2].used_mb),
            ("NVIDIA Overlay.exe", None)
        );
    }
}
