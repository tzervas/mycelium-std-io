//! Contract test for S-STD-IO-TYPED-PRIMS (PKG-LINKAGE, mycelium-lang#44) — `feature =
//! "typed-prims"` only. Dispatches `Node::Op{prim:"prim:std.io.serialize.to_json"}`'s runtime
//! analogue (a direct `TypedPrimRegistry::get_typed` lookup + call, since routing real `Node::Op`
//! IR through the registry is `mycelium-l1`'s S-TYPED-PRIM-ENV/S-TYPED-PRIM-CALL-CHECK — a
//! separate lane, not this crate's) through [`mycelium_std_io::typed_prims::install_typed_prims`]
//! and asserts the result is **byte-identical** to calling
//! [`mycelium_std_io::serialize::to_json`] directly — round-trip parity, not just `Ok(_)`
//! (the package's own "done when" wording for this lane).
//!
//! Only this crate's `pub` surface is used here (an integration test, unlike `src/typed_prims.rs`'s
//! own inline unit tests, which may see crate-private helpers).

#![cfg(feature = "typed-prims")]

use mycelium_core::{Meta, Payload, Provenance, Repr, Value};
use mycelium_interp::typed::TypedPrimRegistry;
use mycelium_interp::EvalError;
use mycelium_std_io::serialize::to_json;
use mycelium_std_io::typed_prims::{install_typed_prims, typed_prim_sigs};

fn bytes_fixture() -> Value {
    Value::new(
        Repr::Bytes,
        Payload::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        Meta::exact(Provenance::Root),
    )
    .expect("well-formed Repr::Bytes fixture")
}

fn dispatch(reg: &TypedPrimRegistry, prim: &str, args: &[&Value]) -> Result<Value, EvalError> {
    let (_, f) = reg
        .get_typed(prim)
        .ok_or_else(|| EvalError::UnknownPrim(prim.to_owned()))?;
    f(prim, args)
}

/// `prim:std.io.serialize.to_json` dispatched through the registry is byte-identical to calling
/// `mycelium_std_io::serialize::to_json` directly (S-STD-IO-TYPED-PRIMS' own contract-test wording,
/// verbatim: `prim:"std.io.serialize.to_json"`).
#[test]
fn to_json_via_typed_registry_is_byte_identical_to_direct_call() {
    let mut reg = TypedPrimRegistry::empty();
    install_typed_prims(&mut reg);

    let fixture = bytes_fixture();
    let direct = to_json(&fixture).expect("direct to_json call");

    let via_prim = dispatch(&reg, "prim:std.io.serialize.to_json", &[&fixture])
        .expect("prim:std.io.serialize.to_json dispatch");
    let via_prim_bytes = match via_prim.payload() {
        Payload::Bytes(b) => b.clone(),
        other => panic!("expected a Repr::Bytes payload back, got {other:?}"),
    };

    assert_eq!(
        via_prim_bytes,
        direct.into_bytes(),
        "prim:-dispatched to_json output must be byte-identical to the direct Rust call"
    );
}

/// Every registered `PrimSig` is pure: `effects` is empty (no `!{ffi}` needed at a `.myc` call
/// site — the entire point of the typed path over `wild`, per the package's own framing).
#[test]
fn every_registered_sig_is_pure_no_effects() {
    for (name, sig) in typed_prim_sigs() {
        assert!(
            sig.effects.is_empty(),
            "{name}: expected no declared effects, got {:?}",
            sig.effects
        );
    }
}

/// `typed_prim_sigs()` and `install_typed_prims()` stay in lockstep: every name `install_typed_prims`
/// registers appears in `typed_prim_sigs()`'s list too (the check-time and run-time surfaces must
/// never silently drift apart — this crate's half of the PKG-LINKAGE "single source of truth"
/// requirement the CLI lane's `install_typed_std` call site is responsible for at the whole-binary
/// level).
#[test]
fn sigs_and_registry_agree_on_every_name() {
    let mut reg = TypedPrimRegistry::empty();
    install_typed_prims(&mut reg);
    for (name, _) in typed_prim_sigs() {
        assert!(
            reg.has_typed(name),
            "{name} is in typed_prim_sigs() but not installed"
        );
    }
    assert_eq!(
        reg.sigs().count(),
        typed_prim_sigs().len(),
        "registry and typed_prim_sigs() must register the same number of entries"
    );
}
