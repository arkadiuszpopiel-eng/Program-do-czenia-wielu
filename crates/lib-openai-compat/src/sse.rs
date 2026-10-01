//! Parser Server-Sent Events (WHATWG HTML §9.2) odporny na podział porcji w dowolnym bajcie.

/// Zdarzenie SSE.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseEvent {
    /// Pole `event:` (brak = `message`).
    pub event: Option<String>,
    /// Połączone linie `data:` (rozdzielone `\n`).
    pub data: String,
}

/// Przyrostowy parser SSE.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    skip_lf: bool,
    event: Option<String>,
    data: String,
    has_data: bool,
}

impl SseParser {
    /// Nowy parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dokłada porcję bajtów; zwraca kompletne zdarzenia.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let mut out = Vec::new();
        for &b in chunk {
            // `\r\n` = jeden koniec linii, także gdy `\r` i `\n` są w różnych porcjach.
            if std::mem::take(&mut self.skip_lf) && b == b'\n' {
                continue;
            }
            match b {
                b'\n' | b'\r' => {
                    self.skip_lf = b == b'\r';
                    let line = std::mem::take(&mut self.buf);
                    if let Some(ev) = self.line(&String::from_utf8_lossy(&line)) {
                        out.push(ev);
                    }
                }
                _ => self.buf.push(b),
            }
        }
        out
    }

    /// Koniec strumienia: niedokończone zdarzenie z danymi jest oddawane (tolerancja dla serwerów
    /// bez pustej linii na końcu).
    pub fn finish(&mut self) -> Option<SseEvent> {
        if !self.buf.is_empty() {
            let line = std::mem::take(&mut self.buf);
            if let Some(ev) = self.line(&String::from_utf8_lossy(&line)) {
                return Some(ev);
            }
        }
        self.dispatch()
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        let event = self.event.take();
        if !self.has_data {
            self.data.clear();
            return None;
        }
        self.has_data = false;
        let mut data = std::mem::take(&mut self.data);
        if data.ends_with('\n') {
            data.pop();
        }
        Some(SseEvent { event, data })
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            return self.dispatch();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
                self.has_data = true;
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(chunks: &[&[u8]]) -> Vec<SseEvent> {
        let mut p = SseParser::new();
        let mut out: Vec<SseEvent> = chunks.iter().flat_map(|c| p.push(c)).collect();
        out.extend(p.finish());
        out
    }

    #[test]
    fn basic_and_multiline() {
        let evs = all(&[b"event: ping\ndata: {}\n\n: komentarz\ndata: a\ndata: b\n\n"]);
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].event.as_deref(), Some("ping"));
        assert_eq!(evs[1].data, "a\nb");
        assert_eq!(evs[1].event, None);
    }

    #[test]
    fn split_anywhere_including_utf8_and_crlf() {
        let text = "event: x\r\ndata: zażółć\r\n\r\ndata: [DONE]\r\n\r\n".as_bytes();
        let whole = all(&[text]);
        for cut in 0..text.len() {
            let (a, b) = text.split_at(cut);
            assert_eq!(all(&[a, b]), whole, "cięcie w {cut}");
        }
        assert_eq!(whole[0].data, "zażółć");
        assert_eq!(whole[1].data, "[DONE]");
    }

    #[test]
    fn bytewise_and_trailing_event_without_blank_line() {
        let text = b"data: 1\n\ndata: 2";
        let chunks: Vec<&[u8]> = text.chunks(1).collect();
        let evs = all(&chunks);
        assert_eq!(
            evs.iter().map(|e| e.data.as_str()).collect::<Vec<_>>(),
            ["1", "2"]
        );
        assert!(
            all(&[b"event: only\n\n"]).is_empty(),
            "zdarzenie bez danych nie jest wysyłane"
        );
        assert_eq!(all(&[b"data\n\n"])[0].data, "");
        assert_eq!(all(&[b"id: 5\nretry: 10\ndata:x\r\r"])[0].data, "x");
    }
}
