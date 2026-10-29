// Aktualizacje i „O programie" (komendy `updates_*`, zdarzenie `UpdateStatus`) — kształt 1:1
// z `crates/app-api/src/dto/updates.rs` (pola snake_case). Wersje jako tekst semver.
export type {
  AboutInfo,
  ChangeItem,
  ChangeLogEntry,
  ChangeSeverity,
  LicenseEntry,
  LicenseSource,
  UpdateChannel,
  UpdateMode,
  UpdatePhase,
  UpdateProgress,
  UpdateRelease,
  UpdatesView,
  WhatsNew,
} from './generated/ipc';
