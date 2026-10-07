//! DTO załączników composera, eksportu rozmowy (Markdown / HTML) i kopii zapasowych z
//! harmonogramem (odpowiedniki `types-files.ts`; realizuje `app-files`).

use serde::{Deserialize, Serialize};

use super::common::Iso8601;

/// Rodzaj załącznika (podgląd w UI i sposób przekazania modelowi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Text,
    Document,
}

/// Co z treści załącznika trafi do modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentDelivery {
    /// Cała treść (tekst albo obraz).
    Full,
    /// Tekst ucięty do limitu.
    Truncated,
    /// Tylko nazwa, typ i rozmiar (plik binarny, obraz ponad limit).
    MetadataOnly,
}

/// Załącznik przygotowany w composerze (kopia w katalogu sesji `…\in`, jeszcze niewysłany).
/// Podgląd w UI przez protokół zasobów (`path`), nigdy bajty przez IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub bytes: u64,
    pub mime: String,
    pub kind: AttachmentKind,
    pub path: String,
    pub tokens: u64,
    pub delivery: AttachmentDelivery,
}

/// Powód odrzucenia pliku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentRejectReason {
    TooLarge,
    TooMany,
    TotalTooLarge,
    Denied,
    NotAFile,
    Unreadable,
    Empty,
}

/// Odrzucony plik (nazwa bez ścieżki).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRejection {
    pub name: String,
    pub reason: AttachmentRejectReason,
}

/// Wynik dodania plików: przyjęte i odrzucone z powodem; `staged` = wszystkie przygotowane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentsAdded {
    pub added: Vec<AttachmentInfo>,
    pub rejected: Vec<AttachmentRejection>,
    pub staged: Vec<AttachmentInfo>,
}

/// Załącznik wysłanej tury (artefakt sesji).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnAttachment {
    pub name: String,
    pub mime: String,
    pub kind: AttachmentKind,
    pub bytes: Option<u64>,
    pub artifact_id: Option<String>,
}

/// Format eksportu rozmowy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationFormat {
    Markdown,
    Html,
}

/// Ustawienia kopii zapasowych (per maszyna, `%LOCALAPPDATA%\Alfa\state\backup.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub dir: Option<String>,
    pub interval_hours: u32,
    pub keep: u32,
    pub include_artifacts: bool,
    pub include_logs: bool,
    pub skip_on_battery: bool,
}

/// Plik kopii w katalogu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupEntry {
    pub file: String,
    pub path: String,
    pub created_at: Iso8601,
    pub bytes: u64,
}

/// Stan kopii zapasowych.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupView {
    pub config: BackupConfig,
    pub password_set: bool,
    pub entries: Vec<BackupEntry>,
    pub last_run: Option<Iso8601>,
    pub last_error: Option<String>,
    pub next_due: Option<Iso8601>,
    pub running: bool,
}

/// Wynik próby przywrócenia (otwarcie, sumy kontrolne, dry-run — nic nie zapisuje).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCheck {
    pub file: String,
    pub ok: bool,
    pub encrypted: bool,
    pub created_at: Option<Iso8601>,
    pub app_version: Option<String>,
    pub items: u64,
    pub sessions: u64,
    pub message: Option<String>,
}
