# Gap note: socket-backed Substrate + general struct JSON

**Context:** port-readiness review 2026-07-22 (`claude/mycelium-readiness-gaps`).
Full plan: `mycelium-lang` `docs/planning/PORT-READINESS-2026-07-22.md`.

`mycelium-std-io` is an honest, fully-tested surface over an **in-memory substrate**,
with the OS-backed substrate deliberately reserved for later. The two port targets need
those reserved seams filled.

## 1. Socket-backed `Substrate` (the reserved seam)

`src/io.rs` documents that the OS-backed substrate — "a file descriptor, a network
**socket**" — is *not* here yet (the abstract `Bytes`-cursor `Source`/`Sink` is
in-memory), and reserves `Substrate::from_fd` for when the real-OS floor lands (M-541).

Both ports need real socket I/O:
- **runner** — HTTPS to `api.github.com` (+ a loopback TCP wake server).
- **relay** — a blocking long-poll read with timeout to `api.telegram.org`.

**Ask:** implement the OS-backed `Substrate` (fd/socket) over the `@std-sys` floor so
`std-io` streams can be backed by real sockets/files. This is the I/O half of the
`std-net` phylum proposed in the plan, and depends on the FFI host-execution seam
(`mycelium-l1/docs/GAP-ffi-host-and-surface.md`) + the real-OS floor
(`mycelium-std-sys/docs/GAP-host-effects.md`).

## 2. General struct JSON codec

`src/serialize.rs` provides `Value`↔JSON (and a `Wire` binary format) for the internal
`mycelium-core::Value` only — not arbitrary user types. Both ports parse/emit JSON of
domain shapes (GitHub API responses / config; Telegram updates / hook payloads).

**Ask:** a general JSON (de)serialization surface for user-defined types. This is pure
computation — buildable in Mycelium with no host dependency — and is **Tier-1/2** in the
plan (does not block on the FFI seam). Could live here or as a dedicated `std-json`
phylum; TOML (relay config) is a sibling need.
