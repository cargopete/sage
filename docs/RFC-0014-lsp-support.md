# RFC-0014: Language Server Protocol (LSP) Support

- **Status:** Implemented (v0.5.1)
- **Created:** 2026-03-13
- **Author:** Pete Pavlovski
- **Depends on:** RFC-0001 (Lexer/Parser), RFC-0003 (Compiler), RFC-0005 (User-Defined Types)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Architecture Overview](#4-architecture-overview)
5. [New Crate: `sage-lsp`](#5-new-crate-sage-lsp)
6. [The `sage lsp` Subcommand](#6-the-sage-lsp-subcommand)
7. [Syntax Highlighting](#7-syntax-highlighting)
8. [Diagnostics](#8-diagnostics)
9. [Parser Changes for LSP](#9-parser-changes-for-lsp)
10. [Document State Management](#10-document-state-management)
11. [VS Code Extension](#11-vs-code-extension)
12. [Zed Extension](#12-zed-extension)
13. [Crate Dependencies](#13-crate-dependencies)
14. [Implementation Plan](#14-implementation-plan)
15. [Future LSP Features](#15-future-lsp-features)
16. [Open Questions](#16-open-questions)
17. [Alternatives Considered](#17-alternatives-considered)

---

## 1. Summary

This RFC introduces **`sage lsp`** — a Language Server Protocol server bundled directly into the `sage` binary. It provides syntax highlighting (via a TextMate grammar shipped with the editor extensions) and inline diagnostics (via LSP) for VS Code and Zed. These are the two features that matter most on day one: without syntax highlighting the language looks like a wall of grey text, and without diagnostics the editor cannot tell you when your program is wrong.

This RFC also introduces two companion repositories:
- **`sage-lang/vscode-sage`** — the VS Code extension
- **`sage-lang/zed-sage`** — the Zed extension

Both extensions follow the same approach: they locate the user's installed `sage` binary and launch `sage lsp` as a child process communicating over stdio.

---

## 2. Motivation

Sage is at the point where the editor experience is becoming the barrier to adoption. A developer evaluating the language today opens a `.sg` file and sees one of two things depending on their editor: monochrome text, or a single diagnostic ("unrecognised language"). Neither is acceptable for a language that wants to be taken seriously.

More concretely, the absence of an LSP means:

- **No inline errors.** The only way to see type errors is to run `sage check` in the terminal. Every iteration requires leaving the editor.
- **No syntax highlighting.** Keywords, strings, agents, and operators are all the same colour. Reading Sage code is significantly harder than it needs to be.
- **No editor feedback loop.** The tight edit→check→fix cycle that makes typed languages pleasant to use is completely absent.

These are not advanced IDE features — they are the baseline. Languages like Gleam, Roc, and even very young research languages ship them from early on precisely because they are the difference between "I can see myself using this" and "I'll check back in a year."

The design follows Gleam's model: the LSP is bundled in the main binary (`gleam lsp` / `sage lsp`), eliminating a separate install step. The VS Code extension just runs `sage lsp` — it needs nothing else.

---

## 3. Design Goals

1. **Bundled, not separate.** `sage lsp` is a subcommand of the main `sage` binary. No separate binary, no separate install. If you have `sage`, you have the LSP.
2. **Syntax highlighting ships on day one.** The TextMate grammar in the VS Code extension works without the LSP server running. Opening a `.sg` file gives immediate colour, even before the server initialises.
3. **Diagnostics are the first LSP feature.** The server surfaces the same errors that `sage check` produces, as you type, inline in the editor.
4. **The parser must never crash.** An LSP server processes broken, incomplete code constantly. Every parser entry point must produce *something* — a partial AST with error nodes — rather than failing.
5. **Full reparse per keystroke is acceptable for now.** A complete reparse of a typical `.sg` file takes 1–5ms. This is well within LSP response budgets. Incremental parsing is a future optimisation.
6. **VS Code and Zed are the targets.** Both use stdio transport. The LSP server is editor-agnostic; only the thin extension layers differ.
7. **The compiler pipeline is reused, not duplicated.** `sage-lsp` depends on `sage-lexer`, `sage-parser`, and `sage-checker`. It does not copy or reimport their logic.

---

## 4. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Editor Process                           │
│                                                                 │
│  ┌─────────────┐   TextMate grammar     ┌──────────────────┐   │
│  │ VS Code /   │ ──── highlighting ───▶ │  .sg file in     │   │
│  │ Zed         │                        │  editor buffer   │   │
│  │             │                        └──────────────────┘   │
│  │  LSP Client │◀──── JSON-RPC ────────────────────────┐       │
│  └─────────────┘        (stdio)                        │       │
└────────────────────────────────────────────────────────┼───────┘
                                                         │ stdin/stdout
┌────────────────────────────────────────────────────────┼───────┐
│                     sage lsp (child process)           │       │
│                                                        │       │
│  ┌──────────────┐   ┌─────────────┐   ┌─────────────┐ │       │
│  │ tower-lsp-   │   │  Document   │   │  Diagnostic │ │       │
│  │ server       │──▶│  Store      │──▶│  Engine     │─┘       │
│  │ (JSON-RPC)   │   │ (per-file   │   │             │         │
│  └──────────────┘   │  AST cache) │   └──────┬──────┘         │
│                     └─────────────┘          │                 │
│                                              │                 │
│                     ┌────────────────────────▼───────────────┐ │
│                     │        sage compiler pipeline           │ │
│                     │                                        │ │
│                     │  sage-lexer → sage-parser →            │ │
│                     │  sage-loader → sage-checker            │ │
│                     └────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

The LSP server is a separate OS process launched by the editor extension. It communicates with the editor over stdin/stdout using the JSON-RPC protocol defined by the Language Server Protocol specification. The editor sends notifications when files are opened, changed, or closed; the server responds with diagnostics.

The key design principle: **`sage-lsp` is a thin orchestration layer on top of the existing compiler crates**. It does not reimplement parsing or type checking. It holds a per-document cache of ASTs and diagnostics, reruns the compiler pipeline when documents change, and translates `miette` diagnostics into LSP `Diagnostic` objects.

---

## 5. New Crate: `sage-lsp`

A new `crates/sage-lsp/` workspace crate holds all LSP logic. It is a library crate — `sage-cli` depends on it and calls its public `run()` function from the `lsp` subcommand.

### 5.1 Crate structure

```
crates/sage-lsp/
├── Cargo.toml
└── src/
    ├── lib.rs          # Public API: run() entry point
    ├── backend.rs      # tower-lsp-server LanguageServer impl
    ├── document.rs     # Per-document state (text, AST, diagnostics)
    ├── store.rs        # DocumentStore: thread-safe map of open documents
    ├── analysis.rs     # Re-run compiler pipeline, produce diagnostics
    ├── convert.rs      # miette Diagnostic → lsp_types::Diagnostic
    └── capabilities.rs # Server capability declarations
```

### 5.2 Public API

```rust
// crates/sage-lsp/src/lib.rs

/// Start the LSP server, reading from stdin and writing to stdout.
/// Blocks until the client sends `shutdown` followed by `exit`.
pub async fn run() -> anyhow::Result<()>;
```

This is the only public export. Everything else is internal.

### 5.3 The `Backend` struct

`Backend` implements `tower_lsp_server::LanguageServer`. It holds a reference to the `DocumentStore` and a `Client` handle for sending notifications back to the editor.

```rust
// crates/sage-lsp/src/backend.rs

use std::sync::Arc;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};
use crate::store::DocumentStore;

pub struct Backend {
    client: Client,
    store: Arc<DocumentStore>,
}

#[tower_lsp_server::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: crate::capabilities::server_capabilities(),
            server_info: Some(ServerInfo {
                name: "sage-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "sage-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_file_change(
            params.text_document.uri,
            params.text_document.text,
            params.text_document.version,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: take the last content update
        if let Some(change) = params.content_changes.into_iter().last() {
            self.on_file_change(
                params.text_document.uri,
                change.text,
                params.text_document.version,
            )
            .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.store.remove(&params.text_document.uri);
        // Clear diagnostics for the closed file
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }
}

impl Backend {
    async fn on_file_change(&self, uri: Url, text: String, version: i32) {
        let diagnostics = crate::analysis::analyse(&uri, &text);
        self.store.update(uri.clone(), text, diagnostics.clone());
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }
}
```

### 5.4 Server capabilities declaration

```rust
// crates/sage-lsp/src/capabilities.rs

use tower_lsp_server::ls_types::*;

pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // Full document sync: client sends the entire file on every change.
        // Incremental sync is a future optimisation.
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )),
        // Semantic tokens for enriched highlighting (Phase 2)
        semantic_tokens_provider: None, // enabled in Phase 2
        // All other capabilities disabled for Phase 1
        ..Default::default()
    }
}
```

### 5.5 The `run()` entry point

```rust
// crates/sage-lsp/src/lib.rs

use std::sync::Arc;
use tower_lsp_server::{LspService, Server};
use crate::{backend::Backend, store::DocumentStore};

pub async fn run() -> anyhow::Result<()> {
    let stdin  = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let store = Arc::new(DocumentStore::new());
    let (service, socket) = LspService::new(|client| Backend {
        client,
        store,
    });

    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
```

---

## 6. The `sage lsp` Subcommand

The `sage-cli` crate gains a new `Lsp` variant in its `Commands` enum:

```rust
// crates/sage-cli/src/main.rs

#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Start the Sage Language Server (for editor integration)
    Lsp,
}
```

The handler is three lines:

```rust
Commands::Lsp => {
    tokio::runtime::Runtime::new()?.block_on(sage_lsp::run())?;
}
```

That is the complete CLI surface. `sage lsp` reads from stdin and writes to stdout. It produces no other output — no startup banner, no progress indicators — because the editor is reading stdout and anything non-JSON-RPC breaks the protocol.

One important detail: the LSP server must **not** initialise the Tokio runtime twice. The existing `sage` commands already use `#[tokio::main]`. The `lsp` subcommand should start its own runtime rather than reusing the outer one, or the outer `main()` should be restructured to call `run()` directly without `#[tokio::main]`. The cleanest approach is to remove `#[tokio::main]` from `main()` and build the runtime manually, then share it across all async subcommands.

---

## 7. Syntax Highlighting

Syntax highlighting in VS Code comes from a **TextMate grammar** — a JSON file containing Oniguruma regular expressions that match token patterns and assign scope names. These scopes are mapped to colours by the active theme. The TextMate grammar runs entirely in the editor process, with no dependency on the LSP server, so it works the instant a `.sg` file is opened.

### 7.1 The grammar file

The grammar lives in the VS Code extension repository at `syntaxes/sage.tmLanguage.json`. Its top-level structure:

```json
{
  "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  "name": "Sage",
  "scopeName": "source.sage",
  "fileTypes": ["sg"],
  "patterns": [
    { "include": "#comments" },
    { "include": "#strings" },
    { "include": "#numbers" },
    { "include": "#types" },
    { "include": "#keywords" },
    { "include": "#agent-declaration" },
    { "include": "#function-declaration" },
    { "include": "#tool-declaration" },
    { "include": "#builtin-functions" },
    { "include": "#operators" },
    { "include": "#identifiers" }
  ],
  "repository": { ... }
}
```

### 7.2 Grammar repository entries

The `repository` section defines all named patterns referenced by `include`:

**Comments:**
```json
"comments": {
  "patterns": [
    {
      "name": "comment.line.double-slash.sage",
      "match": "//.*$"
    }
  ]
}
```

**String literals with interpolation:**
```json
"strings": {
  "name": "string.quoted.double.sage",
  "begin": "\"",
  "end": "\"",
  "patterns": [
    {
      "name": "constant.character.escape.sage",
      "match": "\\\\."
    },
    {
      "name": "variable.other.sage",
      "match": "\\{[a-zA-Z_][a-zA-Z0-9_]*\\}"
    }
  ]
}
```

**Type keywords** (capitalised — `Int`, `Float`, `Bool`, `String`, `Unit`, `List`, `Option`, `Map`, `Result`, `Inferred`, `Agent`):
```json
"types": {
  "match": "\\b(Int|Float|Bool|String|Unit|List|Option|Map|Result|Inferred|Agent)\\b",
  "name": "support.type.sage"
}
```

**Language keywords:**
```json
"keywords": {
  "patterns": [
    {
      "name": "keyword.control.sage",
      "match": "\\b(if|else|for|while|loop|break|return|match)\\b"
    },
    {
      "name": "keyword.declaration.sage",
      "match": "\\b(agent|fn|tool|record|enum|const|pub|mod|use)\\b"
    },
    {
      "name": "keyword.operator.sage",
      "match": "\\b(let|in|on|run|spawn|await|emit|send|receive|receives|try|catch|fails)\\b"
    },
    {
      "name": "variable.language.sage",
      "match": "\\bself\\b"
    },
    {
      "name": "constant.language.sage",
      "match": "\\b(true|false)\\b"
    }
  ]
}
```

**Agent declarations** (to highlight the agent name distinctly):
```json
"agent-declaration": {
  "match": "\\b(agent)\\s+([A-Z][a-zA-Z0-9_]*)\\b",
  "captures": {
    "1": { "name": "keyword.declaration.sage" },
    "2": { "name": "entity.name.class.sage" }
  }
}
```

**Function declarations:**
```json
"function-declaration": {
  "match": "\\b(fn)\\s+([a-z_][a-zA-Z0-9_]*)\\b",
  "captures": {
    "1": { "name": "keyword.declaration.sage" },
    "2": { "name": "entity.name.function.sage" }
  }
}
```

**Tool declarations:**
```json
"tool-declaration": {
  "match": "\\b(tool)\\s+([A-Z][a-zA-Z0-9_]*)\\b",
  "captures": {
    "1": { "name": "keyword.declaration.sage" },
    "2": { "name": "entity.name.class.sage" }
  }
}
```

**Built-in functions** (these are known at grammar time):
```json
"builtin-functions": {
  "match": "\\b(print|infer|len|push|join|str|map_get|map_set|map_has|map_delete|map_keys|map_values|sleep_ms)\\b(?=\\s*\\()",
  "name": "support.function.builtin.sage"
}
```

**Operators:**
```json
"operators": {
  "patterns": [
    {
      "name": "keyword.operator.arithmetic.sage",
      "match": "\\+\\+|[+\\-*/%]"
    },
    {
      "name": "keyword.operator.comparison.sage",
      "match": "==|!=|<=|>=|<|>"
    },
    {
      "name": "keyword.operator.logical.sage",
      "match": "&&|\\|\\||!"
    },
    {
      "name": "keyword.operator.arrow.sage",
      "match": "->"
    }
  ]
}
```

**Numbers:**
```json
"numbers": {
  "patterns": [
    {
      "name": "constant.numeric.float.sage",
      "match": "\\b[0-9]+\\.[0-9]+\\b"
    },
    {
      "name": "constant.numeric.integer.sage",
      "match": "\\b[0-9]+\\b"
    }
  ]
}
```

### 7.3 Language configuration

VS Code also needs a `language-configuration.json` for bracket matching, comment toggling, and auto-indentation:

```json
{
  "comments": {
    "lineComment": "//"
  },
  "brackets": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
    ["<", ">"]
  ],
  "autoClosingPairs": [
    { "open": "{", "close": "}" },
    { "open": "[", "close": "]" },
    { "open": "(", "close": ")" },
    { "open": "\"", "close": "\"", "notIn": ["string"] }
  ],
  "surroundingPairs": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
    ["\"", "\""]
  ],
  "indentationRules": {
    "increaseIndentPattern": "\\{\\s*$",
    "decreaseIndentPattern": "^\\s*\\}"
  },
  "wordPattern": "[a-zA-Z_][a-zA-Z0-9_]*"
}
```

### 7.4 Semantic tokens (Phase 2)

Once the LSP server exists, it can supplement the TextMate grammar with **semantic tokens** — type-aware highlights that the regex grammar cannot produce. For example:

- A variable declared with `let` in a mutable position vs a `const` binding → `variable` with `readonly` modifier
- An agent name at a `spawn` site → `class` token type
- A function call vs a tool call (`Http.get(...)`) → different token types
- A belief/field name → `property` token type

Semantic tokens are declared in `capabilities.rs` under `semantic_tokens_provider` with a `SemanticTokensLegend` listing the token types and modifiers the server emits. The server then implements `textDocument/semanticTokens/full` to return a delta-encoded array. This is **Phase 2** — not in the initial release.

---

## 8. Diagnostics

Diagnostics are the primary LSP feature in Phase 1. When a file changes, the server reruns the compiler pipeline and publishes the resulting errors as LSP `Diagnostic` objects.

### 8.1 Analysis pipeline

```rust
// crates/sage-lsp/src/analysis.rs

use sage_checker::CheckError;
use sage_lexer::LexError;
use sage_parser::ParseError;
use tower_lsp_server::ls_types::*;

/// Run the full compiler pipeline on `source` and return LSP diagnostics.
/// Never panics — all errors are caught and converted.
pub fn analyse(uri: &Url, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Step 1: Lex
    let (tokens, lex_errors) = sage_lexer::lex_recovering(source);
    for e in &lex_errors {
        if let Some(d) = lex_error_to_diagnostic(e, source) {
            diagnostics.push(d);
        }
    }

    // Bail early if lex produced nothing usable
    if tokens.is_empty() && !lex_errors.is_empty() {
        return diagnostics;
    }

    // Step 2: Parse (with error recovery — always returns a partial AST)
    let (program_opt, parse_errors) = sage_parser::parse_recovering(&tokens, source);
    for e in &parse_errors {
        if let Some(d) = parse_error_to_diagnostic(e, source) {
            diagnostics.push(d);
        }
    }

    // Step 3: Type check (only if we have a usable AST)
    if let Some(program) = program_opt {
        let check_errors = sage_checker::check_program_lsp(&program, source);
        for e in &check_errors {
            if let Some(d) = check_error_to_diagnostic(e, source) {
                diagnostics.push(d);
            }
        }
    }

    diagnostics
}
```

### 8.2 Converting `miette` diagnostics to LSP diagnostics

Sage's checker already uses `miette` with `SourceSpan` for error locations. The conversion translates byte offsets into LSP line/character positions (zero-based, UTF-16 encoded per the LSP spec):

```rust
// crates/sage-lsp/src/convert.rs

use sage_types::Span;
use tower_lsp_server::ls_types::*;

/// Convert a Sage Span (byte offsets) to an LSP Range (line/character).
pub fn span_to_range(span: &Span, source: &str) -> Range {
    Range {
        start: offset_to_position(span.start, source),
        end:   offset_to_position(span.end,   source),
    }
}

fn offset_to_position(offset: usize, source: &str) -> Position {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.matches('\n').count() as u32;
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    // LSP positions are UTF-16 code units
    let character = prefix[last_newline..]
        .encode_utf16()
        .count() as u32;
    Position { line, character }
}

/// Convert a Sage CheckError to an LSP Diagnostic.
pub fn check_error_to_diagnostic(
    error: &sage_checker::CheckError,
    source: &str,
) -> Option<Diagnostic> {
    let span = error.span()?;
    Some(Diagnostic {
        range:    span_to_range(&span, source),
        severity: Some(DiagnosticSeverity::ERROR),
        code:     Some(NumberOrString::String(error.code().to_string())),
        source:   Some("sage".to_string()),
        message:  error.to_string(),
        ..Default::default()
    })
}
```

### 8.3 Diagnostic quality

The LSP diagnostic should reproduce exactly what `sage check` shows, including:

- The primary error message
- The error code (`sage::E003`, `sage::undefined_variable`, etc.)
- Correct source range — the squiggly underline should cover exactly the offending tokens
- Related information for multi-span errors (e.g. "first defined here" for duplicate definitions)

For multi-span errors (where `miette` has multiple labels), the primary label becomes the diagnostic `range` and secondary labels become `relatedInformation` entries:

```rust
Diagnostic {
    range:    primary_range,
    message:  primary_message,
    related_information: Some(vec![
        DiagnosticRelatedInformation {
            location: Location { uri: uri.clone(), range: secondary_range },
            message: secondary_message,
        }
    ]),
    ..Default::default()
}
```

---

## 9. Parser Changes for LSP

The most important invariant for an LSP: **the parser must always return something**. The current chumsky parser returns `Result<Program, Vec<ParseError>>` — if it encounters a fatal error it can return `Err(errors)` with no AST. That is unacceptable for an LSP, where the user is always in the middle of typing something broken.

### 9.1 New recovering entry points

Two new public functions are added to `sage-parser`:

```rust
// crates/sage-parser/src/lib.rs (additions)

/// Parse with error recovery. Always returns a (possibly partial) Program
/// and a list of any parse errors encountered.
/// Never returns Err — partial results are returned with error nodes.
pub fn parse_recovering(
    tokens: &[(Token, Span)],
    source: &str,
) -> (Option<Program>, Vec<ParseError>);
```

And to `sage-lexer`:

```rust
// crates/sage-lexer/src/lib.rs (additions)

/// Lex with error recovery. Returns all tokens that could be lexed,
/// plus a list of lex errors for unrecognised input.
pub fn lex_recovering(source: &str) -> (Vec<(Token, Span)>, Vec<LexError>);
```

The existing `parse()` and `lex()` functions are unchanged — they continue to be used by the compiler pipeline which wants clean `Err` on failure.

### 9.2 Chumsky recovery strategies

The parser needs `recover_with()` calls at the key structural boundaries where users most often have incomplete code:

**Top-level declarations** — if an `agent`, `fn`, `record`, `enum`, or `tool` declaration fails to parse, skip to the next top-level keyword rather than giving up on the whole file:

```rust
let agent_decl = agent_parser()
    .recover_with(skip_then_retry_until(
        any().ignored(),
        one_of([
            Token::KwAgent,
            Token::KwFn,
            Token::KwRecord,
            Token::KwEnum,
            Token::KwTool,
            Token::KwRun,
        ]),
    ));
```

**Blocks** — if a block body fails mid-parse, recover at the matching `}`:

```rust
let block = stmt_parser()
    .repeated()
    .delimited_by(just(Token::LBrace), just(Token::RBrace))
    .recover_with(nested_delimiters(
        Token::LBrace,
        Token::RBrace,
        [(Token::LParen, Token::RParen), (Token::LBracket, Token::RBracket)],
        |span| Block { stmts: vec![], span },
    ));
```

**Expressions** — for incomplete expressions (user mid-typing `Http.`), return an `Expr::Error` node:

```rust
let expr = expr_parser()
    .recover_with(skip_then_retry_until(
        any().ignored(),
        one_of([Token::Semi, Token::RBrace, Token::Newline]),
    ))
    .map_err(|_| Expr::Error { span: /* current span */ });
```

### 9.3 Error node variants in the AST

Two new AST variants are added to represent "something was here but we couldn't parse it":

```rust
// crates/sage-parser/src/ast.rs (additions)

pub enum Expr {
    // ... existing variants ...
    /// Placeholder for an expression that failed to parse.
    Error { span: Span },
}

pub enum Stmt {
    // ... existing variants ...
    /// Placeholder for a statement that failed to parse.
    Error { span: Span },
}
```

The checker handles `Expr::Error` and `Stmt::Error` by silently skipping them — they do not produce additional type errors on top of the parse error already reported.

### 9.4 `check_program_lsp` entry point

The type checker gains a new entry point for LSP use that does not propagate `Result` — instead it collects all errors and returns them:

```rust
// crates/sage-checker/src/lib.rs (addition)

/// Type-check a program, returning all errors found.
/// Does not short-circuit on the first error.
/// Intended for LSP use where partial results are valuable.
pub fn check_program_lsp(program: &Program, source: &str) -> Vec<CheckError>;
```

The existing `check_program()` continues to return `Result<(), Vec<CheckError>>` and is used by the compiler pipeline.

---

## 10. Document State Management

The `DocumentStore` maintains the in-memory state of every file the editor has open. It is `Arc<Mutex<...>>` to allow shared access from the async LSP handlers.

```rust
// crates/sage-lsp/src/store.rs

use std::collections::HashMap;
use std::sync::Mutex;
use tower_lsp_server::ls_types::{Diagnostic, Url};

/// State for a single open document.
pub struct Document {
    pub text:        String,
    pub diagnostics: Vec<Diagnostic>,
    pub version:     i32,
}

/// Thread-safe store of all open documents.
pub struct DocumentStore {
    inner: Mutex<HashMap<String, Document>>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }

    pub fn update(&self, uri: Url, text: String, diagnostics: Vec<Diagnostic>) {
        let mut map = self.inner.lock().unwrap();
        map.insert(uri.to_string(), Document {
            text,
            diagnostics,
            version: 0,
        });
    }

    pub fn remove(&self, uri: &Url) {
        self.inner.lock().unwrap().remove(&uri.to_string());
    }

    pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        self.inner
            .lock()
            .unwrap()
            .get(&uri.to_string())
            .map(|d| d.diagnostics.clone())
            .unwrap_or_default()
    }
}
```

### 10.1 Debouncing

Reanalysing on every keystroke is correct but potentially wasteful for very fast typists. A 150ms debounce prevents queuing a backlog of analyses during rapid input:

```rust
// In Backend::on_file_change — debounce rapid changes
use tokio::time::{sleep, Duration};

async fn on_file_change(&self, uri: Url, text: String, version: i32) {
    // Store the pending update
    self.store.set_pending(uri.clone(), text.clone(), version);
    sleep(Duration::from_millis(150)).await;
    // Only analyse if this is still the latest version
    if self.store.is_current_version(&uri, version) {
        let diagnostics = crate::analysis::analyse(&uri, &text);
        self.store.update(uri.clone(), text, diagnostics.clone());
        self.client.publish_diagnostics(uri, diagnostics, Some(version)).await;
    }
}
```

This pattern is optional for the initial release and can be added if performance is found to be an issue.

---

## 11. VS Code Extension

The VS Code extension lives in the `sage-lang/vscode-sage` repository. It is a minimal TypeScript project — the only substantial file is `src/extension.ts`. The heavy lifting (JSON-RPC, stdio transport, reconnection) is handled by the `vscode-languageclient` npm package.

### 11.1 Repository structure

```
vscode-sage/
├── package.json
├── tsconfig.json
├── .vscodeignore
├── language-configuration.json
├── syntaxes/
│   └── sage.tmLanguage.json        # TextMate grammar (§7)
└── src/
    └── extension.ts                # Extension entry point
```

### 11.2 `package.json`

```json
{
  "name": "vscode-sage",
  "displayName": "Sage",
  "description": "Sage language support for Visual Studio Code",
  "version": "0.1.0",
  "publisher": "sage-lang",
  "repository": "https://github.com/sage-lang/vscode-sage",
  "engines": { "vscode": "^1.85.0" },
  "categories": ["Programming Languages"],
  "activationEvents": [],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [
      {
        "id": "sage",
        "aliases": ["Sage", "sage"],
        "extensions": [".sg"],
        "configuration": "./language-configuration.json"
      }
    ],
    "grammars": [
      {
        "language": "sage",
        "scopeName": "source.sage",
        "path": "./syntaxes/sage.tmLanguage.json"
      }
    ],
    "configuration": {
      "title": "Sage",
      "properties": {
        "sage.path": {
          "type": ["string", "null"],
          "default": null,
          "markdownDescription": "Path to the `sage` binary. If `null`, looks up `sage` on `PATH`."
        },
        "sage.trace.server": {
          "type": "string",
          "enum": ["off", "messages", "verbose"],
          "default": "off",
          "description": "Trace LSP communication (for debugging)"
        }
      }
    }
  },
  "scripts": {
    "compile": "tsc -p ./",
    "watch": "tsc -watch -p ./",
    "package": "vsce package"
  },
  "dependencies": {
    "vscode-languageclient": "^9.0.1"
  },
  "devDependencies": {
    "@types/vscode": "^1.85.0",
    "@vscode/vsce": "^3.0.0",
    "typescript": "^5.0.0"
  }
}
```

### 11.3 `src/extension.ts`

```typescript
import * as path from "path";
import { workspace, ExtensionContext, window } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const config = workspace.getConfiguration("sage");
  const sagePath = config.get<string | null>("path") || "sage";

  // sage lsp communicates over stdio
  const serverOptions: ServerOptions = {
    command: sagePath,
    args: ["lsp"],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    // Activate for all .sg files
    documentSelector: [{ scheme: "file", language: "sage" }],
    // Watch sage.toml for project configuration changes
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/sage.toml"),
    },
    traceOutputChannel: window.createOutputChannel("Sage LSP Trace"),
  };

  client = new LanguageClient(
    "sage",
    "Sage Language Server",
    serverOptions,
    clientOptions
  );

  await client.start();
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}
```

If `sage` is not found on PATH and no `sage.path` is configured, `vscode-languageclient` will surface an error through its normal channel. The extension does not attempt to download or install `sage` automatically — that friction is appropriate to leave on the user for now.

### 11.4 Distribution

The extension is published to the VS Code Marketplace as `sage-lang.vscode-sage`. Initial release uses the **PATH lookup strategy** — users install `sage` (e.g. via `brew install sage-lang/tap/sage` or from GitHub Releases), and the extension finds it automatically. A `sage.path` setting overrides this for non-standard install locations.

Future releases can bundle platform-specific binaries using `vsce package --target darwin-arm64` etc., removing the manual install step. This is deferred until the user base justifies the distribution complexity.

---

## 12. Zed Extension

The Zed extension lives in the `sage-lang/zed-sage` repository. Zed extensions are **WASM modules written in Rust**, compiled to `wasm32-wasip1` and loaded by Zed via Wasmtime. The Rust code is minimal — the key work is the tree-sitter grammar, which Zed uses for syntax highlighting instead of TextMate.

### 12.1 Repository structure

```
zed-sage/
├── Cargo.toml
├── extension.toml
├── src/
│   └── lib.rs                  # Extension implementation (~30 lines)
└── languages/
    └── sage/
        ├── config.toml         # Language metadata
        ├── highlights.scm      # tree-sitter highlight queries
        ├── brackets.scm        # Bracket matching queries
        └── indents.scm         # Indentation queries
```

The tree-sitter grammar itself lives in a **separate repository** (`sage-lang/tree-sitter-sage`) because it must be compiled to C/WASM independently. The Zed extension references it by git URL and revision.

### 12.2 `extension.toml`

```toml
id = "zed-sage"
name = "Sage"
version = "0.1.0"
schema_version = 1
authors = ["Sage Team <team@sagelang.dev>"]
description = "Sage language support: syntax highlighting and LSP diagnostics"
repository = "https://github.com/sage-lang/zed-sage"

[grammars.sage]
repository = "https://github.com/sage-lang/tree-sitter-sage"
rev = "COMMIT_SHA_HERE"

[language_servers.sage-lsp]
name = "sage-lsp"
languages = ["Sage"]
```

### 12.3 `languages/sage/config.toml`

```toml
name = "Sage"
grammar = "sage"
path_suffixes = ["sg"]
line_comments = ["// "]
block_comment = ["/*", "*/"]
```

### 12.4 `src/lib.rs`

```rust
use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree, Command};

struct SageExtension;

impl zed::Extension for SageExtension {
    fn new() -> Self {
        SageExtension
    }

    fn language_server_command(
        &mut self,
        _server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        // Look up `sage` on the user's PATH via the worktree environment
        let path = worktree
            .which("sage")
            .ok_or_else(|| {
                "sage binary not found on PATH. \
                 Install Sage from https://sagelang.dev/install \
                 or set `lsp.sage-lsp.binary.path` in Zed settings.".to_string()
            })?;

        Ok(Command {
            command: path,
            args: vec!["lsp".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(SageExtension);
```

### 12.5 Tree-sitter grammar (`tree-sitter-sage`)

The tree-sitter grammar is the significant piece of work for the Zed extension. It lives in `sage-lang/tree-sitter-sage` as a standard tree-sitter project:

```
tree-sitter-sage/
├── grammar.js              # Grammar definition (compiled → parser.c)
├── src/
│   ├── parser.c            # Generated — do not edit
│   └── tree_sitter/
│       └── parser.h
├── queries/
│   ├── highlights.scm      # Highlight queries (referenced by Zed)
│   ├── locals.scm          # Scope/local variable tracking
│   └── tags.scm            # Symbol navigation
├── bindings/               # Language bindings (Rust, Python, etc.)
│   └── rust/
└── test/
    └── corpus/             # Grammar test cases
```

A minimal `grammar.js` for Sage's top-level structure:

```javascript
module.exports = grammar({
  name: "sage",
  extras: ($) => [/\s/, $.comment],
  rules: {
    source_file: ($) =>
      repeat(
        choice(
          $.agent_declaration,
          $.fn_declaration,
          $.record_declaration,
          $.enum_declaration,
          $.tool_declaration,
          $.const_declaration,
          $.use_declaration,
          $.mod_declaration,
          $.run_statement
        )
      ),
    agent_declaration: ($) =>
      seq(
        optional("pub"),
        "agent",
        field("name", $.identifier),
        "{",
        repeat(choice($.field_declaration, $.handler)),
        "}"
      ),
    handler: ($) =>
      seq("on", field("event", $.event_kind), optional($.parameter), $.block),
    event_kind: (_) => choice("start", "stop", "error"),
    // ... more rules
    comment: (_) => token(seq("//", /.*/)),
    identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_]*/,
  },
});
```

The `highlights.scm` query file maps tree-sitter node types to Zed highlight captures:

```scheme
; Keywords
"agent" @keyword
"fn" @keyword
"tool" @keyword
"record" @keyword
"enum" @keyword
"let" @keyword
"const" @keyword
"if" @keyword.control
"else" @keyword.control
"while" @keyword.control
"for" @keyword.control
"return" @keyword.control
"match" @keyword.control
"spawn" @keyword
"await" @keyword
"emit" @keyword
"infer" @keyword
"try" @keyword
"catch" @keyword
"self" @variable.special

; Declarations
(agent_declaration name: (identifier) @type)
(fn_declaration name: (identifier) @function)
(tool_declaration name: (identifier) @type)
(record_declaration name: (identifier) @type)
(enum_declaration name: (identifier) @type)

; Literals
(string_literal) @string
(integer_literal) @number
(float_literal) @number
(boolean_literal) @boolean

; Comments
(comment) @comment

; Types
(type_identifier) @type.builtin
```

**Effort estimate for `tree-sitter-sage`:** 1–2 weeks for a complete, well-tested grammar. This is the primary time investment for the Zed extension. The Rust extension code itself is ~30 lines. Development uses `tree-sitter test` to iterate against a corpus of example `.sg` programs.

### 12.6 Zed user configuration

Users can override the binary path in their Zed `settings.json`:

```json
{
  "lsp": {
    "sage-lsp": {
      "binary": {
        "path": "/opt/homebrew/bin/sage"
      }
    }
  }
}
```

---

## 13. Crate Dependencies

### 13.1 `sage-lsp/Cargo.toml`

```toml
[package]
name = "sage-lsp"
description = "Language Server Protocol implementation for the Sage language"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
# Compiler pipeline
sage-lexer   = { path = "../sage-lexer",   version = "0.3.0" }
sage-parser  = { path = "../sage-parser",  version = "0.3.0" }
sage-checker = { path = "../sage-checker", version = "0.3.0" }
sage-types   = { path = "../sage-types",   version = "0.3.0" }
sage-loader  = { path = "../sage-loader",  version = "0.3.0" }

# LSP protocol
tower-lsp-server = "0.23"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Error handling and utilities
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[lints]
workspace = true
```

### 13.2 `sage-cli/Cargo.toml` addition

```toml
sage-lsp = { path = "../sage-lsp", version = "0.3.0" }
```

### 13.3 Workspace `Cargo.toml` addition

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/sage-lsp",    # NEW
]
```

---

## 14. Implementation Plan

### Phase 1 — Core LSP + Diagnostics (2–3 weeks)

This phase delivers a working `sage lsp` with diagnostics in both editors.

**Step 1: Parser error recovery (4–5 days)**

The most critical prerequisite. Without it, the LSP produces no diagnostics for files with syntax errors — which is exactly the state the file is in while the user is typing.

- Add `Expr::Error` and `Stmt::Error` AST variants to `sage-parser`
- Add `recover_with(nested_delimiters(...))` at block boundaries
- Add `recover_with(skip_then_retry_until(...))` at top-level declaration boundaries
- Add `parse_recovering()` public function returning `(Option<Program>, Vec<ParseError>)`
- Add `lex_recovering()` to `sage-lexer`
- Add `check_program_lsp()` to `sage-checker` that never short-circuits
- Update checker to silently skip `Expr::Error` and `Stmt::Error` nodes
- Tests: verify partial AST is returned for incomplete programs with missing braces, incomplete expressions, and mid-declaration edits

**Step 2: `sage-lsp` crate (3–4 days)**

- Create `crates/sage-lsp/` workspace crate
- Implement `DocumentStore`
- Implement `Backend` with `initialize`, `initialized`, `shutdown`, `did_open`, `did_change`, `did_close`
- Implement `analysis::analyse()` running the full pipeline
- Implement `convert::span_to_range()` with correct UTF-16 position encoding
- Implement `convert::check_error_to_diagnostic()` and equivalents for lex/parse errors
- Declare `TextDocumentSyncKind::FULL` in capabilities
- Implement `run()` entry point using `tower-lsp-server`
- Tests: unit tests for span-to-position conversion (including multibyte UTF-8 characters and UTF-16 surrogate pairs), integration test running the server against a mock client

**Step 3: `sage lsp` CLI subcommand (1 day)**

- Add `Commands::Lsp` to `sage-cli`
- Wire to `sage_lsp::run()`
- Verify no stdout output other than JSON-RPC
- Verify clean shutdown on `exit` notification
- Add to `Cargo.toml` workspace members

**Step 4: VS Code extension — TextMate grammar (1–2 days)**

- Create `sage-lang/vscode-sage` repository
- Write `syntaxes/sage.tmLanguage.json` covering all token types from §7
- Write `language-configuration.json`
- Verify highlighting against a comprehensive set of `.sg` example files
- Test with at least 3 different colour themes (light, dark, high contrast)
- Create minimal `package.json` and `src/extension.ts` (grammar-only, no LSP yet)
- Publish to VS Code Marketplace as pre-release

**Step 5: VS Code extension — LSP wiring (1 day)**

- Add `vscode-languageclient` dependency
- Implement `activate()` to launch `sage lsp`
- Add `sage.path` configuration setting
- Test: open a `.sg` file with a type error → red underline appears
- Test: fix the error → red underline disappears
- Test: `sage` not on PATH → helpful error message, no crash
- Publish updated extension to Marketplace

**Step 6: Zed extension (1–2 weeks, dominated by tree-sitter grammar)**

- Create `sage-lang/tree-sitter-sage` repository
- Write `grammar.js` covering top-level declarations, statements, expressions, and all type syntax
- Write `highlights.scm`, `brackets.scm`, `indents.scm`
- Build tree-sitter corpus test cases from `examples/` in the main repo
- Verify highlighting parity with the VS Code TextMate grammar
- Create `sage-lang/zed-sage` repository with `extension.toml` and `src/lib.rs`
- Test in Zed: syntax highlighting, LSP diagnostics, bracket matching, auto-indent
- Publish to Zed Extension Marketplace

**Phase 1 total estimated effort:** 4–5 weeks

---

### Phase 2 — Semantic Tokens (1–2 weeks, future)

Adds type-aware highlighting on top of the TextMate/tree-sitter baseline.

- Enable `semantic_tokens_provider` in capabilities with a `SemanticTokensLegend`
- Implement `textDocument/semanticTokens/full` handler
- Emit tokens for: agent names at spawn sites, tool names at call sites, field access on agents/records, `infer` calls, function names, type names in annotations
- Add `readonly` modifier for `const` bindings
- Add `async` modifier for `infer` and tool call expressions
- Update VS Code extension to enable semantic token support

---

### Phase 3 — Go-to-definition and Hover (2–3 weeks, future)

- Implement `textDocument/definition` — jump to agent/function/record/enum declarations
- Implement `textDocument/hover` — show the type of the expression under cursor, agent field types, function signatures
- Requires building a symbol index (map from `Url + Position → Definition`) from the type-checked AST
- The checker must retain enough information for position-based lookups — currently it discards spans after type checking

---

### Phase 4 — Completions (3–4 weeks, future)

- Implement `textDocument/completion` with context-aware suggestions
- `self.` → list agent fields
- `ToolName.` → list tool functions
- `spawn AgentName { ` → list required fields with type hints
- `use ` → list importable modules
- Bare identifier → functions, agents, records, enums in scope
- Keyword completions for `on`, `emit`, `try`, etc.

---

## 15. Future LSP Features

Beyond Phase 1, the following features are planned in rough priority order:

**Semantic tokens (Phase 2).** Richer, type-aware highlights. Type names in annotations show differently from value identifiers; `infer` calls are visually distinct from regular function calls; agent names at spawn sites are highlighted as types.

**Go-to-definition (Phase 3).** Jump to any symbol's declaration. Essential for navigating multi-file projects. Requires the checker to retain position information after analysis.

**Hover types (Phase 3).** Hovering over an expression shows its type. Hovering over a function call shows its signature. Hovering over an agent name shows its fields. This is the single most-requested feature after diagnostics in surveys of new language users.

**Completions (Phase 4).** Context-sensitive autocomplete. `self.` triggers field completions for the current agent. `Http.` triggers the tool function list. `spawn ` triggers a list of known agents with their required fields pre-populated.

**Inlay hints.** Show inferred types for `let` bindings that have no explicit annotation. Show argument names for function calls.

**Code actions.** Quick-fixes for common errors: add a missing `use ToolName` declaration, insert a missing `on error` handler, add exhaustive `match` arms.

**Workspace symbols.** Search for agents, functions, records by name across the whole project. Powers the `Go to Symbol in Workspace` VS Code command.

**Find all references.** Given an agent or function, find every call site.

**Rename.** Rename a symbol across all files. Requires the same position-indexed symbol map as go-to-definition.

**Formatting.** `textDocument/formatting` — format the current file. This requires a Sage pretty-printer, which in turn ideally requires a lossless CST (rowan-based) to preserve comments and handle partial formatting. This is a significant investment and is the primary argument for eventually migrating to a hand-written recursive descent parser.

---

## 16. Open Questions

### 16.1 Notification ordering (tower-lsp-server known issue)

`tower-lsp-server` processes notifications asynchronously rather than synchronously, which can cause `textDocument/didChange` notifications to be processed out of order under rapid typing. In practice, Sage's full-sync mode (`TextDocumentSyncKind::FULL`) makes this unlikely to cause visible bugs — each notification carries the complete file content, so an out-of-order pair still lands on a valid state. The debounce in §10.1 further reduces the risk.

If out-of-order processing becomes a problem, `async-lsp` (§17.2) handles notifications synchronously and is a clean migration path.

### 16.2 Multi-file project analysis

Phase 1 analyses files in isolation — each file is parsed and type-checked independently against the symbols defined in that file only. This means errors from cross-module type mismatches are not reported in the editor. Full project-aware analysis requires running the `sage-loader` module tree builder, which reads the filesystem to discover all project files.

The straightforward approach: when a file in a project is opened or changed, load and re-analyse the full module tree. This is expensive for large projects (though Sage projects are unlikely to be large). The correct approach is to maintain a per-project analysis cache, invalidate only affected modules on change, and re-check incrementally. This is deferred to Phase 3 or later.

### 16.3 Tree-sitter vs TextMate grammar maintenance burden

Maintaining two grammars (TextMate for VS Code, tree-sitter for Zed) means every new language feature needs a grammar update in two places. The risk is them drifting out of sync.

Two mitigations: first, the test suite for the tree-sitter grammar (corpus tests against example `.sg` files) will catch regressions quickly. Second, as Zed gains tree-sitter support, VS Code may also adopt it (there is an open VS Code issue for tree-sitter support, #50140). If that happens, the TextMate grammar can be retired.

In the shorter term, the tree-sitter grammar is the better-maintained one because it has a formal test suite. The TextMate grammar should be tested against the same example files, even if the tests are manual.

### 16.4 Long-term parser strategy

This RFC uses chumsky with error recovery for Phase 1. The research (§9) is clear that for production LSP quality — formatting, precise error recovery, refactoring — a hand-written recursive descent parser producing a rowan-based lossless CST is the right long-term architecture.

The migration path is not disruptive because the LSP layer (`sage-lsp`) depends on the public API of `sage-parser`, not its internals. Replacing the parser implementation behind the same `parse_recovering()` interface is transparent to the LSP. The migration should be planned for when formatting support (Phase 4+) is prioritised.

---

## 17. Alternatives Considered

### 17.1 Separate `sage-lsp` binary

Ship a separate `sage-lsp` binary instead of bundling `sage lsp`. This is what Roc does.

**Rejected.** It requires users to install two binaries, which is friction. The Gleam model (one binary, `gleam lsp`) is strictly better UX. The only argument for a separate binary is if the LSP has significantly different runtime requirements (e.g. it needs a GUI dependency) — which does not apply here.

### 17.2 `async-lsp` instead of `tower-lsp-server`

`async-lsp` handles notifications synchronously (correct per the LSP spec) and provides full Tower middleware composition. It is technically superior.

**Deferred, not rejected.** `tower-lsp-server` has a larger community, more examples, and a more ergonomic API for simple use cases. The notification ordering issue (§16.1) is unlikely to cause real problems with full-sync mode. If it does, migration to `async-lsp` is mechanical — the `LanguageServer` trait methods are similar.

### 17.3 `lsp-server` (synchronous, rust-analyzer style)

The synchronous `lsp-server` crate with a manual message dispatch loop. Used by Nushell.

**Rejected for Phase 1.** It requires substantially more boilerplate and is harder to get right. The benefit — full control over request handling — is not needed until the LSP has complex multi-request coordination. Start simple.

### 17.4 TextMate grammar only for Zed (no tree-sitter)

Zed does support some TextMate-like patterns for languages without tree-sitter grammars, but the support is limited and the experience significantly worse. Zed is fundamentally tree-sitter-first.

**Rejected.** If Zed is a target, writing `tree-sitter-sage` is non-negotiable. The effort is justified — the tree-sitter grammar also benefits Neovim, Helix, and any other editor that uses tree-sitter for highlighting.

### 17.5 Download-on-activation in the VS Code extension

Automatically download the `sage` binary on first extension activation, similar to how rust-analyzer's VS Code extension works.

**Deferred.** This adds significant complexity (GitHub API calls, platform detection, binary verification, auto-update logic) and is unnecessary while the user base is small. The PATH-based approach requires one manual install step but is much simpler to implement and maintain. Re-evaluate when the extension has significant adoption.

---

*Ward sees the error before you do. The squiggly red line is Ward's gift.*
