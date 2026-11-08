//! Kroki naprawy — każdy ma operację odwrotną (naprawa cofalna). Nigdy nie tracą danych:
//! pliki idą do kwarantanny, wpisy dzienników do archiwum; usuwana bywa wyłącznie kopia
//! identyczna ze źródłem (cofnięcie kopiowania).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Moduły Jądra — każda naprawa ich dotycząca idzie przez Brokera (AGENTS.md, THREAT_MODEL §7).
pub const KERNEL_MODULES: [&str; 10] = [
    "core-bus",
    "core-registry",
    "core-config",
    "core-log",
    "safety-broker",
    "broker-ui",
    "watchdog",
    "updater",
    "compliance",
    "core",
];

/// Prefiksy kluczy, których Diagnosta nie zmienia nigdy (własna autonomia, Ulepszacz,
/// bezpieczeństwo, prywatność, budżety, evals, Jądro i jego procesy, egress, MCP, role, persony,
/// umiejętności, agentki — przegląd #2, SR2-03). `kernel.*` — wyłącznie przez Brokera.
pub const DIAGNOSTICIAN_FORBIDDEN_PREFIXES: [&str; 28] = [
    "diagnostician",
    "improver",
    "security",
    "safety",
    "autonomy",
    "privacy",
    "compliance",
    "budget",
    "budgets",
    "cost",
    "limits",
    "evals",
    "accounts",
    "secrets",
    "core",
    "broker",
    "watchdog",
    "updater",
    "audit",
    "egress",
    "net",
    "permissions",
    "deny",
    "mcp",
    "roles",
    "personas",
    "skills",
    "agents",
];

/// Segmenty klucza poza zasięgiem Diagnosty w dowolnym miejscu klucza (polityki, adresy,
/// polecenia, prompty, sekrety) — przegląd #2, SR2-03. `kernel.*` nie jest tu zakazany, tylko
/// kierowany do Brokera (`is_kernel_key`).
pub const DIAGNOSTICIAN_FORBIDDEN_SEGMENTS: [&str; 25] = [
    "privacy",
    "egress",
    "allowlist",
    "allow_list",
    "denylist",
    "deny_list",
    "autonomy",
    "permission",
    "permissions",
    "capability",
    "capabilities",
    "threshold",
    "thresholds",
    "approval",
    "approvals",
    "audit",
    "broker",
    "trust",
    "base_url",
    "url",
    "endpoint",
    "command",
    "prompt",
    "api_key",
    "token",
];

/// Krok naprawy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum RepairStep {
    /// Zmiana klucza (przez `core-config`); `old` = oczekiwana wartość bieżąca (porównaj-i-zamień).
    SetConfig {
        /// Klucz.
        key: String,
        /// Wartość przed.
        old: Option<Value>,
        /// Wartość po (`None` = usuń nadpisanie).
        new: Option<Value>,
    },
    /// Przywrócenie rewizji konfiguracji (historia `core-config` / watchdog).
    RollbackConfig {
        /// Rewizja bieżąca.
        from_revision: String,
        /// Rewizja docelowa.
        to_revision: String,
    },
    /// Przeniesienie pliku/katalogu (kwarantanna i powrót).
    MoveFile {
        /// Skąd.
        from: String,
        /// Dokąd.
        to: String,
    },
    /// Kopia (np. z kopii zapasowej); cel nie może istnieć.
    CopyFile {
        /// Źródło.
        from: String,
        /// Cel.
        to: String,
    },
    /// Usunięcie kopii — tylko gdy jej treść jest identyczna ze źródłem (bez utraty danych);
    /// operacja odwrotna do [`RepairStep::CopyFile`].
    RemoveCopy {
        /// Kopia.
        path: String,
        /// Źródło (zostaje).
        source: String,
    },
    /// Restart modułu (bez zmiany stanu; odwrotność — ten sam krok).
    RestartModule {
        /// Moduł.
        module: String,
    },
    /// Przeniesienie najstarszych wpisów magazynu (dziennik cofania, segmenty logów) do archiwum.
    ArchiveEntries {
        /// Magazyn.
        store: String,
        /// Liczba wpisów.
        entries: u64,
        /// Archiwum.
        archive: String,
    },
    /// Przywrócenie wpisów z archiwum.
    RestoreEntries {
        /// Magazyn.
        store: String,
        /// Liczba wpisów.
        entries: u64,
        /// Archiwum.
        archive: String,
    },
    /// Kolejka pobrania (model) ze sprawdzeniem SHA-256.
    QueueDownload {
        /// Element.
        item: String,
        /// Oczekiwany hash.
        sha256: String,
    },
    /// Anulowanie pobrania.
    CancelDownload {
        /// Element.
        item: String,
        /// Hash.
        sha256: String,
    },
}

fn show(v: &Option<Value>) -> String {
    v.as_ref()
        .map_or_else(|| "(brak)".to_owned(), Value::to_string)
}

impl RepairStep {
    /// Operacja odwrotna.
    pub fn inverse(&self) -> RepairStep {
        match self {
            Self::SetConfig { key, old, new } => Self::SetConfig {
                key: key.clone(),
                old: new.clone(),
                new: old.clone(),
            },
            Self::RollbackConfig {
                from_revision,
                to_revision,
            } => Self::RollbackConfig {
                from_revision: to_revision.clone(),
                to_revision: from_revision.clone(),
            },
            Self::MoveFile { from, to } => Self::MoveFile {
                from: to.clone(),
                to: from.clone(),
            },
            Self::CopyFile { from, to } => Self::RemoveCopy {
                path: to.clone(),
                source: from.clone(),
            },
            Self::RemoveCopy { path, source } => Self::CopyFile {
                from: source.clone(),
                to: path.clone(),
            },
            Self::RestartModule { module } => Self::RestartModule {
                module: module.clone(),
            },
            Self::ArchiveEntries {
                store,
                entries,
                archive,
            } => Self::RestoreEntries {
                store: store.clone(),
                entries: *entries,
                archive: archive.clone(),
            },
            Self::RestoreEntries {
                store,
                entries,
                archive,
            } => Self::ArchiveEntries {
                store: store.clone(),
                entries: *entries,
                archive: archive.clone(),
            },
            Self::QueueDownload { item, sha256 } => Self::CancelDownload {
                item: item.clone(),
                sha256: sha256.clone(),
            },
            Self::CancelDownload { item, sha256 } => Self::QueueDownload {
                item: item.clone(),
                sha256: sha256.clone(),
            },
        }
    }

    /// Linia diffu (po polsku).
    pub fn describe(&self) -> String {
        match self {
            Self::SetConfig { key, old, new } => {
                format!("konfiguracja {key}: {} → {}", show(old), show(new))
            }
            Self::RollbackConfig {
                from_revision,
                to_revision,
            } => {
                format!("konfiguracja: rewizja {from_revision} → {to_revision} (ostatnia dobra)")
            }
            Self::MoveFile { from, to } => format!("przenieś {from} → {to}"),
            Self::CopyFile { from, to } => format!("skopiuj {from} → {to}"),
            Self::RemoveCopy { path, source } => {
                format!("usuń kopię {path} (identyczna z {source}, które zostaje)")
            }
            Self::RestartModule { module } => format!("uruchom ponownie moduł {module}"),
            Self::ArchiveEntries {
                store,
                entries,
                archive,
            } => format!("przenieś {entries} najstarszych wpisów {store} → {archive}"),
            Self::RestoreEntries {
                store,
                entries,
                archive,
            } => format!("przywróć {entries} wpisów {archive} → {store}"),
            Self::QueueDownload { item, sha256 } => format!(
                "pobierz ponownie {item} (SHA-256 {})",
                sha256.chars().take(12).collect::<String>()
            ),
            Self::CancelDownload { item, .. } => format!("anuluj pobieranie {item}"),
        }
    }

    /// Czy krok zmienia stan (restart — nie).
    pub fn is_stateful(&self) -> bool {
        !matches!(self, Self::RestartModule { .. })
    }

    /// Klucz konfiguracji, jeśli krok go zmienia.
    pub fn config_key(&self) -> Option<&str> {
        match self {
            Self::SetConfig { key, .. } => Some(key),
            _ => None,
        }
    }
}

/// Czy klucz jest polityką Jądra (`kernel.*`).
pub fn is_kernel_key(key: &str) -> bool {
    key == "kernel" || key.starts_with("kernel.")
}

/// Czy klucz jest poza zasięgiem Diagnosty (także przez Brokera — to nie jest naprawa).
pub fn is_forbidden_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    let first = lower.split('.').next().unwrap_or_default();
    DIAGNOSTICIAN_FORBIDDEN_PREFIXES.contains(&first)
        || lower
            .split('.')
            .any(|s| DIAGNOSTICIAN_FORBIDDEN_SEGMENTS.contains(&s))
}

/// Czy moduł należy do Jądra.
pub fn is_kernel_module(module: &str) -> bool {
    KERNEL_MODULES.contains(&module) || module.starts_with("core-")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn every_step_has_an_inverse_and_double_inverse_is_identity_where_defined() {
        let steps = [
            RepairStep::SetConfig {
                key: "a.b".into(),
                old: Some(json!(1)),
                new: None,
            },
            RepairStep::RollbackConfig {
                from_revision: "r2".into(),
                to_revision: "r1".into(),
            },
            RepairStep::MoveFile {
                from: "a".into(),
                to: "q/a".into(),
            },
            RepairStep::RestartModule {
                module: "voice-stt".into(),
            },
            RepairStep::ArchiveEntries {
                store: "undo".into(),
                entries: 5,
                archive: "arch".into(),
            },
            RepairStep::QueueDownload {
                item: "m".into(),
                sha256: "0".repeat(64),
            },
        ];
        for s in steps {
            assert_eq!(s.inverse().inverse(), s);
            assert!(!s.describe().is_empty());
        }
        let copy = RepairStep::CopyFile {
            from: "b".into(),
            to: "t".into(),
        };
        assert_eq!(copy.inverse().inverse(), copy);
        assert!(matches!(copy.inverse(), RepairStep::RemoveCopy { .. }));
        assert!(is_kernel_key("kernel.egress") && !is_kernel_key("kernels.x"));
        assert!(is_forbidden_key("diagnostician.autonomy") && !is_forbidden_key("router.offline"));
        assert!(
            is_kernel_module("core-config")
                && is_kernel_module("safety-broker")
                && !is_kernel_module("voice-stt")
        );
    }
}
