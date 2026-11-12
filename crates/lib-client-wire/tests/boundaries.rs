//! Własności licznika i granice parsera, niezależne od transportu i wykonawcy.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_client_wire::{CommandDecoder, DecodeError, MAX_REQUEST_ID};
use proptest::prelude::*;
use serde_json::json;
use std::num::{NonZeroU16, NonZeroUsize};

fn decoder() -> CommandDecoder {
    CommandDecoder::new(
        NonZeroU16::new(1).unwrap(),
        NonZeroUsize::new(4096).unwrap(),
    )
}

fn request(id: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({"v":1,"id":id,"cmd":"sessions_list","args":{}})).unwrap()
}

proptest! {
    #[test]
    fn counters_never_accept_a_replayed_or_older_valid_number(id in 1..=MAX_REQUEST_ID, old in 1..=MAX_REQUEST_ID) {
        let mut d = decoder();
        d.decode(&request(id)).unwrap();
        let next = d.decode(&request(old));
        if old <= id { prop_assert_eq!(next.unwrap_err(),DecodeError::Replay); }
        else { prop_assert_eq!(next.unwrap().id,old); }
    }

    #[test]
    fn arbitrary_input_cannot_panic_or_bypass_the_byte_limit(bytes in proptest::collection::vec(any::<u8>(),0..8192)) {
        let result = decoder().decode(&bytes);
        if bytes.len() > 4096 { prop_assert_eq!(result.unwrap_err(),DecodeError::TooLarge); }
    }
}

#[test]
fn malformed_or_wrong_version_frames_do_not_consume_a_valid_counter() {
    let mut d = decoder();
    let wrong = br#"{"v":2,"id":999,"cmd":"sessions_list","args":{}}"#;
    assert_eq!(d.decode(wrong).unwrap_err(), DecodeError::Version);
    assert_eq!(d.decode(b"invalid").unwrap_err(), DecodeError::Malformed);
    assert_eq!(d.decode(&request(1)).unwrap().id, 1);
}

#[test]
fn deep_json_uses_serde_recursion_limit_without_disabling_it() {
    let raw = format!(
        r#"{{"v":1,"id":1,"cmd":"sessions_list","args":{{"value":{}0{}}}}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    assert_eq!(
        decoder().decode(raw.as_bytes()).unwrap_err(),
        DecodeError::Malformed
    );
}

#[test]
fn parser_preserves_unknown_command_for_host_to_deny_and_grants_no_authority() {
    let raw=br#"{"v":1,"id":1,"cmd":"not-a-registered-command","args":{"approved":true,"kind":"LocalWindow"}}"#;
    let f = decoder().decode(raw).unwrap();
    assert_eq!(f.cmd, "not-a-registered-command");
    assert_eq!(f.args["approved"], true);
    // To nadal zwykłe dane: brama musi odrzucić nieznaną komendę i nie ufać args jako dowodowi.
}
