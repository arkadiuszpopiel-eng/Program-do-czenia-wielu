// Wtyczki Wasm (komendy `plugins_*`, Ustawienia → „Wtyczki”) — kształt 1:1
// z `crates/app-api/src/dto/plugins.rs` (pola snake_case). Wersje jako tekst semver.

export type {
  PluginCapabilityView,
  PluginInfo,
  PluginInspection,
  PluginLimitsView,
  PluginOrigin,
  PluginProblem,
  PluginProblemKind,
  PluginR2View,
  PluginStateView,
  PluginToolView,
  PluginsView,
} from './generated/ipc';
