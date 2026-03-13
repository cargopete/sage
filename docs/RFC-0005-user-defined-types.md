# RFC-0005: User-Defined Types

- **Status:** Implemented
- **Created:** 2026-03-13
- **Author:** Sage Contributors

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Language Changes](#4-language-changes)
5. [Records](#5-records)
6. [Enums](#6-enums)
7. [Pattern Matching](#7-pattern-matching)
8. [Inferred\<RecordType\>](#8-inferredrecordtype)
9. [Agent State Syntax Change](#9-agent-state-syntax-change)
10. [Const](#10-const)
11. [Checker Rules](#11-checker-rules)
12. [Codegen](#12-codegen)
13. [New Error Codes](#13-new-error-codes)
14. [Implementation Plan](#14-implementation-plan)
15. [Open Questions](#15-open-questions)

---

## 1. Summary

This RFC introduces user-defined types to Sage: **records** (named data structures with typed fields) and **enums** (named sets of variants). It also introduces **pattern matching** via `match` expressions, **top-level constants** via `const`, and upgrades `Inferred<T>` to support record types as structured LLM output.

As part of this RFC, the `belief` keyword is **removed**. Agent state is now declared as bare typed fields, consistent with how records declare fields. The `const`/`let` split is formalised: `const` is immutable and top-level, `let` is mutable and local.

---

## 2. Motivation

Currently Sage has no way to model structured domain data. Every value is a primitive or a flat list. This forces awkward patterns:

```sage
// Today: parallel lists, no structure
agent Researcher {
    belief topic: String

    on start {
        let summary = try infer("Summarise: {self.topic}");
        let confidence = try infer("Rate confidence 0-1 for: {summary}");
        // topic, summary, confidence are unrelated variables
        // passing them around requires multiple parameters
        emit(summary);
    }
}
```

With records and enums:

```sage
record ResearchResult {
    topic: String
    summary: String
    confidence: Float
}

agent Researcher {
    topic: String

    on start {
        let result: Inferred<ResearchResult> = try infer(
            "Research {self.topic}. Return a summary and confidence score."
        );
        emit(result);
    }
}
```

The data is structured, the LLM output is typed, and the agent emits a meaningful value instead of a raw string.

---

## 3. Design Goals

1. **Familiar syntax.** Records look like records in other languages — no ceremony.
2. **Consistent mutability model.** `const` bindings (including records) are fully immutable. `let` bindings are fully mutable, including their fields.
3. **`Inferred<T>` for records is the headline feature.** Structured LLM output with static type safety is a key differentiator for Sage.
4. **Simple enums first.** Variants carry no data in this RFC. Payload-carrying variants are deferred.
5. **`match` is exhaustive by default.** The checker rejects non-exhaustive matches.

---

## 4. Language Changes Summary

| Change | Description |
|--------|-------------|
| `record` | New top-level declaration for named data types |
| `enum` | New top-level declaration for named variant sets |
| `match` | New expression for pattern matching |
| `const` | New top-level immutable binding |
| `belief` removed | Agent state is now bare `name: Type` declarations |
| `Inferred<T>` upgraded | Now works with any record type |

---

## 5. Records

### 5.1 Declaration

Records are declared at the top level with the `record` keyword:

```sage
record ResearchResult {
    topic: String
    summary: String
    confidence: Float
}
```

Field types can be any Sage type, including other records, lists, and enums:

```sage
record Pipeline {
    name: String
    steps: List<String>
    status: PipelineStatus     // an enum
    result: ResearchResult     // another record
}
```

### 5.2 Construction

Records are constructed by name with field initialisers:

```sage
let result = ResearchResult {
    topic: "quantum computing"
    summary: "Quantum computing uses quantum mechanics..."
    confidence: 0.92
}
```

All fields must be provided. There are no default values. Omitting a field is a compile error (E018).

### 5.3 Field access

Fields are accessed with dot notation:

```sage
print(result.topic)
print(result.confidence)
```

Nested access works naturally:

```sage
print(pipeline.result.summary)
```

### 5.4 Mutability

Mutability follows the `const`/`let` split:

```sage
// const — fully immutable, fields cannot be reassigned
const DEFAULT_RESULT = ResearchResult {
    topic: "unknown"
    summary: "none"
    confidence: 0.0
}

DEFAULT_RESULT.confidence = 0.9   // E021: cannot assign to field of const binding

// let — mutable, fields can be reassigned
let result = ResearchResult {
    topic: "quantum computing"
    summary: "none"
    confidence: 0.0
}

result.confidence = 0.92          // fine
result.summary = "Updated..."     // fine
```

The same rule applies to agent state fields — they are mutable because they behave like `let` bindings:

```sage
agent Tracker {
    count: Int
    last_topic: String

    on start {
        self.count = self.count + 1       // fine
        self.last_topic = "quantum"       // fine
    }
}
```

### 5.5 Records as agent state

Records can be used as agent state fields:

```sage
record Config {
    model: String
    max_retries: Int
}

agent Researcher {
    topic: String
    config: Config

    on start {
        let summary = try infer("Summarise: {self.topic}");
        emit(summary);
    }
}

// Spawning with a record field
let r = spawn Researcher {
    topic: "quantum computing"
    config: Config { model: "gpt-4o", max_retries: 3 }
};
```

### 5.6 Records as function parameters and return types

```sage
fn format_result(result: ResearchResult) -> String {
    "{result.topic}: {result.summary} (confidence: {result.confidence})"
}

fn make_result(topic: String) -> ResearchResult {
    ResearchResult {
        topic: topic
        summary: "pending"
        confidence: 0.0
    }
}
```

---

## 6. Enums

### 6.1 Declaration

Enums declare a fixed set of named variants:

```sage
enum ResearchStatus {
    Pending
    Complete
    Failed
    Timeout
}
```

### 6.2 Usage

Variants are referenced by `EnumName.VariantName`:

```sage
let status = ResearchStatus.Pending

if status == ResearchStatus.Complete {
    print("done")
}
```

Enums can be used as record fields, function parameters, agent state, and `Inferred<T>` types:

```sage
record ResearchResult {
    topic: String
    summary: String
    status: ResearchStatus
}

agent Researcher {
    topic: String
    status: ResearchStatus

    on start {
        self.status = ResearchStatus.Pending
        let summary = try infer("Summarise: {self.topic}");
        self.status = ResearchStatus.Complete
        emit(summary);
    }

    on error(e) {
        self.status = ResearchStatus.Failed
        emit("unavailable")
    }
}
```

### 6.3 Equality

Enum variants support `==` and `!=`:

```sage
if result.status == ResearchStatus.Failed {
    print("research failed")
}
```

### 6.4 Payload-carrying variants

Variants with attached data (e.g. `Failed(String)`) are **deferred** to a future RFC. This RFC covers simple variants only. The design is intentionally forward-compatible — adding payloads later does not break existing code.

---

## 7. Pattern Matching

### 7.1 Basic `match`

`match` branches on a value. Every variant must be covered:

```sage
match result.status {
    ResearchStatus.Pending  => print("still running")
    ResearchStatus.Complete => print("done")
    ResearchStatus.Failed   => print("failed")
    ResearchStatus.Timeout  => print("timed out")
}
```

### 7.2 `match` is an expression

`match` produces a value when all arms produce the same type:

```sage
let label = match result.status {
    ResearchStatus.Pending  => "⏳ pending"
    ResearchStatus.Complete => "✅ complete"
    ResearchStatus.Failed   => "❌ failed"
    ResearchStatus.Timeout  => "⏱ timeout"
}

print(label)
```

### 7.3 Wildcard `_`

The wildcard `_` matches any value not covered by earlier arms:

```sage
let is_done = match result.status {
    ResearchStatus.Complete => true
    _ => false
}
```

When `_` is present, exhaustiveness is satisfied regardless of how many variants remain uncovered. Earlier arms still take priority.

### 7.4 Exhaustiveness

The checker requires every `match` to be exhaustive. If any variant is missing and no `_` is present, the program fails to compile (E020):

```sage
// E020: non-exhaustive match — ResearchStatus.Timeout not covered
match result.status {
    ResearchStatus.Pending  => print("pending")
    ResearchStatus.Complete => print("done")
    ResearchStatus.Failed   => print("failed")
    // missing: Timeout
}
```

### 7.5 Matching on other types

`match` works on any comparable type, not just enums:

```sage
// match on Int
match code {
    0 => print("success")
    1 => print("error")
    _ => print("unknown")
}

// match on String
match topic {
    "quantum" => print("physics topic")
    "crispr"  => print("biology topic")
    _         => print("other topic")
}
```

For non-enum types, `_` is required to ensure exhaustiveness since the compiler cannot enumerate all possible values (E020).

### 7.6 `match` inside agent handlers

```sage
agent Coordinator {
    on start {
        let r = spawn Researcher { topic: "quantum computing" }
        let result = try await r

        let message = match result.status {
            ResearchStatus.Complete => "Research complete: {result.summary}"
            ResearchStatus.Failed   => "Research failed"
            _                       => "Research in unknown state"
        }

        print(message)
        emit(0)
    }
}
```

---

## 8. Inferred\<RecordType\>

This is the headline feature of this RFC. When `T` in `Inferred<T>` is a record type, Sage generates a JSON schema from the record definition and instructs the LLM to respond with structured output matching that schema. The response is automatically deserialised into the record.

### 8.1 Basic usage

```sage
record ResearchResult {
    topic: String
    summary: String
    confidence: Float
}

agent Researcher {
    topic: String

    on start {
        let result: Inferred<ResearchResult> = try infer(
            "Research the topic '{self.topic}'. Return a summary and a confidence score between 0 and 1."
        );

        print("Summary: {result.summary}")
        print("Confidence: {result.confidence}")
        emit(result)
    }

    on error(e) {
        emit(ResearchResult {
            topic: self.topic
            summary: "unavailable"
            confidence: 0.0
        })
    }
}
```

### 8.2 How it works

The compiler generates a JSON schema from the record at compile time:

```json
{
  "type": "object",
  "properties": {
    "topic":      { "type": "string" },
    "summary":    { "type": "string" },
    "confidence": { "type": "number" }
  },
  "required": ["topic", "summary", "confidence"]
}
```

This schema is embedded in the generated Rust code and passed to the LLM runtime alongside the prompt. The runtime uses the OpenAI structured output API (or equivalent) to constrain the response to valid JSON matching the schema. The JSON is then deserialised into a Rust struct generated from the Sage record.

### 8.3 Type mapping

| Sage field type | JSON schema type |
|-----------------|-----------------|
| `String` | `"string"` |
| `Int` | `"integer"` |
| `Float` | `"number"` |
| `Bool` | `"boolean"` |
| `List<T>` | `"array"` with item schema for `T` |
| `Enum` | `"string"` with `"enum"` constraint listing variant names |
| Nested `record` | `"object"` with nested schema |

### 8.4 Inferred\<Enum\>

Enums also work with `Inferred<T>`:

```sage
enum Sentiment {
    Positive
    Neutral
    Negative
}

agent Analyser {
    text: String

    on start {
        let sentiment: Inferred<Sentiment> = try infer(
            "Classify the sentiment of: '{self.text}'"
        );

        match sentiment {
            Sentiment.Positive => print("👍 positive")
            Sentiment.Neutral  => print("😐 neutral")
            Sentiment.Negative => print("👎 negative")
        }

        emit(sentiment)
    }
}
```

The schema for an enum becomes:
```json
{ "type": "string", "enum": ["Positive", "Neutral", "Negative"] }
```

### 8.5 Compile-time schema generation

The schema is generated at compile time by the codegen pass, not at runtime. This means:
- No reflection or runtime type inspection
- Schema is baked into the binary as a string constant
- Adding or renaming a field changes the schema automatically

### 8.6 Nested records in `Inferred<T>`

Records can contain other records, and the schema nests accordingly:

```sage
record Source {
    url: String
    credibility: Float
}

record ResearchResult {
    summary: String
    confidence: Float
    sources: List<Source>
}

agent DeepResearcher {
    topic: String

    on start {
        let result: Inferred<ResearchResult> = try infer(
            "Research '{self.topic}' thoroughly. Include sources."
        );
        emit(result)
    }
}
```

---

## 9. Agent State Syntax Change

### 9.1 Removing `belief`

The `belief` keyword is removed. Agent state fields are declared as bare `name: Type` lines inside the agent body, before any `on` handlers:

**Before (deprecated):**
```sage
agent Researcher {
    belief topic: String
    belief status: ResearchStatus

    on start { ... }
}
```

**After:**
```sage
agent Researcher {
    topic: String
    status: ResearchStatus

    on start { ... }
}
```

### 9.2 Parsing rule

The parser distinguishes state fields from handlers by looking at the first token of each item in the agent body:
- `on` → handler declaration
- identifier → state field declaration (`name: Type`)

This is unambiguous. No keyword is needed.

### 9.3 Spawning with state fields

Spawn syntax is unchanged — fields are still initialised by name:

```sage
let r = spawn Researcher {
    topic: "quantum computing"
    status: ResearchStatus.Pending
}
```

---

## 10. Const

### 10.1 Declaration

`const` declares a top-level immutable binding:

```sage
const MAX_RETRIES = 3
const DEFAULT_MODEL = "gpt-4o"
const FALLBACK_RESULT = ResearchResult {
    topic: "unknown"
    summary: "unavailable"
    confidence: 0.0
}
```

`const` bindings must be initialised with a value that is fully known at compile time (a literal, a record of literals, or another `const`). Calling functions or `infer` in a `const` initialiser is a compile error (E022).

### 10.2 Type inference

`const` supports type inference from the initialiser:

```sage
const MAX_RETRIES = 3          // inferred: Int
const DEFAULT_MODEL = "gpt-4o" // inferred: String
```

Explicit annotation is also valid:

```sage
const MAX_RETRIES: Int = 3
```

### 10.3 Scope

`const` is module-level only. Declaring `const` inside a function or handler is a compile error (E023). Local immutability is achieved by not reassigning a `let` binding — the checker can warn on this in future, but does not enforce it in this RFC.

### 10.4 Usage

`const` bindings can be used anywhere a value of their type is valid:

```sage
const MAX = 10

agent Main {
    on start {
        let x = MAX + 5
        emit(x)
    }
}
```

---

## 11. Checker Rules

### 11.1 Record construction

- All fields must be provided at construction (E018: missing field)
- No extra fields allowed (E019: unknown field — already exists for spawn, now generalised)
- Field values must match declared types

### 11.2 Field access

- Accessing a field that does not exist on a record is E019
- Field access on a non-record type is E003 (type mismatch)

### 11.3 Field mutation

- Assigning to a field of a `const` binding is E021: `const` field assignment
- Assigning to `self.field` where the field does not exist is E019

### 11.4 `match` exhaustiveness

- Non-exhaustive match on an enum without `_` is E020: non-exhaustive match
- Non-exhaustive match on a non-enum type without `_` is also E020
- Unreachable arms (after `_`) produce a warning, not an error

### 11.5 `match` arm type consistency

- All arms in an expression `match` must produce the same type (E003)
- `match` used as a statement (value discarded) has no type constraint

### 11.6 `const` rules

- `const` initialisers must be compile-time values (E022: non-const initialiser)
- `const` inside a function or handler is E023: `const` in local scope

### 11.7 `Inferred<T>` with records

- `T` in `Inferred<T>` must be a primitive, record, enum, or `List<T>` — not a function or agent type (E024: invalid infer type)
- The checker validates that all field types in a record used with `Inferred<T>` are also schema-generatable (recursively)

### 11.8 Removing `belief`

- The parser no longer accepts `belief` as a keyword inside agent bodies
- Programs using `belief` produce a parse error with a helpful migration hint: "use bare field declarations instead: `topic: String`"

---

## 12. Codegen

### 12.1 Records → Rust structs

Sage records generate Rust structs with `serde` derives:

**Sage:**
```sage
record ResearchResult {
    topic: String
    summary: String
    confidence: Float
}
```

**Generated Rust:**
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ResearchResult {
    topic: String,
    summary: String,
    confidence: f64,
}
```

### 12.2 Enums → Rust enums

**Sage:**
```sage
enum ResearchStatus {
    Pending
    Complete
    Failed
    Timeout
}
```

**Generated Rust:**
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
enum ResearchStatus {
    Pending,
    Complete,
    Failed,
    Timeout,
}
```

`rename_all = "PascalCase"` ensures JSON serialisation uses the same names as Sage source, matching the schema the LLM receives.

### 12.3 `match` → Rust `match`

**Sage:**
```sage
let label = match result.status {
    ResearchStatus.Complete => "done"
    ResearchStatus.Failed   => "failed"
    _                       => "other"
}
```

**Generated Rust:**
```rust
let label = match result.status {
    ResearchStatus::Complete => "done".to_string(),
    ResearchStatus::Failed   => "failed".to_string(),
    _                        => "other".to_string(),
};
```

### 12.4 `Inferred<RecordType>` schema generation

For each record used as `Inferred<T>`, the codegen generates a JSON schema constant and passes it to the LLM client:

**Generated Rust:**
```rust
const RESEARCHRESULT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "topic":      { "type": "string" },
    "summary":    { "type": "string" },
    "confidence": { "type": "number" }
  },
  "required": ["topic", "summary", "confidence"],
  "additionalProperties": false
}"#;
```

The `LlmClient::infer` call is updated to accept an optional schema:

```rust
// In sage-runtime
pub async fn infer_structured<T: DeserializeOwned>(
    &self,
    prompt: &str,
    schema: &str,
) -> SageResult<T> { ... }
```

When schema is provided, the runtime uses the OpenAI `response_format` structured output parameter. For non-OpenAI providers that don't support structured output natively, the runtime falls back to prompt-engineering the schema into the system message and parsing the JSON response.

### 12.5 `const` → Rust `const`

**Sage:**
```sage
const MAX_RETRIES = 3
const DEFAULT_MODEL = "gpt-4o"
```

**Generated Rust:**
```rust
const MAX_RETRIES: i64 = 3;
const DEFAULT_MODEL: &str = "gpt-4o";
```

### 12.6 Agent state without `belief`

The codegen for agent state is unchanged in output — it was already generating Rust struct fields. The only change is the parser no longer requires the `belief` keyword.

---

## 13. New Error Codes

| Code | Name | Description |
|------|------|-------------|
| E018 | `MissingRecordField` | Record construction is missing one or more fields |
| E019 | `UnknownField` | Field does not exist on record type (generalised from spawn) |
| E020 | `NonExhaustiveMatch` | `match` does not cover all variants and has no `_` wildcard |
| E021 | `ConstFieldAssignment` | Attempt to assign to a field of a `const` binding |
| E022 | `NonConstInitialiser` | `const` initialiser is not a compile-time value |
| E023 | `ConstInLocalScope` | `const` declared inside a function or handler |
| E024 | `InvalidInferType` | Type used in `Inferred<T>` cannot be represented as a JSON schema |

---

## 14. Implementation Plan

### Phase 1 — Lexer & Parser (2–3 days)

- Add `record`, `enum`, `match`, `const` as keyword tokens
- Remove `belief` as a keyword (keep as identifier for migration error message)
- Parse `record` declarations at top level
- Parse `enum` declarations at top level
- Parse `const` declarations at top level
- Parse agent state as bare `name: Type` fields (removing `belief` requirement)
- Parse `match` expressions with arms (`Pattern => Expr`)
- Parse record construction `RecordName { field: value, ... }`
- Parse dot access `expr.field` (already partially exists for `self.field`)

### Phase 2 — Type System & Checker (3–4 days)

- Add `Type::Record(String)` and `Type::Enum(String)` variants
- Add record and enum definitions to `SymbolTable`
- Implement record construction type checking (E018, E019)
- Implement field access type checking
- Implement field mutation checking (E021)
- Implement `match` exhaustiveness checking (E020)
- Implement `match` arm type consistency checking
- Implement `const` checking (E022, E023)
- Implement `Inferred<T>` validation for record/enum types (E024)
- Remove `belief` handling from checker

### Phase 3 — Codegen (3–4 days)

- Generate Rust structs from record declarations
- Generate Rust enums from enum declarations
- Generate `match` expressions
- Generate `const` declarations
- Implement JSON schema generation from record/enum types
- Update `infer` codegen to pass schema when `T` is a record or enum
- Update agent struct generation to not require `belief` keyword

### Phase 4 — Runtime (1–2 days)

- Add `infer_structured<T>` to `LlmClient` with schema parameter
- Implement structured output via OpenAI `response_format` API
- Implement fallback for non-structured-output providers

### Phase 5 — Tests & polish (2–3 days)

- End-to-end test: record construction, field access, field mutation
- End-to-end test: enum declaration and `match`
- End-to-end test: `Inferred<RecordType>` with mock LLM
- End-to-end test: `Inferred<EnumType>`
- Checker tests for all seven new error codes
- Migration error message for programs still using `belief`
- Update all examples to remove `belief`
- Update guide documentation

**Total estimated effort:** ~2.5 weeks

---

## 15. Open Questions

### 15.1 Record update syntax

Constructing a new record from an existing one with some fields changed is verbose today:

```sage
let updated = ResearchResult {
    topic: result.topic
    summary: "new summary"
    confidence: result.confidence
}
```

A spread/update syntax could help:

```sage
// Hypothetical
let updated = ResearchResult { ...result, summary: "new summary" }
```

Deferred — the verbose form works and there is no ambiguity. Can be added later without breaking changes.

### 15.2 Enum payload variants

`Failed` can't carry a reason, `Complete` can't carry a value. Payload variants unlock much richer modelling:

```sage
// Future RFC
enum AgentResult {
    Success(ResearchResult)
    Failed(String)
    Timeout
}
```

This requires extending `match` to bind payload values, which is a meaningful additional surface area. Deferred to RFC-0006.

### 15.3 Record equality

Currently there is no `==` for records — the checker would need to generate `PartialEq` and the codegen already derives it. Should `==` work on records by structural equality? Likely yes, deferred to avoid scope creep.

### 15.4 Optional fields

Some LLM responses may not always include every field. Should records support optional fields?

```sage
// Hypothetical
record ResearchResult {
    topic: String
    summary: String
    confidence: Float?    // optional
}
```

This interacts with `Inferred<T>` in important ways — the JSON schema would mark the field as not required. Deferred. For now, all fields are required.

### 15.5 Structured output provider compatibility

The OpenAI structured output API is relatively new and not universally supported by OpenAI-compatible providers. The fallback (prompt-engineering the schema) is less reliable. A future RFC should formalise the provider capability model and give users control over which strategy is used.

---

*This RFC moves Sage from a language that can only express agent wiring to one that can model the data flowing through that wiring — and ask the LLM to produce it in the right shape.*
