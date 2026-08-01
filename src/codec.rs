//! Pure JSON + TOML codecs for general user types (S-CODECS / WP-5 L-IO).
//!
//! This module is the **general** codec foundation that ports need for domain
//! shapes (GitHub API JSON, Telegram payloads, `relay.toml` config). It sits
//! beside — not inside — the Value-specific surface in [`crate::serialize`]:
//!
//! | Surface | Module | Domain |
//! |---|---|---|
//! | `to_json` / `from_json` / `serialize` / `deserialize` | [`crate::serialize`] | `mycelium_core::Value` only |
//! | `value_to_json` / `json_to_value` | this module (aliases) | same Value path, S-CODECS names |
//! | `encode_json` / `decode_json` + [`Encode`] / [`Decode`] | this module | any `serde` type |
//! | `parse_toml` / `toml_get` / `encode_toml` / `decode_toml` | this module | config TOML |
//!
//! # Honesty (C1 / G2 — never-silent)
//! - Decode failures return a typed error with a locus or path — never a
//!   partially-filled value, never a silent default for a missing key.
//! - Encode failures (e.g. non-finite `f64` under `serde_json`) are `Err`, never
//!   a silent `null` or dropped field.
//! - `toml_get` returns `Err(TomlError::MissingKey)` when a path segment is
//!   absent — callers that want optional keys use [`toml_get_optional`].
//!
//! # Effects: none
//! Pure over in-memory text/bytes. No OS, no `wild`, no network (ADR-014 floor
//! stays in `std-sys`). Suitable for pure Rust unit tests and early parallel
//! WP-5 work while process/net wait on Tier-0.
//!
//! # Dependencies
//! - `serde` + `serde_json` (already in this crate for Value JSON)
//! - `toml` 0.8 (new; MIT OR Apache-2.0; crates.io only)
//!
//! # What this is not
//! - Not a replacement for the Value wire grammar (RFC-0001 §4.8) — that stays
//!   in [`crate::serialize`].
//! - Not a full schema-validated config crate — typed `decode_toml::<T>` is the
//!   foundation; domain validation belongs to the caller.

use mycelium_core::Value;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{FieldPath, SerError, TomlError};
use crate::serialize::{self, Format};

// ── Value ↔ JSON aliases (S-CODECS naming) ───────────────────────────────────

/// Project a [`Value`] to canonical compact JSON text.
///
/// S-CODECS name for [`crate::serialize::to_json`]. Identical semantics: refuses
/// non-finite `f64` with `Err(SerError::OutOfDomain)` (never silent `null`).
///
/// # Effects: none
#[inline]
pub fn value_to_json(v: &Value) -> Result<String, SerError> {
    serialize::to_json(v)
}

/// Recover a [`Value`] from canonical JSON text.
///
/// S-CODECS name for [`crate::serialize::from_json`].
///
/// # Effects: none
#[inline]
pub fn json_to_value(text: &str) -> Result<Value, SerError> {
    serialize::from_json(text)
}

/// Project a [`Value`] to JSON bytes (Wire/Json grammar container).
///
/// Thin alias of [`crate::serialize::serialize`] with [`Format::Json`].
///
/// # Effects: none
#[inline]
pub fn value_to_json_bytes(v: &Value) -> Result<Vec<u8>, SerError> {
    serialize::serialize(v, Format::Json)
}

/// Recover a [`Value`] from JSON bytes.
///
/// Thin alias of [`crate::serialize::deserialize`] with [`Format::Json`].
///
/// # Effects: none
#[inline]
pub fn json_bytes_to_value(bytes: &[u8]) -> Result<Value, SerError> {
    serialize::deserialize(bytes, Format::Json)
}

// ── General JSON (any serde type) ────────────────────────────────────────────

// ── Never-silent finite-float precheck ───────────────────────────────────────
//
// `serde_json` silently encodes `f32`/`f64` NaN and ±∞ as JSON `null` by default
// (lossy + identity-colliding). The Value path refuses those in `serialize.rs`;
// general encode must match that honesty (C1/G2). We walk the serde graph with a
// side-effect-free checker before emitting bytes.

mod finite_check {
    use serde::ser::{
        self, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
        SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
    };
    use std::fmt;

    use crate::error::{FieldPath, SerError};

    /// Walk `value` and return `Err(OutOfDomain)` on the first non-finite float.
    pub fn reject_non_finite<T: Serialize + ?Sized>(value: &T) -> Result<(), SerError> {
        value
            .serialize(FiniteChecker)
            .map_err(|e| match e {
                CheckError::NonFinite { kind } => SerError::OutOfDomain {
                    path: FieldPath::from_static("<json-encode>"),
                    why: format!(
                        "non-finite {kind} has no JSON representation                          (serde_json would silently emit null, losing NaN/±∞ and colliding identity);                          refused — never-silent (C1/G2)"
                    ),
                },
                CheckError::Other(msg) => SerError::Malformed {
                    at: crate::error::ByteOffset(0),
                    why: msg,
                },
            })
            .map(|_| ())
    }

    #[derive(Debug)]
    enum CheckError {
        NonFinite { kind: &'static str },
        Other(String),
    }

    impl fmt::Display for CheckError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                CheckError::NonFinite { kind } => write!(f, "non-finite {kind}"),
                CheckError::Other(s) => write!(f, "{s}"),
            }
        }
    }

    impl std::error::Error for CheckError {}

    impl ser::Error for CheckError {
        fn custom<T: fmt::Display>(msg: T) -> Self {
            CheckError::Other(msg.to_string())
        }
    }

    /// Serializer that discards structure and only checks float finiteness.
    struct FiniteChecker;

    macro_rules! ok_prim {
        ($($method:ident($ty:ty)),* $(,)?) => {
            $(
                fn $method(self, _v: $ty) -> Result<Self::Ok, Self::Error> {
                    Ok(())
                }
            )*
        };
    }

    impl Serializer for FiniteChecker {
        type Ok = ();
        type Error = CheckError;
        type SerializeSeq = Compound;
        type SerializeTuple = Compound;
        type SerializeTupleStruct = Compound;
        type SerializeTupleVariant = Compound;
        type SerializeMap = Compound;
        type SerializeStruct = Compound;
        type SerializeStructVariant = Compound;

        ok_prim! {
            serialize_bool(bool),
            serialize_i8(i8),
            serialize_i16(i16),
            serialize_i32(i32),
            serialize_i64(i64),
            serialize_u8(u8),
            serialize_u16(u16),
            serialize_u32(u32),
            serialize_u64(u64),
            serialize_char(char),
            serialize_str(&str),
            serialize_bytes(&[u8]),
        }

        fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
            if v.is_finite() {
                Ok(())
            } else {
                Err(CheckError::NonFinite { kind: "f32" })
            }
        }

        fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
            if v.is_finite() {
                Ok(())
            } else {
                Err(CheckError::NonFinite { kind: "f64" })
            }
        }

        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
            value.serialize(self)
        }

        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
        ) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_newtype_struct<T: Serialize + ?Sized>(
            self,
            _name: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            value.serialize(self)
        }

        fn serialize_newtype_variant<T: Serialize + ?Sized>(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error> {
            value.serialize(self)
        }

        fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            Ok(Compound)
        }

        fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Ok(Compound)
        }

        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Ok(Compound)
        }

        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Ok(Compound)
        }

        fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Ok(Compound)
        }

        fn serialize_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            Ok(Compound)
        }

        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            Ok(Compound)
        }
    }

    struct Compound;

    impl SerializeSeq for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_element<T: Serialize + ?Sized>(
            &mut self,
            value: &T,
        ) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl SerializeTuple for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_element<T: Serialize + ?Sized>(
            &mut self,
            value: &T,
        ) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl SerializeTupleStruct for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl SerializeTupleVariant for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl SerializeMap for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
            key.serialize(FiniteChecker)
        }
        fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl SerializeStruct for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_field<T: Serialize + ?Sized>(
            &mut self,
            _key: &'static str,
            value: &T,
        ) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl SerializeStructVariant for Compound {
        type Ok = ();
        type Error = CheckError;
        fn serialize_field<T: Serialize + ?Sized>(
            &mut self,
            _key: &'static str,
            value: &T,
        ) -> Result<(), Self::Error> {
            value.serialize(FiniteChecker)
        }
        fn end(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }
}

/// Encode any [`Serialize`] value to compact UTF-8 JSON text.
///
/// # Fallibility (never-silent)
/// Returns `Err(SerError)` when encoding fails. In particular, non-finite `f64`
/// (`NaN` / `±∞`) is refused by `serde_json` (no JSON number literal) — mapped
/// here to [`SerError::OutOfDomain`], never silently emitted as `null`.
///
/// # Effects: none
pub fn encode_json<T: Serialize + ?Sized>(value: &T) -> Result<String, SerError> {
    finite_check::reject_non_finite(value)?;
    serde_json::to_string(value).map_err(map_json_encode_error)
}

/// Encode any [`Serialize`] value to compact JSON bytes.
///
/// # Effects: none
pub fn encode_json_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, SerError> {
    finite_check::reject_non_finite(value)?;
    serde_json::to_vec(value).map_err(map_json_encode_error)
}

/// Decode a typed value from UTF-8 JSON text.
///
/// # Fallibility (never-silent)
/// Malformed JSON, type mismatches, and missing fields yield `Err(SerError)`
/// with a best-effort locus — never a partially-filled `T`.
///
/// # Effects: none
pub fn decode_json<T: DeserializeOwned>(text: &str) -> Result<T, SerError> {
    serde_json::from_str(text).map_err(|e| map_json_decode_error(e, text.as_bytes()))
}

/// Decode a typed value from JSON bytes.
///
/// # Effects: none
pub fn decode_json_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SerError> {
    serde_json::from_slice(bytes).map_err(|e| map_json_decode_error(e, bytes))
}

/// Types that can be encoded to JSON via this crate's general codec.
///
/// Blanket-implemented for every [`Serialize`] type. Documented as a stable
/// S-CODECS contract surface so ports can bound on `Encode` without naming
/// `serde` at every call site.
pub trait Encode {
    /// Encode `self` to compact JSON text (never-silent on failure).
    fn encode_json(&self) -> Result<String, SerError>;

    /// Encode `self` to compact JSON bytes.
    fn encode_json_bytes(&self) -> Result<Vec<u8>, SerError>;
}

impl<T: Serialize + ?Sized> Encode for T {
    fn encode_json(&self) -> Result<String, SerError> {
        encode_json(self)
    }

    fn encode_json_bytes(&self) -> Result<Vec<u8>, SerError> {
        encode_json_bytes(self)
    }
}

/// Types that can be decoded from JSON via this crate's general codec.
///
/// Blanket-implemented for every [`DeserializeOwned`] type.
pub trait Decode: Sized {
    /// Decode `Self` from UTF-8 JSON text (never-silent on failure).
    fn decode_json(text: &str) -> Result<Self, SerError>;

    /// Decode `Self` from JSON bytes.
    fn decode_json_slice(bytes: &[u8]) -> Result<Self, SerError>;
}

impl<T: DeserializeOwned> Decode for T {
    fn decode_json(text: &str) -> Result<Self, SerError> {
        decode_json(text)
    }

    fn decode_json_slice(bytes: &[u8]) -> Result<Self, SerError> {
        decode_json_slice(bytes)
    }
}

// ── TOML ─────────────────────────────────────────────────────────────────────

/// Owned TOML document value (re-export for call-site ergonomics).
///
/// Callers parse with [`parse_toml`] and walk with [`toml_get`]. Prefer typed
/// [`decode_toml`] when the schema is known (`relay.toml` structs, etc.).
pub type TomlValue = toml::Value;

/// Parse a TOML document from UTF-8 text into a [`TomlValue`] tree.
///
/// # Fallibility (never-silent)
/// Invalid TOML yields `Err(TomlError::Malformed)` (or `Truncated` when the
/// message indicates EOF). Never returns a partial table.
///
/// # Effects: none
///
/// # Example (config foundation)
/// ```
/// use mycelium_std_io::codec::{parse_toml, toml_get};
///
/// let doc = parse_toml(
///     r#"
///     [relay]
///     token = "secret"
///     poll_secs = 30
///     "#,
/// ).expect("fixture is valid TOML");
/// let token = toml_get(&doc, "relay.token").expect("key present");
/// assert_eq!(token.as_str(), Some("secret"));
/// ```
pub fn parse_toml(text: &str) -> Result<TomlValue, TomlError> {
    text.parse::<toml::Value>().map_err(map_toml_parse_error)
}

/// Look up a dotted path in a TOML value tree.
///
/// Path segments are separated by `.` (e.g. `"relay.token"`, `"hooks.0.url"`).
/// Array indices are decimal integers. Empty path returns `value` itself.
///
/// # Fallibility (never-silent)
/// - Missing key / out-of-range index → [`TomlError::MissingKey`]
/// - Indexing into a non-table/non-array → [`TomlError::TypeMismatch`]
///
/// There is **no** silent `None` default. For optional keys use
/// [`toml_get_optional`].
///
/// # Effects: none
pub fn toml_get<'a>(value: &'a TomlValue, path: &str) -> Result<&'a TomlValue, TomlError> {
    if path.is_empty() {
        return Ok(value);
    }
    let mut current = value;
    let mut walked = String::new();
    for segment in path.split('.') {
        if !walked.is_empty() {
            walked.push('.');
        }
        walked.push_str(segment);

        match current {
            TomlValue::Table(table) => match table.get(segment) {
                Some(next) => current = next,
                None => {
                    return Err(TomlError::MissingKey {
                        path: FieldPath(path.to_owned()),
                        missing: segment.to_owned(),
                    });
                }
            },
            TomlValue::Array(arr) => {
                let idx: usize = segment.parse().map_err(|_| TomlError::TypeMismatch {
                    path: FieldPath(walked.clone()),
                    expected: "array index (integer segment)".to_owned(),
                    found: format!("non-integer segment {segment:?}"),
                })?;
                match arr.get(idx) {
                    Some(next) => current = next,
                    None => {
                        return Err(TomlError::MissingKey {
                            path: FieldPath(path.to_owned()),
                            missing: segment.to_owned(),
                        });
                    }
                }
            }
            other => {
                return Err(TomlError::TypeMismatch {
                    path: FieldPath(if walked.len() == segment.len() {
                        segment.to_owned()
                    } else {
                        // parent path = walked without the last segment
                        walked[..walked.len() - segment.len() - 1].to_owned()
                    }),
                    expected: "table or array".to_owned(),
                    found: toml_type_name(other).to_owned(),
                });
            }
        }
    }
    Ok(current)
}

/// Like [`toml_get`], but returns `Ok(None)` when the **final** key is missing.
///
/// Intermediate type mismatches and malformed index segments remain `Err` —
/// only a clean "key not present on a table" (or OOB array index at the leaf)
/// becomes `None`. Intermediate missing keys are still errors so typos in a
/// parent path do not silently look like "optional absent".
///
/// # Effects: none
pub fn toml_get_optional<'a>(
    value: &'a TomlValue,
    path: &str,
) -> Result<Option<&'a TomlValue>, TomlError> {
    match toml_get(value, path) {
        Ok(v) => Ok(Some(v)),
        Err(TomlError::MissingKey { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Require a string at `path` (never-silent type mismatch).
///
/// # Effects: none
pub fn toml_get_str<'a>(value: &'a TomlValue, path: &str) -> Result<&'a str, TomlError> {
    let v = toml_get(value, path)?;
    v.as_str().ok_or_else(|| TomlError::TypeMismatch {
        path: FieldPath(path.to_owned()),
        expected: "string".to_owned(),
        found: toml_type_name(v).to_owned(),
    })
}

/// Require an integer at `path` (never-silent type mismatch).
///
/// # Effects: none
pub fn toml_get_i64(value: &TomlValue, path: &str) -> Result<i64, TomlError> {
    let v = toml_get(value, path)?;
    v.as_integer().ok_or_else(|| TomlError::TypeMismatch {
        path: FieldPath(path.to_owned()),
        expected: "integer".to_owned(),
        found: toml_type_name(v).to_owned(),
    })
}

/// Require a boolean at `path` (never-silent type mismatch).
///
/// # Effects: none
pub fn toml_get_bool(value: &TomlValue, path: &str) -> Result<bool, TomlError> {
    let v = toml_get(value, path)?;
    v.as_bool().ok_or_else(|| TomlError::TypeMismatch {
        path: FieldPath(path.to_owned()),
        expected: "boolean".to_owned(),
        found: toml_type_name(v).to_owned(),
    })
}

/// Decode a typed value from TOML text (`serde::Deserialize`).
///
/// Preferred path for `relay.toml`-shaped configs: define a struct, derive
/// `Deserialize`, call `decode_toml`. Missing fields / type errors are
/// `Err(TomlError)` — never silent defaults unless the type uses
/// `#[serde(default)]` explicitly (caller's choice, reified in the type).
///
/// # Effects: none
pub fn decode_toml<T: DeserializeOwned>(text: &str) -> Result<T, TomlError> {
    toml::from_str(text).map_err(map_toml_de_error)
}

/// Encode a typed value to TOML text (`serde::Serialize`).
///
/// # Effects: none
pub fn encode_toml<T: Serialize + ?Sized>(value: &T) -> Result<String, TomlError> {
    toml::to_string(value).map_err(|e| TomlError::Encode { why: e.to_string() })
}

/// Encode a typed value to a pretty (multi-line) TOML document.
///
/// # Effects: none
pub fn encode_toml_pretty<T: Serialize + ?Sized>(value: &T) -> Result<String, TomlError> {
    toml::to_string_pretty(value).map_err(|e| TomlError::Encode { why: e.to_string() })
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn toml_type_name(v: &TomlValue) -> &'static str {
    match v {
        TomlValue::String(_) => "string",
        TomlValue::Integer(_) => "integer",
        TomlValue::Float(_) => "float",
        TomlValue::Boolean(_) => "boolean",
        TomlValue::Datetime(_) => "datetime",
        TomlValue::Array(_) => "array",
        TomlValue::Table(_) => "table",
    }
}

fn map_toml_parse_error(e: toml::de::Error) -> TomlError {
    let why = e.to_string();
    let lower = why.to_lowercase();
    if lower.contains("eof") || lower.contains("unexpected end") {
        TomlError::Truncated { why }
    } else {
        TomlError::Malformed { why }
    }
}

fn map_toml_de_error(e: toml::de::Error) -> TomlError {
    let why = e.to_string();
    let lower = why.to_lowercase();
    if lower.contains("missing field") {
        // Extract field name when present: "missing field `token`"
        let missing = extract_backticked(&why).unwrap_or_else(|| why.clone());
        TomlError::MissingKey {
            path: FieldPath(missing.clone()),
            missing,
        }
    } else if lower.contains("eof") || lower.contains("unexpected end") {
        TomlError::Truncated { why }
    } else if lower.contains("invalid type") || lower.contains("expected") {
        TomlError::OutOfDomain {
            path: FieldPath::from_static("<toml>"),
            why,
        }
    } else {
        TomlError::Malformed { why }
    }
}

fn extract_backticked(msg: &str) -> Option<String> {
    let start = msg.find('`')?;
    let rest = &msg[start + 1..];
    let end = rest.find('`')?;
    Some(rest[..end].to_owned())
}

fn map_json_encode_error(e: serde_json::Error) -> SerError {
    let msg = e.to_string();
    let lower = msg.to_lowercase();
    // serde_json refuses NaN/±∞ with a message about a number / infinite / NaN.
    if lower.contains("nan")
        || lower.contains("infinit")
        || lower.contains("not a number")
        || lower.contains("cannot serialize")
    {
        return SerError::OutOfDomain {
            path: FieldPath::from_static("<json-encode>"),
            why: format!(
                "{msg}; refused — non-finite or non-JSON-representable value \
                 (never silent null; C1/G2)"
            ),
        };
    }
    SerError::Malformed {
        at: crate::error::ByteOffset(0),
        why: msg,
    }
}

fn map_json_decode_error(e: serde_json::Error, input: &[u8]) -> SerError {
    // Reuse the same locus / classification discipline as Value JSON by mapping
    // through the shared approximate-offset logic (duplicated lightly here so
    // codec stays independent of serialize internals that are private).
    let byte_offset = crate::error::ByteOffset(approx_byte_offset(input, e.line(), e.column()));
    let msg = e.to_string();
    let lower = msg.to_lowercase();

    if input.is_empty() || lower.contains("eof") || lower.contains("unexpected end") {
        SerError::Truncated { at: byte_offset }
    } else if lower.contains("missing field") {
        let field = extract_backticked(&msg).unwrap_or_else(|| "<unknown>".to_owned());
        SerError::OutOfDomain {
            path: FieldPath(field),
            why: msg,
        }
    } else if msg.contains("unknown variant") || msg.contains("unknown field") {
        SerError::UnknownTag {
            path: FieldPath::from_static("<json>"),
            tag: extract_backticked(&msg).unwrap_or_else(|| msg.clone()),
        }
    } else {
        SerError::Malformed {
            at: byte_offset,
            why: msg,
        }
    }
}

fn approx_byte_offset(input: &[u8], line: usize, col: usize) -> u64 {
    if input.is_empty() || line == 0 {
        return input.len() as u64;
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    for (i, &b) in input.iter().enumerate() {
        if current_line == line {
            let col_offset = col.saturating_sub(1);
            return (line_start + col_offset).min(input.len()) as u64;
        }
        if b == b'\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    input.len() as u64
}
