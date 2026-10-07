//! Kontener `.alfa` = ZIP (manifest pierwszy, nieskompresowany; reszta deflate), opcjonalnie
//! zaszyfrowany w całości ([`crate::crypto`]). Zapis: wpisy do archiwum pośredniego → archiwum
//! końcowe z manifestem na początku (kopiowanie surowe, bez ponownej kompresji) → szyfrowanie →
//! atomowa zamiana pliku. Odczyt: limity (zip-bomb), reguła ścieżek (zip-slip), zgodność listy
//! wpisów z manifestem, sumy SHA-256 i rozmiary przy każdym odczycie.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use transfer_contract::migrate::upcast_manifest;
use transfer_contract::{
    Limits, MANIFEST_PATH, Manifest, PackageSink, PackageSource, TransferError, UpcastStep, limit,
    sha256_hex, validate_entry_path,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::crypto::{DecryptingReader, EncHeader, Sealer};
use crate::tempfile::TempFile;

fn zip_err(e: zip::result::ZipError) -> TransferError {
    TransferError::corrupt(format!("uszkodzone lub obcięte archiwum: {e}"))
}

fn options(method: CompressionMethod) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(method)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
}

/// Wpisy do archiwum pośredniego.
struct ZipSink {
    zip: ZipWriter<File>,
}

impl PackageSink for ZipSink {
    fn add(&mut self, path: &str, bytes: &[u8]) -> Result<(), TransferError> {
        self.zip
            .start_file(path, options(CompressionMethod::Deflated))
            .map_err(|e| TransferError::io(e.to_string()))?;
        self.zip.write_all(bytes)?;
        Ok(())
    }
}

/// Zapisuje paczkę `dest` atomowo; `build` dodaje wpisy i zwraca manifest. Przy błędzie
/// (także anulowaniu) nie zostaje ani plik docelowy, ani pliki tymczasowe.
pub fn write_package(
    dest: &Path,
    sealer: Option<&Sealer>,
    build: impl FnOnce(&mut dyn PackageSink) -> Result<Manifest, TransferError>,
) -> Result<(Manifest, u64), TransferError> {
    let dir = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    std::fs::create_dir_all(&dir)?;
    let staging = TempFile::new(&dir, dest)?;
    let mut sink = ZipSink {
        zip: ZipWriter::new(staging.create()?),
    };
    let manifest = build(&mut sink)?;
    sink.zip
        .finish()
        .map_err(|e| TransferError::io(e.to_string()))?;

    let plain = TempFile::new(&dir, dest)?;
    let mut zip = ZipWriter::new(plain.create()?);
    zip.start_file(MANIFEST_PATH, options(CompressionMethod::Stored))
        .map_err(|e| TransferError::io(e.to_string()))?;
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| TransferError::invalid(MANIFEST_PATH, e))?;
    zip.write_all(&json)?;
    let mut staged =
        ZipArchive::new(BufReader::new(File::open(staging.path())?)).map_err(zip_err)?;
    for i in 0..staged.len() {
        let file = staged.by_index_raw(i).map_err(zip_err)?;
        zip.raw_copy_file(file)
            .map_err(|e| TransferError::io(e.to_string()))?;
    }
    zip.finish()
        .map_err(|e| TransferError::io(e.to_string()))?
        .sync_all()?;
    drop(staged);
    drop(staging);

    let size = match sealer {
        None => plain.persist(dest)?,
        Some(sealer) => {
            let sealed = TempFile::new(&dir, dest)?;
            let mut out = std::io::BufWriter::new(sealed.create()?);
            let mut input = BufReader::new(File::open(plain.path())?);
            sealer.seal(&mut input, &mut out)?;
            out.into_inner()
                .map_err(|e| TransferError::io(e.to_string()))?
                .sync_all()?;
            drop(plain);
            sealed.persist(dest)?
        }
    };
    Ok((manifest, size))
}

/// Czytnik pliku jawnego albo zaszyfrowanego.
pub enum PackageReader {
    /// ZIP jawny.
    Plain(BufReader<File>),
    /// ZIP w szyfrogramie.
    Encrypted(Box<DecryptingReader<BufReader<File>>>),
}

impl Read for PackageReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            PackageReader::Plain(r) => r.read(buf),
            PackageReader::Encrypted(r) => r.read(buf),
        }
    }
}

impl Seek for PackageReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            PackageReader::Plain(r) => r.seek(pos),
            PackageReader::Encrypted(r) => r.seek(pos),
        }
    }
}

/// Otwarta i zweryfikowana paczka.
pub struct ZipSource {
    archive: ZipArchive<PackageReader>,
    manifest: Manifest,
    migrations: Vec<UpcastStep>,
    index: BTreeMap<String, usize>,
    limits: Limits,
    read_total: u64,
}

const ZIP_MAGIC: &[u8; 4] = b"PK\x03\x04";

/// Dostawca klucza dla nagłówka szyfrowania (hasło albo klucz maszyny).
pub type KeyProvider<'a> =
    dyn Fn(&EncHeader) -> Result<zeroize::Zeroizing<[u8; 32]>, TransferError> + 'a;

/// Otwiera paczkę: nagłówek szyfrowania (klucz przez `key`), archiwum, manifest (z migracją),
/// indeks wpisów z regułą ścieżek i limitami.
pub fn open_package(
    path: &Path,
    limits: &Limits,
    key: &KeyProvider<'_>,
) -> Result<ZipSource, TransferError> {
    let mut file = BufReader::new(File::open(path)?);
    let reader = match crate::crypto::read_header(&mut file)? {
        Some((header, aad)) => {
            let key = key(&header)?;
            let reader = DecryptingReader::open(file, &header, aad, &key)?;
            PackageReader::Encrypted(Box::new(reader))
        }
        None => {
            let mut magic = [0u8; 4];
            let n = file.read(&mut magic)?;
            if n < 4 || &magic != ZIP_MAGIC {
                return Err(TransferError::corrupt("to nie jest paczka .alfa"));
            }
            file.seek(SeekFrom::Start(0))?;
            PackageReader::Plain(file)
        }
    };
    let mut archive = ZipArchive::new(reader).map_err(zip_err)?;
    let entries = archive.len() as u64;
    if entries > limits.max_entries {
        return Err(limit("liczba wpisów", entries, limits.max_entries));
    }
    let manifest_raw = read_manifest(&mut archive, limits)?;
    let (manifest, migrations) = upcast_manifest(manifest_raw)?;
    manifest.validate(limits)?;
    let index = index_entries(&mut archive, &manifest, limits)?;
    Ok(ZipSource {
        archive,
        manifest,
        migrations,
        index,
        limits: *limits,
        read_total: 0,
    })
}

fn read_manifest(
    archive: &mut ZipArchive<PackageReader>,
    limits: &Limits,
) -> Result<serde_json::Value, TransferError> {
    let file = archive.by_index(0).map_err(zip_err)?;
    if file.name() != MANIFEST_PATH {
        return Err(TransferError::corrupt(
            "pierwszym wpisem nie jest manifest.json",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limits.max_manifest_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| TransferError::corrupt(e.to_string()))?;
    if bytes.len() as u64 > limits.max_manifest_bytes {
        return Err(limit(
            MANIFEST_PATH,
            bytes.len() as u64,
            limits.max_manifest_bytes,
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| TransferError::invalid(MANIFEST_PATH, e))
}

/// Indeks wpisów: każda nazwa zgodna z regułą ścieżek, bez duplikatów i dowiązań, zgodna
/// z manifestem (w obie strony), rozsądny stopień kompresji.
fn index_entries(
    archive: &mut ZipArchive<PackageReader>,
    manifest: &Manifest,
    limits: &Limits,
) -> Result<BTreeMap<String, usize>, TransferError> {
    let mut index = BTreeMap::new();
    for i in 1..archive.len() {
        let file = archive.by_index_raw(i).map_err(zip_err)?;
        let name = file.name().to_owned();
        validate_entry_path(&name).map_err(|reason| TransferError::UnsafePath {
            path: name.clone(),
            reason,
        })?;
        if file.is_symlink() || file.is_dir() || file.encrypted() {
            return Err(TransferError::corrupt(format!(
                "niedozwolony rodzaj wpisu `{name}`"
            )));
        }
        let expected = manifest
            .entry(&name)
            .ok_or_else(|| TransferError::corrupt(format!("wpis `{name}` spoza manifestu")))?;
        if file.size() != expected.bytes {
            return Err(TransferError::Checksum { path: name });
        }
        let ratio = file.size() / file.compressed_size().max(1);
        if file.size() > 1 << 20 && ratio > limits.max_ratio {
            return Err(limit(
                &format!("stopień kompresji `{name}`"),
                ratio,
                limits.max_ratio,
            ));
        }
        if index.insert(name.clone(), i).is_some() || name == MANIFEST_PATH {
            return Err(TransferError::corrupt(format!("powtórzony wpis `{name}`")));
        }
    }
    if let Some(missing) = manifest
        .content
        .iter()
        .find(|e| !index.contains_key(&e.path))
    {
        return Err(TransferError::corrupt(format!(
            "brak wpisu `{}` (obcięte archiwum?)",
            missing.path
        )));
    }
    Ok(index)
}

impl ZipSource {
    /// Kopia manifestu (po migracji).
    pub fn manifest_owned(&self) -> Manifest {
        self.manifest.clone()
    }
}

impl PackageSource for ZipSource {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn migrations(&self) -> Vec<UpcastStep> {
        self.migrations.clone()
    }

    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, TransferError> {
        let Some(expected) = self.manifest.entry(path).cloned() else {
            return Ok(None);
        };
        let i = *self
            .index
            .get(path)
            .ok_or_else(|| TransferError::corrupt(format!("brak wpisu `{path}`")))?;
        self.read_total += expected.bytes;
        if self.read_total > self.limits.max_total_bytes.saturating_mul(4) {
            return Err(limit(
                "odczyt paczki",
                self.read_total,
                self.limits.max_total_bytes,
            ));
        }
        let file = self.archive.by_index(i).map_err(zip_err)?;
        let capacity = usize::try_from(expected.bytes.min(64 << 20)).unwrap_or(0);
        let mut bytes = Vec::with_capacity(capacity);
        file.take(expected.bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| TransferError::Checksum {
                path: path.to_owned(),
            })?;
        if bytes.len() as u64 != expected.bytes || sha256_hex(&bytes) != expected.sha256 {
            return Err(TransferError::Checksum {
                path: path.to_owned(),
            });
        }
        Ok(Some(bytes))
    }
}
