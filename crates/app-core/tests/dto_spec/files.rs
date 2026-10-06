//! Specyfikacje komend załączników (`attachments_*`), eksportu rozmowy
//! (`sessions_export_conversation`) i kopii zapasowych (`backups_*`) dla `dto_roundtrip.rs`.

use app_core::dto::*;

use crate::{Check, parse_only, roundtrip};

use super::Spec;

/// Specyfikacja komendy plików (`None` — komenda spoza tej części).
pub fn spec(command: &str) -> Option<Spec> {
    let s: Check = roundtrip::<String>;
    let added: Check = roundtrip::<AttachmentsAdded>;
    let view: Check = roundtrip::<BackupView>;
    Some(match command {
        "attachments_pick" | "attachments_add_dropped" | "attachments_paste" => {
            (vec![("sessionId", s)], added)
        }
        "attachments_list" => (vec![("sessionId", s)], roundtrip::<Vec<AttachmentInfo>>),
        "attachments_remove" => (
            vec![("sessionId", s), ("attachmentId", s)],
            roundtrip::<Vec<AttachmentInfo>>,
        ),
        "sessions_export_conversation" => (
            vec![
                ("sessionId", s),
                ("format", roundtrip::<ConversationFormat>),
                ("turnId", roundtrip::<Option<String>>),
            ],
            roundtrip::<ExportResult>,
        ),
        "backups_status" | "backups_choose_dir" | "backups_run_now" => (vec![], view),
        "backups_configure" => (vec![("config", roundtrip::<BackupConfig>)], view),
        "backups_set_password" => (vec![("password", parse_only::<Option<SecretInput>>)], view),
        "backups_verify" => (vec![("file", s)], roundtrip::<BackupCheck>),
        "backups_restore" => (vec![("file", s)], s),
        _ => return None,
    })
}
