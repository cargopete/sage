# RFC-0012: Built-In Testing Framework

- **Status:** Draft
- **Created:** 2026-03-14
- **Author:** Sage Contributors
- **Depends on:** RFC-0001 (POC), RFC-0005 (User-Defined Types), RFC-0007 (Error Handling)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Language Changes](#4-language-changes)
5. [Test File Convention](#5-test-file-convention)
6. [Assertions](#6-assertions)
7. [Agent Testing](#7-agent-testing)
8. [LLM Mocking](#8-llm-mocking)
9. [Test Runner & CLI](#9-test-runner--cli)
10. [Output Format](#10-output-format)
11. [Checker Rules](#11-checker-rules)
12. [Codegen](#12-codegen)
13. [New Error Codes](#13-new-error-codes)
14. [Implementation Plan](#14-implementation-plan)
15. [Open Questions](#15-open-questions)

---

## 1. Summary

This RFC introduces a built-in testing framework to Sage. Tests live in dedicated `_test.sg` files, use a small set of new assertion builtins, and are run via `sage test`. The framework covers three categories: **unit tests** for pure functions, **agent behaviour tests** for spawn/emit/message-passing correctness, and **LLM mock tests** for programs that call `infer`. No third-party test library is needed — the framework is part of the language.

---

## 2. Motivation

Sage programs interact with LLMs, spawn concurrent agents, and pass typed messages — all of which are hard to test without framework-level support. Without built-in testing:

- Developers have no standard way to assert function output
- Agent behaviour (what a spawned agent emits) requires a full runtime roundtrip with a real LLM
- Any test harness has to be written in the application language itself, leading to boilerplate

Other languages that embed testing at the language level (Rust's `#[test]`, Go's `_test.go` convention) see higher test coverage and more consistent tooling. Sage should follow this pattern from the start, before ecosystem habits form around ad-hoc approaches.

The specific pain points this RFC addresses:

```sage
// Today: no way to test this without running it against a real LLM
agent Summariser {
    topic: String

    on start {
        let result: Inferred<String> = try infer(
            "Summarise {self.topic} in one sentence"
        );
        emit(result);
    }
}
```

With this RFC:

```sage
// summariser_test.sg
test "summariser emits a non-empty string" {
    mock infer -> "Quantum computing uses quantum mechanics to process information.";

    let result = await spawn Summariser { topic: "quantum computing" };
    assert_not_empty(result);
}
```

---

## 3. Design Goals

1. **No new file types.** Test files are valid `.sg` files with the `_test.sg` suffix. They use a small set of new keywords and builtins — nothing else is special.
2. **Tests are first-class, not annotations.** `test "name" { ... }` is a top-level declaration, not a decorator on a function.
3. **Agent tests feel natural.** `spawn` and `await` work inside tests exactly as they do in programs. The test body is async by default.
4. **LLM calls are always mocked in tests.** Calling `infer` without a `mock infer` declaration is a compile error in test files. Tests must never make real network calls.
5. **Fast by default.** Tests run concurrently unless explicitly marked `serial`.
6. **Readable output.** Pass/fail with source locations and diffs, using miette for error formatting.

---

## 4. Language Changes

### 4.1 New top-level declaration: `test`

```sage
test "description" {
    // test body
}
```

`test` is only valid in `_test.sg` files. Using it in a regular `.sg` file is a compile error (E030).

### 4.2 New statement: `mock infer`

```sage
mock infer -> "some fixed string";
mock infer -> SomeRecord { field: "value" };
mock infer -> SomeEnum.Variant;
```

`mock infer` is only valid inside a `test` block. It replaces all `infer` calls for the duration of that test with the given value. The value must be type-compatible with the `Inferred<T>` type at the call site — this is checked at compile time.

Multiple `mock infer` declarations in a single test are allowed and are consumed in order — the first `infer` call gets the first mock, the second gets the second, and so on. If more `infer` calls are made than mocks provided, the test fails at runtime with a clear error: `ran out of infer mocks`.

### 4.3 New assertion builtins

See Section 6 for the full list.

### 4.4 New `@serial` annotation

```sage
@serial
test "this test must not run concurrently" {
    // ...
}
```

By default tests run concurrently. `@serial` forces the test to run in isolation, after all concurrent tests complete. Useful for tests that modify shared state or need predictable timing.

---

## 5. Test File Convention

### 5.1 Naming

Test files follow the `<name>_test.sg` suffix convention:

```
src/
  researcher.sg
  researcher_test.sg
  coordinator.sg
  coordinator_test.sg
```

The `_test.sg` suffix is the sole marker. No configuration is needed.

### 5.2 Imports

Test files can import from the module they are testing using the same `use` syntax as regular files:

```sage
// researcher_test.sg
use agents::Researcher;
use types::ResearchResult;

test "researcher emits a result" {
    mock infer -> ResearchResult {
        topic: "fusion"
        summary: "Nuclear fusion combines atoms to release energy."
        confidence: 0.91
    };

    let result: ResearchResult = await spawn Researcher { topic: "fusion" };
    assert_eq(result.topic, "fusion");
    assert_gt(result.confidence, 0.5);
}
```

### 5.3 Isolation

Each test block runs in a fresh environment. Beliefs set in one test do not affect another. Agent handles do not escape test scope — awaiting an agent that was spawned in a different test is a compile error (E033).

### 5.4 No `run` statement

Test files must not contain a `run` statement. The test runner drives execution. Using `run` in a `_test.sg` file is a compile error (E031).

---

## 6. Assertions

All assertion builtins are available in test files without any import. They produce a test failure (not a panic) on violation, with a source span pointing at the failing assertion.

### 6.1 Core assertions

| Builtin | Signature | Description |
|---|---|---|
| `assert(cond)` | `(Bool) -> Unit` | Fails if `cond` is false |
| `assert_eq(a, b)` | `(T, T) -> Unit` | Fails if `a != b` |
| `assert_neq(a, b)` | `(T, T) -> Unit` | Fails if `a == b` |
| `assert_gt(a, b)` | `(T, T) -> Unit` | Fails if `a <= b` |
| `assert_lt(a, b)` | `(T, T) -> Unit` | Fails if `a >= b` |
| `assert_gte(a, b)` | `(T, T) -> Unit` | Fails if `a < b` |
| `assert_lte(a, b)` | `(T, T) -> Unit` | Fails if `a > b` |
| `assert_true(v)` | `(Bool) -> Unit` | Alias for `assert(v)` |
| `assert_false(v)` | `(Bool) -> Unit` | Fails if `v` is true |

### 6.2 String assertions

| Builtin | Signature | Description |
|---|---|---|
| `assert_contains(s, sub)` | `(String, String) -> Unit` | Fails if `sub` not in `s` |
| `assert_not_contains(s, sub)` | `(String, String) -> Unit` | Fails if `sub` is in `s` |
| `assert_empty(s)` | `(String) -> Unit` | Fails if `s` is not `""` |
| `assert_not_empty(s)` | `(String) -> Unit` | Fails if `s` is `""` |
| `assert_starts_with(s, prefix)` | `(String, String) -> Unit` | Fails if `s` doesn't start with `prefix` |
| `assert_ends_with(s, suffix)` | `(String, String) -> Unit` | Fails if `s` doesn't end with `suffix` |

### 6.3 List assertions

| Builtin | Signature | Description |
|---|---|---|
| `assert_len(list, n)` | `(List<T>, Int) -> Unit` | Fails if `len(list) != n` |
| `assert_empty_list(list)` | `(List<T>) -> Unit` | Fails if list is not empty |
| `assert_not_empty_list(list)` | `(List<T>) -> Unit` | Fails if list is empty |

### 6.4 Failure assertion

```sage
test "bad spawn fails at type check time — no runtime assertion needed" {
    // Type errors are caught at compile time, not via assert_fails
}
```

For cases where a runtime failure is expected (e.g. an agent that calls `emit` on an error path):

```sage
test "agent propagates error correctly" {
    mock infer -> fail("simulated LLM failure");

    let handle = spawn Summariser { topic: "test" };
    assert_fails(await handle);
}
```

`assert_fails` expects the expression to propagate an error (via RFC-0007's error handling). It fails the test if the expression succeeds instead.

### 6.5 Failure messages

All assertions accept an optional trailing string for a custom failure message:

```sage
assert_eq(result.confidence, 0.9, "confidence should be exactly 0.9");
assert_not_empty(result.summary, "summary must not be blank");
```

---

## 7. Agent Testing

### 7.1 Spawning agents in tests

`spawn` and `await` work exactly as in normal programs:

```sage
// counter_test.sg
use agents::Counter;

test "counter increments from initial value" {
    let c = spawn Counter { initial: 10 };
    let result = await c;
    assert_eq(result, 15);  // Counter adds 5
}
```

### 7.2 Testing message passing

Agents that use `receives` can be sent messages inside tests:

```sage
// worker_test.sg
use agents::Worker;
use types::WorkerMsg;

test "worker responds to ping" {
    let w = spawn Worker { id: 1 };
    send(w, WorkerMsg.Ping);
    send(w, WorkerMsg.Shutdown);
    let result = await w;
    assert_eq(result, 0);
}
```

### 7.3 Concurrent agent tests

Spawn multiple agents in a single test to verify concurrent behaviour:

```sage
test "two counters run concurrently and both produce correct results" {
    let c1 = spawn Counter { initial: 0 };
    let c2 = spawn Counter { initial: 100 };

    let r1 = await c1;
    let r2 = await c2;

    assert_eq(r1, 5);
    assert_eq(r2, 105);
}
```

### 7.4 Testing emit type

The return type of `await` in a test is the same as in a normal program — inferred from the agent's `emit` calls. No special assertion is needed for the type itself; the type checker enforces it.

---

## 8. LLM Mocking

### 8.1 The `mock infer` statement

`mock infer` replaces the LLM backend for the duration of a test. No real HTTP calls are made. The mock value is returned immediately, synchronously, as if the LLM had responded.

```sage
test "infer returns the mocked value" {
    mock infer -> "This is a mocked LLM response.";

    let result: Inferred<String> = try infer("Summarise something");
    assert_eq(result, "This is a mocked LLM response.");
}
```

### 8.2 Typed mocks

For `Inferred<T>` where `T` is a record, provide a record literal:

```sage
record Summary {
    text: String
    confidence: Float
}

test "structured infer returns typed mock" {
    mock infer -> Summary {
        text: "Quantum computing is fast."
        confidence: 0.88
    };

    let s: Inferred<Summary> = try infer("Summarise quantum computing");
    assert_eq(s.text, "Quantum computing is fast.");
    assert_gte(s.confidence, 0.8);
}
```

### 8.3 Sequential mocks

When a test contains multiple `infer` calls, provide multiple `mock infer` declarations. They are consumed in order:

```sage
test "coordinator gets separate results for each researcher" {
    mock infer -> "Quantum computing summary.";
    mock infer -> "CRISPR summary.";

    let r1 = await spawn Researcher { topic: "quantum computing" };
    let r2 = await spawn Researcher { topic: "CRISPR" };

    assert_contains(r1, "Quantum");
    assert_contains(r2, "CRISPR");
}
```

The order of mock consumption follows the order in which `infer` is called at runtime. For concurrent agents, this is non-deterministic — see Section 8.5.

### 8.4 Unmocked `infer` is a compile error

In a `_test.sg` file, any agent or function that calls `infer` must be covered by a `mock infer` declaration. If the test runner detects an `infer` call with no mock queued at runtime, the test fails immediately with:

```
error[E034]: infer called with no mock available
  --> researcher_test.sg:12:5
   |
12 |     let result = try infer("Summarise {self.topic}");
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = help: add `mock infer -> <value>;` before this point in the test
```

The compile-time check is best-effort (it can warn but not always prove coverage statically). The runtime check is definitive.

### 8.5 Mocking with concurrent agents

When multiple agents run concurrently and each calls `infer`, mock ordering is non-deterministic. For these cases, use `@serial` to force sequential execution:

```sage
@serial
test "sequential mock ordering is predictable" {
    mock infer -> "Summary A.";
    mock infer -> "Summary B.";

    let r1 = await spawn Researcher { topic: "A" };
    let r2 = await spawn Researcher { topic: "B" };

    assert_eq(r1, "Summary A.");
    assert_eq(r2, "Summary B.");
}
```

Or, if ordering doesn't matter, assert properties that hold for either mock:

```sage
test "both researchers emit non-empty strings (order irrelevant)" {
    mock infer -> "Summary A.";
    mock infer -> "Summary B.";

    let r1 = await spawn Researcher { topic: "A" };
    let r2 = await spawn Researcher { topic: "B" };

    assert_not_empty(r1);
    assert_not_empty(r2);
}
```

### 8.6 Mock errors

To test error handling paths, use `mock infer -> fail(message)`:

```sage
test "agent handles infer failure gracefully" {
    mock infer -> fail("rate limit exceeded");

    let handle = spawn ResilientResearcher { topic: "fusion" };
    let result = await handle;
    assert_eq(result, "unavailable");  // agent's fallback value
}
```

`mock infer -> fail(msg)` causes the corresponding `infer` call to return the error described by `msg`, as if the LLM backend had failed. The agent's `on error` handler (RFC-0007) is invoked as normal.

---

## 9. Test Runner & CLI

### 9.1 `sage test`

```
sage test [OPTIONS] [FILTER]
```

Discovers and runs all `_test.sg` files in the current project. Exits with code `0` if all tests pass, `1` if any fail.

**Options:**

| Flag | Description |
|---|---|
| `--filter <pattern>` | Only run tests whose name contains `pattern` |
| `--file <path>` | Only run tests in the specified `_test.sg` file |
| `--serial` | Run all tests serially, regardless of `@serial` annotation |
| `--verbose` | Show output for passing tests as well as failing ones |
| `--no-colour` | Disable ANSI colour output |

**Examples:**

```bash
# Run all tests
sage test

# Run only tests containing "researcher"
sage test --filter researcher

# Run a specific file
sage test --file src/researcher_test.sg

# Run with verbose output
sage test --verbose
```

### 9.2 Discovery

`sage test` walks the project source directory and collects all files ending in `_test.sg`. Files in `hearth/` (the build output directory) are excluded. Symlinks are not followed.

### 9.3 Execution model

Tests within a file run concurrently by default using Tokio tasks. Tests marked `@serial` are deferred until all concurrent tests in the file have completed, then run one at a time in declaration order.

Across files, files themselves run concurrently. `@serial` only applies within a file — there is no cross-file serialisation in this RFC.

### 9.4 Timeout

Each test has a default timeout of **10 seconds**. A test that does not complete within this window fails with:

```
FAIL researcher_test.sg::slow test [timeout after 10s]
```

The timeout is configurable via `sage.toml`:

```toml
[test]
timeout_ms = 30000
```

---

## 10. Output Format

### 10.1 Default output

```
sage test
   Compiling project...
   Running tests in src/counter_test.sg (3 tests)
   Running tests in src/researcher_test.sg (2 tests)

PASS counter_test.sg::counter increments from initial value     [12ms]
PASS counter_test.sg::counter with zero initial value           [8ms]
PASS counter_test.sg::two counters run concurrently             [15ms]
PASS researcher_test.sg::researcher emits a non-empty string    [6ms]
FAIL researcher_test.sg::researcher confidence is above 0.9     [4ms]

────────────────────────────────────────────────────────────────

FAILED: researcher_test.sg::researcher confidence is above 0.9

  --> src/researcher_test.sg:18:5
   |
18 |     assert_gt(result.confidence, 0.9);
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = left:  0.72
   = right: 0.9
   = note:  expected confidence > 0.9, got 0.72

────────────────────────────────────────────────────────────────

test result: FAILED. 4 passed, 1 failed, 0 skipped
```

### 10.2 Verbose output (`--verbose`)

Passing tests also show their assertion count and any `print` output from the test body.

### 10.3 Assertion diffs

For `assert_eq` failures on strings, a character-level diff is shown:

```
= left:  "Quantum computing uses quantum mechanics"
= right: "Quantum computing uses classical mechanics"
                                      ^^^^^^^^
                                      differs here
```

For records, field-by-field diff:

```
= left:  Summary { text: "...", confidence: 0.72 }
= right: Summary { text: "...", confidence: 0.91 }
                                            ^^^^
                                            field `confidence` differs
```

---

## 11. Checker Rules

### 11.1 `test` block rules

- `test` declarations are only valid in `_test.sg` files (E030)
- `test` descriptions must be unique within a file — duplicate names are E035
- `run` statements in `_test.sg` files are E031
- Agent handles spawned in one test cannot be used in another test (E033)

### 11.2 `mock infer` rules

- `mock infer` is only valid inside a `test` block (E036)
- The type of the mock value must be assignable to the `Inferred<T>` type at the corresponding call site. A best-effort static check is performed; the definitive check is at runtime (E037)
- `mock infer -> fail(msg)` requires `msg` to be a `String` literal or variable (E038)

### 11.3 Assertion rules

- All assertion builtins require that the argument types support `==` (for `assert_eq`/`assert_neq`) or ordering (for `assert_gt` etc.). Passing a type that doesn't support the operation is E039
- Assertion builtins are not available outside `_test.sg` files (E030 — same code, same message: "test constructs are only available in `_test.sg` files")

### 11.4 `@serial` rules

- `@serial` is only valid on `test` declarations
- Applying `@serial` to any other construct is ignored with a warning (W002)

---

## 12. Codegen

### 12.1 Test compilation

Test files are compiled separately from the main project. `sage test` runs the full compiler pipeline (lex → parse → check → codegen) on each `_test.sg` file, with the following differences:

- The entry point is the test runner harness, not `run <AgentName>`
- `mock infer` declarations are compiled into a mock queue injected into the agent runtime's LLM client
- Assertion builtins compile to Rust functions that record failures without panicking, allowing all assertions in a test to be evaluated before the test is marked failed (soft assertions)

### 12.2 Mock queue implementation

The generated Rust code for a test wraps the existing `LlmClient` with a `MockLlmClient`:

```rust
// Generated for a test with two mock infer declarations
let mock_client = MockLlmClient::new(vec![
    MockResponse::value("Quantum computing summary."),
    MockResponse::value("CRISPR summary."),
]);
```

`MockLlmClient` implements the same `LlmClient` trait as the real client, so no changes are needed to the agent runtime or generated agent code. The mock client pops responses from the queue on each `infer` call and returns an error if the queue is empty.

### 12.3 Test harness

Each test block generates a Tokio async task. The harness runs all non-serial tasks concurrently via `tokio::join_all`, then runs serial tasks in sequence. Results are collected and formatted by the output layer.

---

## 13. New Error Codes

| Code | Message | Trigger |
|---|---|---|
| E030 | test constructs are only available in `_test.sg` files | `test`, assertion builtins, or `mock infer` used outside a test file |
| E031 | `run` statement not allowed in test files | `run` in a `_test.sg` file |
| E032 | test timeout exceeded | Runtime: test did not complete within the configured timeout |
| E033 | agent handle escapes test scope | Handle from one test used in another |
| E034 | `infer` called with no mock available | Runtime: infer called but mock queue is empty |
| E035 | duplicate test name | Two `test` blocks in the same file have the same description |
| E036 | `mock infer` outside test block | `mock infer` used outside a `test` block |
| E037 | mock type mismatch | Mock value type is incompatible with the `Inferred<T>` call site |
| E038 | `fail` argument must be a String | Non-string passed to `mock infer -> fail(...)` |
| E039 | type does not support this assertion | e.g. `assert_gt` on a type with no ordering |

---

## 14. Implementation Plan

### Phase 1 — Lexer & Parser (2–3 days)

- Add `test` as a keyword token
- Add `mock` as a keyword token
- Parse `test "description" { ... }` as a top-level declaration
- Parse `mock infer -> expr;` as a statement (only inside test blocks)
- Parse `@serial` annotation on test declarations
- Add assertion builtins to the known-builtins list in the parser

### Phase 2 — File Discovery & Project Integration (1 day)

- Implement `_test.sg` file discovery in `sage-loader`
- Separate compilation pipeline for test files
- Extend `sage.toml` schema with `[test]` section (timeout)

### Phase 3 — Type Checker (2–3 days)

- Enforce E030: test constructs outside test files
- Enforce E031: `run` in test files
- Enforce E035: duplicate test names
- Enforce E036: `mock infer` outside test block
- Best-effort E037: mock type compatibility check
- Enforce E039: assertion type compatibility
- Track test scope for E033 (agent handle escape)

### Phase 4 — Mock Infrastructure (2 days)

- Implement `MockLlmClient` in `sage-runtime`
- Implement mock queue (ordered, thread-safe for concurrent agents)
- Implement `MockResponse::value` and `MockResponse::fail`
- Runtime E034: empty mock queue error

### Phase 5 — Codegen (2–3 days)

- Generate test harness entry point
- Generate `MockLlmClient` instantiation from `mock infer` declarations
- Generate assertion builtin calls
- Generate `@serial` grouping logic
- Wire timeout via Tokio `timeout`

### Phase 6 — CLI & Output (2 days)

- Implement `sage test` subcommand in `sage-cli`
- Implement `--filter`, `--file`, `--serial`, `--verbose`, `--no-colour` flags
- Implement pass/fail output with miette formatting
- Implement assertion diff output for strings and records

### Phase 7 — Tests & Polish (2–3 days)

- Self-test: write `_test.sg` files for existing example programs
- Checker tests for all ten new error codes
- Test the mock queue under concurrent agent execution
- Documentation: add Testing section to the guide
- Update README with `sage test` example

**Total estimated effort:** ~2.5 weeks

---

## 15. Open Questions

### 15.1 Test-only imports

Should `_test.sg` files be able to declare helper functions and agents that are only used across test files? For example, a shared `mock_researcher.sg` that provides a fake `Researcher` agent for multiple test files. This would require a `_testutil.sg` or similar convention. Deferred — the single-file test convention covers the majority of cases.

### 15.2 Coverage reporting

`sage test --coverage` producing line-level coverage data would be valuable but requires tracking which lines of generated Rust correspond to which lines of Sage source (source maps). This is a prerequisite for coverage and is out of scope for this RFC. Deferred to a follow-up.

### 15.3 Property-based testing

Fuzzing assertions with generated inputs (à la Hypothesis or QuickCheck) is a natural extension, especially for pure functions. The interface would be something like:

```sage
// Hypothetical
test "addition is commutative" for all(a: Int, b: Int) {
    assert_eq(a + b, b + a);
}
```

Deferred — the `for all` quantifier requires a value generation strategy that has its own design surface. Out of scope for this RFC.

### 15.4 Snapshot testing for `infer` output

Rather than mocking `infer` with a fixed value, snapshot testing would record the first LLM response and assert future runs match it. This is useful for prompt regression testing. Deferred — snapshot storage, update workflow, and determinism concerns need their own RFC.

### 15.5 Cross-file `@serial`

Currently `@serial` only serialises within a file. If two test files both test agents that write to shared external state, there is no cross-file serialisation primitive. Deferred — the common case (pure agents, mocked LLM) has no shared state and doesn't need this.

---

*Tests are how you know the agents are wise, not just confident. Ward insists.*
