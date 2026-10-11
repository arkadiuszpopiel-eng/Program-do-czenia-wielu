//! Wolna pamięć karty graficznej — pomiar przed uruchomieniem silnika, który ma liczyć na karcie.
//! Źródło: `nvidia-smi` (instalowany ze sterownikiem NVIDIA). Bez niego pomiaru nie ma i decyzję
//! podejmuje wywołujący (dziś: próba na karcie, a po nieudanym starcie — silnik zapasowy).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Najdłuższe czekanie na odpowiedź `nvidia-smi`.
const TIMEOUT: Duration = Duration::from_secs(2);

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
    let mut cmd = Command::new("nvidia-smi");
    cmd.args(["--query-gpu=memory.free", "--format=csv,noheader,nounits"])
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
    parse_free_mb(&String::from_utf8_lossy(&output.stdout))
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
}
