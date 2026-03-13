# RFC-0011: First-Class Tool Support via MCP

- **Status:** Draft
- **Created:** 2026-03-13
- **Author:** Pete Pavlovski
- **Depends on:** RFC-0003 (Compile to Rust), RFC-0005 (User-Defined Types), RFC-0007 (Error Handling), RFC-0010 (Maps, Tuples, Result)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [The `tool` Declaration](#4-the-tool-declaration)
5. [Built-in Tools](#5-built-in-tools)
6. [Agents Using Tools](#6-agents-using-tools)
7. [Type System](#7-type-system)
8. [Error Handling](#8-error-handling)
9. [Configuration and Connection](#9-configuration-and-connection)
10. [Codegen](#10-codegen)
11. [Checker Rules](#11-checker-rules)
12. [New Error Codes](#12-new-error-codes)
13. [Implementation Plan](#13-implementation-plan)
14. [Future: Runtime-Only MCP (Without Language Primitives)](#14-future-runtime-only-mcp-without-language-primitives)
15. [Future: Sage as an MCP Server](#15-future-sage-as-an-mcp-server)
16. [Open Questions](#16-open-questions)
17. [Alternatives Considered](#17-alternatives-considered)

---

## 1. Summary

This RFC introduces **`tool`** as a first-class language primitive in Sage. A `tool` declaration defines a typed interface to an external capability — a filesystem, an HTTP endpoint, a database, a browser — backed at runtime by the Model Context Protocol (MCP). Agents declare which tools they use, call tool functions with typed arguments, and receive typed results. The compiler knows the full tool signature at compile time, enabling type checking, autocompletion, and clear error messages.

This RFC also specifies the first **built-in tool library** that ships with Sage: `Http`. More built-in tools (`Fs`, `Browser`, `Kv`) are defined and ready for implementation in subsequent releases.

Finally, this RFC includes forward-looking notes on two future directions: using MCP as a pure runtime feature without language-level declarations (§14), and exposing Sage agents as MCP servers consumable by other systems (§15).

---

## 2. Motivation

### The dominant AI app pattern is missing from Sage

Sage was built around multi-agent coordination: `spawn`, `await`, `send`, `receive`. That model excels when you have multiple specialised AI reasoners working together. But the pattern that is actually dominant in production AI systems today is simpler:

```
User → Agent → Tools → Response
```

One agent. A set of tools. The agent reasons, decides which tool to call, calls it, observes the result, reasons again. This is what Claude Code does. This is what ChatGPT plugins do. This is what most production agents at AI companies ship.

Sage currently has no answer to this. An agent cannot read a file, make an HTTP request, query a database, or interact with any external system except through another agent or through a raw LLM call. This is the single largest gap between Sage and practical usefulness for the AI application developers it targets.

### Why MCP and not a bespoke Sage tool system

The Model Context Protocol is becoming the de facto standard for how AI agents interact with external tools. It has growing adoption across major AI providers, a rich existing ecosystem of MCP servers (filesystem, GitHub, Postgres, Slack, browsers, and hundreds more), and a well-defined transport and schema layer.

By grounding Sage's tool system in MCP, Sage agents can immediately use any existing MCP server without requiring a new Sage-specific integration for each one. The protocol handles the transport; Sage handles the types.

### Why language-level primitives and not just a library

The VISION.md sketch of tool support as a library call is inadequate:

```sage
// Not good enough
let result = call_tool("filesystem", "read", { "path": "/tmp/data.txt" })
```

This loses all type safety, requires runtime string matching, gives no autocomplete, and produces runtime errors where compile-time errors belong. The whole point of Sage is that the type system understands what agents do. A tool call is as fundamental as a `spawn` or an `infer` — it deserves the same first-class treatment.

Language-level `tool` declarations give the compiler full knowledge of every tool a program uses:

```sage
tool Fs {
    fn read(path: String) -> Result<String, String>
    fn write(path: String, content: String) -> Result<Unit, String>
    fn exists(path: String) -> Bool
}
```

The checker can verify that every call matches the declared signature, that error cases are handled, and that no agent uses a tool it hasn't declared. The codegen knows exactly what MCP calls to emit.

---

## 3. Design Goals

1. **`tool` is syntactically parallel to `agent` and `record`.** It is a top-level declaration. It is named. It contains typed function signatures. It should feel native to Sage, not bolted on.
2. **Agents declare tool dependencies explicitly.** `use Fs, Http` in an agent declaration. No implicit ambient tool access. The compiler rejects calls to tools the agent hasn't declared.
3. **Tool calls are fallible by default.** Every tool function that touches an external system can fail. The type system enforces that failures are handled — either via `Result<T, E>` return types, `try`, or `catch`.
4. **Built-in tools ship with Sage.** `Http` is in the first release. `Fs`, `Browser`, and `Kv` follow in subsequent releases. These use well-known MCP servers that Sage can either bundle or connect to automatically.
5. **User-defined tools connect to any MCP server.** A programmer can declare a `tool` whose signature mirrors any MCP server's tool list and connect it via `sage.toml` configuration.
6. **Tool declarations are statically verified against the MCP server at build time (optional).** When network access is available, `sage check` can fetch the server's tool manifest and verify that the declared signatures match. This is opt-in.
7. **The runtime MCP path is left open for future use.** This RFC does not close the door on dynamic tool discovery or late-bound tool calls (§14).

---

## 4. The `tool` Declaration

### 4.1 Syntax

A `tool` declaration lives at the top level of a `.sg` file, alongside `agent`, `record`, and `enum`:

```sage
tool Fs {
    fn read(path: String) -> Result<String, String>
    fn write(path: String, content: String) -> Result<Unit, String>
    fn list(dir: String) -> Result<List<String>, String>
    fn exists(path: String) -> Bool
    fn delete(path: String) -> Result<Unit, String>
}
```

A `tool` body contains only function signatures — no implementations, no bodies. It is a pure interface declaration. Every function in a `tool` declaration is implicitly `async` at the runtime level (all MCP calls are async) but Sage source code does not require `await` on tool calls — the compiler inserts the async machinery transparently, consistent with how `infer` works.

### 4.2 Function signature rules

- Parameter types may be any Sage type that is JSON-serialisable: primitives, records, enums, lists, maps with string keys, tuples. The MCP protocol uses JSON for arguments and results.
- Return types may be any JSON-serialisable Sage type, `Unit`, or `Result<T, E>` where both `T` and `E` are JSON-serialisable.
- Functions that can fail **should** return `Result<T, E>`. Functions that are defined to always succeed (e.g. a pure computation tool) may return a plain type.
- Variadic or optional parameters are not supported in this RFC. All parameters are required. Default values are out of scope.

### 4.3 `pub` visibility

Tool declarations follow the same visibility rules as agents and records. `pub tool Fs { ... }` makes the tool importable by other modules. Without `pub`, it is private to its file.

### 4.4 Tool declarations are not instantiated

A `tool` is not constructed or spawned like an agent. It is a type-level declaration. When an agent declares `use Fs`, the runtime binds a connection to the configured MCP server for `Fs` when the agent starts. The agent then calls `Fs.read(...)` as if calling a function — the runtime handles the MCP transport.

---

## 5. Built-in Tools

Built-in tools are shipped with Sage. They do not need to be declared by the user — they are available to any agent that imports them with `use`. Their signatures are defined in the Sage prelude. Their runtime backends are MCP servers that Sage either bundles or connects to automatically.

### 5.1 `Http` — first release

The `Http` tool provides outbound HTTP requests. It is backed by a lightweight MCP server bundled with the Sage runtime.

```sage
// Defined in prelude — not user-writable, but semantically equivalent to:

tool Http {
    fn get(url: String) -> Result<HttpResponse, String>
    fn post(url: String, body: String) -> Result<HttpResponse, String>
    fn post_json(url: String, body: String) -> Result<HttpResponse, String>
    fn put(url: String, body: String) -> Result<HttpResponse, String>
    fn delete(url: String) -> Result<HttpResponse, String>
    fn get_with_headers(url: String, headers: Map<String, String>) -> Result<HttpResponse, String>
}

record HttpResponse {
    status: Int
    body: String
    headers: Map<String, String>
}
```

`Http` does not require any configuration. It is always available. Timeouts and retries are controlled via environment variables (`SAGE_HTTP_TIMEOUT_MS`, `SAGE_HTTP_MAX_RETRIES`).

### 5.2 `Fs` — next release

The `Fs` tool provides filesystem access. It requires explicit opt-in in `sage.toml` because filesystem access carries security implications.

```sage
// Prelude definition (next release):
tool Fs {
    fn read(path: String) -> Result<String, String>
    fn read_bytes(path: String) -> Result<List<Int>, String>
    fn write(path: String, content: String) -> Result<Unit, String>
    fn append(path: String, content: String) -> Result<Unit, String>
    fn exists(path: String) -> Bool
    fn list(dir: String) -> Result<List<String>, String>
    fn delete(path: String) -> Result<Unit, String>
    fn mkdir(path: String) -> Result<Unit, String>
}
```

`Fs` is sandboxed to the paths declared in `sage.toml`:

```toml
[tools.fs]
allow_paths = ["./data", "/tmp"]
```

### 5.3 `Browser` — future release

The `Browser` tool provides web page fetching and interaction, backed by a headless browser MCP server (e.g. Playwright-based).

```sage
// Prelude definition (future):
tool Browser {
    fn fetch(url: String) -> Result<String, String>         // returns HTML
    fn screenshot(url: String) -> Result<String, String>    // returns base64 PNG
    fn click(selector: String) -> Result<Unit, String>
    fn type_text(selector: String, text: String) -> Result<Unit, String>
    fn get_text(selector: String) -> Result<String, String>
}
```

### 5.4 `Kv` — future release

A simple key-value store for agent state persistence between runs.

```sage
// Prelude definition (future):
tool Kv {
    fn get(key: String) -> Option<String>
    fn set(key: String, value: String) -> Unit
    fn delete(key: String) -> Unit
    fn list(prefix: String) -> List<String>
}
```

---

## 6. Agents Using Tools

### 6.1 Declaring tool usage

An agent declares which tools it uses with `use` inside the agent body, before any field declarations and handlers:

```sage
agent WebResearcher {
    use Http

    topic: String

    on start {
        let response = try Http.get("https://en.wikipedia.org/wiki/{self.topic}")
        let summary: Inferred<String> = try infer(
            "Summarise this Wikipedia page in 3 sentences: {response.body}"
        )
        emit(summary)
    }

    on error(e) {
        emit("Could not research {self.topic}: {e.message}")
    }
}
```

### 6.2 Call syntax

Tool functions are called with dot notation on the tool name:

```sage
Http.get(url)
Fs.read(path)
MyCustomTool.query(sql)
```

Tool function names are resolved at compile time against the tool declaration. Calling a function that doesn't exist on the declared tool is E036.

### 6.3 Multiple tools

An agent may use multiple tools:

```sage
agent ContentPipeline {
    use Http, Fs

    url: String
    output_path: String

    on start {
        let page = try Http.get(self.url)
        let summary: Inferred<String> = try infer(
            "Extract the main content from: {page.body}"
        )
        try Fs.write(self.output_path, summary)
        emit(0)
    }
}
```

### 6.4 Tool calls are `try`-able

All tool functions that return `Result<T, E>` are fallible and follow RFC-0007's `try`/`catch` model:

```sage
// Propagate to on error handler
let content = try Fs.read("/data/config.json")

// Handle inline
let content = Fs.read("/data/config.json") catch { "{}" }

// Explicit match
let result = Fs.read("/data/config.json")
match result {
    Ok(content)  => process(content)
    Err(reason)  => print("Read failed: {reason}")
}
```

### 6.5 User-defined tools

Beyond built-ins, programmers can declare their own tool interfaces and connect them to any MCP server. A `tool` declaration is just a typed interface — the MCP server URL is configured separately:

```sage
// src/tools/database.sg

pub tool Database {
    fn query(sql: String) -> Result<List<Map<String, String>>, String>
    fn execute(sql: String) -> Result<Int, String>   // returns rows affected
    fn begin() -> Result<Unit, String>
    fn commit() -> Result<Unit, String>
    fn rollback() -> Result<Unit, String>
}
```

```toml
# sage.toml
[tools.database]
mcp_server = "http://localhost:5432/mcp"
```

The tool name in `[tools.database]` must match the tool declaration name (case-insensitive). If no `mcp_server` is configured for a user-defined tool and the program attempts to use it, a clear runtime error is produced.

---

## 7. Type System

### 7.1 `Tool<T>` handle type

A tool is not a value in the type system in this RFC. You cannot store a tool in a variable, pass it as a function argument, or put it in a list. Tools are used only via `ToolName.function(...)` call syntax within agent handlers where the tool is declared.

This is intentional and consistent with how `infer` works — `infer` is also not a value you can pass around. Tools are capabilities of the agent, not objects. A future RFC may introduce `Tool<T>` handles for dynamic tool injection (§16.1).

### 7.2 JSON-serialisability constraint

Tool function parameters and return types must be JSON-serialisable. The same predicate used for `Inferred<T>` (RFC-0005 §11.7) applies here. Non-serialisable types (`Agent<T>`, `Fn(...)`, non-string-keyed maps) are rejected at the call site (E037).

### 7.3 `Result<T, E>` in tool return types

The error type `E` in `Result<T, E>` for tool functions should be `String` or the RFC-0007 `Error` type. Both are acceptable. `String` errors are the convention for built-in tools (simple error messages). The checker does not enforce which error type is used in the tool declaration, but it enforces that whatever is declared is used consistently at call sites.

### 7.4 Type checking tool calls

At every tool call site, the checker verifies:

- The agent has declared `use ToolName` (E038)
- The function name exists on the tool (E036)
- The argument count matches (E039)
- Each argument type matches the declared parameter type (E003, existing)
- The result type is used consistently with the declared return type (E003, existing)
- If the return type is `Result<T, E>`, the result is either `try`-ed, `catch`-ed, or matched — not silently discarded (E013, existing from RFC-0007)

---

## 8. Error Handling

Tool calls are the most error-prone operations an agent performs. Network failures, timeouts, permission errors, malformed responses — all are commonplace. The error handling model must be clear and enforced.

### 8.1 Required handling for `Result`-returning tools

Any tool function that returns `Result<T, E>` must be handled. The checker uses the existing E013 (`UnhandledError`) rule from RFC-0007: calling a fallible function without `try` or `catch` is a compile error.

```sage
// ✗ E013: result discarded
Http.get(url)

// ✓ try propagates to on error
let r = try Http.get(url)

// ✓ catch provides a fallback
let r = Http.get(url) catch { HttpResponse { status: 0, body: "", headers: {} } }

// ✓ explicit match
match Http.get(url) {
    Ok(r)  => process(r)
    Err(e) => handle_error(e)
}
```

### 8.2 `on error` handler receives tool errors

When `try` is used and a tool call fails, the error propagates to the agent's `on error` handler exactly as LLM errors do (RFC-0007). The `e.kind` is `ErrorKind::Tool` — a new kind added by this RFC.

```sage
agent Fetcher {
    use Http

    url: String

    on start {
        let response = try Http.get(self.url)
        emit(response.body)
    }

    on error(e) {
        match e.kind {
            ErrorKind.Llm  => emit("LLM failed: {e.message}")
            ErrorKind.Tool => emit("Tool failed: {e.message}")
            _              => emit("Unknown error: {e.message}")
        }
    }
}
```

This requires adding `ErrorKind.Tool` as a new variant to the `ErrorKind` enum defined in RFC-0007. This is a non-breaking addition.

---

## 9. Configuration and Connection

### 9.1 Built-in tool configuration

Built-in tools (`Http`, `Fs`, etc.) are configured via environment variables and `sage.toml`. They do not require `[tools.*]` sections to exist — defaults are applied automatically.

| Tool | Env var | Default |
|------|---------|---------|
| `Http` | `SAGE_HTTP_TIMEOUT_MS` | `30000` |
| `Http` | `SAGE_HTTP_MAX_RETRIES` | `3` |
| `Fs`   | Configured in `sage.toml` only | — |

### 9.2 User-defined tool configuration

User-defined tools are connected via `sage.toml`:

```toml
[tools.database]
mcp_server = "http://localhost:5432/mcp"
timeout_ms = 5000

[tools.github]
mcp_server = "https://mcp.github.com"
api_key_env = "GITHUB_TOKEN"        # env var name to use as bearer token
```

The `api_key_env` field tells the runtime which environment variable holds the authentication credential for this server. The runtime injects it as a bearer token in the MCP connection handshake.

### 9.3 MCP transport

The runtime uses the MCP HTTP+SSE transport (the standard MCP transport for networked servers). For locally-running MCP servers, the `stdio` transport is also supported:

```toml
[tools.local_db]
mcp_server = "stdio"
command = "./bin/db-mcp-server"
args = ["--port", "5432"]
```

This launches the command as a subprocess and communicates over stdin/stdout using the MCP stdio transport.

### 9.4 Connection lifecycle

- Tool connections are established lazily on first use, not at program start.
- A failed connection on first use causes the tool call to return `Err(...)`.
- Connections are kept alive for the duration of the agent's lifetime and reused for subsequent calls.
- When an agent terminates (emits its result), its tool connections are released.
- Multiple agents using the same tool share a connection pool at the runtime level.

---

## 10. Codegen

### 10.1 Tool declarations → Rust traits

Each `tool` declaration generates a Rust trait:

**Sage:**
```sage
tool Database {
    fn query(sql: String) -> Result<List<Map<String, String>>, String>
    fn execute(sql: String) -> Result<Int, String>
}
```

**Generated Rust:**
```rust
/// Generated from Sage `tool Database` declaration.
#[async_trait::async_trait]
trait DatabaseTool: Send + Sync {
    async fn query(&self, sql: String) -> Result<Vec<HashMap<String, String>>, String>;
    async fn execute(&self, sql: String) -> Result<i64, String>;
}
```

### 10.2 MCP tool client → Rust struct

The runtime provides a generic `McpToolClient` that implements any tool trait by dispatching calls over MCP:

```rust
// In sage-runtime
pub struct McpToolClient {
    transport: Arc<dyn McpTransport>,
    tool_name: String,
}

impl McpToolClient {
    pub async fn call<T: DeserializeOwned>(
        &self,
        function: &str,
        args: serde_json::Value,
    ) -> SageResult<T> { ... }
}
```

### 10.3 Agent `use` → field injection

When an agent declares `use Http`, the generated Rust struct gains a field for the tool client:

**Sage:**
```sage
agent WebResearcher {
    use Http
    topic: String
    ...
}
```

**Generated Rust:**
```rust
struct WebResearcher {
    topic: String,
    http: Arc<sage_runtime::tools::HttpClient>,   // injected by codegen
}
```

The tool client is passed in when the agent is spawned. The `spawn` codegen path is updated to acquire a tool client from the runtime's connection pool and pass it to the agent struct.

### 10.4 Tool call → async MCP dispatch

**Sage:**
```sage
let response = try Http.get(self.url)
```

**Generated Rust:**
```rust
let response = self.http.get(self.url.clone()).await
    .map_err(|e| SageError::tool(e))?;
```

The method `self.http.get(...)` is generated on the `HttpClient` struct in `sage-runtime`. It serialises arguments to JSON, makes the MCP call, deserialises the response into the declared return type, and surfaces errors as `SageError::Tool`.

### 10.5 Built-in `Http` tool → direct reqwest

The `Http` built-in does not actually go through an MCP server at runtime. It is implemented directly using `reqwest` in `sage-runtime`, which avoids the overhead of the MCP protocol for the most common case. The interface is identical to MCP-backed tools — this is a runtime optimisation, invisible to the user.

This is the pragmatic exception: for built-in tools that are simple enough, direct implementation beats protocol overhead. The interface contract (the Sage `tool` declaration) is the same regardless of whether the backend is MCP or native.

### 10.6 `sage.toml` tool configuration → compile-time assertions

At compile time, `sage check` and `sage build` read `sage.toml` and verify:

- Every user-defined tool used in the program has a `[tools.*]` entry in `sage.toml`
- No tool entry in `sage.toml` references a tool not declared in the program (warning, not error)

The actual MCP server connection is not established at compile time by default. With `--verify-tools`, `sage check` will connect to each configured MCP server and verify that the declared function signatures match the server's published tool manifest (E041).

---

## 11. Checker Rules

| Code | Rule |
|------|------|
| E036 | Tool function `name` does not exist on tool `T` |
| E037 | Tool function parameter or return type is not JSON-serialisable |
| E038 | Agent calls tool `T` without declaring `use T` |
| E039 | Tool function called with wrong number of arguments |
| E040 | User-defined tool has no `[tools.*]` entry in `sage.toml` *(already used in RFC-0009 — reassign)* |
| E041 | Tool signature mismatch with MCP server manifest (only emitted with `--verify-tools`) |
| E042 | `tool` declaration contains a function body (tool functions are signatures only) |
| E043 | Duplicate `use` declaration within an agent |
| E044 | `use` in a non-agent context (tool use is only valid inside agent bodies) |

Note: E040 was previously assigned to closure parameter type errors in RFC-0009. That code should be reassigned to E045, and E040 used here for the missing tool configuration error.

---

## 12. New Error Codes

| Code | Name | Description |
|------|------|-------------|
| E036 | `UndefinedToolFunction` | Called function does not exist on the tool declaration |
| E037 | `NonSerializableToolType` | Tool parameter or return type cannot be JSON-serialised |
| E038 | `UndeclaredToolUse` | Agent calls a tool without declaring `use ToolName` |
| E039 | `ToolCallArity` | Tool function called with wrong number of arguments |
| E040 | `MissingToolConfig` | User-defined tool has no MCP server configured in `sage.toml` |
| E041 | `ToolSignatureMismatch` | Declared tool signature does not match MCP server manifest |
| E042 | `ToolFunctionHasBody` | `tool` declaration contains a function body |
| E043 | `DuplicateToolUse` | Same tool declared twice in `use` |
| E044 | `ToolUseOutsideAgent` | `use ToolName` appears outside an agent declaration |

---

## 13. Implementation Plan

### Phase 1 — Lexer & Parser (2–3 days)

- Add `tool` as a keyword token (`KwTool`)
- Add `use` as a keyword token inside agent bodies (`KwUse`) — distinct from module-level `use` statements; the parser disambiguates by context
- Add `ToolDecl` AST node: `{ name: Ident, is_pub: bool, functions: Vec<ToolFnDecl>, span: Span }`
- Add `ToolFnDecl` AST node: `{ name: Ident, params: Vec<Param>, return_ty: TypeExpr, span: Span }`
- Extend `AgentDecl` AST node with `tools: Vec<Ident>` (the `use` list)
- Parse `tool` declarations at top level
- Parse `use ToolName, OtherTool` inside agent bodies (before field declarations)
- Parse `ToolName.function(args)` as a new expression variant `Expr::ToolCall`
- Add `Program.tools: Vec<ToolDecl>` to the top-level AST
- Parser tests for all new constructs

### Phase 2 — Type System & Checker (3–4 days)

- Add `ToolInfo` to `SymbolTable`: `{ name, functions: HashMap<String, ToolFnInfo>, is_pub, module_path }`
- Add `ToolFnInfo`: `{ params: Vec<(String, Type)>, return_ty: Type }`
- Register built-in tools (`Http`, and stubs for `Fs`, `Browser`, `Kv`) in the prelude
- Register user-defined tools from the program's `tool` declarations in the symbol table
- Extend `AgentInfo` with `tools: Vec<String>` (the names from `use`)
- Check `use` clauses: validate tool names exist (E038), no duplicates (E043)
- Check `Expr::ToolCall`: validate agent has declared the tool (E038), function exists (E036), arity (E039), argument types (E003)
- Apply JSON-serialisability predicate to tool parameter and return types (E037)
- Add `ErrorKind::Tool` to the `ErrorKind` enum in the type system
- Emit E042 if a tool declaration contains a body
- Emit E044 if `use` appears outside an agent
- Load `sage.toml` tool configuration during checking; emit E040 for user-defined tools without config
- Checker tests for all nine new error codes

### Phase 3 — Runtime: `Http` built-in (2–3 days)

- Add `sage-runtime/src/tools/mod.rs`
- Implement `HttpClient` in `sage-runtime/src/tools/http.rs` using `reqwest`
- Implement `HttpResponse` Rust struct with serde derives
- Implement all five `Http` functions: `get`, `post`, `post_json`, `put`, `delete`, `get_with_headers`
- Add `ErrorKind::Tool` variant to `SageError` and `ErrorKind` in `sage-runtime/src/error.rs`
- Add `SageError::tool(msg)` constructor
- Add HTTP timeout and retry configuration via env vars

### Phase 4 — Runtime: MCP client (3–4 days)

- Add `sage-runtime/src/tools/mcp.rs`
- Implement `McpTransport` trait with two backends: HTTP+SSE and stdio
- Implement `McpToolClient` as a generic MCP caller using `serde_json::Value` for args/results
- Implement connection pool: one pool per MCP server URL, shared across agents
- Implement lazy connection: first call triggers handshake
- Add `McpToolConfig` struct parsed from `sage.toml` `[tools.*]` sections
- Add `ToolRegistry` to the Sage runtime: maps tool names to their `McpToolClient` instances
- Wire `ToolRegistry` into the `AgentContext` so spawned agents can acquire tool clients

### Phase 5 — Codegen (3–4 days)

- Generate Rust trait definitions from `tool` declarations
- Generate `HttpClient` field injection for agents declaring `use Http`
- Generate `McpToolClient` field injection for agents declaring user-defined tools
- Update `spawn` codegen to pass tool clients when constructing agent structs
- Generate `Expr::ToolCall` as `self.tool_name.function(args).await.map_err(SageError::tool)?`
- Generate `SageError::tool` wrapping for `Result`-returning tool functions
- Generate `ErrorKind::Tool` in match expressions
- Codegen snapshot tests for `Http.get`, `Http.post`, and a user-defined MCP tool call

### Phase 6 — Tests & Polish (2–3 days)

- End-to-end test: `Http.get` call with mock HTTP server
- End-to-end test: `Http.post_json` with structured request body
- End-to-end test: chaining LLM + tool (`infer` then `Http.get` on the result)
- End-to-end test: tool error propagates to `on error` handler
- End-to-end test: user-defined tool connected to a mock MCP server
- Checker tests for all nine error codes
- Update guide documentation with tool tutorial
- Add `examples/web_researcher.sg` demonstrating `Http` in a real agent

**Total estimated effort:** ~4 weeks

---

## 14. Future: Runtime-Only MCP (Without Language Primitives)

This RFC takes the position that tool declarations should be language-level primitives. But there is a valid alternative path that is worth specifying for future consideration: **runtime-only MCP integration**, where tools are discovered and called dynamically without any compile-time declarations.

### The use case

Some AI applications need to discover available tools at runtime rather than declare them statically:

```sage
// Future syntax — not in this RFC
agent DynamicOrchestrator {
    on start {
        let available = discover_mcp_tools("http://my-server/mcp")
        let plan = infer("You have these tools: {available}. Use them to complete: {self.task}")
        // Execute the plan's tool calls dynamically
        emit(execute_plan(plan, available))
    }
}
```

This pattern — where the agent itself reasons about which tools exist and decides at runtime which to call — is genuinely useful for open-ended orchestration. It cannot be modelled with static `tool` declarations.

### The approach

Runtime-only MCP would be exposed as a standard library module, not a language primitive:

```sage
use stdlib::mcp

agent DynamicAgent {
    server_url: String
    task: String

    on start {
        let tools = try mcp.connect(self.server_url)
        let names = mcp.list_tools(tools)
        let result = try mcp.call(tools, "some_tool", { "arg": "value" })
        emit(result)
    }
}
```

Key design principles for this future path:

- `mcp.connect(url)` returns a `McpConnection` handle (a new opaque type)
- `mcp.list_tools(conn)` returns `List<ToolSpec>` where `ToolSpec` is a record with `name: String` and `schema: String` (JSON schema as string)
- `mcp.call(conn, tool_name, args)` takes `Map<String, String>` args and returns `Result<String, String>` — dynamically typed, no compile-time safety
- The `McpConnection` type would need to be storable in agent fields, which requires opaque/handle types (a future RFC)
- This path trades compile-time safety for runtime flexibility — appropriate for open-ended orchestration but not for typed tool usage

This RFC does not implement `stdlib::mcp`. It is noted here so that if and when it is implemented, it is designed consistently with the language-level `tool` primitive rather than in conflict with it.

---

## 15. Future: Sage as an MCP Server

The inverse of tool consumption: exposing Sage agents as MCP tools that other systems (Claude, ChatGPT, other Sage programs) can call.

### The vision

```sage
// Future syntax — not in this RFC
@mcp_tool(
    name = "research",
    description = "Research a topic and return a structured summary"
)
pub agent Researcher {
    topic: String

    on start {
        let result: Inferred<ResearchResult> = try infer(
            "Research {self.topic} thoroughly."
        )
        emit(result)
    }
}
```

Running `sage serve` would start an MCP server that exposes this agent as a callable tool. Another system could then connect and call it:

```
POST /mcp/tools/research
{ "topic": "quantum computing" }
→ { "summary": "...", "confidence": 0.9 }
```

### What this would require

**Schema generation from agent fields.** The MCP tool manifest needs a JSON schema for the tool's inputs and outputs. Agent fields (analogous to tool parameters) would generate the input schema. The `emit` type would generate the output schema. The JSON schema machinery from RFC-0005 (`Inferred<T>`) already does this — it just needs to be repurposed.

**MCP server runtime.** A new `sage serve` CLI command that starts an HTTP server implementing the MCP protocol. Each `@mcp_tool` agent becomes one tool in the manifest. Incoming tool calls become `spawn` invocations; the emitted value is returned as the tool result.

**`@mcp_tool` attribute syntax.** Attributes (`@name(args)`) are not currently part of the Sage grammar. This would require a new lexer/parser feature. It is the right syntax choice — consistent with how Rust and Python annotate items without polluting the type system.

**Authentication and transport.** `sage serve` would need to support at minimum HTTP+SSE transport and bearer token authentication.

**Error mapping.** Agent `on error` handler results need to map to MCP error responses.

### Relationship to this RFC

The server path does not depend on this RFC's client path — they are independent features. However, the JSON schema generation for built-in tools (§10.4, structured output) and the MCP transport implementation (§13, Phase 4) would both be shared infrastructure. Implementing the client path first gives the server path a head start.

---

## 16. Open Questions

### 16.1 Tool handles as first-class values

This RFC does not allow storing tool references in variables or passing them as function arguments. This means you cannot write a function that takes a tool as a parameter:

```sage
// Not supported in this RFC
fn fetch_and_parse(http: Http, url: String) -> Result<String, String> {
    let r = try http.get(url)
    return Ok(r.body)
}
```

This limits code reuse. A `Tool<Http>` type that can be stored and passed would unlock this, at the cost of making tools feel more like objects. Deferred — the agent-owns-tool model is simpler to start with and covers the majority of use cases.

### 16.2 Tool mocking in tests

When the Sage testing framework arrives (a future RFC), test code will need to inject mock tool implementations. The natural mechanism would be dependency injection at the `spawn` site:

```sage
// Hypothetical test syntax
let mock_http = MockHttp { ... }
let agent = spawn WebResearcher { topic: "rust" } with_tools { Http: mock_http }
```

This requires the tool system to have an injection point — which the generated Rust trait approach (§10.1) supports naturally. The trait abstraction means tests can substitute any `impl HttpTool` for the real `HttpClient`. This is a deliberate design choice in this RFC that pays off at test time.

### 16.3 Tool versioning

MCP servers can evolve. If a tool's `fn read` signature changes on the server, but the Sage declaration is not updated, the program compiles but fails at runtime. The `--verify-tools` flag in `sage check` (§10.6) provides a safety net, but it requires network access.

A more robust solution would be to hash the tool manifest at last verification and store it in `sage.lock`, then warn at build time if the lock is stale. This is analogous to how lockfiles work for package versions. Deferred to a follow-up.

### 16.4 Tool call batching

MCP supports batch calls (multiple tool invocations in one round trip). Sage does not expose this in the first release — every tool call is individual. For high-throughput agents that make many small tool calls, batching could reduce latency significantly. A future `batch_call` builtin on tool handles could expose this.

### 16.5 Streaming tool results

Some MCP tools support streaming responses (e.g. a tool that streams a large file). This RFC's model — a tool call returns a value — cannot represent streaming. A future `Stream<T>` type and corresponding tool return type would be needed. Deferred.

---

## 17. Alternatives Considered

### 17.1 No tool declarations — just a `call_tool` builtin

```sage
let result = try call_tool("http", "get", { "url": "https://example.com" })
```

**Rejected.** No type safety, no autocomplete, runtime errors where compile-time errors belong. Every tool call would return an untyped `String` or `Map<String, String>`. This is exactly the LangChain/CrewAI ergonomics that Sage exists to improve on.

### 17.2 Tools as regular records with a special annotation

```sage
@tool(mcp_server = "http://localhost:5432/mcp")
record Database {
    fn query(sql: String) -> Result<List<Map<String, String>>, String>
}
```

**Rejected.** Records are data types; tools are capability interfaces. Conflating them would mean the checker needs to distinguish "record with tool annotation" from "record" in many places. A dedicated `tool` keyword is clearer and mirrors the existing `agent`/`record`/`enum` pattern.

### 17.3 Tools as a special kind of agent

Model a tool as an always-running agent you send messages to:

```sage
agent HttpTool receives HttpRequest {
    on start {
        loop {
            let req = receive()
            // make the request, send back the result
        }
    }
}
```

**Rejected.** This requires the programmer to understand message passing to use a simple HTTP client. It also can't be used in `try`/`catch` naturally. Tools are fundamentally different from agents — they are synchronous-ish request-response interfaces to external systems, not concurrent reasoning entities.

### 17.4 Implicit tool use — no `use` declaration needed

Let any agent call any tool without declaring it:

```sage
agent WebResearcher {
    on start {
        let r = try Http.get("https://example.com")  // no 'use Http' needed
        ...
    }
}
```

**Rejected.** Explicit `use` declarations serve two purposes: documentation (the reader can see at a glance what an agent depends on) and validation (the compiler can check that configured tools match declared tools). They also mirror the module `use` pattern, keeping the language internally consistent.

---

*Ward fetches the page. Ward reads the bytes. Ward knows where the data lives. Tools are how agents touch the world.*
