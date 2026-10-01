//! Zdolności (PLAN §8.1): `fs.read(zakres)`, `fs.write(zakres)`, `shell.exec(zakres)`,
//! `gui.control(aplikacja)`, `net.egress(host)`, `secrets.read(id)`, `system.admin(op)`.

use std::fmt;

use risk_classifier_contract::ActionClass;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::scope::{AdminOp, AppSelector, HostPattern, PathScope, SecretId};

/// Zdolność z zakresem. Relacja „podzbiór” tylko w obrębie tej samej rodziny (zapis nie
/// implikuje odczytu, shell nie implikuje zapisu) — reguły mogą wyłącznie zawężać.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "cap", content = "scope")]
pub enum Capability {
    /// `fs.read(zakres)`.
    #[serde(rename = "fs.read")]
    FsRead(PathScope),
    /// `fs.write(zakres)` — zapis, przeniesienie, usunięcie.
    #[serde(rename = "fs.write")]
    FsWrite(PathScope),
    /// `shell.exec(zakres)` — katalog roboczy i zasięg skutków.
    #[serde(rename = "shell.exec")]
    ShellExec(PathScope),
    /// `gui.control(aplikacja)`.
    #[serde(rename = "gui.control")]
    GuiControl(AppSelector),
    /// `net.egress(host)`.
    #[serde(rename = "net.egress")]
    NetEgress(HostPattern),
    /// `secrets.read(id)`.
    #[serde(rename = "secrets.read")]
    SecretsRead(SecretId),
    /// `system.admin(op)`.
    #[serde(rename = "system.admin")]
    SystemAdmin(AdminOp),
}

impl Capability {
    /// Nazwa rodziny (`fs.read`, …).
    pub fn family(&self) -> &'static str {
        match self {
            Self::FsRead(_) => "fs.read",
            Self::FsWrite(_) => "fs.write",
            Self::ShellExec(_) => "shell.exec",
            Self::GuiControl(_) => "gui.control",
            Self::NetEgress(_) => "net.egress",
            Self::SecretsRead(_) => "secrets.read",
            Self::SystemAdmin(_) => "system.admin",
        }
    }

    /// Klasa akcji dla klasyfikatora ryzyka.
    pub fn action_class(&self) -> ActionClass {
        match self {
            Self::FsRead(_) => ActionClass::Read,
            Self::FsWrite(_) => ActionClass::Write,
            Self::ShellExec(_) => ActionClass::Shell,
            Self::GuiControl(_) => ActionClass::GuiControl,
            Self::NetEgress(_) => ActionClass::Egress,
            Self::SecretsRead(_) => ActionClass::SecretsRead,
            Self::SystemAdmin(_) => ActionClass::Admin,
        }
    }

    /// Zakres ścieżki (dla `fs.*` i `shell.exec`).
    pub fn path_scope(&self) -> Option<&PathScope> {
        match self {
            Self::FsRead(s) | Self::FsWrite(s) | Self::ShellExec(s) => Some(s),
            _ => None,
        }
    }

    /// Atenuacja: czy `self` (potomek) mieści się w `parent`.
    pub fn is_subset_of(&self, parent: &Capability) -> bool {
        match (self, parent) {
            (Self::FsRead(c), Self::FsRead(p))
            | (Self::FsWrite(c), Self::FsWrite(p))
            | (Self::ShellExec(c), Self::ShellExec(p)) => c.is_subset_of(p),
            (Self::GuiControl(c), Self::GuiControl(p)) => c == p,
            (Self::NetEgress(c), Self::NetEgress(p)) => c.is_subset_of(p),
            (Self::SecretsRead(c), Self::SecretsRead(p)) => c == p,
            (Self::SystemAdmin(c), Self::SystemAdmin(p)) => c == p,
            _ => false,
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scope = match self {
            Self::FsRead(s) | Self::FsWrite(s) | Self::ShellExec(s) => s.to_string(),
            Self::GuiControl(a) => a.to_string(),
            Self::NetEgress(h) => h.to_string(),
            Self::SecretsRead(id) => id.as_str().to_owned(),
            Self::SystemAdmin(op) => format!("{op:?}"),
        };
        write!(f, "{}({scope})", self.family())
    }
}
