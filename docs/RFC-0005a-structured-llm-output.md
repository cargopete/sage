# RFC-0005a: Structured LLM Output via Prompt Engineering

- **Status:** Implemented
- **Created:** 2026-03-13
- **Author:** Sage Contributors
- **Amends:** RFC-0005 §8 (Inferred\<RecordType\>)

---

## Summary

RFC-0005 specified that `Inferred<RecordType>` would use the OpenAI `response_format: json_schema` API for structured output, with a prompt-engineering fallback for other providers. This RFC replaces that two-path approach with a single strategy: **schema-injected prompt engineering with retry**, which works uniformly across all providers including local LLMs via Ollama.

---

## Motivation

Sage's primary target includes developers running local models via Ollama. The OpenAI structured output API (`response_format: json_schema`) is not supported by Ollama or most OpenAI-compatible providers. Building and maintaining two code paths adds complexity for marginal reliability gain. A well-constructed prompt with retry achieves the same result everywhere.

---

## Design

### Single runtime method

`LlmClient` gains one new method:

```rust
pub async fn infer_structured<T: DeserializeOwned>(
    &self,
    prompt: &str,
    schema: &str,
) -> SageResult<T>
```

No provider detection. No `response_format` parameter. Works on OpenAI, Ollama, Mistral, Anthropic, or any OpenAI-compatible endpoint.

### Schema injection

The schema is injected as a system message prepended to every structured inference call:

```
You are a precise assistant that always responds with valid JSON.
You must respond with a JSON object matching this exact schema:

<schema>

Respond with JSON only. No explanation, no markdown, no code blocks.
```

The user's prompt follows as the user message. The schema is the compile-time constant generated from the record or enum definition by the codegen pass (unchanged from RFC-0005).

### Retry on parse failure

Local models occasionally produce malformed JSON. The runtime retries up to 3 times (configurable via `SAGE_INFER_RETRIES`). On retry, the parse error is fed back to the model as context:

```
Your previous response could not be parsed: <error>
Please try again, responding with valid JSON only.
```

Three attempts is sufficient for all well-known local models in practice.

### Ollama `format: json`

When the configured `SAGE_LLM_URL` points to a local Ollama instance (`localhost` or `127.0.0.1`), the runtime automatically adds `"format": "json"` to the request body. This activates Ollama's JSON output mode, which eliminates structural JSON errors (unclosed braces, trailing commas) without requiring schema awareness from the model. It is a best-effort hint, not a replacement for schema injection.

```rust
fn is_ollama(base_url: &str) -> bool {
    base_url.contains("localhost") || base_url.contains("127.0.0.1")
}
```

### Response parsing

Unchanged from the existing `infer<T>` implementation — strip markdown fences, attempt `serde_json::from_str::<T>`. If parsing succeeds, return the value. If all retries are exhausted, return `SageError::Llm` with the last parse error included.

---

## What changes from RFC-0005

| RFC-0005 | This RFC |
|----------|----------|
| Two code paths: OpenAI structured output + fallback | Single path: prompt engineering everywhere |
| Provider capability detection | Ollama auto-detection only (for `format: json` hint) |
| `infer_structured` + `infer_with_schema` methods | Single `infer_structured` method |
| No retry specified | 3 retries, configurable via `SAGE_INFER_RETRIES` |

All other aspects of RFC-0005 §8 are unchanged: schema generation from record/enum definitions, codegen calling `infer_structured` when `T` is a record or enum, and the JSON schema type mapping table.

---

## New environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SAGE_INFER_RETRIES` | Max retries for structured inference | `3` |

---

## Implementation

All changes are confined to `sage-runtime/src/llm.rs`. No changes to the parser, checker, or codegen beyond what RFC-0005 already specifies.
