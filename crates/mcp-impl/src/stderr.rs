//! Końcówka standardowego wyjścia błędów serwera MCP — przyczyna nieudanego startu (np. `npm ERR!
//! 404`, brak modułu Pythona) zamiast samego „połączenie zamknięte”. Wartości zmiennych
//! przekazanych serwerowi są maskowane; potok jest czytany do końca, żeby serwer nie zawisł na
//! pełnym buforze.

use std::sync::{Arc, Mutex, PoisonError};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// Najwięcej znaków zapamiętanej końcówki.
const TAIL_MAX: usize = 1_500;

/// Zbierana w tle końcówka stderr.
#[derive(Clone, Default)]
pub(crate) struct StderrTail {
    text: Arc<Mutex<String>>,
    done: Arc<tokio::sync::Notify>,
    finished: Arc<std::sync::atomic::AtomicBool>,
}

impl StderrTail {
    /// Czyta potok w tle do końca.
    pub fn collect<R: AsyncRead + Send + Unpin + 'static>(reader: R) -> Self {
        let tail = Self::default();
        let out = tail.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut text = out.text.lock().unwrap_or_else(PoisonError::into_inner);
                text.push_str(line.trim_end());
                text.push('\n');
                let len = text.chars().count();
                if len > TAIL_MAX {
                    *text = text.chars().skip(len - TAIL_MAX).collect();
                }
            }
            out.finished
                .store(true, std::sync::atomic::Ordering::SeqCst);
            out.done.notify_waiters();
        });
        tail
    }

    /// Końcówka po zakończeniu potoku (najdłużej `wait`), z zamaskowanymi wartościami `secrets`.
    pub async fn text(&self, wait: std::time::Duration, secrets: &[&str]) -> String {
        if !self.finished.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = tokio::time::timeout(wait, self.done.notified()).await;
        }
        let mut text = self
            .text
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .trim()
            .to_owned();
        for s in secrets.iter().filter(|s| s.len() >= 4) {
            text = text.replace(s, "***");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_tail_and_masks_values() {
        let input = format!(
            "{}\nnpm ERR! 404 tajne-123 nie znaleziono\n",
            "x".repeat(3_000)
        );
        let tail = StderrTail::collect(std::io::Cursor::new(input.into_bytes()));
        let text = tail
            .text(std::time::Duration::from_secs(2), &["tajne-123"])
            .await;
        assert!(text.ends_with("npm ERR! 404 *** nie znaleziono"), "{text}");
        assert!(text.chars().count() <= TAIL_MAX);
    }
}
