//! Karty graficzne na żywo: wolna pamięć przed uruchomieniem silnika, który ma liczyć na karcie,
//! oraz podgląd dla strony „Urządzenia” (pamięć, obciążenie, temperatura, moc, procesy).
//! Źródła: `nvidia-smi` (instalowany ze sterownikiem NVIDIA — daje też temperaturę i moc), a bez
//! niego liczniki wydajności Windows (`HardwarePort::gpu_usage`: pamięć, obciążenie i procesy dla
//! każdego producenta — AMD, Intel, NVIDIA). Bez obu pomiaru nie ma i decyzję podejmuje wywołujący
//! (próba na karcie, a po nieudanym starcie — silnik zapasowy).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use device_profile_contract::{GPU_TTS_MIN_VRAM_MB, has_memory_pool};
use platform_contract::{GpuAdapter, GpuUsage, HardwarePort};

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
            alfa: is_alfa(stem),
        });
    }
    sort_processes(&mut out);
    out
}

/// Program uruchomiony przez Alfę (nazwa pliku z rozszerzeniem `.exe` albo bez).
fn is_alfa(file: &str) -> bool {
    let stem = file
        .strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".EXE"))
        .unwrap_or(file);
    ALFA_PROGRAMS.iter().any(|p| stem.eq_ignore_ascii_case(p))
}

/// Programy Alfy na górze, potem największe zużycie, potem nazwa — kolejność stała między odczytami.
fn sort_processes(out: &mut [GpuProcess]) {
    out.sort_by(|a, b| {
        b.alfa
            .cmp(&a.alfa)
            .then(b.used_mb.cmp(&a.used_mb))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.pid.cmp(&b.pid))
    });
}

/// Sprzętowa karta (bez adaptera programowego i zdalnego pulpitu Microsoftu 0x1414).
pub(crate) fn hardware(a: &GpuAdapter) -> bool {
    !a.software && a.vendor_id != 0x1414
}

/// Karta z własną pulą pamięci (`device_profile_contract::has_memory_pool`): osobna albo zintegrowana
/// z rezerwą z BIOS-u ≥ 6 GB. Bez flagi sterownika „zintegrowana” = mniej niż 1 GB własnej pamięci.
pub(crate) fn discrete(a: &GpuAdapter) -> bool {
    let integrated = a.integrated.unwrap_or(a.dedicated_vram_mb < 1_024);
    hardware(a) && has_memory_pool(integrated, mb32(a.dedicated_vram_mb))
}

fn mb32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Główna karta z listy adapterów: z własną pulą pamięci, osobna przed zintegrowaną (z dużą
/// rezerwą z BIOS-u), potem największa — jak `Profile::primary_gpu`. Na niej liczą model rozmowy
/// i Qwen3-ASR (`crate::stt_device`), więc to jej wolną pamięć mierzymy.
pub fn main_card(adapters: &[GpuAdapter]) -> Option<&GpuAdapter> {
    adapters.iter().filter(|a| discrete(a)).max_by_key(|a| {
        (
            u8::from(a.integrated != Some(true)),
            a.dedicated_vram_mb,
            std::cmp::Reverse(a.index),
        )
    })
}

/// Czy komputer ma kartę z własną pulą pamięci (sama lista kart DXGI — kilka ms, bez liczników);
/// `None` — nie wiadomo (poza Windows albo błąd odczytu).
pub fn has_main_card() -> Option<bool> {
    HardwarePort::gpus(&platform_windows_impl::WinHardware)
        .ok()
        .map(|a| main_card(&a).is_some())
}

/// Wolna pamięć głównej karty z odczytu liczników systemu (MiB); `None` — brak karty z pulą albo
/// odczytu dla niej. Pamięć karty zintegrowanej z małą rezerwą to RAM — nie jest liczona.
pub fn free_from(adapters: &[GpuAdapter], usage: &GpuUsage) -> Option<u32> {
    let main = main_card(adapters)?;
    let used = usage.adapters.iter().find(|u| u.luid == main.luid)?;
    Some(mb32(
        main.dedicated_vram_mb
            .saturating_sub(used.dedicated_used_mb),
    ))
}

/// Karty i procesy z odczytu liczników systemu (bez temperatury i mocy — tych system nie podaje).
/// Karta zintegrowana pokazuje pamięć współdzieloną z RAM.
pub fn cards_from(adapters: &[GpuAdapter], usage: &GpuUsage) -> (Vec<GpuCard>, Vec<GpuProcess>) {
    let cards = adapters
        .iter()
        .filter(|a| hardware(a))
        .filter_map(|a| {
            let u = usage.adapters.iter().find(|u| u.luid == a.luid)?;
            let (total, used) = if discrete(a) {
                (a.dedicated_vram_mb, u.dedicated_used_mb)
            } else {
                (
                    a.dedicated_vram_mb + a.shared_memory_mb,
                    u.dedicated_used_mb + u.shared_used_mb,
                )
            };
            Some(GpuCard {
                index: a.index,
                name: a.name.trim().to_owned(),
                total_mb: mb32(total),
                used_mb: mb32(used.min(total)),
                free_mb: mb32(total.saturating_sub(used)),
                load_pct: u.load_pct,
                temp_c: None,
                power_w: None,
            })
        })
        .collect();
    let mut processes: Vec<GpuProcess> = Vec::new();
    for p in &usage.processes {
        let name = p.name.clone().unwrap_or_else(|| format!("PID {}", p.pid));
        let used = Some(mb32(p.dedicated_used_mb));
        if let Some(seen) = processes.iter_mut().find(|q| q.pid == p.pid) {
            // Ten sam proces na dwóch kartach — jedna pozycja z sumą pamięci.
            seen.used_mb = seen.used_mb.zip(used).map(|(a, b)| a.saturating_add(b));
            continue;
        }
        processes.push(GpuProcess {
            pid: p.pid,
            alfa: is_alfa(&name),
            name,
            used_mb: used,
        });
    }
    sort_processes(&mut processes);
    (cards, processes)
}

/// Odczyt liczników systemu: karty i zużycie (`None` — platforma bez liczników albo błąd).
fn system_read() -> Option<(Vec<GpuAdapter>, GpuUsage)> {
    let hw = platform_windows_impl::WinHardware;
    let adapters = hw.gpus().ok()?;
    let usage = hw.gpu_usage().ok()?;
    Some((adapters, usage))
}

/// Wolna pamięć głównej karty w MiB dla każdego producenta: karta NVIDIA — `nvidia-smi`, inne —
/// liczniki systemu. Komputer bez karty z własną pulą (sama zintegrowana albo żadna) daje `Some(0)` —
/// rozpoznawanie mowy nie zajmuje wtedy karty zintegrowanej, którą profil przeznacza na model
/// rozmowy. `None` — nie da się zmierzyć (wywołujący próbuje karty, a po nieudanym starcie bierze
/// silnik zapasowy). Bez listy kart (poza Windows) — jak dawniej: `nvidia-smi` albo `None`.
pub fn free_vram_mb() -> Option<u32> {
    let hw = platform_windows_impl::WinHardware;
    let Ok(adapters) = hw.gpus() else {
        return nvidia_free_mb();
    };
    // Lista kart wystarcza, żeby stwierdzić brak karty z pulą — liczniki mogą nie odpowiadać.
    let Some(main) = main_card(&adapters) else {
        return Some(0);
    };
    // Mierzymy kartę, na której liczy Qwen3-ASR: przy AMD obok NVIDIA nie bierzemy `nvidia-smi`.
    if main.vendor_id == 0x10DE
        && let Some(free) = nvidia_free_mb()
    {
        return Some(free);
    }
    free_from(&adapters, &hw.gpu_usage().ok()?)
}

/// Karta (numer adaptera DXGI) dla głosu Supertonic przez DirectML: największa karta z własną pulą
/// pamięci (osobna albo zintegrowana z dużą rezerwą z BIOS-u) z ≥ [`GPU_TTS_MIN_VRAM_MB`]. `None` — głos liczy na procesorze.
pub fn tts_dml_device(adapters: &[GpuAdapter]) -> Option<u32> {
    adapters
        .iter()
        .filter(|a| discrete(a) && a.dedicated_vram_mb >= u64::from(GPU_TTS_MIN_VRAM_MB))
        // Jak główna karta profilu: osobna przed zintegrowaną z dużą rezerwą, potem największa.
        .max_by_key(|a| {
            (
                u8::from(a.integrated != Some(true)),
                a.dedicated_vram_mb,
                std::cmp::Reverse(a.index),
            )
        })
        .map(|a| a.index)
}

/// [`tts_dml_device`] dla tej maszyny (DXGI; poza Windows — `None`).
pub fn tts_dml_device_here() -> Option<u32> {
    let adapters = HardwarePort::gpus(&platform_windows_impl::WinHardware).ok()?;
    tts_dml_device(&adapters)
}

/// Karty i procesy na żywo dla każdego producenta: `nvidia-smi` (z temperaturą i mocą), a bez
/// niego liczniki systemu. `None` — pomiaru nie da się zrobić.
pub fn live() -> Option<(Vec<GpuCard>, Vec<GpuProcess>)> {
    nvidia_live().or_else(|| {
        let (adapters, usage) = system_read()?;
        let (cards, processes) = cards_from(&adapters, &usage);
        (!cards.is_empty()).then_some((cards, processes))
    })
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
#[path = "gpu_mem_tests.rs"]
mod tests;
