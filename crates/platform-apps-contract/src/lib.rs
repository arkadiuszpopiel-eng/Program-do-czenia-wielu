//! Porty aplikacji i systemu dla computer use (F6; docs/modules/platform-apps/SPEC.md, PLAN §7.1
//! „API/CLI/COM > UIA > wizja > wejście”, §7.2, §8.5).
//!
//! - [`OfficePort`] — Word/Excel przez COM: odczyt tekstu, tabel i zakresów, edycja **kopii
//!   roboczej** (nowe bajty, oryginał nietknięty), makra zawsze wyłączone, Protected View dla
//!   plików z Internetu, formuły z listy dozwolonej;
//! - [`BrowserPort`] — izolowana przeglądarka z CDP przez potok: osobny profil Alfy, filtr egressu
//!   per żądanie, bez haseł i autouzupełniania, pobrania w kwarantannie;
//! - [`RegistryPort`] — rejestr `HKCU`/`HKLM` tylko do odczytu z deny-listą kluczy z sekretami;
//! - [`SysPort`] — procesy (zakończenie z tożsamością, strażnik celów), usługi, Dziennik zdarzeń,
//!   zmienne środowiskowe (sekrety ukryte, zapis tylko zmiennych użytkownika) dla `tools-system`;
//! - [`DownloadStore`] — kwarantanna pobrań `tools-net` (nowy plik, bez dowiązań, MOTW).
//!
//! Implementacja Windows: `platform-windows-office-impl` (Office, przeglądarka, rejestr)
//! i `platform-windows-sys-impl` (`SysPort`, `DownloadStore`); atrapa: `platform-apps-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod browser;
mod cells;
mod edit;
mod office;
mod registry;
// F6 `tools-system` / `tools-net`: port systemu, polityki, kwarantanna pobrań.
mod downloads;
mod sys;
mod sys_policy;

pub use browser::{
    BROWSER_CALL_TIMEOUT_MS, BrowserError, BrowserKind, BrowserPort, BrowserSessionId, BrowserSpec,
    DownloadInfo, EgressFilter, MAX_TYPE_CHARS, MAX_URL_LEN, PageInfo, PageNode, PageSnapshot,
    check_navigation_url, chromium_args, is_local_url, is_user_browser_profile,
    profile_preferences, request_allowed, url_host,
};
pub use cells::{
    cell_name, check_formula, check_sheet_name, is_allowed_function, parse_cell, parse_range,
    safe_cell_text,
};
pub use downloads::{
    DownloadError, DownloadSink, DownloadStore, FALLBACK_NAME, MAX_DOWNLOAD_NAME,
    disposition_file_name, is_executable_name, numbered_name, sanitize_file_name, zone_identifier,
};
pub use edit::{CellInput, OfficeEdit, OfficeEdited, TextPosition, check_edits};
pub use office::{
    AUTOMATION_SECURITY_FORCE_DISABLE, CellValue, FileZone, MAX_EDIT_CHARS, MAX_EDITS,
    MAX_FORMULA_CHARS, MAX_OFFICE_BYTES, MAX_RANGE_CELLS, OFFICE_CALL_TIMEOUT_MS, OfficeApp,
    OfficeContent, OfficeError, OfficeFile, OfficePort, OfficeQuery, OfficeSession, SheetInfo,
    app_for_file, check_session,
};
pub use registry::{
    MAX_REG_BINARY_PREVIEW, MAX_REG_DATA_CHARS, MAX_REG_ENTRIES, RegData, RegHive, RegKey,
    RegListing, RegValue, RegistryError, RegistryPort, check_key, guard_listing, is_secret_segment,
    is_secret_value_name,
};
pub use sys::{
    EnvScope, EnvVar, EventLevel, EventLogName, EventQuery, EventRecord, MAX_EVENT_MESSAGE_CHARS,
    MAX_EVENTS, MAX_SERVICE_WAIT_MS, ProcessDetails, ProcessEntry, ProcessIdentity, ServiceCommand,
    ServiceEntry, ServiceState, SysError, SysPort, is_protected_entry,
};
pub use sys_policy::{
    CRITICAL_PROCESSES, CRITICAL_SERVICES, MAX_ENV_NAME, MAX_ENV_VALUE, check_env_name,
    check_env_value, check_provider, check_service_name, clip_chars, env_write_denied, event_xpath,
    guard_env, is_critical_process, is_critical_service, is_secret_env_name, protected_process,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_browser;
#[cfg(test)]
mod tests_sys;
