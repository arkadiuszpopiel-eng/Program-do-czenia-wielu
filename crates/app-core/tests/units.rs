//! Testy jednostkowe publicznych pomocników: protokół `alfa://`, powiadomienia natywne,
//! grupowanie zdarzeń w paczki, reguła AltGr skrótów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod protocol {
    use app_core::protocol::*;

    #[test]
    fn whitelisted_actions_parse() {
        assert_eq!(parse_uri("alfa://open"), Some(ProtocolAction::Open));
        assert_eq!(parse_uri("alfa://quick/"), Some(ProtocolAction::QuickAsk));
        assert_eq!(
            parse_uri("alfa://session/0192abcd-ef"),
            Some(ProtocolAction::OpenSession("0192abcd-ef".into()))
        );
        assert_eq!(
            parse_uri("alfa://new?text=Cze%C5%9B%C4%87+Alfa"),
            Some(ProtocolAction::NewChat(Some("Cześć Alfa".into())))
        );
        assert_eq!(parse_uri("alfa://new"), Some(ProtocolAction::NewChat(None)));
    }

    #[test]
    fn everything_else_is_rejected() {
        for bad in [
            "https://alfa/open",
            "alfa://run?cmd=calc",
            "alfa://session/../../etc",
            "alfa://session/a%2Fb",
            "alfa://new?text=%00",
            "alfa://new?text=%ZZ",
            "alfa://settings/kernel",
        ] {
            assert_eq!(parse_uri(bad), None, "{bad}");
        }
        assert_eq!(
            parse_uri(&format!("alfa://new?text={}", "a".repeat(3000))),
            None
        );
        assert_eq!(
            from_args(["alfa.exe".to_owned(), "alfa://quick".to_owned()]),
            Some(ProtocolAction::QuickAsk)
        );
    }
}

mod notify {
    use app_core::dto::{AlfaEvent, StopReason};
    use app_core::notify::*;

    use app_core::dto::{ApprovalPending, ApprovalStatus, RiskLevel};

    #[test]
    fn approval_notice_never_carries_an_approve_action() {
        let n = native_notice(&AlfaEvent::ApprovalPending {
            session_id: "s".into(),
            turn_id: "s:t1".into(),
            approval: ApprovalPending {
                id: "ap".into(),
                what: "Usuń pliki".into(),
                why: "x".into(),
                reversible: false,
                risk: RiskLevel::High,
                status: ApprovalStatus::Pending,
                broker_window: false,
                expires_at: None,
            },
        })
        .unwrap();
        assert!(n.body.contains("Brokera"));
        assert!(!n.body.to_lowercase().contains("zatwierdź"));
        assert!(native_notice(&AlfaEvent::MicLevel { level: 0.1 }).is_none());
        let done = native_notice(&AlfaEvent::Stop {
            session_id: "s".into(),
            turn_id: "s:t2".into(),
            reason: StopReason::End,
        });
        assert_eq!(done.unwrap().session_id.as_deref(), Some("s"));
    }
}

mod events {
    use std::time::Duration;

    use app_core::dto::AlfaEvent;
    use app_core::events::*;

    use app_core::dto::{BlockKind, RenderedBlock};

    fn delta(turn: &str, text: &str, blocks: &[(u64, &str, bool)]) -> AlfaEvent {
        AlfaEvent::TextDelta {
            session_id: "s".into(),
            turn_id: turn.into(),
            text: text.into(),
            blocks: blocks
                .iter()
                .map(|(i, html, closed)| RenderedBlock {
                    index: *i,
                    kind: BlockKind::Text,
                    lang: None,
                    html_sanitized: (*html).into(),
                    closed: *closed,
                })
                .collect(),
        }
    }

    #[test]
    fn adjacent_deltas_of_one_turn_merge() {
        let mut batch = Vec::new();
        push_coalesced(&mut batch, delta("t1", "Ala ", &[(0, "<p>Ala</p>", false)]));
        push_coalesced(
            &mut batch,
            delta(
                "t1",
                "ma\n\nkota",
                &[(0, "<p>Ala ma</p>", true), (1, "<p>kota</p>", false)],
            ),
        );
        push_coalesced(&mut batch, delta("t2", "x", &[]));
        assert_eq!(batch.len(), 2);
        let AlfaEvent::TextDelta { text, blocks, .. } = &batch[0] else {
            panic!("oczekiwano TextDelta");
        };
        assert_eq!(text, "Ala ma\n\nkota");
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].closed);
        assert_eq!(blocks[0].html_sanitized, "<p>Ala ma</p>");
    }

    #[test]
    fn mic_levels_keep_latest() {
        let mut batch = Vec::new();
        push_coalesced(&mut batch, AlfaEvent::MicLevel { level: 0.1 });
        push_coalesced(&mut batch, AlfaEvent::MicLevel { level: 0.7 });
        assert_eq!(batch, vec![AlfaEvent::MicLevel { level: 0.7 }]);
    }

    #[tokio::test(start_paused = true)]
    async fn events_arrive_in_one_batch_per_frame() {
        let hub = EventHub::start(Duration::from_millis(16));
        let mut rx = hub.subscribe();
        for i in 0..5 {
            hub.emit(delta("t1", &i.to_string(), &[]));
        }
        hub.emit(AlfaEvent::SessionRemoved {
            session_id: "s".into(),
        });
        let batch = rx.recv().await.unwrap();
        assert_eq!(batch.len(), 2, "5 delt scalonych + SessionRemoved");
        let AlfaEvent::TextDelta { text, .. } = &batch[0] else {
            panic!("oczekiwano TextDelta");
        };
        assert_eq!(text, "01234");
    }
}

mod shortcuts {
    use app_core::validate_chord;

    #[test]
    fn altgr_rule_and_kill_switch_are_enforced() {
        assert!(validate_chord("Ctrl+Alt+S").is_err());
        assert!(validate_chord("Ctrl+Alt+Shift+z").is_err());
        assert!(validate_chord("Ctrl+Shift+F12").is_err());
        assert!(validate_chord("Ctrl+Alt+Space").is_ok());
        assert!(validate_chord("Ctrl+Alt+K").is_ok());
        assert!(validate_chord("Ctrl+Shift+P").is_ok());
    }
}
