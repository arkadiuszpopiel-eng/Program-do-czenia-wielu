//! Urządzenia widziane przez `llama-server` i przypinanie modelu do konkretnej karty. Na maszynie
//! z dwiema kartami (np. laptop: zintegrowany Intel + NVIDIA) kompilacja Vulkan widzi obie i bez
//! wskazania potrafi wybrać słabszą albo rozłożyć model na obie. Serwer wypisuje swoje urządzenia
//! po `--list-devices` (`CUDA0: NVIDIA GeForce RTX 4050 Laptop GPU (6140 MiB, 5080 MiB free)`);
//! kartę z profilu urządzenia dopasowujemy po nazwie i przekazujemy jej identyfikator w `--device`.
//! Numery adapterów systemu (DXGI) nie muszą się zgadzać z numeracją serwera, stąd nazwa.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::BackendKey;

/// Urządzenie z listy `llama-server --list-devices`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDevice {
    /// Identyfikator dla `--device` (np. `CUDA0`, `Vulkan1`).
    pub id: String,
    /// Nazwa karty.
    pub name: String,
    /// Pamięć razem (MiB).
    pub total_mb: u32,
    /// Pamięć wolna (MiB).
    pub free_mb: u32,
}

/// Najdłuższe oczekiwanie na listę urządzeń — serwer wypisuje ją i kończy w ułamku sekundy.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// Urządzenia z wyjścia `--list-devices` (wiersze po nagłówku „Available devices:”).
pub fn parse_devices(output: &str) -> Vec<EngineDevice> {
    output
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("Available devices"))
        .skip(1)
        .filter_map(|line| {
            let (id, rest) = line.trim().split_once(": ")?;
            if id.is_empty() || id.contains(' ') {
                return None;
            }
            // „NAZWA (6140 MiB, 5080 MiB free)” — nawias z pamięcią jest ostatni.
            let (name, memory) = rest.rsplit_once(" (")?;
            let mut numbers = memory
                .split(|c: char| !c.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.parse::<u32>().ok());
            Some(EngineDevice {
                id: id.to_owned(),
                name: name.trim().to_owned(),
                total_mb: numbers.next().unwrap_or(0),
                free_mb: numbers.next().unwrap_or(0),
            })
        })
        .collect()
}

/// Uruchamia serwer z `--list-devices` i czyta listę; błąd albo brak odpowiedzi = pusta lista
/// (model startuje wtedy bez przypięcia, jak dotąd).
pub fn probe(program: &Path, timeout: Duration) -> Vec<EngineDevice> {
    let mut command = Command::new(program);
    command
        .arg("--list-devices")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Bez okna konsoli (`CREATE_NO_WINDOW`).
        command.creation_flags(0x0800_0000);
    }
    let Ok(mut child) = command.spawn() else {
        return Vec::new();
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                // Serwer nie zna opcji albo zawisł — nie czekamy dłużej.
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
    let Ok(output) = child.wait_with_output() else {
        return Vec::new();
    };
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    parse_devices(&text)
}

/// Urządzenie serwera odpowiadające karcie o nazwie `gpu_name` dla danego backendu: identyfikator
/// zaczyna się od nazwy backendu (`CUDA…`, `Vulkan…`), a nazwy kart pasują do siebie tak samo
/// jak w wyborze karty dzierżawy ([`model_residency_contract::same_card_name`]).
pub fn device_for<'a>(
    devices: &'a [EngineDevice],
    backend: BackendKey,
    gpu_name: &str,
) -> Option<&'a EngineDevice> {
    let prefix = match backend {
        BackendKey::Cuda => "cuda",
        BackendKey::Vulkan => "vulkan",
        BackendKey::Cpu => return None,
    };
    devices
        .iter()
        .filter(|d| d.id.to_ascii_lowercase().starts_with(prefix))
        .find(|d| model_residency_contract::same_card_name(&d.name, gpu_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wyjście zebrane na laptopie właściciela (2026-10-09, kompilacja CUDA) i typowe wyjście
    /// kompilacji Vulkan na maszynie z dwiema kartami.
    const CUDA: &str = "ggml_cuda_init: found 1 CUDA devices:\n  Device 0: NVIDIA GeForce RTX 4050 Laptop GPU, compute capability 8.9, VMM: yes\nload_backend: loaded CUDA backend from C:\\x\\ggml-cuda.dll\nAvailable devices:\n  CUDA0: NVIDIA GeForce RTX 4050 Laptop GPU (6140 MiB, 5080 MiB free)\n";
    const VULKAN: &str = "Available devices:\n  Vulkan0: Intel(R) Iris(R) Xe Graphics (8012 MiB, 7200 MiB free)\n  Vulkan1: NVIDIA GeForce RTX 4050 Laptop GPU (6140 MiB, 5900 MiB free)\n";

    #[test]
    fn device_list_is_parsed_and_noise_is_ignored() {
        let cuda = parse_devices(CUDA);
        assert_eq!(
            cuda,
            [EngineDevice {
                id: "CUDA0".into(),
                name: "NVIDIA GeForce RTX 4050 Laptop GPU".into(),
                total_mb: 6_140,
                free_mb: 5_080,
            }]
        );
        let vulkan = parse_devices(VULKAN);
        assert_eq!(vulkan.len(), 2);
        assert_eq!(vulkan[0].id, "Vulkan0");
        assert_eq!(vulkan[0].name, "Intel(R) Iris(R) Xe Graphics");
        assert!(parse_devices("Available devices:\n").is_empty());
        assert!(parse_devices("load_backend: loaded CPU backend\n").is_empty());
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn card_from_the_device_profile_is_matched_by_name_and_backend() {
        let vulkan = parse_devices(VULKAN);
        let pick = |backend, name: &str| device_for(&vulkan, backend, name).map(|d| d.id.as_str());
        // Karta główna i zintegrowana trafiają na różne urządzenia tej samej kompilacji.
        assert_eq!(
            pick(BackendKey::Vulkan, "NVIDIA GeForce RTX 4050 Laptop GPU"),
            Some("Vulkan1")
        );
        assert_eq!(
            pick(BackendKey::Vulkan, "Intel(R) Iris(R) Xe Graphics"),
            Some("Vulkan0")
        );
        // Nazwa z systemu bez znaczków handlowych też pasuje; inny backend i CPU — nie.
        assert_eq!(
            pick(BackendKey::Vulkan, "Intel Iris Xe Graphics"),
            Some("Vulkan0")
        );
        assert_eq!(
            pick(BackendKey::Cuda, "NVIDIA GeForce RTX 4050 Laptop GPU"),
            None
        );
        assert_eq!(
            pick(BackendKey::Cpu, "NVIDIA GeForce RTX 4050 Laptop GPU"),
            None
        );
        assert_eq!(pick(BackendKey::Vulkan, "AMD Radeon RX 7600"), None);
        assert_eq!(pick(BackendKey::Vulkan, ""), None);
        let cuda = parse_devices(CUDA);
        assert_eq!(
            device_for(
                &cuda,
                BackendKey::Cuda,
                "NVIDIA GeForce RTX 4050 Laptop GPU"
            )
            .map(|d| d.id.as_str()),
            Some("CUDA0")
        );
    }

    #[test]
    fn missing_or_silent_server_gives_no_devices() {
        let none = probe(
            Path::new("/nie/ma/takiego/llama-server"),
            Duration::from_millis(200),
        );
        assert!(none.is_empty());
    }
}
