//! Próby obejścia (ACC-F3-safety-broker-02 w logice): agentka zmienia Jądro, podnosi sobie
//! poziom, zatwierdza sama siebie, fałszuje tokeny i dowody. Oczekiwane: 0 sukcesów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use core_bus_contract::{AgentId, SessionId};
use safety_broker_contract::contract_tests::{
    delta, exact, host, request, session_l4, test_policy, tree,
};
use safety_broker_contract::{
    AdminOp, AppSelector, ApprovalChannel, ApprovalDecision, AutonomyChangeRequest, AutonomyLevel,
    AutonomyTarget, Broker, Capability, ChangeOrigin, CommandOrigin, Decision, InputSource,
    KernelRule, Nonce, ServiceAction, broker_ui_only,
};

/// Wynik próby: `true` = atak się powiódł.
type Outcome = bool;

fn targets() -> Vec<AutonomyTarget> {
    let s = SessionId::new("s1");
    let a = AgentId::new("delta");
    vec![
        AutonomyTarget::Global,
        AutonomyTarget::Session { session: s.clone() },
        AutonomyTarget::Agent { agent: a.clone() },
        AutonomyTarget::SessionAgent {
            session: s,
            agent: a,
        },
    ]
}

async fn self_escalation_attempts() -> Vec<(String, Outcome)> {
    let (b, _, _) = common::engine();
    let mut out = Vec::new();
    for target in targets() {
        for until_ms in [None, Some(u64::MAX), Some(1)] {
            let req = AutonomyChangeRequest {
                target: target.clone(),
                level: AutonomyLevel::L4,
                until_ms,
                origin: ChangeOrigin::Agent("delta".into()),
            };
            let r = b.request_autonomy_change(req).await;
            // Sukces ataku = żądanie przyjęte; bez terminu musi to być blokada Jądra.
            let expected = until_ms != Some(1)
                && r != Err(safety_broker_contract::BrokerError::KernelBlock(
                    KernelRule::SelfEscalation,
                ));
            let ok = r.is_ok() || expected;
            out.push((
                format!("agentka podnosi {target:?} do L4 (until {until_ms:?})"),
                ok,
            ));
        }
    }
    let level = b.autonomy(&SessionId::new("s1"), Some(&AgentId::new("delta")));
    out.push(("poziom po próbach".into(), level != AutonomyLevel::L3));
    out.push((
        "prośby utworzone przez agentkę".into(),
        !b.pending().is_empty(),
    ));
    out
}

async fn policy_attempts() -> Vec<(String, Outcome)> {
    let (b, _, _) = common::engine();
    let mut out = Vec::new();
    let mut variants = Vec::new();
    let mut p = test_policy();
    p.egress_allowlist.push(host("*.evil.example"));
    variants.push(("allowlista", p));
    let mut p = test_policy();
    p.token_ttl_max_ms = 24 * 3_600_000;
    variants.push(("TTL", p));
    let mut p = test_policy();
    p.kernel_paths.clear();
    variants.push(("ścieżki Jądra", p));
    let mut p = test_policy();
    p.deny_lists.domains.clear();
    variants.push(("deny-lista domen", p));
    let mut p = test_policy();
    p.risk.bulk_threshold = 1_000_000;
    variants.push(("progi ryzyka", p));
    let mut p = test_policy();
    p.hello_required_for.clear();
    variants.push(("Hello", p));
    for (name, policy) in variants {
        let r = b
            .request_policy_change(policy, ChangeOrigin::Agent("delta".into()))
            .await;
        out.push((format!("agentka zmienia politykę: {name}"), r.is_ok()));
    }
    out
}

async fn forged_proof_attempts() -> Vec<(String, Outcome)> {
    let (b, _, clock) = common::engine();
    let mut out = Vec::new();
    for i in 0u8..40 {
        let req = AutonomyChangeRequest {
            target: AutonomyTarget::Session {
                session: SessionId::new("s1"),
            },
            level: AutonomyLevel::L4,
            until_ms: None,
            origin: ChangeOrigin::UserInterface,
        };
        let id = b.request_autonomy_change(req).await.unwrap().unwrap();
        let real = b
            .pending()
            .into_iter()
            .find(|c| c.request.id == id)
            .unwrap()
            .nonce;
        use watchdog_contract::Clock;
        let now = clock.now_ms();
        let (nonce, injected, at) = match i % 4 {
            0 => (Nonce([i; 16]), false, now),
            1 => (real, true, now),
            2 => (real, false, now + 60_000),
            _ => {
                let mut n = real;
                n.0[usize::from(i) % 16] ^= 0x01;
                (n, false, now)
            }
        };
        let source = if i % 2 == 0 {
            InputSource::Keyboard
        } else {
            InputSource::MouseClick
        };
        let proof = broker_ui_only::physical_input_proof(id, nonce, source, injected, at);
        let ok = b.resolve(id, ApprovalDecision::Allow, proof).await.is_ok();
        out.push((format!("fałszywy dowód #{i}"), ok));
    }
    let level = b.autonomy(&SessionId::new("s1"), None);
    out.push((
        "poziom po fałszywych dowodach".into(),
        level != AutonomyLevel::L3,
    ));
    out
}

async fn kernel_target_attempts() -> Vec<(String, Outcome)> {
    let (b, _, clock) = common::engine();
    session_l4(&b, &clock).await;
    let mut caps: Vec<(String, Capability)> = Vec::new();
    for p in [
        r"C:\ProgramData\AlfaBroker\policy.json",
        r"c:\programdata\alfabroker\AUDIT\x",
        r"\\?\C:\ProgramData\AlfaBroker\keys",
        r"C:\ProgramData\x\..\AlfaBroker\a",
        r"%APPDATA%\Alfa\kernel\policy.toml",
        r"%LOCALAPPDATA%\Alfa\audit\audit.ndjson",
        r"\\localhost\c$\ProgramData\AlfaBroker\a",
        r"C:\Windows\System32\config\SAM",
        r"C:\Boot\BCD",
    ] {
        caps.push((format!("fs.write {p}"), Capability::FsWrite(exact(p))));
    }
    for p in [
        r"%USERPROFILE%\.claude",
        r"C:\Users\ala\.codex\auth.json",
        r"C:\Users\ala\AppData\Roaming\Microsoft\Credentials",
    ] {
        caps.push((format!("fs.read {p}"), Capability::FsRead(tree(p))));
    }
    for app in [
        "alfa.exe",
        "alfa-core",
        "alfa-broker.exe",
        "ALFA-BROKER-UI.EXE",
        "alfa-watchdog",
        "alfa-uiaccess-helper.exe",
        r"C:\x\alfa-broker.exe.",
    ] {
        caps.push((
            format!("gui.control {app}"),
            Capability::GuiControl(AppSelector::parse(app).unwrap()),
        ));
    }
    for h in [
        "claude.ai",
        "chatgpt.com",
        "*.openai.com",
        "gemini.google.com",
        "https://user@claude.ai:443/x",
    ] {
        caps.push((format!("net.egress {h}"), Capability::NetEgress(host(h))));
    }
    for op in [
        AdminOp::DisableAudit,
        AdminOp::Bootloader,
        AdminOp::ChangeKernelPolicy,
        AdminOp::FormatDisk { drive: 'C' },
        AdminOp::ServiceControl {
            service: "AlfaBroker".into(),
            action: ServiceAction::Delete,
        },
        AdminOp::ServiceControl {
            service: "EventLog".into(),
            action: ServiceAction::Disable,
        },
        AdminOp::Other {
            command: "sc stop alfawatchdog".into(),
        },
    ] {
        caps.push((format!("system.admin {op:?}"), Capability::SystemAdmin(op)));
    }
    let mut out = Vec::new();
    for (name, cap) in caps {
        let r = b.decide(request(&delta(), cap, CommandOrigin::Agent)).await;
        out.push((name, !matches!(r, Ok(Decision::Deny(_)))));
    }
    for cmd in [
        "wevtutil cl Security",
        "sc stop AlfaWatchdog",
        "taskkill /im alfa-broker-ui.exe /f",
        "auditpol /clear /y",
        "format c:",
        "bcdedit /deletevalue x",
        "powershell -enc AAAA",
        "del /q C:\\ProgramData\\AlfaBroker\\audit\\*",
        "type %USERPROFILE%\\.claude\\x",
        "rd /s /q %SystemRoot%",
    ] {
        let mut r = request(
            &delta(),
            Capability::ShellExec(tree(r"C:\Users\ala\proj")),
            CommandOrigin::Agent,
        );
        r.facts.command = Some(cmd.into());
        let d = b.decide(r).await;
        out.push((format!("shell {cmd}"), !matches!(d, Ok(Decision::Deny(_)))));
    }
    out
}

async fn token_forgery_attempts() -> Vec<(String, Outcome)> {
    let (b, _, _) = common::engine();
    let (other, _, _) = common::engine();
    let mut out = Vec::new();
    let read = request(
        &delta(),
        Capability::FsRead(tree(r"C:\Users\ala\Docs")),
        CommandOrigin::UserText,
    );
    let Ok(Decision::Allow(t)) = b.decide(read.clone()).await else {
        panic!()
    };
    let Ok(Decision::Allow(foreign)) = other.decide(read).await else {
        panic!()
    };
    let file = Capability::FsRead(exact(r"C:\Users\ala\Docs\a.txt"));
    out.push((
        "token innego uruchomienia".into(),
        b.verify(&foreign, &file, &delta()).is_ok(),
    ));
    let mut resigned = foreign.clone();
    resigned.boot = t.boot;
    out.push((
        "podmiana boot".into(),
        b.verify(&resigned, &file, &delta()).is_ok(),
    ));
    for i in 0..10u8 {
        let mut x = t.clone();
        x.mac[usize::from(i)] ^= 0xff;
        out.push((
            format!("MAC bajt {i}"),
            b.verify(&x, &file, &delta()).is_ok(),
        ));
    }
    let wider = Capability::FsRead(tree(r"C:\Users\ala"));
    out.push((
        "użycie szerzej niż token".into(),
        b.verify(&t, &wider, &delta()).is_ok(),
    ));
    let creds = Capability::FsRead(exact(r"C:\Users\ala\Docs\.claude\x"));
    out.push((
        "deny-lista w zakresie tokenu".into(),
        b.verify(&t, &creds, &delta()).is_ok(),
    ));
    out
}

#[tokio::test]
async fn zero_successful_attacks() {
    let mut all = Vec::new();
    all.extend(self_escalation_attempts().await);
    all.extend(policy_attempts().await);
    all.extend(forged_proof_attempts().await);
    all.extend(kernel_target_attempts().await);
    all.extend(token_forgery_attempts().await);
    let successes: Vec<&String> = all.iter().filter(|(_, ok)| *ok).map(|(n, _)| n).collect();
    eprintln!(
        "scenariusze negatywne: {}, sukcesy ataku: {}",
        all.len(),
        successes.len()
    );
    assert!(all.len() >= 100, "za mało scenariuszy: {}", all.len());
    assert!(successes.is_empty(), "udane ataki: {successes:#?}");
}

/// Regresja: obniżenie „na czas” nadpisujące jawny wpis właściciela nie może po wygaśnięciu
/// podnieść poziomu (L2 właściciela → agentka L1 na 1 ms → powrót do globalnego L4).
#[tokio::test]
async fn timed_lowering_cannot_escalate_after_expiry() {
    use safety_broker_contract::contract_tests::set_level;
    let (b, _, clock) = common::engine();
    set_level(&b, &clock, AutonomyTarget::Global, AutonomyLevel::L4).await;
    let s1 = SessionId::new("s1");
    let session = AutonomyTarget::Session {
        session: s1.clone(),
    };
    let by_owner = AutonomyChangeRequest {
        target: session.clone(),
        level: AutonomyLevel::L2,
        until_ms: None,
        origin: ChangeOrigin::UserInterface,
    };
    assert_eq!(b.request_autonomy_change(by_owner).await, Ok(None));
    use watchdog_contract::Clock;
    let now = clock.now_ms();
    let past = AutonomyChangeRequest {
        target: session.clone(),
        level: AutonomyLevel::L1,
        until_ms: Some(now),
        origin: ChangeOrigin::Agent("delta".into()),
    };
    assert!(b.request_autonomy_change(past).await.is_err());
    assert_eq!(b.autonomy(&s1, None), AutonomyLevel::L2);
    let timed = AutonomyChangeRequest {
        target: session,
        level: AutonomyLevel::L1,
        until_ms: Some(now + 1),
        origin: ChangeOrigin::Agent("delta".into()),
    };
    assert_eq!(b.request_autonomy_change(timed).await, Ok(None));
    clock.advance(10);
    let level = b.autonomy(&s1, None);
    assert!(level <= AutonomyLevel::L2, "po wygaśnięciu: {level:?}");
}
