//! Czytanie linii z limitem długości: linia dłuższa niż limit jest pomijana w całości
//! (bez alokacji ponad limit), a czytelnik wraca do następnej linii. Ta sama logika co
//! `mcp-impl::lines` — moduły nie zależą od cudzych `-impl`.

use tokio::io::{AsyncBufRead, AsyncBufReadExt};

/// Domyślny limit linii strumienia CLI (4 MiB).
pub const DEFAULT_MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// Wynik odczytu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// Pełna linia (bez `\n` i `\r`), UTF-8 z zastąpieniem błędnych bajtów.
    Text(String),
    /// Linia przekroczyła limit; podano jej długość w bajtach.
    TooLong(usize),
}

/// Czyta następną linię; `Ok(None)` na końcu strumienia (niepełna ostatnia linia jest zwracana).
pub async fn read_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> std::io::Result<Option<Line>> {
    let mut buf: Vec<u8> = Vec::new();
    let mut total = 0usize;
    let mut overflow = false;
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            if total == 0 {
                return Ok(None);
            }
            break;
        }
        let (take, done) = match chunk.iter().position(|b| *b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        let content = if done {
            &chunk[..take - 1]
        } else {
            &chunk[..take]
        };
        total += content.len();
        if !overflow {
            if total > max {
                overflow = true;
                buf = Vec::new();
            } else {
                buf.extend_from_slice(content);
            }
        }
        reader.consume(take);
        if done {
            break;
        }
    }
    if overflow {
        return Ok(Some(Line::TooLong(total)));
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    Ok(Some(Line::Text(String::from_utf8_lossy(&buf).into_owned())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn reads_lines_and_skips_overlong() {
        let data = format!("a\r\n{}\nb\nc", "x".repeat(100));
        let mut r = BufReader::with_capacity(8, data.as_bytes());
        assert_eq!(
            read_line(&mut r, 10).await.unwrap(),
            Some(Line::Text("a".into()))
        );
        assert_eq!(
            read_line(&mut r, 10).await.unwrap(),
            Some(Line::TooLong(100))
        );
        assert_eq!(
            read_line(&mut r, 10).await.unwrap(),
            Some(Line::Text("b".into()))
        );
        assert_eq!(
            read_line(&mut r, 10).await.unwrap(),
            Some(Line::Text("c".into()))
        );
        assert_eq!(read_line(&mut r, 10).await.unwrap(), None);
        let mut bad = BufReader::new(&b"\xff\n"[..]);
        assert_eq!(
            read_line(&mut bad, 10).await.unwrap(),
            Some(Line::Text("\u{fffd}".into()))
        );
    }
}
