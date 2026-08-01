//! White-box tests for the general pure JSON + TOML codec foundation (S-CODECS).
//!
//! Tests live here (M-797 as-touched), not inline in `src/codec.rs`.

use crate::codec::{
    decode_json, decode_toml, encode_json, encode_toml, json_to_value, parse_toml, toml_get,
    toml_get_bool, toml_get_i64, toml_get_optional, toml_get_str, value_to_json, Decode, Encode,
};
use crate::error::{SerError, TomlError};
use crate::serialize::{from_json, to_json};
use mycelium_core::{
    meta::{Meta, Provenance},
    repr::Repr,
    value::{Payload, Value},
};
use serde::{Deserialize, Serialize};

// ── Fixtures ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RelayConfig {
    token: String,
    poll_secs: u64,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct HookPayload {
    update_id: i64,
    text: String,
}

fn binary_value(bits: &[bool]) -> Value {
    Value::new(
        Repr::Binary {
            width: bits.len() as u32,
        },
        Payload::Bits(bits.to_vec()),
        Meta::exact(Provenance::Root),
    )
    .expect("well-formed binary value")
}

// ── Value aliases ────────────────────────────────────────────────────────────

#[test]
fn value_to_json_matches_to_json() {
    let v = binary_value(&[true, false, true]);
    assert_eq!(
        value_to_json(&v).expect("value_to_json"),
        to_json(&v).expect("to_json"),
        "S-CODECS alias must match serialize::to_json"
    );
}

#[test]
fn json_to_value_matches_from_json() {
    let v = binary_value(&[false, true]);
    let text = to_json(&v).expect("to_json");
    assert_eq!(
        json_to_value(&text).expect("json_to_value"),
        from_json(&text).expect("from_json"),
    );
}

// ── General JSON ─────────────────────────────────────────────────────────────

#[test]
fn encode_decode_json_user_struct_round_trip() {
    let p = HookPayload {
        update_id: 42,
        text: "hello".to_owned(),
    };
    let text = encode_json(&p).expect("encode_json");
    let back: HookPayload = decode_json(&text).expect("decode_json");
    assert_eq!(p, back);
}

#[test]
fn encode_decode_via_traits() {
    let p = HookPayload {
        update_id: 7,
        text: "via trait".to_owned(),
    };
    let text = p.encode_json().expect("Encode::encode_json");
    let back = HookPayload::decode_json(&text).expect("Decode::decode_json");
    assert_eq!(p, back);
}

#[test]
fn decode_json_malformed_is_err_never_silent() {
    let err = decode_json::<HookPayload>("{ not json }").expect_err("must Err");
    match err {
        SerError::Malformed { .. } | SerError::Truncated { .. } => {}
        other => panic!("unexpected variant for garbage JSON: {other:?}"),
    }
}

#[test]
fn decode_json_missing_field_is_err() {
    // missing `text` — never a partial HookPayload (C1).
    let err = decode_json::<HookPayload>(r#"{"update_id": 1}"#).expect_err("must Err");
    let s = err.to_string();
    assert!(
        s.contains("text") || s.contains("missing") || matches!(err, SerError::OutOfDomain { .. }),
        "error must name the failure (got {s})"
    );
}

#[test]
fn encode_json_refuses_nan_never_silent_null() {
    #[derive(Serialize)]
    struct Nasty {
        x: f64,
    }
    let result = encode_json(&Nasty { x: f64::NAN });
    assert!(
        result.is_err(),
        "NaN must not encode to silent JSON null (C1/G2); got Ok({:?})",
        result.ok()
    );
    let err = result.expect_err("checked");
    match err {
        SerError::OutOfDomain { .. } | SerError::Malformed { .. } => {}
        other => panic!("expected OutOfDomain/Malformed for NaN, got {other:?}"),
    }
}

// ── TOML parse + get ─────────────────────────────────────────────────────────

const RELAY_TOML: &str = r#"
# Minimal relay.toml-shaped fixture for codec foundation tests.
[relay]
token = "secret-token"
poll_secs = 30
dry_run = false

[[hooks]]
url = "https://example.test/a"
enabled = true

[[hooks]]
url = "https://example.test/b"
enabled = false
"#;

#[test]
fn parse_toml_relay_shaped() {
    let doc = parse_toml(RELAY_TOML).expect("valid fixture");
    assert!(doc.get("relay").is_some());
}

#[test]
fn toml_get_nested_string_and_int() {
    let doc = parse_toml(RELAY_TOML).expect("parse");
    assert_eq!(
        toml_get_str(&doc, "relay.token").expect("token"),
        "secret-token"
    );
    assert_eq!(toml_get_i64(&doc, "relay.poll_secs").expect("poll"), 30);
    assert!(!toml_get_bool(&doc, "relay.dry_run").expect("dry_run"));
}

#[test]
fn toml_get_array_index() {
    let doc = parse_toml(RELAY_TOML).expect("parse");
    assert_eq!(
        toml_get_str(&doc, "hooks.0.url").expect("hook0"),
        "https://example.test/a"
    );
    assert_eq!(
        toml_get_str(&doc, "hooks.1.url").expect("hook1"),
        "https://example.test/b"
    );
}

#[test]
fn toml_get_missing_key_is_err_never_silent() {
    let doc = parse_toml(RELAY_TOML).expect("parse");
    let err = toml_get(&doc, "relay.absent").expect_err("missing must Err");
    match err {
        TomlError::MissingKey { missing, .. } => assert_eq!(missing, "absent"),
        other => panic!("expected MissingKey, got {other:?}"),
    }
}

#[test]
fn toml_get_type_mismatch_is_err() {
    let doc = parse_toml(RELAY_TOML).expect("parse");
    let err = toml_get_str(&doc, "relay.poll_secs").expect_err("int is not string");
    match err {
        TomlError::TypeMismatch {
            expected, found, ..
        } => {
            assert_eq!(expected, "string");
            assert_eq!(found, "integer");
        }
        other => panic!("expected TypeMismatch, got {other:?}"),
    }
}

#[test]
fn toml_get_optional_absent_is_none() {
    let doc = parse_toml(RELAY_TOML).expect("parse");
    assert!(toml_get_optional(&doc, "relay.absent")
        .expect("optional lookup")
        .is_none());
    assert!(toml_get_optional(&doc, "relay.token")
        .expect("present")
        .is_some());
}

#[test]
fn parse_toml_malformed_is_err() {
    let err = parse_toml("[[[ not valid").expect_err("must Err");
    match err {
        TomlError::Malformed { .. } | TomlError::Truncated { .. } => {}
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn decode_encode_toml_typed_round_trip() {
    let cfg = RelayConfig {
        token: "abc".to_owned(),
        poll_secs: 15,
        dry_run: true,
    };
    let text = encode_toml(&cfg).expect("encode_toml");
    let back: RelayConfig = decode_toml(&text).expect("decode_toml");
    assert_eq!(cfg, back);
}

#[test]
fn decode_toml_missing_required_field_is_err() {
    // token required; dry_run has serde default but token does not
    let err = decode_toml::<RelayConfig>("poll_secs = 1").expect_err("must Err");
    let s = err.to_string();
    assert!(!s.is_empty(), "error display must not be empty (G11)");
}
