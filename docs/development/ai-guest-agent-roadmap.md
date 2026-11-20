# AI Guest Agent Roadmap

> **Not the same thing as the [in-guest agent](../features/guest-agent.md).**
> This doc is about `src/ai/` — an **LLM copilot that reads a static
> `EvidenceSnapshot`** and answers questions about it (tool-calling loop,
> read-only, `--features ai`). The in-guest agent (`src/agent/`,
> `guestkitd`) is a **live daemon that runs inside the guest OS**,
> talking to the host over virtio-serial/QGA — no LLM involved. Same
> "agent" word, unrelated code paths; don't confuse the two.

Evolution of GuestKit's optional AI layer into a **Guest Intelligence Agent** — an offline, evidence-grounded co-pilot for migration, security, and fleet analysis. Core inspection stays deterministic and auditable; AI is additive (`--features ai`).

## Philosophy

- **EvidenceSnapshot is the source of truth** — collectors populate typed structs; AI tools read the snapshot, not raw Guestfs.
- **Graceful degradation** — malformed unit files, missing hives, or partial mounts never abort collection.
- **Grounded output** — every AI finding should cite file paths, unit names, or registry keys from evidence.
- **Air-gap friendly** — OpenAI today; xAI/Anthropic/Ollama via env configuration.

## Phase 0 — Richer evidence

| Component | Status |
|-----------|--------|
| `SystemdInfo`, `SystemdUnit`, problem hints | Shipped |
| `collect_systemd_guest` / `collect_systemd_live` | Shipped |
| Windows services, apps, event log summary | Shipped |
| `SystemdStaticCheck` in doctor | Shipped |
| TUI **Systemd Deep Dive** view | Shipped |
| Windows persistence (Run keys, tasks) | Shipped |

## Phase 1 — Semantic analysis

| Component | Status |
|-----------|--------|
| Dependency graph from unit `After`/`Before`/`Requires`/`Wants` | Shipped (`src/ai/semantic.rs`) |
| Sandboxing score per service | Shipped |
| Windows service risk flags | Shipped |
| Improved AI context injection | Shipped (`build_intelligence`) |
| TUI timers/sockets/problems in Systemd Deep Dive | Shipped |

## Phase 2 — Agentic loop

| Component | Status |
|-----------|--------|
| Tool registry over snapshot | Shipped (`src/ai/tools.rs`) |
| Multi-step agent loop | Shipped (`src/ai/agent.rs`) |
| Native tool-calling (OpenAI) | Shipped (`src/ai/rig_tools.rs`, rig-core `AgentBuilder`/`multi_turn`) — schema-validated, provider-parsed tool calls instead of regex/JSON-scraped completion text. xAI/Anthropic/Ollama still use the original text-instructed loop (no rig client wired up for xAI/Anthropic yet; Ollama has none upstream) |
| `doctor --explain --ai`, `migrate-plan --explain --ai` | Shipped |
| Providers: OpenAI, xAI, Anthropic, Ollama | Shipped (`src/ai/providers.rs`) |
| Cross-run memory | Shipped (`src/ai/memory.rs`) — a re-run against the same VM (keyed by canonicalized image path) folds a short summary of prior findings into the query, capped at the last 20 runs. `GUESTKIT_AI_MEMORY_DIR` / `GUESTKIT_AI_MEMORY=0` to relocate/disable |
| TUI **AI Insights** panel | Shipped |

## Phase 3 — Local AI & what-if

| Component | Status |
|-----------|--------|
| Ollama integration (`OLLAMA_HOST`, `--features local-ai`) | Shipped |
| What-if simulator (disable unit → boot score delta) | Shipped (`src/ai/whatif.rs`) |
| AI narrative sections for reports | Shipped (`src/ai/reports.rs`) |
| Proactive recommendations engine | Shipped (`src/ai/recommendations.rs`) |
| Fleet semantic drift explanations | Shipped (`src/ai/drift.rs`) — wired to `guestkit fleet watch` (`src/fleet/baseline.rs`), a scheduled drift monitor that diffs current evidence against a stored per-VM golden baseline. See [migration-assurance.md](../features/migration-assurance.md#guestkit-fleet-watch--scheduled-drift-monitoring) |

## Phase 4 — Platform integration

| Component | Status |
|-----------|--------|
| Machina dashboard export type | Shipped (`src/ai/platform.rs`) |
| Policy DSL hints from CIS-lite profile | Shipped |
| CIS-style security profiles | Shipped (`src/ai/security_profiles.rs`) |
| Full `.evtx` parsing for forensic profiles | Shipped (`evtx` crate + `WindowsForensicProfile`) |
| MCP server (external hosts) | Shipped (`src/ai/mcp.rs`, `--features mcp`) — `guestkit mcp-serve <disk> [--target <target>]` exposes the same 6 read-only tools over stdio to Claude Desktop / other MCP hosts, independent of guestkit's own AI copilot loop |

## Module layout

```text
src/ai/
  mod.rs              — public API
  semantic.rs         — Phase 1 analysis
  tools.rs            — Phase 2 snapshot tool registry (feature ai)
  agent.rs            — Phase 2 agent loop (feature ai)
  rig_tools.rs        — Phase 2 native rig-core Tool impls (feature ai)
  memory.rs           — Phase 2 cross-run memory (feature ai)
  prompts.rs          — versioned system prompts
  providers.rs        — OpenAI / xAI / Anthropic / Ollama (feature ai)
  recommendations.rs  — Phase 3 proactive engine
  whatif.rs           — Phase 3 boot score simulator
  drift.rs            — Phase 3 fleet drift
  reports.rs          — Phase 3 report narratives
  security_profiles.rs— Phase 4 CIS-lite
  platform.rs         — Phase 4 Machina export
  mcp.rs              — Phase 4 MCP server (feature mcp)
  intelligence.rs     — bundled output for doctor/TUI
src/cli/commands/
  mcp.rs              — guestkit mcp-serve CLI command (feature mcp)
src/evidence/collectors/
  systemd.rs, windows.rs — Phase 0 collectors
```

## CLI usage

```bash
# Deterministic intelligence (no LLM)
guestkit doctor disk.qcow2 --explain
guestkit migrate-plan disk.qcow2 --target kubevirt --explain

# LLM agent (requires --features ai + API key or Ollama)
cargo build --release --features ai
export OPENAI_API_KEY=...
guestkit doctor disk.qcow2 --explain --ai

# Local Ollama
export OLLAMA_HOST=http://127.0.0.1:11434
export GUESTKIT_AI_PROVIDER=ollama
guestkit migrate-plan disk.qcow2 --target kvm --ai

# MCP server for external hosts (requires --features mcp)
cargo build --release --features mcp
guestkit mcp-serve disk.qcow2 --target kvm   # stdio — point an MCP host at this command
```

## Related docs

- [roadmap.md](roadmap.md) — product-wide roadmap
- [../features/guest-agent.md](../features/guest-agent.md) — live in-guest agent (virtio-serial RPC)
