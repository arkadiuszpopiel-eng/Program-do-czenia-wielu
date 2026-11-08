//! Szyfrowanie paczki: round-trip z losowym dostępem, wykrywanie zmian i obcięcia, złe hasło,
//! granice parametrów Argon2id z nagłówka obcej paczki.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Cursor, Read, Seek, SeekFrom};

use accounts_hub_contract::SecretString;
use proptest::prelude::*;
use transfer_contract::{PackageKind, TransferError};
use transfer_impl::KdfParams;
use transfer_impl::crypto::{CHUNK, DecryptingReader, Kdf, Sealer, key_for, read_header};

const FAST: KdfParams = KdfParams {
    m_kib: 64,
    t: 1,
    p: 1,
};

fn seal(data: &[u8], password: &str) -> Vec<u8> {
    let sealer =
        Sealer::with_password(PackageKind::Export, &SecretString::from(password), FAST).unwrap();
    let mut out = Vec::new();
    sealer.seal(&mut Cursor::new(data), &mut out).unwrap();
    out
}

fn open(
    bytes: Vec<u8>,
    password: &str,
) -> Result<DecryptingReader<Cursor<Vec<u8>>>, TransferError> {
    let mut cur = Cursor::new(bytes);
    let (header, aad) =
        read_header(&mut cur)?.ok_or_else(|| TransferError::corrupt("brak nagłówka"))?;
    let pwd = SecretString::from(password);
    let key = key_for(&header, Some(&pwd), &|_| None)?;
    DecryptingReader::open(cur, &header, aad, &key)
}

#[test]
fn round_trip_all_boundary_sizes() {
    for len in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK + 5] {
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let sealed = seal(&data, "hasło testowe 123");
        assert!(sealed.starts_with(b"ALFAENC1"));
        let mut r = open(sealed, "hasło testowe 123").unwrap();
        assert_eq!(r.plain_len(), len as u64);
        let mut back = Vec::new();
        r.read_to_end(&mut back).unwrap();
        assert_eq!(back, data, "rozmiar {len}");
    }
}

#[test]
fn wrong_password_tamper_and_truncation() {
    let data = vec![7u8; 2 * CHUNK + 100];
    let sealed = seal(&data, "hasło testowe 123");
    assert_eq!(
        open(sealed.clone(), "inne hasło 999").err(),
        Some(TransferError::WrongPassword)
    );
    let read_all = |bytes: Vec<u8>| -> Result<Vec<u8>, String> {
        let mut r = open(bytes, "hasło testowe 123").map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        r.read_to_end(&mut out).map_err(|e| e.to_string())?;
        Ok(out)
    };
    assert_eq!(read_all(sealed.clone()).unwrap(), data);
    let mut tampered = sealed.clone();
    let n = tampered.len();
    tampered[n - 40] ^= 1;
    assert!(read_all(tampered).is_err());
    // Obcięcie dokładnie na granicy fragmentu: przedostatni fragment nie ma flagi „ostatni”.
    let header_len = sealed.len() - (2 * (CHUNK + 16) + 100 + 16);
    let cut = sealed[..header_len + 2 * (CHUNK + 16)].to_vec();
    assert!(read_all(cut).is_err());
    let mut header = sealed.clone();
    header[15] ^= 1;
    assert!(read_all(header).is_err());
    assert!(open(b"ALFAENC1\x00\x00".to_vec(), "x").is_err());
}

#[test]
fn foreign_header_limits_and_password_requirements() {
    let sealed = seal(b"abc", "hasło testowe 123");
    let mut cur = Cursor::new(sealed);
    let (mut header, _) = read_header(&mut cur).unwrap().unwrap();
    assert!(matches!(
        key_for(&header, None, &|_| None),
        Err(TransferError::PasswordRequired)
    ));
    if let Kdf::Argon2id { params, .. } = &mut header.kdf {
        params.m_kib = 4 << 20;
    }
    let pwd = SecretString::from("hasło testowe 123");
    assert!(matches!(
        key_for(&header, Some(&pwd), &|_| None),
        Err(TransferError::LimitExceeded { .. })
    ));
    header.kdf = Kdf::MachineKey {
        key: "transfer/snapshot-key".into(),
    };
    assert!(matches!(
        key_for(&header, None, &|_| None),
        Err(TransferError::NotFound { .. })
    ));
    let mut plain = Cursor::new(b"PK\x03\x04zip".to_vec());
    assert!(read_header(&mut plain).unwrap().is_none());
    assert_eq!(plain.position(), 0);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, failure_persistence: None, ..ProptestConfig::default() })]

    /// Losowy dostęp (`Seek`) daje te same bajty co odczyt sekwencyjny.
    #[test]
    fn random_access_matches(len in 0usize..(3 * CHUNK), reads in proptest::collection::vec((any::<u32>(), 1usize..5000), 1..12)) {
        let data: Vec<u8> = (0..len).map(|i| (i * 31 % 256) as u8).collect();
        let mut r = open(seal(&data, "hasło testowe 123"), "hasło testowe 123").unwrap();
        for (pos, n) in reads {
            let pos = if len == 0 { 0 } else { pos as usize % len };
            r.seek(SeekFrom::Start(pos as u64)).unwrap();
            let mut buf = vec![0u8; n];
            let mut got = 0;
            while got < n {
                let k = r.read(&mut buf[got..]).unwrap();
                if k == 0 { break; }
                got += k;
            }
            let end = (pos + n).min(len);
            prop_assert_eq!(&buf[..got], &data[pos..end]);
        }
    }
}
