<p align="center">
  <img src="assets/ward.png" alt="Ward the Owl" width="200">
</p>

<h1 align="center">Sage</h1>

<p align="center">
  <strong>A programming language where agents are first-class citizens.</strong><br>
  <em>Ward is watching.</em>
</p>

<p align="center">
  <a href="#status">Status</a> •
  <a href="#language-syntax">Syntax</a> •
  <a href="#building">Building</a> •
  <a href="#implementation-progress">Progress</a> •
  <a href="docs/RFC-0001-poc.md">Specification</a>
</p>

---

Sage is not a library or framework — agents are a **semantic primitive** baked into the compiler and runtime. It targets professional software developers building AI-native systems.

Instead of wrestling with Python frameworks like LangChain or CrewAI, you write agents as naturally as you write functions:

```sage
agent Researcher {
    belief topic: String

    on start {
        let summary: Inferred<String> = infer(
            "Write a concise 2-sentence summary of: {self.topic}"
        );
        emit(summary);
    }
}

agent Coordinator {
    on start {
        let r1 = spawn Researcher { topic: "quantum computing" };
        let r2 = spawn Researcher { topic: "CRISPR gene editing" };

        let s1 = await r1;
        let s2 = await r2;

        print(s1);
        print(s2);
        emit(0);
    }
}

run Coordinator;
```

## Status

**Work in progress** — currently implementing the POC (Proof of Concept).

| | |
|---|---|
| **Language** | Sage |
| **Extension** | `.sg` |
| **Mascot** | Ward the Owl |
| **Implementation** | Rust |

See [docs/RFC-0001-poc.md](docs/RFC-0001-poc.md) for the full specification.

## Language Syntax

### Agents & Beliefs

Agents are the core abstraction — autonomous units with beliefs (state) and event handlers:

```sage
agent Worker {
    belief value: Int
    belief multiplier: Int

    on start {
        let result = self.value * self.multiplier;
        emit(result);
    }
}

agent Main {
    on start {
        let w = spawn Worker { value: 10, multiplier: 2 };
        let result = await w;
        emit(result);
    }
}

run Main;
```

### Functions

```sage
fn factorial(n: Int) -> Int {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}
```

### Control Flow

```sage
if x > 5 {
    emit(1);
} else {
    emit(0);
}

for item in [1, 2, 3] {
    print(str(item));
}
```

### Types

| Type | Description |
|------|-------------|
| `Int` | Integer numbers |
| `Float` | Floating-point numbers |
| `Bool` | `true` or `false` |
| `String` | Text strings |
| `Unit` | No value (like Rust's `()`) |
| `List<T>` | Lists, e.g., `[1, 2, 3]` |
| `Inferred<T>` | LLM inference results |

### Expressions

| Operator | Description |
|----------|-------------|
| `+`, `-`, `*`, `/` | Arithmetic |
| `==`, `!=`, `<`, `>`, `<=`, `>=` | Comparison |
| `&&`, `\|\|`, `!` | Logical |
| `++` | String concatenation |
| `"Hello, {name}!"` | String interpolation |

### Built-in Functions

| Function | Description |
|----------|-------------|
| `print(msg)` | Output to console |
| `str(value)` | Convert any type to string |
| `len(list)` | Get list length |
| `infer(prompt)` | LLM inference |

### Semicolons

Following Rust conventions:
- **Required** after: `let`, `return`, assignments, expression statements, `run`
- **Not required** after block statements: `if`/`else`, `for`

## Building

```bash
cargo build --release
```

## Usage

Run a Sage program:

```bash
# With mock LLM (for testing)
sage run examples/hello.sg --mock

# With real LLM (requires SAGE_API_KEY)
export SAGE_API_KEY="your-openai-api-key"
sage run examples/research.sg
```

Check a program for errors without running:

```bash
sage check examples/hello.sg
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SAGE_API_KEY` | OpenAI API key for LLM inference | Required for `infer` |
| `SAGE_LLM_URL` | Base URL for OpenAI-compatible API | `https://api.openai.com/v1` |
| `SAGE_MODEL` | Model to use | `gpt-4o-mini` |

## Implementation Progress

### Milestone 1: Project Scaffolding
- [x] **TASK-001** — Initialize Cargo workspace
- [x] **TASK-002** — Set up CI (GitHub Actions)
- [x] **TASK-003** — Define shared types crate (`sage-types`)

### Milestone 2: Lexer
- [x] **TASK-004** — Define Token enum
- [x] **TASK-005** — Implement lexer public API
- [x] **TASK-006** — Lexer tests *(comprehensive coverage included in TASK-004/005)*

### Milestone 3: Parser
- [x] **TASK-007** — Define AST types
- [x] **TASK-008** — Parser: top-level structure
- [x] **TASK-009** — Parser: agent declarations
- [x] **TASK-010** — Parser: statements
- [x] **TASK-011** — Parser: expressions
- [x] **TASK-012** — Parser: function declarations
- [x] **TASK-013** — Parser error recovery
- [x] **TASK-014** — Parser tests

### Milestone 4: Name Resolution + Type Checker
- [x] **TASK-015** — Name resolver
- [x] **TASK-016** — Type environment
- [x] **TASK-017** — Type checker: agents
- [x] **TASK-018** — Type checker: expressions
- [x] **TASK-019** — Type checker: statements
- [x] **TASK-020** — Type checker: functions
- [x] **TASK-021** — Entry agent validation
- [x] **TASK-022** — Type checker tests

### Milestone 5: Interpreter & Runtime
- [x] **TASK-023** — Value enum and runtime environment
- [x] **TASK-024** — Prelude built-in functions
- [x] **TASK-025** — Expression evaluator
- [x] **TASK-026** — Statement evaluator
- [x] **TASK-027** — Agent task spawning
- [x] **TASK-028** — Await and send implementation
- [x] **TASK-029** — LLM backend
- [x] **TASK-030** — Wire infer expression to LLM backend
- [x] **TASK-031** — Runtime entry point
- [x] **TASK-032** — Minimal supervision (fail-fast)
- [x] **TASK-033** — Interpreter tests

### Milestone 6: CLI
- [x] **TASK-034** — CLI binary with clap
- [x] **TASK-035** — Release binary and README

### Milestone 7: Examples and Demo
- [x] **TASK-036** — hello.sg
- [x] **TASK-037** — infer.sg
- [x] **TASK-038** — two_agents.sg
- [x] **TASK-039** — research.sg (full demo)

### Milestone 8: Polish
- [x] **TASK-040** — Error message polish
- [x] **TASK-041** — Compiler warning for unused beliefs
- [x] **TASK-042** — CONTRIBUTING.md and issue templates

## Project Structure

```
sage/
├── crates/
│   ├── sage-types/        # Shared type definitions (Span, Ident, TypeExpr)
│   ├── sage-lexer/        # Tokenizer (logos-based)
│   ├── sage-parser/       # Parser (chumsky-based)
│   ├── sage-checker/      # Name resolution + type checker
│   ├── sage-interpreter/  # Tree-walking interpreter + runtime
│   └── sage-cli/          # CLI entry point
├── docs/
│   └── RFC-0001-poc.md    # Full language specification
├── assets/
│   └── ward.png           # Ward the Owl mascot
└── examples/              # Example .sg programs
```

## License

MIT
