//! Współdzielone testy kontraktowe (feature `contract-tests`) dla `-impl` i `-fake`:
//! ścieżka szczęśliwa (rozmowa → szkic → manifest → podgląd → test na sucho → zapis) i ≥ 30
//! prób ataku (uprawnienia Jądra, podnoszenie autonomii, zakresy, prompty, podmiana) = 0 sukcesów.
//! Kreator musi być zbudowany nad [`sample_tools`] i domyślną polityką (sufit L3).

mod attacks;

pub use crate::samples::{DESCRIPTION, sample_tools, scenario};
pub use attacks::{ATTACKS_MIN, attacks};

use risk_classifier_contract::AutonomyLevel;
use serde_json::json;

use crate::{
    AgentBuilder, BuildError, BuilderApproval, BuilderApprovalOrigin, DryExpect, DryScenario,
    DryStep,
};

/// Rozmowa → szkic → manifest → podgląd → test na sucho → zapis po zatwierdzeniu.
pub async fn happy_path(b: &dyn AgentBuilder) {
    let p = b.propose(DESCRIPTION);
    let d = &p.draft;
    assert_eq!(d.name.as_deref(), Some("Ola"));
    assert!(
        d.role
            .as_ref()
            .is_some_and(|r| r.tools.contains(&"fs".to_owned()))
    );
    assert_eq!(
        d.limits.fs_write,
        vec!["%USERPROFILE%\\Downloads\\**".to_owned()]
    );
    let built = b.build(d).unwrap_or_else(|e| panic!("{e}"));
    let m = &built.manifest;
    assert_eq!(m.persona.forms.vocative, "Olu");
    assert!(m.limits.autonomy <= AutonomyLevel::L3);
    let pv = b.preview(m);
    assert!(pv.system_prompt.contains("Ola") && pv.system_prompt.contains("rodzaju żeńskim"));
    assert!(
        pv.tools.contains(&"fs_move".to_owned()) && !pv.tools.contains(&"shell_run".to_owned())
    );
    let ui = |hash: &str| BuilderApproval {
        origin: BuilderApprovalOrigin::Ui,
        reviewed_hash: hash.to_owned(),
    };
    assert_eq!(
        b.save(m, ui(&built.hash)).await,
        Err(BuildError::DryRunRequired)
    );
    let failing = DryScenario {
        steps: vec![DryStep {
            tool: "shell_run".into(),
            args: json!({}),
            expect: DryExpect::Allowed,
        }],
    };
    assert!(
        !b.dry_run(m, &failing)
            .await
            .unwrap_or_else(|e| panic!("{e}"))
            .passed
    );
    assert_eq!(
        b.save(m, ui(&built.hash)).await,
        Err(BuildError::DryRunRequired)
    );
    let report = b
        .dry_run(m, &scenario())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(report.passed, "{:#?}", report.steps);
    let voice = BuilderApproval {
        origin: BuilderApprovalOrigin::Voice,
        reviewed_hash: built.hash.clone(),
    };
    assert!(matches!(
        b.save(m, voice).await,
        Err(BuildError::Approval(_))
    ));
    assert!(matches!(
        b.save(m, ui("00")).await,
        Err(BuildError::Approval(_))
    ));
    let saved = b
        .save(m, ui(&built.hash))
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(saved.persona.as_str(), "ola");
    assert_eq!(b.library().len(), 1);
    assert!(
        matches!(b.build(d), Err(BuildError::Conflict(_))),
        "druga Ola koliduje"
    );
}
