# CROSS-REF — mycelium-std-io

Mycelium-internal dependencies only (steer handoff §6.1; external crates stay in Cargo
metadata). Pinned revs are the fixed (buildable) tips recorded by the Phase-B wave;
content hash = git tree hash of the pinned rev.

| Interface consumed | Repo | Pinned rev | Content hash | Notes |
|---|---|---|---|---|
| mycelium-core | https://github.com/tzervas/mycelium-core | `57ef45b453eef1f02bdfb1f0cad1034dabd32b1f` | tree `997c54d5d59e80ab1d2a3069578b194113aa37fa` | Rust API of `mycelium-core` (see monorepo `docs/api-index/INDEX.md#mycelium-core`). Row corrected to match `Cargo.toml`'s actual pin — this file had drifted stale (still names the pre-#14 rev) as of this PR. |
| mycelium-std-core | https://github.com/tzervas/mycelium-std-core | `19398dfd847eb9a5fc518149a714d7646abbb316` | tree `dca04bc5aa079038baaee8f02a05dd56969657f4` | Rust API of `mycelium-std-core` (see monorepo `docs/api-index/INDEX.md#mycelium-std-core`). Row corrected to match `Cargo.toml`'s actual pin (PR #14) — this file had drifted stale as of this PR. |
| mycelium-interp | https://github.com/tzervas/mycelium-runtime | `f683712fe5b701ad8b062a4b04195fbcdc15143b` | tree `4c6c97d26d030ed090166d40736194f3928388b5` | S-STD-IO-TYPED-PRIMS (PKG-LINKAGE, mycelium-lang#44): `PrimSig`/`TySpec`/`TypedPrimRegistry` (`src/typed.rs`). Optional — only linked by the `typed-prims` feature. Same `mycelium-interp` git-dep shape `mycelium-l1`'s `Cargo.toml` already uses. |

**Owning docs:** `docs/spec/stdlib/io.md` (slice in this repo) · RFC-0016.
**Source provenance:** extracted from `tzervas/mycelium` archive `aad96b7a…`; fixed by
the course-correction Phase B (workspace root, git pins, toolchain + supply-chain
replicas, CI v2). Full program record: monorepo
`docs/planning/course-correction-2026-07-18/PROGRAM.md`.
