# RFC-0010: Maps, Enum Payloads, Tuples, While, Result, and Record Equality

- **Status:** Implemented
- **Created:** 2026-03-13
- **Author:** Pete Pavlovski
- **Depends on:** RFC-0005 (User-Defined Types), RFC-0007 (Error Handling), RFC-0009 (Closures)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Feature 1 — `Map<K, V>`](#4-feature-1--mapk-v)
5. [Feature 2 — Enum Payload Variants](#5-feature-2--enum-payload-variants)
6. [Feature 3 — Tuple Types and Multi-Return](#6-feature-3--tuple-types-and-multi-return)
7. [Feature 4 — `while` Loop](#7-feature-4--while-loop)
8. [Feature 5 — `Result<T, E>`](#8-feature-5--resultt-e)
9. [Feature 6 — Record Equality](#9-feature-6--record-equality)
10. [Interactions Between Features](#10-interactions-between-features)
11. [Checker Rules](#11-checker-rules)
12. [Codegen](#12-codegen)
13. [New Error Codes](#13-new-error-codes)
14. [Implementation Plan](#14-implementation-plan)
15. [Open Questions](#15-open-questions)

---

## 1. Summary

This RFC introduces six interdependent language features that collectively close the most significant expressiveness gaps in Sage:

- **`Map<K, V>`** — a first-class key-value collection type with builtin operations
- **Enum payload variants** — enum variants that carry typed data, enabling sum types and proper `Option`/`Result` modelling
- **Tuple types** — anonymous product types for multi-value returns and lightweight grouping
- **`while` loop** — conditional loop statement to complement `loop`/`break` and `for`
- **`Result<T, E>`** — a first-class result type that integrates with the existing `try`/`catch`/`on error` error handling model
- **Record equality** — `==` and `!=` operators for records, with structural semantics

These features are specified in a single RFC because they interact: enum payload variants are needed to define `Result<T, E>` properly; `Result<T, E>` relies on pattern matching with payload binding; and record equality needs to compose with the equality rules for all other types. Specifying them together ensures the design is consistent.

---

## 2. Motivation

### The gaps today

After RFC-0005 through RFC-0009, Sage can express typed agent wiring, structured LLM output, error handling, and closures. But it cannot express programs of realistic complexity because:

**No map type.** Every real agent program accumulates keyed data — cache LLM results by topic, count occurrences, track agent state by ID. Today, the only collection is `List<T>`. Programmers must reach for parallel lists (`List<String>` keys + `List<String>` values), which is exactly the awkward pattern RFC-0005 set out to eliminate.

**Enum variants can't carry data.** `Failed` cannot explain why it failed. `Complete` cannot carry its result. This forces one-off wrapper records for every sum type, polluting the namespace and making enums essentially just integer constants with names.

**No multi-return.** A function that computes two related values must either define a record (heavyweight for throwaway groupings), use output parameters (not idiomatic), or return a `List` and rely on index-based access (loses type safety entirely). This is felt constantly.

**No `while` loop.** Only `loop`/`break` and `for` exist. The canonical pattern for "keep going while a condition holds" requires `loop { if !cond { break } }` — three lines that belong on one.

**`Result<T, E>` is not a value.** The `try`/`catch` model from RFC-0007 is imperative-only. You cannot pass a `Result` as a function argument, store one in a record field, or put several in a `List`. This limits composability significantly.

**Record equality is missing.** The codegen already derives `PartialEq` for Sage records. The type checker simply never emits `==` for `Type::Named`, leaving a capability on the floor. Equality is necessary for `match` on records, deduplication in collections, and agent state comparisons.

---

## 3. Design Goals

1. **`Map<K, V>` is a builtin, not a library type.** It is handled by the type checker and codegen like `List<T>` — not user-definable, always available.
2. **Enum payload syntax is minimal.** One variant per line, payload in parentheses. `match` binds payload variables with the same syntax as today's wildcard binding.
3. **Tuples are structural, not nominal.** `(Int, String)` is the type; `(42, "hello")` is the value. No declaration needed. Two tuples with the same element types are the same type.
4. **`while` is a statement, not an expression.** It does not produce a value. This is consistent with `for` and `loop`.
5. **`Result<T, E>` is a stdlib enum, not a compiler magic type.** It is defined in the prelude as a regular Sage enum with payload variants. The `try`/`catch` syntax from RFC-0007 is sugar over it.
6. **Record equality is structural.** Two records are `==` if all their fields are `==`. This composes naturally with the equality rules for primitives, enums, and tuples.
7. **All features must be usable with `Inferred<T>` and agent beliefs** where the type supports it.

---

## 4. Feature 1 — `Map<K, V>`

### 4.1 Type syntax

`Map<K, V>` is a parameterised builtin type. `K` must be a comparable type (primitives, enums, tuples of comparable types). `V` can be any Sage type.

```sage
let scores: Map<String, Int> = {}
let cache: Map<String, ResearchResult> = {}
let registry: Map<Int, Agent<String>> = {}
```

The empty map literal is `{}`. A map with initial entries uses `key: value` pairs:

```sage
let labels = { "pending": 0, "complete": 1, "failed": 2 }
```

This syntax is unambiguous because the empty record does not exist in Sage (records must be named), and block statements use `{` followed by a statement keyword, not a string literal or identifier followed by `:`.

### 4.2 Builtin map operations

Map operations are builtin functions, consistent with `len` and `join` for lists.

```sage
// Insert or update
map_set(m, key, value)          // returns Unit, mutates in place

// Lookup — returns Option<V>
let v: Option<String> = map_get(m, "topic")

// Delete
map_delete(m, key)              // returns Unit, mutates in place

// Check membership
let exists: Bool = map_has(m, key)

// Number of entries
let n: Int = len(m)             // len is overloaded for maps

// Keys and values as lists
let ks: List<String> = map_keys(m)
let vs: List<Int>    = map_values(m)
```

`map_get` returns `Option<V>` rather than `V` because the key may not be present. This forces the caller to handle the missing case, which is correct.

### 4.3 Iteration

Maps are iterable in `for` loops. The iteration variable is a tuple:

```sage
for entry in m {
    let (key, value) = entry
    print("{key}: {value}")
}
```

Or with direct tuple destructuring in the loop variable (see §6.4):

```sage
for (key, value) in m {
    print("{key}: {value}")
}
```

The iteration order is not guaranteed. This matches Rust's `HashMap` semantics and is consistent with the expectation that maps are unordered.

### 4.4 Constraints on `K`

Key types must implement equality and hashing. The comparable types are:

- All primitives: `Int`, `Float`, `Bool`, `String`
- Enums (any variant, with or without payload)
- Tuples where all element types are comparable
- Records where all field types are comparable (this RFC adds record equality — §9)

The checker enforces this at the use site (E025).

### 4.5 `Map` in `Inferred<T>` and records

`Map<K, V>` is a valid field type in records:

```sage
record AgentReport {
    topic: String
    findings: Map<String, Float>     // topic → confidence
}
```

`Inferred<Map<K, V>>` is valid when both `K` and `V` are JSON-representable. JSON objects have string keys, so `K` must be `String` for `Inferred<Map<String, V>>`. The codegen rejects other key types for `Inferred<T>` (E026).

The JSON schema for `Map<String, Float>` is:

```json
{ "type": "object", "additionalProperties": { "type": "number" } }
```

### 4.6 Examples

```sage
// Cache LLM results to avoid redundant calls
agent CachingResearcher {
    topics: List<String>
    cache: Map<String, String>

    on start {
        for topic in self.topics {
            if !map_has(self.cache, topic) {
                let summary: Inferred<String> = try infer("Summarise: {topic}")
                map_set(self.cache, topic, summary)
            }
            print(map_get(self.cache, topic))
        }
        emit(self.cache)
    }
}

// Tally votes from sub-agents
agent Tallier {
    votes: Map<String, Int>

    on start {
        let r1 = spawn Voter { candidate: "A" }
        let r2 = spawn Voter { candidate: "B" }
        let r3 = spawn Voter { candidate: "A" }

        for vote in [await r1, await r2, await r3] {
            let current = map_get(self.votes, vote)
            match current {
                Some(n) => map_set(self.votes, vote, n + 1)
                None    => map_set(self.votes, vote, 1)
            }
        }
        emit(self.votes)
    }
}
```

---

## 5. Feature 2 — Enum Payload Variants

### 5.1 Declaration syntax

Variants may optionally carry a single typed payload in parentheses. Variants within the same enum may mix payload and no-payload:

```sage
enum AgentResult {
    Success(String)
    Failed(String)
    Timeout
    Retrying(Int)       // retry count
}

enum ParsedValue {
    Integer(Int)
    Float(Float)
    Text(String)
    Boolean(Bool)
    Nothing
}
```

A payload can be any Sage type, including records, other enums, tuples, lists, and maps:

```sage
enum SearchResult {
    Found(ResearchResult)
    NotFound
    Error(String)
}
```

### 5.2 Construction

Payload variants are constructed like function calls:

```sage
let result = AgentResult.Success("research complete")
let err    = AgentResult.Failed("rate limit exceeded")
let retry  = AgentResult.Retrying(3)
let done   = AgentResult.Timeout       // no-payload variant, unchanged
```

### 5.3 Pattern matching with payload binding

`match` arms for payload variants bind the payload to a name:

```sage
match result {
    AgentResult.Success(msg)     => print("✅ {msg}")
    AgentResult.Failed(reason)   => print("❌ {reason}")
    AgentResult.Retrying(n)      => print("🔄 attempt {n}")
    AgentResult.Timeout          => print("⏱ timed out")
}
```

The binding name (`msg`, `reason`, `n`) is scoped to the arm body. It is not a declaration — it is a pattern binding, introduced by the `match` syntax. If the payload is not needed, `_` discards it:

```sage
match result {
    AgentResult.Success(_) => print("ok")
    _                      => print("not ok")
}
```

Exhaustiveness checking is unchanged: every variant must be covered or `_` must be present.

### 5.4 Multi-field payloads via tuples

A variant carries exactly one payload type. Multiple values are expressed as a tuple (see §6):

```sage
enum Measurement {
    Value(Float, String)    // amount and unit, as a tuple
    Unavailable
}

let m = Measurement.Value(98.6, "°F")

match m {
    Measurement.Value(amount, unit) => print("{amount} {unit}")
    Measurement.Unavailable         => print("no data")
}
```

The payload type `(Float, String)` is a tuple type (§6). Binding in the match arm destructures it directly.

### 5.5 Equality for enums with payloads

Two enum values are `==` if they are the same variant and (for payload variants) their payloads are `==`. The payload type must itself be comparable (E027).

```sage
AgentResult.Success("a") == AgentResult.Success("a")   // true
AgentResult.Success("a") == AgentResult.Success("b")   // false
AgentResult.Success("a") == AgentResult.Failed("a")    // false
AgentResult.Timeout      == AgentResult.Timeout        // true
```

### 5.6 `Inferred<T>` with payload enums

Payload enums are supported in `Inferred<T>`. The JSON schema uses a tagged union format:

```sage
enum Classification {
    Topic(String)
    Sentiment(String)
    Unknown
}
```

Generated schema:

```json
{
  "oneOf": [
    { "type": "object", "properties": { "Topic":     { "type": "string" } }, "required": ["Topic"] },
    { "type": "object", "properties": { "Sentiment": { "type": "string" } }, "required": ["Sentiment"] },
    { "type": "object", "properties": { "Unknown":   { "type": "null"   } }, "required": ["Unknown"] }
  ]
}
```

The runtime deserialises the tagged object and constructs the appropriate variant. For providers that do not support structured output, the schema is prompt-engineered as before (RFC-0005a).

---

## 6. Feature 3 — Tuple Types and Multi-Return

### 6.1 Type syntax

A tuple type is a comma-separated list of types in parentheses, with two or more elements:

```sage
(Int, String)
(Bool, Float, String)
(ResearchResult, ResearchStatus)
```

The unit type `()` is not the zero-element tuple — it remains the `Unit` keyword in Sage source, mapping to Rust's `()`. Tuples must have at least two elements.

### 6.2 Value syntax

Tuple values are constructed the same way:

```sage
let pair: (Int, String) = (42, "hello")
let triple = (true, 3.14, "pi")      // type inferred as (Bool, Float, String)
```

### 6.3 Field access

Tuple fields are accessed by zero-based numeric index using dot notation:

```sage
let pair = (42, "hello")
let n = pair.0    // Int: 42
let s = pair.1    // String: "hello"
```

### 6.4 Destructuring

Tuples are destructured in `let` bindings:

```sage
let (n, s) = (42, "hello")
// n: Int = 42, s: String = "hello"
```

And in `for` loop variables when iterating maps (§4.3) or lists of tuples:

```sage
let pairs: List<(String, Int)> = [("a", 1), ("b", 2)]
for (key, value) in pairs {
    print("{key} = {value}")
}
```

Destructuring must be exhaustive — every position must be bound or discarded with `_`:

```sage
let (first, _) = (42, "ignored")
```

### 6.5 Multi-return from functions

The primary motivation. Functions return a tuple to express multiple values:

```sage
fn parse_score(raw: String) -> (Bool, Float) {
    // returns (success, value)
    if raw == "" {
        return (false, 0.0)
    }
    return (true, 9.5)    // simplified; real parse logic would be more complex
}

agent Main {
    on start {
        let (ok, score) = parse_score("9.5")
        if ok {
            print("Score: {score}")
        }
        emit(0)
    }
}
```

### 6.6 Tuples in records and agent state

Tuples are valid field types:

```sage
record BoundingBox {
    origin: (Float, Float)
    size:   (Float, Float)
}

agent Locator {
    bounds: (Float, Float)
    ...
}
```

### 6.7 Tuple equality

Two tuples are `==` if they have the same element types and all elements are `==`. Comparing tuples of different arities or different element types is a type error (E028):

```sage
(1, "a") == (1, "a")     // true
(1, "a") == (1, "b")     // false
(1, "a") == (1, 2)       // E028: type mismatch
```

### 6.8 Tuples as map keys

Tuples of comparable types are valid map keys:

```sage
let grid: Map<(Int, Int), String> = {}
map_set(grid, (0, 0), "origin")
map_set(grid, (1, 2), "point")
```

### 6.9 Nested tuples

Tuples may be nested:

```sage
let nested: ((Int, Int), String) = ((1, 2), "coord")
let inner = nested.0      // (Int, Int)
let x = inner.0           // Int
```

---

## 7. Feature 4 — `while` Loop

### 7.1 Syntax

```sage
while condition {
    body
}
```

`condition` is any expression of type `Bool`. The body is a block. `break` works inside `while` exactly as it does inside `loop`. `while` is a statement, not an expression — it produces `Unit` and cannot be used in a value position.

```sage
let i = 0
while i < 10 {
    print(i)
    i = i + 1
}
```

### 7.2 Relation to `loop`/`break`

`while cond { body }` is exactly equivalent to:

```sage
loop {
    if !cond { break }
    body
}
```

The desugaring is transparent — the codegen emits a Rust `while` loop directly, not a desugared `loop { if ... break }`.

### 7.3 Interaction with `break`

`break` exits the nearest enclosing `while`, `loop`, or `for`. The checker already tracks `in_loop` for `break` validation. `while` enters the same `in_loop` scope.

### 7.4 Interaction with `receive()`

Agents that listen indefinitely typically use `loop { ... receive() ... }` today. `while` provides an explicit termination condition:

```sage
agent Worker {
    receives Command

    active: Bool

    on start {
        self.active = true
        while self.active {
            let cmd = receive()
            match cmd {
                Command.Process(task) => self.handle(task)
                Command.Shutdown      => self.active = false
            }
        }
        emit(0)
    }
}
```

---

## 8. Feature 5 — `Result<T, E>`

### 8.1 Design overview

`Result<T, E>` is defined in the Sage prelude as a regular enum using the payload variant syntax introduced in §5. It is not a compiler-magic type. The `try`/`catch` syntax from RFC-0007 is sugar over it.

```sage
// Defined in prelude — not user-writable, but semantically equivalent to:
enum Result<T, E> {
    Ok(T)
    Err(E)
}
```

Because Sage does not yet have generics for user-defined types, `Result<T, E>` is a special-cased builtin enum in the type system and codegen, with the same treatment as `List<T>`, `Option<T>`, and `Map<K, V>`. User-defined generic types are out of scope for this RFC.

### 8.2 Type syntax

```sage
Result<String, String>
Result<ResearchResult, AgentError>
Result<Int, Error>          // Error is the existing RFC-0007 error type
```

### 8.3 Construction

```sage
let ok:  Result<Int, String> = Ok(42)
let err: Result<Int, String> = Err("something went wrong")
```

`Ok` and `Err` are constructor functions available in the prelude, not enum variant access syntax. This matches the ergonomic convention from Rust and avoids the `Result.Ok(...)` verbosity.

### 8.4 Pattern matching

```sage
match outcome {
    Ok(value)  => print("got {value}")
    Err(e)     => print("failed: {e}")
}
```

Exhaustiveness applies: both `Ok` and `Err` must be covered or `_` must be present.

### 8.5 Integration with `try`/`catch`

RFC-0007's `try`/`catch` integrates with `Result<T, E>` as follows:

`try expr` unwraps `Ok(v)` to `v`, and propagates `Err(e)` as a `SageError` to the nearest `on error` handler. This is the same semantics as before — RFC-0007 callers are unaffected.

`catch` now also applies to functions returning `Result<T, E>`:

```sage
fn maybe_parse(s: String) -> Result<Int, String> fails {
    if s == "" {
        return Err("empty input")
    }
    return Ok(42)
}

agent Main {
    on start {
        let n = try maybe_parse("42")           // propagates on Err
        let m = maybe_parse("") catch { -1 }    // uses -1 on Err
        emit(n + m)
    }
}
```

When `try` is used on a `Result<T, E>` function, `E` must be convertible to `SageError`. For `E = Error` (the RFC-0007 error type), this is automatic. For `E = String`, the string is wrapped as `SageError::Runtime`. For other `E` types, the error is formatted via `Display`.

### 8.6 `Result<T, E>` as a first-class value

The motivation for this feature. `Result` can be stored in records, passed to functions, and put in collections:

```sage
record BatchResult {
    topic: String
    outcome: Result<String, String>
}

fn collect_results(handles: List<Agent<Result<String, String>>>) -> List<Result<String, String>> {
    let results: List<Result<String, String>> = []
    for h in handles {
        results = results + [await h]
    }
    return results
}

agent BatchCoordinator {
    topics: List<String>

    on start {
        let handles = []
        for topic in self.topics {
            handles = handles + [spawn FallibleResearcher { topic: topic }]
        }

        let results = collect_results(handles)
        let successes = 0
        let failures  = 0

        for r in results {
            match r {
                Ok(_)  => successes = successes + 1
                Err(_) => failures  = failures + 1
            }
        }

        print("Done: {successes} succeeded, {failures} failed")
        emit(successes)
    }
}
```

### 8.7 `Result` in `Inferred<T>`

`Inferred<Result<T, E>>` is valid. The LLM is asked to return either a success value or an error description. The schema is the same tagged-union format as payload enums (§5.6):

```json
{
  "oneOf": [
    { "type": "object", "properties": { "Ok":  { "<T schema>" } }, "required": ["Ok"]  },
    { "type": "object", "properties": { "Err": { "<E schema>" } }, "required": ["Err"] }
  ]
}
```

---

## 9. Feature 6 — Record Equality

### 9.1 Semantics

Two records are `==` if:
1. They are the same record type (structural equality is not cross-type — `Point { x: 1, y: 2 }` is never `==` to `Size { width: 1, height: 2 }`).
2. All corresponding fields are `==`.

Equality is defined recursively. A record field that is itself a record, tuple, list, map, or enum uses the equality rules for that type. All field types must be comparable (E029).

```sage
record Point { x: Int, y: Int }

let a = Point { x: 1, y: 2 }
let b = Point { x: 1, y: 2 }
let c = Point { x: 3, y: 4 }

a == b    // true
a == c    // false
a != c    // true
```

### 9.2 Records in `match`

Record equality enables pattern matching on record values:

```sage
let origin = Point { x: 0, y: 0 }

match p {
    origin => print("at origin")
    _      => print("elsewhere")
}
```

In a `match` arm, a record literal is treated as a pattern that matches by equality. This requires all field types of the record to be comparable (E029).

### 9.3 Records as map keys

Records with all comparable fields may be used as `Map` keys:

```sage
let distances: Map<Point, Float> = {}
map_set(distances, Point { x: 0, y: 0 }, 0.0)
map_set(distances, Point { x: 3, y: 4 }, 5.0)
```

### 9.4 Non-comparable field types

Records containing `Agent<T>`, function types `Fn(...)`, or maps with non-comparable values are not comparable. Using `==` on such a record is E029.

```sage
record Task {
    id: Int
    handler: Fn(String) -> Unit    // not comparable
}

let a = Task { ... }
let b = Task { ... }
a == b    // E029: record contains non-comparable field `handler: Fn(String) -> Unit`
```

### 9.5 Codegen

The Rust codegen already derives `PartialEq` for all record structs (RFC-0005 §12.1). The checker change needed is simply to allow `==` on `Type::Named(record)` when all fields are comparable, rather than rejecting it with a type error. No codegen change is required for records themselves. The `PartialEq` derive handles structural comparison automatically.

---

## 10. Interactions Between Features

The six features compose in ways that must be handled consistently:

### `Map<K, V>` key types

A `Map` key can be a tuple (§6.8), a record with all comparable fields (§9.3), or an enum (including payload enums, §5.5). All use the equality rules defined per type.

### `Result<T, E>` and payload enums

`Result<T, E>` is implemented as a payload enum (§5). The `Ok(T)` and `Err(E)` constructors follow the variant construction syntax. The match binding syntax (`Ok(v)`) follows the payload binding syntax. No special cases in the parser — it's regular Sage.

### Tuple destructuring in `match`

When a payload variant carries a tuple, destructuring in `match` is two layers deep:

```sage
enum Scored {
    Result(Float, String)    // score and label, as tuple
    NoResult
}

match s {
    Scored.Result(score, label) => print("{score}: {label}")
    Scored.NoResult             => print("n/a")
}
```

The parser sees `Scored.Result(score, label)` as a payload variant pattern where the payload is a tuple, and destructures it in one step. This is syntactic sugar — under the hood, the payload is `(Float, String)` and the binding destructures the tuple.

### Record equality and `Inferred<T>`

A record used in `Inferred<T>` that has all comparable fields is now also usable as a map key and in `==` comparisons. There is no conflict — the two properties are independent attributes of the type.

### `while` and `Result<T, E>`

A common pattern: loop until a fallible operation succeeds or a retry limit is hit:

```sage
let attempts = 0
let result: Result<String, String> = Err("not started")

while attempts < 3 {
    let r: Result<String, String> = risky_infer(self.topic)
    match r {
        Ok(v)  => { result = Ok(v); break }
        Err(_) => attempts = attempts + 1
    }
}
```

---

## 11. Checker Rules

### Map type

| Code | Rule |
|------|------|
| E025 | `Map<K, V>` key type `K` is not comparable (not a primitive, enum, comparable tuple, or comparable record) |
| E026 | `Inferred<Map<K, V>>` used with non-String key type `K` |

### Enum payload variants

| Code | Rule |
|------|------|
| E027 | `==` applied to an enum with payload variants where the payload type is not comparable |
| E030 | Payload variant constructed with wrong argument type |
| E031 | Payload variant pattern binding used for a no-payload variant |
| E032 | No-payload variant pattern used for a payload variant |

### Tuple types

| Code | Rule |
|------|------|
| E028 | Tuple `==` applied to tuples of different arities or incompatible element types |
| E033 | Tuple destructuring has wrong number of bindings (e.g. `let (a, b) = triple`) |
| E034 | Tuple index out of bounds (e.g. `.2` on a pair) |

### Record equality

| Code | Rule |
|------|------|
| E029 | `==` applied to a record that contains a non-comparable field |
| E035 | Record used as `Map` key but contains a non-comparable field |

All existing error codes from prior RFCs are unchanged.

### Type inference for tuples

Type inference for tuple literals follows the existing inference rules: if the expected type is known, it is used to check each element; otherwise the element types are inferred independently and the tuple type is assembled from them. Tuple types are never partially inferred — either all elements have known types, or the tuple type annotation is required (E003 type mismatch if it cannot be resolved).

---

## 12. Codegen

### 12.1 `Map<K, V>` → `std::collections::HashMap`

```sage
let scores: Map<String, Int> = {}
map_set(scores, "alice", 10)
let v = map_get(scores, "alice")
```

Generated Rust:

```rust
let mut scores: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
scores.insert("alice".to_string(), 10_i64);
let v: Option<i64> = scores.get("alice").copied();
```

`map_set` → `.insert(k, v)`
`map_get` → `.get(k).cloned()` (for Clone types) or `.get(k).copied()` (for Copy types)
`map_delete` → `.remove(k)`
`map_has` → `.contains_key(k)`
`map_keys` → `.keys().cloned().collect::<Vec<_>>()`
`map_values` → `.values().cloned().collect::<Vec<_>>()`
`len(m)` → `.len() as i64`

For map iteration:

```rust
for (key, value) in &scores {
    // key: &String, value: &i64
}
```

### 12.2 Enum payload variants → Rust enum variants with data

```sage
enum AgentResult {
    Success(String)
    Failed(String)
    Timeout
}
```

Generated Rust:

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
enum AgentResult {
    Success(String),
    Failed(String),
    Timeout,
}
```

Construction:

```sage
let r = AgentResult.Success("done")
```

Generated Rust:

```rust
let r = AgentResult::Success("done".to_string());
```

Pattern match with binding:

```sage
match r {
    AgentResult.Success(msg) => print(msg)
    AgentResult.Failed(e)    => print(e)
    AgentResult.Timeout      => print("timeout")
}
```

Generated Rust:

```rust
match r {
    AgentResult::Success(msg) => { println!("{}", msg); }
    AgentResult::Failed(e)    => { println!("{}", e); }
    AgentResult::Timeout      => { println!("timeout"); }
}
```

### 12.3 Tuple types → Rust tuples

```sage
let pair: (Int, String) = (42, "hello")
let n = pair.0
let s = pair.1
let (a, b) = pair
```

Generated Rust:

```rust
let pair: (i64, String) = (42_i64, "hello".to_string());
let n = pair.0;
let s = pair.1.clone();
let (a, b) = pair.clone();
```

Multi-return from a function:

```sage
fn parse_score(raw: String) -> (Bool, Float) {
    return (true, 9.5)
}
```

Generated Rust:

```rust
fn parse_score(raw: String) -> (bool, f64) {
    return (true, 9.5_f64);
}
```

### 12.4 `while` loop → Rust `while`

```sage
while i < 10 {
    i = i + 1
}
```

Generated Rust:

```rust
while i < 10 {
    i += 1;
}
```

No desugaring. Direct emission.

### 12.5 `Result<T, E>` → Rust `Result<T, E>`

The prelude maps Sage `Result<T, E>` directly to Rust's standard `Result<T, E>`. `Ok(v)` and `Err(e)` are the standard constructors.

```sage
let r: Result<Int, String> = Ok(42)
let e: Result<Int, String> = Err("oops")
```

Generated Rust:

```rust
let r: Result<i64, String> = Ok(42_i64);
let e: Result<i64, String> = Err("oops".to_string());
```

`try expr` on a `Result`-returning function:

```sage
let n = try maybe_parse("42")
```

Generated Rust:

```rust
let n = maybe_parse("42".to_string()).map_err(SageError::from)?;
```

### 12.6 Record equality

No codegen change required. The `#[derive(PartialEq)]` is already emitted for every record (RFC-0005 §12.1). The checker change — permitting `==` on `Type::Named(record)` when fields are comparable — is the only required change. The checker emits a standard binary `==` expression which the codegen already handles.

---

## 13. New Error Codes

| Code | Name | Description |
|------|------|-------------|
| E025 | `NonComparableMapKey` | `Map<K, V>` used with a key type `K` that is not comparable |
| E026 | `InferredMapNonStringKey` | `Inferred<Map<K, V>>` with non-`String` key type |
| E027 | `NonComparableEnumPayload` | `==` on enum with payload variant whose payload type is not comparable |
| E028 | `TupleArityMismatch` | `==` or destructuring applied to tuples of incompatible arities or types |
| E029 | `NonComparableRecordField` | `==` on a record that contains a field of a non-comparable type |
| E030 | `PayloadTypeMismatch` | Payload variant constructed with a value of the wrong type |
| E031 | `BindingOnNoPayload` | Payload pattern binding used on a variant that carries no data |
| E032 | `NoBindingOnPayload` | No-payload pattern used on a variant that carries data |
| E033 | `TupleDestructureArity` | Destructuring binding count does not match tuple element count |
| E034 | `TupleIndexOutOfBounds` | Numeric tuple index exceeds the tuple's element count |
| E035 | `NonComparableMapKeyRecord` | Record used as `Map` key contains a non-comparable field |

---

## 14. Implementation Plan

### Phase 1 — Lexer & Parser (2–3 days)

- Add `while` as a keyword token
- Add `Stmt::While { condition: Expr, body: Block }` to AST
- Add `TypeExpr::Map(Box<TypeExpr>, Box<TypeExpr>)` to the type expression enum
- Add `TypeExpr::Tuple(Vec<TypeExpr>)` to the type expression enum
- Add `TypeExpr::Result(Box<TypeExpr>, Box<TypeExpr>)` to the type expression enum
- Add `Expr::Tuple(Vec<Expr>)` for tuple literal values
- Add `Expr::TupleIndex { expr: Box<Expr>, index: usize }` for `.0`, `.1`, ... access
- Add `Stmt::LetTuple { names: Vec<Ident>, value: Expr }` for tuple destructuring in `let`
- Extend `EnumVariant` AST node with `payload: Option<TypeExpr>`
- Extend `Pattern::Variant` with `binding: Option<Ident>` (or `Vec<Ident>` for tuple payloads)
- Add map literal parsing: `{ key: value, ... }` — guarded by checking first token is a string/int literal or identifier followed by `:`
- Add `Ok(expr)` and `Err(expr)` as constructor expressions (handled as named calls in the AST, resolved to `Result` in the checker)
- Parser tests for all new constructs

### Phase 2 — Type System & Checker (4–5 days)

- Add `Type::Map(Box<Type>, Box<Type>)` to the resolved type enum
- Add `Type::Tuple(Vec<Type>)` to the resolved type enum
- Add `Type::Result(Box<Type>, Box<Type>)` to the resolved type enum
- Implement comparable-type predicate used by E025, E027, E028, E029, E035
- Type-check `while` condition (must be `Bool`), register `in_loop` (already tracked for `loop`/`break`)
- Type-check map literals: infer `K` and `V` from entries; validate key comparability (E025)
- Type-check all map builtins: `map_set`, `map_get`, `map_delete`, `map_has`, `map_keys`, `map_values`
- Overload `len` for `Type::Map` (already works for `List`)
- Type-check tuple literals and index access; validate index bounds (E034)
- Type-check tuple destructuring; validate arity (E033)
- Extend `for` loop type checking to handle `Map` iteration (yields `(K, V)` tuples)
- Type-check payload variant construction (E030) and pattern binding (E031, E032)
- Add `Result<T, E>` to prelude as a built-in parameterised enum
- Allow `==` on `Type::Named(record)` when all fields pass the comparable-type predicate (E029)
- Allow `==` on `Type::Tuple` (E028)
- Allow `==` on enums with payloads when payload types pass comparable-type predicate (E027)
- Update exhaustiveness checking to handle payload variants correctly
- Validate `Map` key types when record or tuple keys are used (E035)
- Checker tests for all eleven new error codes

### Phase 3 — Codegen (3–4 days)

- Emit `std::collections::HashMap<K, V>` for `Type::Map`
- Emit map literal `HashMap::from([...])` for non-empty literals; `HashMap::new()` for `{}`
- Emit all map builtins as `.insert`, `.get`, `.remove`, `.contains_key`, `.keys`, `.values`
- Emit Rust tuple types and tuple value literals
- Emit `.0`, `.1`, ... for tuple index access
- Emit tuple destructuring in `let` bindings
- Emit `for (k, v) in &map` for map iteration
- Emit `while cond { ... }` directly
- Emit payload enum variants in Rust (e.g. `AgentResult::Success(String)`)
- Emit `Ok(v)` and `Err(e)` as standard Rust constructors (no mapping needed — same names)
- Emit `Result<T, E>` type as Rust `Result<T, E>`
- Update `try` codegen to use `.map_err(SageError::from)?` on `Result`-returning functions
- Update JSON schema generation: emit tagged-union `oneOf` schema for payload enums and `Result`
- Codegen snapshot tests for all new constructs

### Phase 4 — Runtime (1 day)

- Add `SageError::from(String)` impl for string-typed `Err` values used with `try`
- Add `infer_structured` schema generation support for payload enums and `Result<T, E>`
- Verify `HashMap` keys implement `Hash + Eq` in generated code (this falls out of the comparable-type predicate; the codegen can emit `#[derive(Hash)]` on comparable records)

### Phase 5 — Tests & Polish (2–3 days)

- End-to-end: map construction, `map_get`/`map_set`/`map_delete`, iteration
- End-to-end: payload enum construction and match binding
- End-to-end: tuple multi-return and destructuring
- End-to-end: `while` loop basic use and `break` interaction
- End-to-end: `Result<T, E>` as a stored value, pattern matched, in a list
- End-to-end: record equality in `if`, `match`, and as map key
- End-to-end: `Inferred<Map<String, Float>>` with mock LLM
- End-to-end: `Inferred<Result<ResearchResult, String>>` with mock LLM
- Update guide documentation for all six features
- Update examples to demonstrate `Result<T, E>` and `Map<K, V>` in agent programs

**Total estimated effort:** ~3 weeks

---

## 15. Open Questions

### 15.1 Map ordering: `HashMap` vs `BTreeMap`

The current spec maps `Map<K, V>` to `HashMap`. This means iteration order is non-deterministic, which can make programs that print maps harder to test and reason about.

**Option A:** Always `HashMap` — fast, consistent with the "maps are unordered" contract.
**Option B:** Always `BTreeMap` — ordered by key, deterministic iteration, requires `K: Ord` instead of `K: Hash + Eq`.
**Option C:** Let the programmer choose (`Map` vs `OrderedMap`).

Recommendation: use `HashMap` for now with a note that a future RFC may introduce `OrderedMap<K, V>`. Requiring `Ord` on keys would exclude some valid key types (e.g. records with `Float` fields, which are `PartialOrd` but not `Ord`).

### 15.2 Multi-field payload variants vs tuples

The spec introduces multi-field payloads as tuple payloads: `Value(Float, String)`. An alternative is to allow multiple positional fields directly in the variant: `Value(Float, String)` would still be the syntax, but the payload would not be a first-class tuple — it would be an unnamed positional struct.

The difference matters if you want to do `let t = variant_value.payload` and get back a `(Float, String)`. Under this RFC the payload is a tuple and this works. Under the alternative it does not.

Recommendation: keep the tuple approach — it is more composable and simpler to specify. The generated Rust uses a tuple variant `Foo(f64, String)` either way.

### 15.3 `Result<T, E>` and the existing `Error` type

RFC-0007 introduced a runtime `Error` type with `.message` and `.kind` fields. `on error(e)` handlers receive a `SageError`, which the Sage runtime exposes as this `Error` type. Should `try` on a `Result<T, E>`-returning function automatically convert `E` to `Error`, or should `E` be freely chosen?

The current spec says "string errors are wrapped, other types are formatted." A cleaner alternative: `try` only works on `Result<T, Error>` (where `Error` is the RFC-0007 type), forcing functions to use `Error` as their error type. This simplifies the conversion story but reduces flexibility.

Recommendation: keep the flexible approach (any `E` for stored `Result` values; `E` is converted when `try` propagates to `on error`). This can be tightened later without breaking existing code.

### 15.4 Tuple types and `Inferred<T>`

This RFC does not explicitly address `Inferred<(Int, String)>`. Tuples do not have a clean JSON schema representation (they become arrays with positional semantics). Possible schemas: `{ "type": "array", "prefixItems": [...] }` (JSON Schema draft 2020-12). This is valid but not universally supported.

Recommendation: defer `Inferred<Tuple>` — emit E024 (invalid infer type, from RFC-0005) if a tuple type appears in `Inferred<T>`. Users should wrap tuples in a named record for LLM output.

### 15.5 Record spread/update syntax

This RFC adds record equality, which makes it easier to notice when you want to create a slightly modified copy of a record. The verbose form still works:

```sage
let updated = ResearchResult {
    topic: old.topic
    summary: "new summary"
    confidence: old.confidence
}
```

A spread syntax (`{ ...old, summary: "new summary" }`) would make this much cleaner. Deferred to a follow-up RFC as it has no dependency on this one.

---

*Ward watches the types align. The map is drawn, the results are typed, and the while loop turns. Sage grows up.*
