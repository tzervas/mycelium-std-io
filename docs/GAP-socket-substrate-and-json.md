# Gap note: socket-backed Substrate + general struct JSON

**Context:** port-readiness review 2026-07-22 (`claude/mycelium-readiness-gaps`).
Full plan: `mycelium-lang` `docs/planning/PORT-READINESS-2026-07-22.md`.

`mycelium-std-io` is an honest, fully-tested surface over an **in-memory substrate**,
with the OS-backed substrate deliberately reserved for later. The two port targets need
those reserved seams filled.

## 1. Socket-backed `Substrate` (the reserved seam) — OPEN

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

## 2. General struct JSON + TOML codec — FOUNDATION LANDED (WP-5 L-IO)

**Status (2026-08-01):** pure foundation shipped in `src/codec.rs` (S-CODECS).

| Need | API | Notes |
|---|---|---|
| Value JSON (pre-existing) | `serialize::{to_json,from_json}` | `mycelium_core::Value` only |
| S-CODECS Value aliases | `codec::{value_to_json,json_to_value}` | thin aliases of the above |
| General user-type JSON | `codec::{encode_json,decode_json}` + `Encode`/`Decode` | any `serde` type; never-silent |
| TOML config | `codec::{parse_toml,toml_get,decode_toml,encode_toml}` | `relay.toml`-shaped; missing key = `Err` |

**Honesty residual:**
- General JSON is `serde_json` over user types — not a second Value wire grammar.
- TOML path helpers are a solid foundation (dotted path + typed get); not a full
  schema-validated config crate. Domain validation stays with the caller.
- Socket/OS substrate (section 1) remains open and is **out of scope** for pure codecs.

**Non-goals retained:** no new `std-json` repo; no `wild`; no process/net here.
