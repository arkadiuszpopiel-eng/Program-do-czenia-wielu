//! Odczyt losowy szyfrogramu STREAM (fragment po fragmencie, jeden fragment w pamięci).

use std::io::{self, Read, Seek, SeekFrom};

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use transfer_contract::TransferError;
use zeroize::Zeroizing;

use super::{CHUNK, EncHeader, PREFIX, TAG, nonce, read_full, unhex};

/// Czytnik jawnej treści paczki zaszyfrowanej (`Read + Seek`); każdy fragment jest
/// uwierzytelniany przy pierwszym odczycie.
pub struct DecryptingReader<R> {
    inner: R,
    cipher: XChaCha20Poly1305,
    prefix: [u8; PREFIX],
    aad: Vec<u8>,
    body_len: u64,
    chunks: u64,
    plain_len: u64,
    pos: u64,
    cache: Option<(u64, Zeroizing<Vec<u8>>)>,
}

const BLOCK: u64 = (CHUNK + TAG) as u64;

impl<R: Read + Seek> DecryptingReader<R> {
    /// Otwiera szyfrogram (po nagłówku `aad`) i sprawdza pierwszy fragment (złe hasło → błąd).
    pub fn open(
        mut inner: R,
        header: &EncHeader,
        aad: Vec<u8>,
        key: &[u8; 32],
    ) -> Result<Self, TransferError> {
        let total = inner.seek(SeekFrom::End(0))?;
        let body_len = total.saturating_sub(aad.len() as u64);
        let chunks = body_len.div_ceil(BLOCK);
        let last_len = body_len.saturating_sub(chunks.saturating_sub(1) * BLOCK);
        if chunks == 0 || last_len < TAG as u64 {
            return Err(TransferError::corrupt("obcięty szyfrogram"));
        }
        let prefix: [u8; PREFIX] = unhex(&header.nonce_prefix)
            .and_then(|p| p.try_into().ok())
            .ok_or_else(|| TransferError::corrupt("zły prefiks nonce"))?;
        let mut reader = Self {
            inner,
            cipher: XChaCha20Poly1305::new(key.into()),
            prefix,
            aad,
            body_len,
            chunks,
            plain_len: body_len - chunks * TAG as u64,
            pos: 0,
            cache: None,
        };
        reader.load(0).map_err(|e| match e {
            TransferError::Corrupt { .. } => TransferError::WrongPassword,
            other => other,
        })?;
        Ok(reader)
    }

    /// Długość jawnej treści.
    pub fn plain_len(&self) -> u64 {
        self.plain_len
    }

    fn load(&mut self, index: u64) -> Result<(), TransferError> {
        if self.cache.as_ref().is_some_and(|(i, _)| *i == index) {
            return Ok(());
        }
        let offset = self.aad.len() as u64 + index * BLOCK;
        let len = BLOCK.min(self.body_len - index * BLOCK);
        let mut buf = vec![0u8; usize::try_from(len).unwrap_or(0)];
        self.inner.seek(SeekFrom::Start(offset))?;
        if read_full(&mut self.inner, &mut buf)? < buf.len() {
            return Err(TransferError::corrupt("obcięty szyfrogram"));
        }
        let last = index + 1 == self.chunks;
        let plain = self
            .cipher
            .decrypt(
                &nonce(&self.prefix, index, last)?,
                Payload {
                    msg: &buf,
                    aad: &self.aad,
                },
            )
            .map_err(|_| {
                TransferError::corrupt(format!("uszkodzony szyfrogram (fragment {index})"))
            })?;
        self.cache = Some((index, Zeroizing::new(plain)));
        Ok(())
    }
}

impl<R: Read + Seek> Read for DecryptingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.plain_len || buf.is_empty() {
            return Ok(0);
        }
        let index = self.pos / CHUNK as u64;
        let within = usize::try_from(self.pos % CHUNK as u64).unwrap_or(0);
        self.load(index)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let Some((_, chunk)) = &self.cache else {
            return Ok(0);
        };
        let available = chunk.len().saturating_sub(within);
        let n = available.min(buf.len());
        buf[..n].copy_from_slice(&chunk[within..within + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for DecryptingReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(d) => self.plain_len.checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        let target = target
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "pozycja poza plikiem"))?;
        self.pos = target;
        Ok(target)
    }
}
