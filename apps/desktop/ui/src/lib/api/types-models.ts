// Menedżer modeli i silników (komendy `models_*`, `embed_model_activate`, `search_reindex_*`,
// zdarzenia `ModelProgress`, `ModelChanged`, `ReindexStatus`) — kształt 1:1
// z `crates/app-api/src/dto/models.rs` (pola snake_case). Rozmiary i liczniki w bajtach / sztukach.
export type {
  BundleFit,
  BundleFitKind,
  BundleItemView,
  BundleRequirements,
  BundleState,
  CommercialUse,
  EmbedderView,
  LicenseCard,
  ModelBundle,
  ModelCard,
  ModelFileView,
  ModelItem,
  ModelItemKind,
  ModelItemState,
  ModelProgressView,
  ModelsView,
  QualityNote,
  ReindexView,
  VoiceLicense,
} from './generated/ipc';

/** Zgoda TOFU: nazwa pliku → SHA-256 pokazany na karcie. */
export type TrustedHashes = Readonly<Record<string, string>>;
