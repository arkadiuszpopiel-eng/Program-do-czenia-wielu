//! Przypadki: test szpiegowski sekretów, szyfrowanie, brak eksportu sekretów, sesje prywatne,
//! polityki Jądra i nakładka maszyny, kopie z rotacją, błędy zakresu, anulowanie.

use std::borrow::Cow;

use accounts_hub_contract::{SecretName, SecretString};
use core_log_contract::{REDACTED, Redactor, RegexRedactor};
use sessions_contract::SessionId;

use super::cases::{doc, find, full_scope, pwd};
use super::fixtures::{SECRET_PATTERN, SECRET_PLAIN, SHARED_TOML, seed};
use super::{Harness, ok, wipe, world};
use crate::backup::BackupRequest;
use crate::error::TransferError;
use crate::manifest::PackageKind;
use crate::report::{PlannedAction, Warning};
use crate::scope::{CancelToken, Category, ExportRequest, ExportScope, ImportOptions, Selection};

/// `ACC-F1-transfer-02` (test szpiegowski): paczka pełnego zakresu nie zawiera żadnej wartości
/// z `SecretStore` ani ciągów wyglądających na klucze (skan `RegexRedactor`), choć sekrety były
/// wklejone w sesje, konfigurację i pamięć.
pub fn no_secrets_in_plain_export(h: &dyn Harness) {
    seed(h, true);
    let pkg = h.path("szpieg.alfa");
    let scope = ExportScope {
        include_private: false,
        ..full_scope()
    };
    let report = ok(h.transfer().export(&ExportRequest::new(scope, &pkg)));
    assert!(report.manifest.redactions >= 4, "{:?}", report.manifest);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::Redacted { .. }))
    );
    let redactor = RegexRedactor::default();
    let entries = h.entries(&pkg);
    assert!(entries.len() > 5);
    for (path, bytes) in &entries {
        for secret in [SECRET_PATTERN, SECRET_PLAIN] {
            assert!(
                !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                "sekret w `{path}`"
            );
        }
        let text = String::from_utf8_lossy(bytes).replace(REDACTED, "~");
        assert!(
            matches!(redactor.redact(&text), Cow::Borrowed(_)),
            "ciąg wyglądający na klucz w `{path}`"
        );
    }
}

/// Paczka szyfrowana: bez hasła / złe hasło → czytelny błąd; słabe hasło odrzucone.
pub fn encrypted_package(h: &dyn Harness) {
    seed(h, false);
    let pkg = h.path("szyfr.alfa");
    let mut req = ExportRequest::new(ExportScope::default(), &pkg);
    req.password = Some(SecretString::from("krótkie"));
    assert!(matches!(
        h.transfer().export(&req),
        Err(TransferError::WeakPassword { .. })
    ));
    req.password = Some(pwd());
    ok(h.transfer().export(&req));
    assert_eq!(
        h.transfer().inspect(&pkg, &ImportOptions::default()).err(),
        Some(TransferError::PasswordRequired)
    );
    let wrong = ImportOptions {
        password: Some(SecretString::from("inne hasło 999")),
        ..ImportOptions::default()
    };
    assert_eq!(
        h.transfer().inspect(&pkg, &wrong).err(),
        Some(TransferError::WrongPassword)
    );
    let right = ImportOptions {
        password: Some(pwd()),
        ..ImportOptions::default()
    };
    let inspection = ok(h.transfer().inspect(&pkg, &right));
    assert!(inspection.manifest.encryption.is_some());
    assert_eq!(inspection.report.writes(), 0);
}

/// CX-a: sekrety nigdy nie opuszczają magazynu — paczki rodzaju `secrets` nie da się utworzyć
/// (także z hasłem), a pełny eksport szyfrowany nie ma sekcji ani licznika sekretów.
pub fn secrets_never_leave_the_store(h: &dyn Harness) {
    seed(h, false);
    let pkg = h.path("sekrety.alfa");
    let mut req = ExportRequest::new(full_scope(), &pkg);
    req.kind = PackageKind::Secrets;
    req.password = Some(pwd());
    assert!(h.transfer().export(&req).is_err(), "paczka sekretów");
    req.kind = PackageKind::Export;
    let report = ok(h.transfer().export(&req));
    assert_eq!(report.manifest.scope.counts.secrets, 0);
    assert!(!report.manifest.scope.keys.iter().any(|k| k == "secrets"));
    assert!(
        report
            .manifest
            .content
            .iter()
            .all(|e| e.path != crate::paths::SECRETS_PATH)
    );
    let name = ok(SecretName::new("accounts/acc-2"));
    assert!(ok(h.secrets().get(&name)).is_some(), "magazyn nietknięty");
}

/// Sesje prywatne: pomijane bez jawnego wyboru; z wyborem — tylko w paczce szyfrowanej.
pub fn private_sessions(h: &dyn Harness) {
    let seeded = seed(h, false);
    let pkg = h.path("prywatne.alfa");
    let mut scope = ExportScope {
        sessions: Selection::All,
        ..ExportScope::default()
    };
    let plain = ok(h
        .transfer()
        .export(&ExportRequest::new(scope.clone(), &pkg)));
    assert!(!plain.manifest.scope.sessions.contains(&seeded.private));
    assert!(plain.warnings.contains(&Warning::PrivateSession {
        id: seeded.private.clone(),
        skipped: true
    }));
    scope.include_private = true;
    assert!(matches!(
        h.transfer()
            .export(&ExportRequest::new(scope.clone(), &pkg)),
        Err(TransferError::EncryptionRequired { .. })
    ));
    let mut req = ExportRequest::new(scope, &pkg);
    req.password = Some(pwd());
    let enc = ok(h.transfer().export(&req));
    assert!(enc.manifest.scope.sessions.contains(&seeded.private));
}

/// Klucze `kernel.*` nigdy nie są importowane; nakładka maszyny tylko na wyraźne życzenie.
pub fn kernel_keys_and_machine_overlay(h: &dyn Harness) {
    seed(h, false);
    let with_kernel = format!("{SHARED_TOML}\n[kernel.egress]\nallow = [\"*\"]\n");
    ok(h.store(Category::ConfigCommon)
        .write("shared.toml", with_kernel.as_bytes()));
    let pkg = h.path("jadro.alfa");
    let scope = ExportScope {
        config_machine: true,
        ..ExportScope::default()
    };
    ok(h.transfer().export(&ExportRequest::new(scope, &pkg)));
    wipe(h);
    let report = ok(h.transfer().import(&pkg, &ImportOptions::default()));
    assert!(report.warnings.iter().any(|w| matches!(w, Warning::KernelKeysSkipped { keys, .. } if keys == &vec!["kernel.egress.allow".to_owned()])));
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::MachineOverlaySkipped { .. }))
    );
    let shared = ok(h.store(Category::ConfigCommon).read("shared.toml")).unwrap_or_default();
    let shared = String::from_utf8(shared).unwrap_or_default();
    assert!(
        !shared.contains("kernel") && shared.contains("piper"),
        "{shared}"
    );
    let overlay = format!("{}.toml", h.machine_id());
    assert_eq!(ok(h.store(Category::ConfigMachine).read(&overlay)), None);
    let opts = ImportOptions {
        include_machine_overlay: true,
        ..ImportOptions::default()
    };
    let inspected = ok(h.transfer().inspect(&pkg, &opts));
    assert_eq!(
        find(&inspected.report, &doc(Category::ConfigMachine, &overlay)).action,
        PlannedAction::Add
    );
    ok(h.transfer().import(&pkg, &opts));
    assert!(ok(h.store(Category::ConfigMachine).read(&overlay)).is_some());
}

/// Kopia zapasowa = eksport `backup` z rotacją N ostatnich; najnowsza przywraca stan.
pub fn backup_rotation(h: &dyn Harness) {
    seed(h, false);
    let dir = h.path("kopie");
    let request = BackupRequest {
        dir: dir.clone(),
        scope: full_scope(),
        keep: 2,
        password: Some(pwd()),
        cancel: None,
    };
    let mut last = None;
    for _ in 0..4 {
        let report = ok(h.transfer().backup(&request));
        assert_eq!(report.export.manifest.kind, PackageKind::Backup);
        last = Some(report.export.path);
    }
    let files = h.packages_in(&dir);
    assert_eq!(files.len(), 2, "{files:?}");
    let last = last.unwrap_or_else(|| panic!("brak kopii"));
    assert!(files.contains(&last));
    let before = world(h);
    wipe(h);
    let opts = ImportOptions {
        password: Some(pwd()),
        include_machine_overlay: true,
        ..ImportOptions::default()
    };
    ok(h.transfer().import(&last, &opts));
    assert_eq!(world(h), before);
}

/// Sesja spoza katalogu w zakresie → czytelny błąd, bez pliku.
pub fn unknown_session_is_error(h: &dyn Harness) {
    seed(h, false);
    let pkg = h.path("brak.alfa");
    let scope = ExportScope {
        sessions: Selection::Only(vec![SessionId::new("nie-ma-takiej")]),
        ..ExportScope::default()
    };
    assert!(
        h.transfer()
            .export(&ExportRequest::new(scope, &pkg))
            .is_err()
    );
    assert!(
        h.transfer()
            .inspect(&pkg, &ImportOptions::default())
            .is_err()
    );
}

/// Anulowanie przerywa eksport (bez pliku docelowego).
pub fn cancel_stops_export(h: &dyn Harness) {
    seed(h, false);
    let pkg = h.path("anulowany.alfa");
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut req = ExportRequest::new(full_scope(), &pkg);
    req.cancel = Some(cancel);
    req.password = Some(pwd());
    assert_eq!(
        h.transfer().export(&req).err(),
        Some(TransferError::Cancelled)
    );
    assert!(
        h.transfer()
            .inspect(&pkg, &ImportOptions::default())
            .is_err()
    );
}
