//! Kontrakt formatu. Atrapa wykonawcy liczy wejścia; nie ma sieci ani prawdziwych działań.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_client_wire::{CommandDecoder, DecodeError, MAX_REQUEST_ID, ProtocolVersions};
use serde_json::json;
use std::num::{NonZeroU16, NonZeroUsize};

fn decoder(limit: usize) -> CommandDecoder {
    CommandDecoder::new(
        NonZeroU16::new(2).unwrap(),
        NonZeroUsize::new(limit).unwrap(),
    )
}

fn frame(v: u16, id: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({"v":v,"id":id,"cmd":"sessions_list","args":{}})).unwrap()
}

#[test]
fn valid_roundtrip_preserves_arguments_without_executing() {
    let raw = r#"{"v":2,"id":1,"cmd":"sessions_rename","args":{"sessionId":"synthetic","title":"Zażółć"}}"#.as_bytes();
    let result = decoder(raw.len()).decode(raw).unwrap();
    assert_eq!(result.cmd, "sessions_rename");
    assert_eq!(result.args["title"], "Zażółć");
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::from_slice::<serde_json::Value>(raw).unwrap()
    );
}

#[test]
fn wrong_version_never_reaches_executor() {
    let mut calls = 0;
    for v in [0, 1, 3, u16::MAX] {
        if decoder(1024).decode(&frame(v, 1)).is_ok() {
            calls += 1;
        }
    }
    assert_eq!(calls, 0);
}

#[test]
fn only_exact_positive_javascript_ids_are_accepted() {
    for id in [0, MAX_REQUEST_ID + 1, u64::MAX] {
        assert_eq!(
            decoder(1024).decode(&frame(2, id)).unwrap_err(),
            DecodeError::RequestId
        );
    }
    assert_eq!(
        decoder(1024).decode(&frame(2, MAX_REQUEST_ID)).unwrap().id,
        MAX_REQUEST_ID
    );
}

#[test]
fn replay_and_out_of_order_requests_are_rejected() {
    let mut d = decoder(1024);
    d.decode(&frame(2, 3)).unwrap();
    for id in [3, 2, 1] {
        assert_eq!(d.decode(&frame(2, id)).unwrap_err(), DecodeError::Replay);
    }
    d.decode(&frame(2, 7)).unwrap();
}

#[test]
fn consumed_id_cannot_be_retried_after_a_later_acl_denial() {
    let mut d = decoder(1024);
    let _denied_by_future_host_acl = d.decode(&frame(2, 1)).unwrap();
    assert_eq!(d.decode(&frame(2, 1)).unwrap_err(), DecodeError::Replay);
}

#[test]
fn byte_budget_is_checked_before_json_and_includes_utf8_bytes() {
    let raw = frame(2, 1);
    assert!(decoder(raw.len()).decode(&raw).is_ok());
    assert_eq!(
        decoder(raw.len() - 1).decode(&raw).unwrap_err(),
        DecodeError::TooLarge
    );
    assert_eq!(
        decoder(1).decode(&[0xff; 2]).unwrap_err(),
        DecodeError::TooLarge
    );
    let utf8 = "🦉".as_bytes();
    assert_eq!(
        decoder(utf8.len() - 1).decode(utf8).unwrap_err(),
        DecodeError::TooLarge
    );
}

#[test]
fn closed_decoder_rejects_even_malformed_or_oversized_input() {
    let mut d = decoder(1);
    d.close();
    d.close();
    assert_eq!(d.decode(&frame(2, 1)).unwrap_err(), DecodeError::Closed);
    assert_eq!(d.decode(&[]).unwrap_err(), DecodeError::Closed);
}

#[test]
fn malformed_envelopes_and_forged_authority_are_rejected() {
    for text in [
        r#"{"v":2,"id":1,"cmd":"sessions_list","args":{},"kind":"LocalWindow"}"#,
        r#"{"v":2,"id":1,"cmd":"sessions_list","args":{},"authenticated":true}"#,
        r#"{"v":2,"id":1,"cmd":"sessions_list","args":{},"acl":"all"}"#,
        r#"{"v":2,"id":1,"cmd":"sessions_list","args":null}"#,
        r#"{"v":2,"id":1,"cmd":"sessions_list","args":[]}"#,
        r#"{"v":2,"id":1,"cmd":"sessions_list"}"#,
        r#"{"v":2,"id":1,"id":2,"cmd":"sessions_list","args":{}}"#,
        r#"{"v":2,"id":1.5,"cmd":"sessions_list","args":{}}"#,
        r#"{"v":2,"id":1,"cmd":"sessions_list","args":{}} {}"#,
        "[]",
        "null",
        "",
    ] {
        assert_eq!(
            decoder(4096).decode(text.as_bytes()).unwrap_err(),
            DecodeError::Malformed,
            "{text}"
        );
    }
    assert_eq!(
        decoder(1024).decode(&[0xff]).unwrap_err(),
        DecodeError::Malformed
    );
}

#[test]
fn errors_and_debug_do_not_contain_untrusted_arguments() {
    let raw = br#"{"v":2,"id":1,"cmd":"secret-command","args":{"secret":"sensitive-marker"}}"#;
    let f = decoder(1024).decode(raw).unwrap();
    assert!(!format!("{f:?}").contains("secret"));
    assert!(!format!("{f:?}").contains("sensitive-marker"));
    let e = decoder(1).decode(raw).unwrap_err();
    assert!(!format!("{e:?}: {e}").contains("secret"));
}

#[test]
fn protocol_negotiation_obeys_minimum_and_previous_version_only() {
    let p =
        ProtocolVersions::new(NonZeroU16::new(3).unwrap(), NonZeroU16::new(1).unwrap()).unwrap();
    assert_eq!(p.negotiate(&[1, 2, 3, 4]).unwrap().get(), 3);
    assert_eq!(p.negotiate(&[2]).unwrap().get(), 2);
    assert_eq!(p.negotiate(&[0, 1, 4]), None);
    let p =
        ProtocolVersions::new(NonZeroU16::new(3).unwrap(), NonZeroU16::new(3).unwrap()).unwrap();
    assert_eq!(p.negotiate(&[2]), None);
    assert!(
        ProtocolVersions::new(NonZeroU16::new(1).unwrap(), NonZeroU16::new(2).unwrap()).is_none()
    );
    let p =
        ProtocolVersions::new(NonZeroU16::new(1).unwrap(), NonZeroU16::new(1).unwrap()).unwrap();
    assert_eq!(p.negotiate(&[0]), None);
    assert_eq!(p.negotiate(&[1]).unwrap().get(), 1);
}
