//! Pomiar zakończenia procesu bez uznawania błędu odczytu za dowód zakończenia.

use std::io;
use std::time::Duration;

fn terminated(stat: &io::Result<String>) -> bool {
    match stat {
        Err(error) => error.kind() == io::ErrorKind::NotFound,
        Ok(stat) => stat
            .rsplit_once(')')
            .and_then(|(_, fields)| fields.split_whitespace().next())
            .is_some_and(|state| state == "Z"),
    }
}

pub async fn wait_for_exit(pid: u32, limit: Duration) -> Result<(), String> {
    wait_with(limit, || {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
    })
    .await
}

async fn wait_with(
    limit: Duration,
    mut read: impl FnMut() -> io::Result<String>,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + limit;
    loop {
        let snapshot = read();
        if terminated(&snapshot) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("ostatni odczyt /proc: {snapshot:?}"));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_missing_process_or_zombie_proves_exit() {
        assert!(terminated(&Err(io::ErrorKind::NotFound.into())));
        assert!(terminated(&Ok("123 (fake) Z 1 123".into())));
        assert!(!terminated(&Ok("123 (fake) S 1 123".into())));
        for kind in [io::ErrorKind::Interrupted, io::ErrorKind::PermissionDenied] {
            assert!(!terminated(&Err(kind.into())), "{kind:?}");
        }
        assert!(!terminated(&Ok(String::new())));
    }

    #[test]
    fn command_name_may_contain_spaces_and_parentheses() {
        assert!(terminated(&Ok("123 (fake ) process) Z 1 123".into())));
        assert!(!terminated(&Ok("123 (fake Z process) S 1 123".into())));
    }

    #[tokio::test]
    async fn unknown_read_does_not_skip_a_still_running_process() {
        let mut calls = 0;
        wait_with(Duration::from_secs(1), || {
            calls += 1;
            match calls {
                1 => Err(io::ErrorKind::Interrupted.into()),
                2 => Ok("123 (fake) S 1 123".into()),
                _ => Ok("123 (fake) Z 1 123".into()),
            }
        })
        .await
        .unwrap();
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn live_or_unknown_process_fails_with_snapshot() {
        for snapshot in ["123 (fake) S 1 123", ""] {
            let error = wait_with(Duration::ZERO, || Ok(snapshot.into()))
                .await
                .unwrap_err();
            assert!(error.contains("ostatni odczyt /proc"));
        }
    }
}
