//! Typed-prim entry points for `serialize` (S-STD-IO-TYPED-PRIMS, PKG-LINKAGE mycelium-lang#44).
//!
//! `feature = "typed-prims"` only — pulls in `mycelium-interp`'s checked `prim:` schema
//! ([`mycelium_interp::typed::PrimSig`]/`TySpec`/`TypedPrimRegistry`, landed on `mycelium-runtime`
//! main via runtime#18 + the owned-`name`/width-carrying-`Float` correction #19). This is the
//! keystone's payload deliverable: it un-stubs `io.myc`'s FLAG-io-2 residual ("Value codec not
//! ported to .myc") by giving `to_json`/`from_json`/`serialize`/`deserialize` a real, checked call
//! path that never requires `Value` or `Repr` to be a `.myc`-nameable type — the conversion between
//! a checked argument's runtime [`mycelium_core::Value`] and the wire form happens here, invisibly
//! to the `.myc` caller (S-STD-IO-TYPED-PRIMS rationale).
//!
//! **This module writes no new codec.** `src/serialize.rs`'s pure, tested `Value <-> JSON` codec
//! (proptest round-trip, never-silent `NaN`/`Inf` refusal) is unchanged; every `PrimFn` below calls
//! straight through to it.
//!
//! # Honesty ledger — which of the four entry points get an *exact* [`TySpec`], and which do not
//!
//! [`TySpec`] v0 is deliberately monomorphic (a concrete literal only — see its own doc) and may
//! never name `Value`/`Repr` (the self-hosting-leak scope guard both this crate's package and
//! `mycelium-runtime`'s module doc state). `to_json`/`serialize`/`from_json`/`deserialize` are each
//! effectively polymorphic over **any** well-formed `Value`, so **no single `PrimSig` can honestly
//! claim to cover the fully generic Rust signature** — claiming one would be exactly the
//! trusted-not-verified failure S-STD-IO-TYPED-PRIMS exists to remove. This module resolves that
//! the way the package's own risk note proposes: **register one `PrimSig` per concretely-
//! instantiated shape actually exercised by this module's own tests**, using [`PrimSig::name`]'s
//! owned-`String` correction (#19) to generate a distinct dispatch key per instantiation — never by
//! widening `TySpec` itself. Two shapes are registered here: the canonical `Repr::Bytes` (an
//! unsuffixed name, e.g. `std.io.serialize.to_json`) and `Repr::Binary{width:8}` (a `.binary8`-
//! suffixed name), the same shape this crate's own `lib.rs` test fixture (`binary_value()`) already
//! exercises. Both are *fixed, hardcoded* literals — this module does not generate names at
//! runtime, so every dispatch key here is a plain `&'static str`, but the mechanism (owned
//! `PrimSig::name`) is proven reusable for a future provider that does need runtime-generated names
//! (see `mycelium-runtime`'s own `signature_names_may_be_generated_at_runtime` test).
//!
//! Given those two fixed instantiations, the four entry points split honestly into two classes:
//!
//! - **`to_json` / `serialize`** — the **param** `TySpec` (the promised input shape) is exact by
//!   construction: it is one of the two registered literals, and each `PrimFn` below defensively
//!   re-checks the argument's actual runtime `Repr` against it before calling through (never trusts
//!   the checker's own enforcement alone — belt-and-suspenders, since this crate does not own the
//!   `mycelium-l1` checker that is supposed to guarantee it). The **result** `TySpec` (`Bytes`,
//!   standing in for `to_json`'s `String` — `TySpec` has no dedicated text tag, and a `String`'s
//!   bytes *are* a byte string, so `Bytes` is an honest, non-overclaiming mirror) is *also* exact
//!   and **unconditionally so** for both registered input shapes: `serialize.rs`'s own
//!   `check_json_representable` only refuses a non-finite `f64` in a `Scalars`/`Hypervector`/`Seq`
//!   payload, and neither `Bytes` (payload `Payload::Bytes`) nor `Binary` (payload `Payload::Bits`)
//!   ever carries an `f64` — so, restricted to these two registered shapes, `to_json`/`serialize`
//!   are **total**, not just fallible-but-checked. `Err` is still handled (never a panic), it is
//!   just unreachable in practice for the registered domain.
//!
//! - **`from_json` / `deserialize`** — the **param** (`Bytes`, the JSON/wire text as raw bytes) is
//!   exact for the same reason as above. The **result**, however, is **not** honestly exact in the
//!   same unconditional sense: `mycelium_core::Value`'s `serde` form is self-describing (the `Repr`
//!   tag travels with the data, M-104), so the *decoded* value's actual shape is determined by the
//!   **input bytes' content**, not by anything the type system can see ahead of time. No `PrimSig`
//!   for a truly general `from_json`/`deserialize` could honestly claim "always produces exactly
//!   this `TySpec`" as a statically-provable fact — declaring one would be precisely the wrong-
//!   signature failure this surface exists to prevent. This module's honest position: the two
//!   registered result shapes are **checked runtime postconditions**, not provable preconditions —
//!   each `PrimFn` decodes, then explicitly verifies the decoded `Value`'s `Repr` matches the
//!   registered `TySpec` *before* returning `Ok`, and returns a distinct, located
//!   `EvalError::PrimType` (never a silently-mistyped `Value`, never a panic) on any mismatch. This
//!   mirrors the crate's own pre-existing guarantee tagging in `serialize.rs`: `to_json`/`serialize`
//!   are already documented there as `Exact`; `from_json`/`deserialize` are already documented
//!   there as `Empirical` (round-trip, not a checked theorem) — this module's `GuaranteeStrength`
//!   choice below is that same pre-existing distinction, not a new judgment call.
//!
//! In short: **`to_json`/`serialize` are exact; `from_json`/`deserialize` are checked-but-not-
//! provable**, and this module says so in both the `GuaranteeStrength` it registers and in this
//! doc comment, rather than papering over the difference with a uniform `Exact` tag.
//!
//! # Format: only `Wire` is exposed under `serialize`/`deserialize`
//!
//! `serialize.rs`'s own `Format::Wire` and `Format::Json` arms produce **byte-identical output**
//! (documented in `serialize()`'s own body comment: "the two arms intentionally produce identical
//! bytes") and `deserialize()` does not even branch on its `_format` parameter. There is therefore
//! no second, distinct `Format::Json`-flavoured `serialize`/`deserialize` prim to add beyond
//! `to_json`/`from_json` themselves (which already are the `Format::Json` + `String`-container
//! specialisation, exactly as `serialize.rs`'s own module doc describes them: "the one canonical
//! JSON projection"). This module's `serialize`/`deserialize` prims fix `Format::Wire`.

use mycelium_core::{Meta, Payload, Provenance, Repr, Value};
use mycelium_interp::prims::PrimFn;
use mycelium_interp::typed::{PrimSig, TySpec, TypedPrimRegistry, WidthSpec};
use mycelium_interp::EvalError;

use crate::serialize::{deserialize, from_json, serialize, to_json, Format};

// ── Dispatch-key names (frozen: canonical + one width-instantiated pair per op) ──────────────────

const TO_JSON_BYTES: &str = "std.io.serialize.to_json";
const TO_JSON_BINARY8: &str = "std.io.serialize.to_json.binary8";
const SERIALIZE_BYTES: &str = "std.io.serialize.serialize";
const SERIALIZE_BINARY8: &str = "std.io.serialize.serialize.binary8";
const FROM_JSON_BYTES: &str = "std.io.serialize.from_json";
const FROM_JSON_BINARY8: &str = "std.io.serialize.from_json.binary8";
const DESERIALIZE_BYTES: &str = "std.io.serialize.deserialize";
const DESERIALIZE_BINARY8: &str = "std.io.serialize.deserialize.binary8";

// ── Shared helpers ─────────────────────────────────────────────────────────────────────────────

/// Extract exactly one argument or refuse with a located, named [`EvalError::PrimType`] (never a
/// panic on the wrong arity).
fn arg1<'a>(prim: &str, args: &[&'a Value]) -> Result<&'a Value, EvalError> {
    match args {
        [v] => Ok(*v),
        other => Err(EvalError::PrimType {
            prim: prim.to_owned(),
            why: format!("expects exactly 1 argument, got {}", other.len()),
        }),
    }
}

/// Defensively re-check `v`'s actual runtime [`Repr`] against the shape this dispatch key
/// promises. This is a **belt-and-suspenders** check: `mycelium-l1`'s checker (a separate lane,
/// S-TYPED-PRIM-CALL-CHECK) is supposed to guarantee the call site's argument already has this
/// shape before the `Op` ever reaches this `PrimFn` — this crate does not own that checker, so it
/// does not trust that alone. A mismatch is a distinct, located error, never a silent narrow/widen.
fn expect_repr(prim: &str, v: &Value, ok: bool, want: &str) -> Result<(), EvalError> {
    if ok {
        Ok(())
    } else {
        Err(EvalError::PrimType {
            prim: prim.to_owned(),
            why: format!("expected {want}, got {:?}", v.repr()),
        })
    }
}

fn is_bytes(r: &Repr) -> bool {
    matches!(r, Repr::Bytes)
}

fn is_binary8(r: &Repr) -> bool {
    matches!(r, Repr::Binary { width: 8 })
}

/// Wrap raw bytes as a `Repr::Bytes` [`Value`] (never fails: `Payload::Bytes` is well-formed for
/// any byte content — RFC-0032 D4).
fn bytes_value(bytes: Vec<u8>) -> Value {
    Value::new(
        Repr::Bytes,
        Payload::Bytes(bytes),
        Meta::exact(Provenance::Root),
    )
    .expect("Repr::Bytes + Payload::Bytes is well-formed for any byte content (RFC-0032 D4)")
}

/// The raw bytes of a `Repr::Bytes` [`Value`] (caller has already `expect_repr`-checked the shape).
fn as_bytes(v: &Value) -> &[u8] {
    match v.payload() {
        Payload::Bytes(b) => b,
        other => unreachable!("expect_repr(Repr::Bytes) guarantees Payload::Bytes, got {other:?}"),
    }
}

// ── to_json ────────────────────────────────────────────────────────────────────────────────────

fn to_json_prim(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let v = arg1(prim, args)?;
    let text = to_json(v).map_err(|e| EvalError::PrimType {
        prim: prim.to_owned(),
        why: e.to_string(),
    })?;
    Ok(bytes_value(text.into_bytes()))
}

fn prim_to_json_bytes(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let v = arg1(prim, args)?;
    expect_repr(prim, v, is_bytes(v.repr()), "Repr::Bytes")?;
    to_json_prim(prim, args)
}

fn prim_to_json_binary8(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let v = arg1(prim, args)?;
    expect_repr(prim, v, is_binary8(v.repr()), "Repr::Binary{width:8}")?;
    to_json_prim(prim, args)
}

// ── serialize (Format::Wire) ──────────────────────────────────────────────────────────────────

fn serialize_prim(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let v = arg1(prim, args)?;
    let bytes = serialize(v, Format::Wire).map_err(|e| EvalError::PrimType {
        prim: prim.to_owned(),
        why: e.to_string(),
    })?;
    Ok(bytes_value(bytes))
}

fn prim_serialize_bytes(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let v = arg1(prim, args)?;
    expect_repr(prim, v, is_bytes(v.repr()), "Repr::Bytes")?;
    serialize_prim(prim, args)
}

fn prim_serialize_binary8(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let v = arg1(prim, args)?;
    expect_repr(prim, v, is_binary8(v.repr()), "Repr::Binary{width:8}")?;
    serialize_prim(prim, args)
}

// ── from_json (result is a checked postcondition, not a provable precondition — see module doc)──

fn from_json_prim(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let text_v = arg1(prim, args)?;
    expect_repr(
        prim,
        text_v,
        is_bytes(text_v.repr()),
        "Repr::Bytes (UTF-8 JSON text)",
    )?;
    let text = std::str::from_utf8(as_bytes(text_v)).map_err(|e| EvalError::PrimType {
        prim: prim.to_owned(),
        why: format!("from_json input is not valid UTF-8: {e}"),
    })?;
    from_json(text).map_err(|e| EvalError::PrimType {
        prim: prim.to_owned(),
        why: e.to_string(),
    })
}

fn prim_from_json_bytes(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let decoded = from_json_prim(prim, args)?;
    expect_repr(
        prim,
        &decoded,
        is_bytes(decoded.repr()),
        "the promised Repr::Bytes (postcondition refused, never silently returned)",
    )?;
    Ok(decoded)
}

fn prim_from_json_binary8(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let decoded = from_json_prim(prim, args)?;
    expect_repr(
        prim,
        &decoded,
        is_binary8(decoded.repr()),
        "the promised Repr::Binary{width:8} (postcondition refused, never silently returned)",
    )?;
    Ok(decoded)
}

// ── deserialize (Format::Wire; same postcondition discipline as from_json) ───────────────────────

fn deserialize_prim(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let bytes_v = arg1(prim, args)?;
    expect_repr(
        prim,
        bytes_v,
        is_bytes(bytes_v.repr()),
        "Repr::Bytes (wire bytes)",
    )?;
    deserialize(as_bytes(bytes_v), Format::Wire).map_err(|e| EvalError::PrimType {
        prim: prim.to_owned(),
        why: e.to_string(),
    })
}

fn prim_deserialize_bytes(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let decoded = deserialize_prim(prim, args)?;
    expect_repr(
        prim,
        &decoded,
        is_bytes(decoded.repr()),
        "the promised Repr::Bytes (postcondition refused, never silently returned)",
    )?;
    Ok(decoded)
}

fn prim_deserialize_binary8(prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let decoded = deserialize_prim(prim, args)?;
    expect_repr(
        prim,
        &decoded,
        is_binary8(decoded.repr()),
        "the promised Repr::Binary{width:8} (postcondition refused, never silently returned)",
    )?;
    Ok(decoded)
}

// ── S-STD-IO-TYPED-PRIMS frozen surface ───────────────────────────────────────────────────────

/// Every registered signature this crate contributes, keyed by its dispatch name (bare, no
/// `prim:` prefix — [`mycelium_interp::typed::TypedPrimRegistry::register_typed`] accepts bare or
/// prefixed). Exposed separately from [`install_typed_prims`] so a checker (`mycelium-l1`'s
/// `TypedPrimEnv`, S-TYPED-PRIM-ENV) can resolve `use std_io::serialize.to_json` at `myc check`
/// time without needing a live [`TypedPrimRegistry`]/[`PrimFn`] table.
#[must_use]
pub fn typed_prim_sigs() -> Vec<(&'static str, PrimSig)> {
    vec![
        (
            TO_JSON_BYTES,
            PrimSig {
                name: TO_JSON_BYTES.to_owned(),
                params: vec![TySpec::Bytes],
                ret: TySpec::Bytes,
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Exact,
            },
        ),
        (
            TO_JSON_BINARY8,
            PrimSig {
                name: TO_JSON_BINARY8.to_owned(),
                params: vec![TySpec::Binary(WidthSpec(8))],
                ret: TySpec::Bytes,
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Exact,
            },
        ),
        (
            SERIALIZE_BYTES,
            PrimSig {
                name: SERIALIZE_BYTES.to_owned(),
                params: vec![TySpec::Bytes],
                ret: TySpec::Bytes,
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Exact,
            },
        ),
        (
            SERIALIZE_BINARY8,
            PrimSig {
                name: SERIALIZE_BINARY8.to_owned(),
                params: vec![TySpec::Binary(WidthSpec(8))],
                ret: TySpec::Bytes,
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Exact,
            },
        ),
        (
            FROM_JSON_BYTES,
            PrimSig {
                name: FROM_JSON_BYTES.to_owned(),
                params: vec![TySpec::Bytes],
                ret: TySpec::Bytes,
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Empirical,
            },
        ),
        (
            FROM_JSON_BINARY8,
            PrimSig {
                name: FROM_JSON_BINARY8.to_owned(),
                params: vec![TySpec::Bytes],
                ret: TySpec::Binary(WidthSpec(8)),
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Empirical,
            },
        ),
        (
            DESERIALIZE_BYTES,
            PrimSig {
                name: DESERIALIZE_BYTES.to_owned(),
                params: vec![TySpec::Bytes],
                ret: TySpec::Bytes,
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Empirical,
            },
        ),
        (
            DESERIALIZE_BINARY8,
            PrimSig {
                name: DESERIALIZE_BINARY8.to_owned(),
                params: vec![TySpec::Bytes],
                ret: TySpec::Binary(WidthSpec(8)),
                effects: vec![],
                guarantee: mycelium_core::GuaranteeStrength::Empirical,
            },
        ),
    ]
}

/// Register this crate's typed prims into `reg` (S-STD-IO-TYPED-PRIMS frozen signature:
/// `install_typed_prims(reg: &mut TypedPrimRegistry)`). All eight entries are **pure — `effects`
/// is empty** (a caller needs no `!{ffi}`/`!{net}`; that is the entire point of the typed path
/// over `wild`), matching `serialize.rs`'s own documented "Effects: none" for every function
/// wrapped here.
pub fn install_typed_prims(reg: &mut TypedPrimRegistry) {
    let sigs = typed_prim_sigs();
    let fns: [PrimFn; 8] = [
        prim_to_json_bytes,
        prim_to_json_binary8,
        prim_serialize_bytes,
        prim_serialize_binary8,
        prim_from_json_bytes,
        prim_from_json_binary8,
        prim_deserialize_bytes,
        prim_deserialize_binary8,
    ];
    for ((name, sig), f) in sigs.into_iter().zip(fns) {
        reg.register_typed(name, sig, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary8_value(byte: u8) -> Value {
        let bits: Vec<bool> = (0..8).rev().map(|i| (byte >> i) & 1 == 1).collect();
        Value::new(
            Repr::Binary { width: 8 },
            Payload::Bits(bits),
            Meta::exact(Provenance::Root),
        )
        .expect("well-formed Binary{8} value")
    }

    fn dispatch(reg: &TypedPrimRegistry, prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
        let (_, f) = reg
            .get_typed(prim)
            .ok_or_else(|| EvalError::UnknownPrim(prim.to_owned()))?;
        f(prim, args)
    }

    /// The registry is empty-by-design until `install_typed_prims` runs (mirrors
    /// `mycelium-interp`'s own `TypedPrimRegistry` empty-by-default posture).
    #[test]
    fn registry_is_empty_before_install() {
        let reg = TypedPrimRegistry::empty();
        assert!(!reg.has_typed(TO_JSON_BYTES));
        assert_eq!(reg.sigs().count(), 0);
    }

    /// All eight declared signatures land under their exact frozen names, and every one is pure
    /// (`effects` empty — a typed-prim caller needs no `!{ffi}`).
    #[test]
    fn install_registers_all_eight_pure_signatures() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);
        assert_eq!(reg.sigs().count(), 8);
        for name in [
            TO_JSON_BYTES,
            TO_JSON_BINARY8,
            SERIALIZE_BYTES,
            SERIALIZE_BINARY8,
            FROM_JSON_BYTES,
            FROM_JSON_BINARY8,
            DESERIALIZE_BYTES,
            DESERIALIZE_BINARY8,
        ] {
            assert!(reg.has_typed(name), "missing {name}");
            let (sig, _) = reg.get_typed(name).unwrap();
            assert!(sig.effects.is_empty(), "{name} must be pure (no effects)");
        }
    }

    /// Dispatching `prim:std.io.serialize.to_json` through the registry is byte-identical to
    /// calling `mycelium_std_io::serialize::to_json` directly (S-STD-IO-TYPED-PRIMS contract test
    /// — output equality asserted, not just `Ok(_)`).
    #[test]
    fn to_json_dispatch_matches_direct_call() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);

        let v = binary8_value(0b1011_0010);
        let direct = to_json(&v).expect("direct to_json");

        let via_prim = dispatch(&reg, "prim:std.io.serialize.to_json.binary8", &[&v])
            .expect("prim: dispatch to_json.binary8");
        let via_bytes = match via_prim.payload() {
            Payload::Bytes(b) => b.clone(),
            other => panic!("expected Repr::Bytes payload, got {other:?}"),
        };
        assert_eq!(
            via_bytes,
            direct.into_bytes(),
            "prim:-dispatched to_json must be byte-identical to the direct Rust call"
        );
    }

    /// `from_json` round-trips through the checked prim path exactly like the direct call, and the
    /// Binary{8}-postcondition is actually verified (not just asserted in a doc comment).
    #[test]
    fn from_json_round_trips_and_enforces_postcondition() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);

        let v = binary8_value(0b1011_0010);
        let text = to_json(&v).expect("to_json");
        let text_value = bytes_value(text.into_bytes());

        let decoded = dispatch(
            &reg,
            "prim:std.io.serialize.from_json.binary8",
            &[&text_value],
        )
        .expect("prim: dispatch from_json.binary8");
        assert_eq!(
            decoded, v,
            "round-trip must recover the original value exactly"
        );
    }

    /// A `from_json.binary8` call whose JSON text decodes to a *different* shape (here `Bytes`,
    /// not `Binary{8}`) is refused with a distinct, located error — never a silently mistyped
    /// `Value` (the postcondition this module's honesty ledger describes).
    #[test]
    fn from_json_binary8_refuses_a_mismatched_decoded_shape() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);

        let bytes_val = bytes_value(vec![1, 2, 3]);
        let text = to_json(&bytes_val).expect("to_json of a Bytes value");
        let text_value = bytes_value(text.into_bytes());

        let err = dispatch(
            &reg,
            "prim:std.io.serialize.from_json.binary8",
            &[&text_value],
        )
        .expect_err("decoded Bytes shape must refuse the Binary{8} postcondition");
        assert!(
            matches!(&err, EvalError::PrimType { prim, .. } if prim == "prim:std.io.serialize.from_json.binary8"),
            "expected a located PrimType refusal, got {err:?}"
        );
    }

    /// A `to_json.binary8` call site handed a `Bytes`-shaped argument (the wrong registered
    /// param shape) is refused defensively, never silently accepted or misinterpreted.
    #[test]
    fn to_json_binary8_refuses_a_bytes_argument() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);

        let bytes_val = bytes_value(vec![9, 9, 9]);
        let err = dispatch(&reg, "prim:std.io.serialize.to_json.binary8", &[&bytes_val])
            .expect_err("a Bytes argument must refuse the Binary{8}-only prim");
        assert!(
            matches!(&err, EvalError::PrimType { .. }),
            "expected PrimType, got {err:?}"
        );
    }

    /// `serialize`/`deserialize` (Format::Wire) round-trip through the checked prim path too, and
    /// `Wire` output is byte-identical to `to_json`'s output for the same value (both formats share
    /// one grammar — `serialize.rs`'s own documented fact).
    #[test]
    fn serialize_deserialize_round_trip_and_match_to_json_bytes() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);

        let v = binary8_value(0b0110_1101);
        let ser = dispatch(&reg, "prim:std.io.serialize.serialize.binary8", &[&v])
            .expect("prim: dispatch serialize.binary8");
        let json = dispatch(&reg, "prim:std.io.serialize.to_json.binary8", &[&v])
            .expect("prim: dispatch to_json.binary8");
        assert_eq!(
            ser, json,
            "Wire and Json share one grammar (serialize.rs's own documented fact)"
        );

        let round = dispatch(&reg, "prim:std.io.serialize.deserialize.binary8", &[&ser])
            .expect("prim: dispatch deserialize.binary8");
        assert_eq!(
            round, v,
            "serialize -> deserialize must recover the original value"
        );
    }

    /// An unknown `prim:` name is a loud, typed miss (mirrors `mycelium-interp`'s own
    /// `unknown_typed_prim_is_loud_miss`) — never a panic, never a silent no-op.
    #[test]
    fn unknown_typed_prim_name_is_loud_miss() {
        let mut reg = TypedPrimRegistry::empty();
        install_typed_prims(&mut reg);
        let v = binary8_value(0);
        let err = dispatch(&reg, "prim:std.io.serialize.not_a_real_prim", &[&v])
            .expect_err("unknown prim: must miss");
        assert!(
            matches!(&err, EvalError::UnknownPrim(p) if p == "prim:std.io.serialize.not_a_real_prim")
        );
    }
}
