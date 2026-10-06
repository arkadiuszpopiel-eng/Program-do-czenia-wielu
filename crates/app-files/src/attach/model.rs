//! Załączniki tury dla modelu i dla UI. Dla modelu: obraz (≤ limit) jako blok `Image` base64,
//! tekst jako **treść niezaufana** w delimitacji `tools-common` (ucięty do limitu), reszta —
//! tylko nazwa, typ i rozmiar. Treść czytana z artefaktu sesji i sprawdzana SHA-256 z chwili
//! wysłania — plik zmieniony później nie podmienia historii (append-only), model dostaje notkę.

use std::path::Path;

use app_api::dto::{AttachmentDelivery, AttachmentKind, TurnAttachment};
use artifacts_contract::{ArtifactId, Artifacts};
use base64::Engine as _;
use providers_contract::{ContentBlock, ImageSource};
use sessions_contract::{AttachmentRef, Block, SessionId};
use tools_common_contract::text::{truncate_chars, wrap_untrusted};

use super::limits::{AttachmentLimits, classify};

/// Rozmiar czytelny dla człowieka (PL: przecinek dziesiętny).
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    let text = if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / KB / KB)
    };
    text.replace('.', ",")
}

fn kind_of(mime: &str) -> AttachmentKind {
    if mime.starts_with("image/") && mime != "image/svg+xml" {
        AttachmentKind::Image
    } else if mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/toml"
                | "application/xml"
                | "application/yaml"
                | "image/svg+xml"
        )
    {
        AttachmentKind::Text
    } else {
        AttachmentKind::Document
    }
}

/// Załączniki tury do DTO (bez we/wy).
pub fn turn_attachments(blocks: &[Block]) -> Vec<TurnAttachment> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::Attachment { attachment: a } => Some(TurnAttachment {
                name: a.name.clone(),
                mime: a.mime.clone(),
                kind: kind_of(&a.mime),
                bytes: a.bytes,
                artifact_id: a.artifact_id.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn label(a: &AttachmentRef) -> String {
    let size = a.bytes.map(human_size).unwrap_or_else(|| "?".into());
    format!("[Załącznik: „{}” ({}, {size})]", a.name, a.mime)
}

/// Treść pliku artefaktu, jeśli zgadza się z hashem z chwili wysłania.
fn content(
    artifacts: &dyn Artifacts,
    session: &SessionId,
    a: &AttachmentRef,
    max: u64,
) -> Result<Vec<u8>, &'static str> {
    let id = a.artifact_id.as_ref().ok_or("brak artefaktu")?;
    let artifact = artifacts
        .get(session, &ArtifactId(id.clone()))
        .map_err(|_| "artefakt niedostępny")?;
    // Wersja z chwili wysłania (ten sam hash), a bez hasha — najnowsza.
    let path = artifact
        .versions
        .iter()
        .rev()
        .find(|v| a.sha256.as_ref().is_none_or(|sha| &v.sha256 == sha))
        .or_else(|| artifact.latest())
        .map(|v| v.path.clone())
        .ok_or("artefakt bez wersji")?;
    read_capped(&path, max)
        .ok_or("plik niedostępny")
        .and_then(|bytes| match &a.sha256 {
            Some(sha) if artifacts_contract::sha256_hex(&bytes) != *sha => {
                Err("plik zmienił się od wysłania")
            }
            _ => Ok(bytes),
        })
}

fn read_capped(path: &Path, max: u64) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    (meta.is_file() && meta.len() <= max)
        .then(|| std::fs::read(path).ok())
        .flatten()
}

/// Bloki dla dostawcy z załączników tury (kolejność jak w turze; bloki inne niż `Attachment`
/// są pomijane — tekst tury dokłada wywołujący).
pub fn provider_blocks(
    artifacts: &dyn Artifacts,
    session: &SessionId,
    blocks: &[Block],
    limits: &AttachmentLimits,
) -> Vec<ContentBlock> {
    let mut out = Vec::new();
    for block in blocks {
        let Block::Attachment { attachment: a } = block else {
            continue;
        };
        let head = label(a);
        let bytes = match content(artifacts, session, a, limits.max_file_bytes) {
            Ok(bytes) => bytes,
            Err(why) => {
                out.push(ContentBlock::text(format!(
                    "{head} — treść niedostępna: {why}."
                )));
                continue;
            }
        };
        let sniff = &bytes[..bytes.len().min(artifacts_contract::BINARY_SNIFF_BYTES)];
        let text = (!artifacts_contract::looks_binary(sniff))
            .then(|| String::from_utf8_lossy(&bytes).into_owned());
        let c = classify(
            &a.mime,
            sniff,
            bytes.len() as u64,
            text.as_ref().map(|t| t.chars().count()),
            limits,
        );
        match (c.kind, c.delivery, text) {
            (AttachmentKind::Image, AttachmentDelivery::Full, _) => {
                out.push(ContentBlock::text(head));
                out.push(ContentBlock::Image {
                    source: ImageSource::Base64 {
                        media_type: a.mime.clone(),
                        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                    },
                });
            }
            (AttachmentKind::Text, _, Some(text)) => {
                let (body, _) = truncate_chars(&text, limits.max_text_chars);
                out.push(ContentBlock::text(format!(
                    "{head}\n{}",
                    wrap_untrusted(&body, "załącznik", &a.name)
                )));
            }
            _ => out.push(ContentBlock::text(format!(
                "{head} — plik binarny albo za duży; treść niedostępna dla modelu (kopia w katalogu \
                 sesji: {}/{}).",
                super::INBOX,
                a.name
            ))),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_polish() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1,5 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5,0 MB");
    }

    #[test]
    fn turn_dto_lists_only_attachments() {
        let blocks = vec![
            Block::Text { text: "x".into() },
            Block::Attachment {
                attachment: AttachmentRef {
                    name: "a.png".into(),
                    mime: "image/png".into(),
                    artifact_id: Some("1".into()),
                    sha256: None,
                    bytes: Some(10),
                },
            },
            Block::Attachment {
                attachment: AttachmentRef {
                    name: "b.svg".into(),
                    mime: "image/svg+xml".into(),
                    artifact_id: None,
                    sha256: None,
                    bytes: None,
                },
            },
        ];
        let list = turn_attachments(&blocks);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].kind, AttachmentKind::Image);
        assert_eq!(list[1].kind, AttachmentKind::Text);
    }
}
