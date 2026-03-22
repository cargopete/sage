# Changelog

All notable changes to Sage are documented in this file.

## [2.0.1] - 2026-03-22

### Added

#### Extern Functions (Rust FFI)
- `extern fn` declarations for calling Rust functions directly from Sage
- `fails` modifier for extern functions that return `Result<T, String>`
- `[extern]` section in `grove.toml` for specifying Rust source modules and Cargo dependencies
- Extern modules compiled and linked automatically with generated code

### Fixed
- sage-persistence version pinning for crates.io compatibility
- Internal dependency versions aligned for crates.io publish

## [2.0.0] - 2026-03-18

### The Steward Architecture

v2.0 introduces agents as **stewards of long-lived systems** — agents that own a domain, maintain it over time, react to change, coordinate with other stewards, and survive crashes.

### Added

#### Persistent Beliefs
- `@persistent` annotation for agent fields that survive restarts
- Automatic checkpointing on field updates
- SQLite, PostgreSQL, and file backends
- Checkpoint namespacing per agent instance

#### Supervision Trees
- `supervisor` declarations with OTP-style restart strategies
- `OneForOne`, `OneForAll`, `RestForOne` strategies
- `Permanent`, `Transient`, `Temporary` restart policies
- Circuit breaker (restart intensity limiting)
- Nested supervisor support (up to 8 levels)

#### Session Types
- `protocol` declarations for typed communication contracts
- `follows X as Role` clause for protocol participation
- `reply()` expression for protocol-compliant responses
- Compile-time protocol verification (E070-E076 errors)

#### Effect Handlers
- `handler X handles Infer` declarations for LLM configuration
- Per-agent LLM model, temperature, and token settings
- Handler assignment in supervisor child specs

#### Lifecycle Hooks
- `on waking` — runs after persistent state loaded, before `on start`
- `on pause` — runs when supervisor signals graceful pause
- `on resume` — runs when agent unpaused
- `on resting` — runs after `yield`, before exit (alias: `on stop`)

#### Observability
- `trace()` statement for structured logging
- `span "name" { }` blocks for distributed tracing
- NDJSON and OTLP backends
- Automatic agent lifecycle and LLM call tracing
- `sage trace` subcommand for trace analysis

#### Built-in Tools
- `use Http` — HTTP client (`get`, `post`, `put`, `delete`)
- `use Database` — SQL client (`query`, `execute`)
- `use Fs` — Filesystem (`read`, `write`, `exists`, `list`, `delete`)
- `use Shell` — Command execution (`run`)
- Capability-based tool access via `use` declarations
- Tool mocking in tests (`mock tool Http.get -> ...`)

#### Standard Library (Commons)
- List operations: `map`, `filter`, `find`, `reduce`, `flatten`, `zip`, `sort_by`, `take`, `drop`, `any`, `all`, `unique`, `enumerate`, `chunk`
- String operations: `split`, `trim`, `upper`, `lower`, `replace`, `parse_int`, `parse_float`, `lines`, `chars`, `join`
- Option operations: `unwrap_or`, `map_option`, `is_some`, `is_none`, `or_option`
- Result operations: `unwrap_or_result`, `map_result`, `map_err`, `is_ok`, `is_err`
- Time operations: `now_ms`, `now_s`, `format_timestamp`, `sleep_ms`
- JSON operations: `json_get`, `json_get_int`, `json_get_bool`, `json_stringify`

#### Testing Enhancements
- `mock tool X.method -> value` for tool mocking
- `mock tool X.method -> fail("error")` for failure testing
- In-memory persistence backend for tests

### Changed
- `run` statement now accepts supervisor names
- `Agent<T>` can also be written as `AgentHandle<T>`
- `on stop` is now `on resting` (alias preserved for compatibility)

### Performance
- Startup time (check): ~10ms
- Checkpoint latency: <1ms per write
- Supervisor restart latency: ~1ms

## [1.0.5] - 2026-03-16

### Added
- Turbofish syntax in string interpolation (`"{Either::<Int, String>::Left(1)}"`)
- `grove.toml` supervision configuration
- Tool documentation in LSP hover

### Fixed
- Module resolution for nested paths
- Generic type inference in closures

## [1.0.4] - 2026-03-15

### Added
- `span` blocks for observability
- `trace` statement

## [1.0.3] - 2026-03-14

### Added
- Reference programs (`webapp_steward.sg`, `db_guardian.sg`)
- Integration tests for v2 features

## [1.0.2] - 2026-03-13

### Added
- Tool mocking infrastructure
- `@persistent` field codegen

## [1.0.1] - 2026-03-12

### Added
- Session type checking (E070-E076)
- Protocol state machine codegen

## [1.0.0] - 2026-03-10

Initial stable release.

### Core Features
- Agent declarations with `on start`, `on error`, `on message` handlers
- `summon` and `await` for agent spawning
- `divine` for LLM inference with structured output
- Generics and first-class functions
- Records, enums with payloads, pattern matching
- Module system with `pub`, `mod`, `use`
- Testing framework with `mock divine`
