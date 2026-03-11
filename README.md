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
        )
        emit(summary)
    }
}

agent Coordinator {
    on start {
        let r1 = spawn Researcher { topic: "quantum computing" }
        let r2 = spawn Researcher { topic: "CRISPR gene editing" }

        let s1 = await r1
        let s2 = await r2

        print(s1)
        print(s2)
    }
}

run Coordinator
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

## Building

```bash
cargo build --release
```

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
- [ ] **TASK-023** — Value enum and runtime environment
- [ ] **TASK-024** — Prelude built-in functions
- [ ] **TASK-025** — Expression evaluator
- [ ] **TASK-026** — Statement evaluator
- [ ] **TASK-027** — Agent task spawning
- [ ] **TASK-028** — Await and send implementation
- [ ] **TASK-029** — LLM backend
- [ ] **TASK-030** — Wire infer expression to LLM backend
- [ ] **TASK-031** — Runtime entry point
- [ ] **TASK-032** — Minimal supervision (fail-fast)
- [ ] **TASK-033** — Interpreter tests

### Milestone 6: CLI
- [ ] **TASK-034** — CLI binary with clap
- [ ] **TASK-035** — Release binary and README

### Milestone 7: Examples and Demo
- [ ] **TASK-036** — hello.sg
- [ ] **TASK-037** — infer.sg
- [ ] **TASK-038** — two_agents.sg
- [ ] **TASK-039** — research.sg (full demo)

### Milestone 8: Polish
- [ ] **TASK-040** — Error message polish
- [ ] **TASK-041** — Compiler warning for unused beliefs
- [ ] **TASK-042** — CONTRIBUTING.md and issue templates

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
