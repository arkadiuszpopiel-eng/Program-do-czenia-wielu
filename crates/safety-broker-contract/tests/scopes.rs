//! Zakresy, atenuacja, format przewodowy tokenu, strażnik Jądra (testy jednostkowe i negatywne).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use compliance_contract::PathEnv;
use safety_broker_contract::{
    ActionRequest, AdminOp, AppSelector, BootId, CapToken, Capability, CommandOrigin,
    DeclaredFacts, HostPattern, KernelGuard, KernelPolicy, KernelRule, PathScope, ScopeError,
    SecretId, ServiceAction, SessionSecurity, TokenId,
};
use safety_broker_contract::{Holder, ipc};

fn env() -> PathEnv {
    PathEnv::windows_profile(r"C:\Users\ala")
}

fn tree(p: &str) -> PathScope {
    PathScope::tree(p, &env()).unwrap()
}

fn exact(p: &str) -> PathScope {
    PathScope::exact(p, &env()).unwrap()
}

#[test]
fn path_scope_normalization_and_subset() {
    assert_eq!(
        tree(r"C:\Users\Ala\Docs\.\x\..").canonical(),
        r"c:\users\ala\docs"
    );
    assert_eq!(
        tree(r"\\?\C:\Users\ala\Docs\").canonical(),
        r"c:\users\ala\docs"
    );
    assert_eq!(tree("%USERPROFILE%/Docs").canonical(), r"c:\users\ala\docs");
    assert_eq!(tree(r"c:\").canonical(), r"c:\");
    assert!(matches!(
        PathScope::tree("docs", &env()),
        Err(ScopeError::RelativePath(_))
    ));
    let docs = tree(r"C:\Users\ala\Docs");
    assert!(exact(r"C:\Users\ala\Docs\a.txt").is_subset_of(&docs));
    assert!(tree(r"C:\Users\ala\Docs\sub").is_subset_of(&docs));
    assert!(docs.is_subset_of(&docs));
    // Ucieczki i rodzeństwo z tym samym prefiksem tekstowym.
    assert!(!exact(r"C:\Users\ala\Docs\..\secret.txt").is_subset_of(&docs));
    assert!(!tree(r"C:\Users\ala\Docs2").is_subset_of(&docs));
    assert!(!tree(r"D:\Users\ala\Docs").is_subset_of(&docs));
    assert!(!tree(r"C:\Users\ala").is_subset_of(&docs));
    // Plik dokładny nie obejmuje poddrzewa ani dzieci.
    let file = exact(r"C:\Users\ala\Docs\a.txt");
    assert!(!tree(r"C:\Users\ala\Docs\a.txt").is_subset_of(&file));
    assert!(!exact(r"C:\Users\ala\Docs\a.txt\b").is_subset_of(&file));
    // ADS i końcowe kropki nie tworzą nowej ścieżki.
    assert_eq!(
        exact(r"C:\Users\ala\Docs\a.txt:evil").canonical(),
        file.canonical()
    );
    assert_eq!(
        exact(r"C:\Users\ala\Docs\a.txt. ").canonical(),
        file.canonical()
    );
    assert!(docs.overlaps(&file) && file.overlaps(&docs));
    assert_eq!(docs.to_string(), r"c:\users\ala\docs\**");
}

#[test]
fn path_scope_serde_rejects_non_canonical() {
    let docs = tree(r"C:\Users\ala\Docs");
    let json = serde_json::to_string(&docs).unwrap();
    assert_eq!(serde_json::from_str::<PathScope>(&json).unwrap(), docs);
    let bad = r#"{"path":"C:\\Users\\ala\\Docs","subtree":true}"#;
    assert!(serde_json::from_str::<PathScope>(bad).is_err());
    let rel = r#"{"path":"docs","subtree":true}"#;
    assert!(serde_json::from_str::<PathScope>(rel).is_err());
}

#[test]
fn host_patterns() {
    let exact_host = HostPattern::parse("https://API.example.com/v1").unwrap();
    assert_eq!(exact_host.to_string(), "api.example.com");
    let wild = HostPattern::parse("*.example.com").unwrap();
    assert!(wild.matches("example.com") && wild.matches("a.b.example.com"));
    assert!(!wild.matches("example.com.evil.net") && !wild.matches("notexample.com"));
    assert!(exact_host.is_subset_of(&wild));
    assert!(!wild.is_subset_of(&exact_host));
    assert!(
        !HostPattern::parse("*.other.com")
            .unwrap()
            .is_subset_of(&wild)
    );
    for bad in ["*", "*.com", "a*.example.com", "", "  ", "ex ample.com"] {
        assert!(HostPattern::parse(bad).is_err(), "{bad}");
    }
    let back: HostPattern = serde_json::from_str("\"*.example.com\"").unwrap();
    assert_eq!(back, wild);
}

#[test]
fn app_and_secret_selectors() {
    assert_eq!(
        AppSelector::parse(r"C:\Program Files\Office\WINWORD.EXE")
            .unwrap()
            .exe(),
        "winword.exe"
    );
    assert_eq!(AppSelector::parse("notepad").unwrap().exe(), "notepad.exe");
    for bad in ["*", "", "a?.exe", "c:", ".exe"] {
        assert!(AppSelector::parse(bad).is_err(), "{bad}");
    }
    assert!(SecretId::parse("anthropic:main").is_ok());
    assert!(SecretId::parse("Bad Id").is_err());
}

#[test]
fn capability_attenuation_is_family_strict() {
    let docs = tree(r"C:\Users\ala\Docs");
    let read = Capability::FsRead(docs.clone());
    let write = Capability::FsWrite(docs.clone());
    assert!(!read.is_subset_of(&write) && !write.is_subset_of(&read));
    assert!(!Capability::ShellExec(docs.clone()).is_subset_of(&write));
    let a = Capability::GuiControl(AppSelector::parse("word").unwrap());
    let b = Capability::GuiControl(AppSelector::parse("excel").unwrap());
    assert!(a.is_subset_of(&a) && !a.is_subset_of(&b));
    assert_eq!(read.to_string(), r"fs.read(c:\users\ala\docs\**)");
    let json = serde_json::to_value(&write).unwrap();
    assert_eq!(json["cap"], "fs.write");
}

fn sample_token() -> CapToken {
    CapToken {
        id: TokenId(42),
        parent: Some(TokenId(7)),
        cap: Capability::FsWrite(tree(r"C:\Users\ala\Docs")),
        holder: Holder::agent("s1", "delta").with_role("wykonawczyni"),
        boot: BootId([9; 16]),
        key_epoch: 3,
        issued_at_ms: 1_000,
        expires_at_ms: 2_000,
        mac: [5; 32],
    }
}

#[test]
fn wire_format_round_trip_and_strictness() {
    let t = sample_token();
    let wire = t.to_wire();
    assert_eq!(CapToken::from_wire(&wire).unwrap(), t);
    let json = serde_json::to_string(&t).unwrap();
    assert_eq!(serde_json::from_str::<CapToken>(&json).unwrap(), t);
    assert!(CapToken::from_wire(&wire[..wire.len() - 1]).is_err());
    let mut longer = wire.clone();
    longer.push(0);
    assert!(CapToken::from_wire(&longer).is_err());
    assert!(CapToken::from_wire(&[]).is_err());
    assert!(serde_json::from_str::<CapToken>(&json.to_uppercase()).is_err());
    assert_eq!(t.remaining_ms(1_500), 500);
    assert_eq!(t.remaining_ms(5_000), 0);
}

fn guard() -> KernelGuard {
    let mut p = KernelPolicy::baseline(r"C:\Users\ala", r"C:\ProgramData\AlfaBroker").unwrap();
    p.egress_allowlist = vec![HostPattern::parse("api.example.com").unwrap()];
    p.allowed_apps = vec![AppSelector::parse("winword.exe").unwrap()];
    assert!(p.validate().is_ok());
    KernelGuard::new(p, env())
}

fn rule(cap: Capability) -> Option<KernelRule> {
    guard().check_request(&cap, &DeclaredFacts::new("t"))
}

#[test]
fn kernel_guard_capabilities() {
    use KernelRule as K;
    assert_eq!(
        rule(Capability::FsRead(tree(r"%USERPROFILE%\.claude"))),
        Some(K::CredentialDenylist)
    );
    assert_eq!(
        rule(Capability::FsRead(exact(r"C:\Users\ala\.codex\auth.json"))),
        Some(K::CredentialDenylist)
    );
    assert_eq!(
        rule(Capability::FsRead(exact(r"C:\Users\ala\CLAUDE~1\x"))),
        Some(K::CredentialDenylist)
    );
    assert_eq!(
        rule(Capability::FsRead(tree(
            r"%LOCALAPPDATA%\Google\Chrome\User Data"
        ))),
        Some(K::CredentialDenylist)
    );
    assert_eq!(rule(Capability::FsRead(tree(r"C:\Users\ala\Docs"))), None);
    assert_eq!(
        rule(Capability::FsWrite(tree(
            r"C:\ProgramData\AlfaBroker\audit"
        ))),
        Some(K::KernelPolicyChange)
    );
    assert_eq!(
        rule(Capability::FsWrite(exact(
            r"C:\Windows\System32\drivers\x.sys"
        ))),
        Some(K::SystemRootDeletion)
    );
    assert_eq!(
        rule(Capability::FsWrite(tree(r"C:\Boot"))),
        Some(K::BootloaderModification)
    );
    assert_eq!(rule(Capability::FsRead(tree(r"C:\Windows"))), None);
    let mut destroy = DeclaredFacts::new("t");
    destroy.destructive = risk_classifier_contract::Destructiveness::Permanent;
    assert_eq!(
        guard().check_request(&Capability::FsWrite(tree(r"C:\")), &destroy),
        Some(K::SystemRootDeletion)
    );
    assert_eq!(
        guard().check_request(&Capability::FsWrite(tree(r"C:\")), &DeclaredFacts::new("t")),
        None
    );
    for app in [
        "alfa-broker.exe",
        "ALFA-BROKER-UI",
        r"C:\Program Files\Alfa\alfa-watchdog.exe",
        "alfa-b~1.exe",
    ] {
        assert_eq!(
            rule(Capability::GuiControl(AppSelector::parse(app).unwrap())),
            Some(K::GuiControlOfKernelProcess),
            "{app}"
        );
    }
    assert_eq!(
        rule(Capability::GuiControl(
            AppSelector::parse("winword").unwrap()
        )),
        None
    );
    for host in [
        "claude.ai",
        "https://www.chatgpt.com/x",
        "*.openai.com",
        "*.claude.ai",
    ] {
        assert_eq!(
            rule(Capability::NetEgress(HostPattern::parse(host).unwrap())),
            Some(K::ProviderWebUi),
            "{host}"
        );
    }
    assert_eq!(
        rule(Capability::NetEgress(
            HostPattern::parse("api.example.com").unwrap()
        )),
        None
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::DisableAudit)),
        Some(K::AuditDisable)
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::FormatDisk { drive: 'C' })),
        Some(K::SystemDiskFormat)
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::FormatDisk { drive: 'e' })),
        None
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::ServiceControl {
            service: "AlfaWatchdog".into(),
            action: ServiceAction::Stop
        })),
        Some(K::KillSwitchDisable)
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::ServiceControl {
            service: "AlfaWatchdog".into(),
            action: ServiceAction::Start
        })),
        None
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::RegistryMachine {
            key: r"HKLM\SYSTEM\CurrentControlSet\Services\AlfaBroker".into()
        })),
        Some(K::KillSwitchDisable)
    );
    assert_eq!(
        rule(Capability::SystemAdmin(AdminOp::Other {
            command: "bcdedit /set x".into()
        })),
        Some(K::BootloaderModification)
    );
}

#[test]
fn derive_facts_uses_broker_knowledge() {
    use risk_classifier_contract::ScopeRelation;
    let g = guard();
    let req = |cap: Capability| ActionRequest {
        holder: Holder::agent("s1", "delta"),
        capability: cap,
        facts: DeclaredFacts::new("tool"),
        origin: CommandOrigin::Agent,
        ttl_ms: None,
    };
    let clean = SessionSecurity::default();
    let f = g.derive_facts(
        &req(Capability::FsWrite(tree(r"C:\Users\ala\Docs"))),
        &clean,
    );
    assert_eq!(f.scope, ScopeRelation::InScope);
    let f = g.derive_facts(&req(Capability::FsWrite(tree(r"D:\data"))), &clean);
    assert_eq!(f.scope, ScopeRelation::Outside);
    let tainted = SessionSecurity {
        tainted: true,
        private_data: true,
        ..SessionSecurity::default()
    };
    let f = g.derive_facts(
        &req(Capability::NetEgress(
            HostPattern::parse("api.example.com").unwrap(),
        )),
        &tainted,
    );
    assert!(f.tainted && f.egress_allowlisted && f.trifecta());
    let f = g.derive_facts(
        &req(Capability::GuiControl(
            AppSelector::parse("winword").unwrap(),
        )),
        &clean,
    );
    assert_eq!(f.scope, ScopeRelation::AllowedApp);
    let mut untrusted = req(Capability::FsRead(tree(r"C:\Users\ala")));
    untrusted.origin = CommandOrigin::UntrustedContent;
    assert!(g.derive_facts(&untrusted, &clean).tainted);
}

#[test]
fn ipc_roles_and_frames() {
    use ipc::{ClientRole as R, Request};
    let kill = Request::KillAll {
        reason: watchdog_contract::KillReason::Hotkey,
    };
    assert!(
        kill.permitted(R::Watchdog) && kill.permitted(R::BrokerUi) && !kill.permitted(R::Agent)
    );
    assert!(Request::PendingApprovals.permitted(R::BrokerUi));
    for role in [R::Agent, R::Core, R::Watchdog] {
        assert!(!Request::PendingApprovals.permitted(role), "{role:?}");
    }
    let frame = ipc::encode_frame(&Request::Metrics).unwrap();
    let len = ipc::frame_len(frame[..4].try_into().unwrap()).unwrap();
    let back: Request = ipc::decode_body(&frame[4..4 + len]).unwrap();
    assert_eq!(back, Request::Metrics);
    assert!(ipc::frame_len((u32::MAX).to_le_bytes()).is_err());
    assert!(ipc::decode_body::<Request>(b"{\"op\":\"nope\"}").is_err());
}
