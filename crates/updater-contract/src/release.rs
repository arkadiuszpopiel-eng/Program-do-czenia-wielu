//! Manifest wydań z własnego repo: wersja, SHA-256 paczki, podpis minisign (komentarz zaufany
//! wiąże wersję), notatki „Co nowego”, `min_previous`.

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::UpdaterError;

/// Wersja schematu manifestu wydań.
pub const RELEASES_SCHEMA: u32 = 1;

/// Limit notatek „Co nowego” w manifeście.
pub const MAX_NOTES_BYTES: usize = 64 * 1024;

/// Jedno wydanie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Release {
    /// Wersja.
    #[schemars(with = "String")]
    pub version: Version,
    /// Adres paczki: `https://…` albo względny wobec adresu manifestu (`alfa-1.2.0-x64.zip`).
    pub url: String,
    /// SHA-256 paczki (hex).
    pub sha256: String,
    /// Podpis minisign paczki (pełny tekst `.minisig`); komentarz zaufany musi zawierać
    /// `version:<wersja>` (wiązanie podpisu z wersją — ochrona przed podsunięciem starszej paczki).
    pub minisign: String,
    /// „Co nowego” (Markdown; renderowany w Rust jak każdy Markdown).
    pub notes: String,
    /// Najstarsza wersja, z której można przejść wprost na to wydanie.
    #[schemars(with = "Option<String>")]
    pub min_previous: Option<Version>,
}

impl Release {
    /// Walidacja pól (adres `https://` albo bezpieczny względny, skrót 64 hex, niepusty podpis,
    /// notatki ≤ 64 KiB).
    pub fn validate(&self) -> Result<(), UpdaterError> {
        let url_ok = if self.url.contains("://") {
            crate::feed::is_allowed_url(&self.url, false)
        } else {
            crate::feed::is_safe_relative(&self.url)
        };
        if !url_ok {
            return Err(UpdaterError::invalid(format!(
                "adres wydania {} nie jest https",
                self.version
            )));
        }
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(UpdaterError::invalid(format!(
                "zły skrót wydania {}",
                self.version
            )));
        }
        if self.notes.len() > MAX_NOTES_BYTES {
            return Err(UpdaterError::invalid(format!(
                "za długie notatki wydania {}",
                self.version
            )));
        }
        if self.minisign.trim().is_empty() {
            return Err(UpdaterError::SignatureInvalid {
                reason: "brak podpisu".to_owned(),
            });
        }
        Ok(())
    }
}

/// Manifest wydań kanału.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReleaseManifest {
    /// Wersja schematu.
    pub schema: u32,
    /// Kanał (`stable`).
    pub channel: String,
    /// Wydania.
    pub releases: Vec<Release>,
}

/// Najnowsze wydanie nowsze od `current`, osiągalne wprost (`min_previous`) i niewycofane.
pub fn select_update<'a>(
    manifest: &'a ReleaseManifest,
    current: &Version,
    bad: &[Version],
) -> Option<&'a Release> {
    manifest
        .releases
        .iter()
        .filter(|r| r.version > *current && !bad.contains(&r.version))
        .filter(|r| r.min_previous.as_ref().is_none_or(|min| current >= min))
        .filter(|r| r.validate().is_ok())
        .max_by(|a, b| a.version.cmp(&b.version))
}

/// Znacznik wersji w komentarzu zaufanym podpisu.
pub fn version_tag(version: &Version) -> String {
    format!("version:{version}")
}

/// Czy komentarz zaufany wiąże podpis z wersją (token `version:<wersja>` rozdzielony białymi znakami).
pub fn comment_binds_version(trusted_comment: &str, version: &Version) -> bool {
    let tag = version_tag(version);
    trusted_comment.split_whitespace().any(|t| t == tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(v: &str, min: Option<&str>) -> Release {
        Release {
            version: Version::parse(v).unwrap_or(Version::new(0, 0, 0)),
            url: format!("https://repo.example/alfa-{v}.zip"),
            sha256: "a".repeat(64),
            minisign: "podpis".into(),
            notes: "Co nowego".into(),
            min_previous: min.and_then(|m| Version::parse(m).ok()),
        }
    }

    #[test]
    fn selects_newest_reachable_release() {
        let m = ReleaseManifest {
            schema: RELEASES_SCHEMA,
            channel: "stable".into(),
            releases: vec![
                rel("1.1.0", None),
                rel("1.3.0", Some("1.2.0")),
                rel("1.2.0", None),
                rel("0.9.0", None),
            ],
        };
        let cur = Version::new(1, 0, 0);
        assert_eq!(
            select_update(&m, &cur, &[]).map(|r| r.version.to_string()),
            Some("1.2.0".into())
        );
        let cur = Version::new(1, 2, 0);
        assert_eq!(
            select_update(&m, &cur, &[]).map(|r| r.version.to_string()),
            Some("1.3.0".into())
        );
        assert!(select_update(&m, &cur, &[Version::new(1, 3, 0)]).is_none());
        assert!(select_update(&m, &Version::new(2, 0, 0), &[]).is_none());
        let mut insecure = rel("5.0.0", None);
        insecure.url = "http://x".into();
        assert!(insecure.validate().is_err());
        insecure.url = "../alfa.zip".into();
        assert!(insecure.validate().is_err());
        insecure.url = "alfa-5.0.0-x64.zip".into();
        assert!(insecure.validate().is_ok());
        insecure.notes = "x".repeat(MAX_NOTES_BYTES + 1);
        assert!(insecure.validate().is_err());
    }

    #[test]
    fn version_binding_in_trusted_comment() {
        let v = Version::new(1, 2, 3);
        assert!(comment_binds_version(
            "timestamp:1\tfile:alfa.zip\tversion:1.2.3",
            &v
        ));
        assert!(!comment_binds_version("timestamp:1\tversion:1.2.30", &v));
        assert!(!comment_binds_version("timestamp:1", &v));
    }
}
