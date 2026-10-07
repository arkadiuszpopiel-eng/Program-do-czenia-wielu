//! Weryfikacja paczki wydania: SHA-256 i podpis minisign (strumieniowo, jeden przebieg),
//! wiązanie podpisu z wersją przez komentarz zaufany (`version:<wersja>`).

use std::io::Read;
use std::path::Path;

use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};
use updater_contract::{Release, UpdaterError, comment_binds_version};

fn invalid(reason: impl std::fmt::Display) -> UpdaterError {
    UpdaterError::SignatureInvalid {
        reason: reason.to_string(),
    }
}

/// Klucz publiczny: sam base64 (`RW…`) albo pełny plik `.pub` (z komentarzem).
pub fn public_key(text: &str) -> Result<PublicKey, UpdaterError> {
    let text = text.trim();
    let parsed = if text.lines().count() > 1 {
        PublicKey::decode(text)
    } else {
        PublicKey::from_base64(text)
    };
    parsed.map_err(|e| invalid(format!("zły klucz publiczny: {e}")))
}

/// Weryfikuje paczkę `package` względem `release`.
pub fn verify(
    release: &Release,
    package: &Path,
    key: Option<&str>,
    require_version_tag: bool,
) -> Result<(), UpdaterError> {
    release.validate()?;
    let pk = public_key(key.ok_or(UpdaterError::NoPublicKey)?)?;
    let signature = Signature::decode(&release.minisign)
        .map_err(|e| invalid(format!("zły format podpisu: {e}")))?;
    if require_version_tag && !comment_binds_version(signature.trusted_comment(), &release.version)
    {
        return Err(invalid(format!(
            "podpis nie wiąże wersji {}",
            release.version
        )));
    }
    let mut verifier = pk.verify_stream(&signature).map_err(invalid)?;
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(package)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        verifier.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if !hex.eq_ignore_ascii_case(&release.sha256) {
        return Err(UpdaterError::HashMismatch);
    }
    verifier.finalize().map_err(invalid)
}
