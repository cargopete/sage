<p align="center">
  <img src="assets/ward.png" alt="Ward the Owl" width="200">
</p>

<h1 align="center">Sage</h1>

<p align="center">
  <strong>A programming language where agents are first-class citizens.</strong><br>
  <em>Ward is watching.</em>
</p>

<p align="center">
  <a href="#installation">Install</a> •
  <a href="#language-syntax">Syntax</a> •
  <a href="#usage">Usage</a> •
  <a href="docs/RFC-0001-poc.md">Specification</a> •
  <a href="docs/VISION.md">Roadmap</a>
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

**v0.1.1 released** — Compiles to native binaries. No Rust installation required.

| | |
|---|---|
| **Latest** | [v0.1.1](https://github.com/cargopete/sage/releases/tag/v0.1.1) |
| **Extension** | `.sg` |
| **Platforms** | macOS (ARM), Linux (x86_64, ARM) |
| **Build time** | ~0.5s |

See [docs/RFC-0001-poc.md](docs/RFC-0001-poc.md) for the language specification.

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

while count < 10 {
    count = count + 1;
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

## Installation

### Prerequisites

**macOS:** Xcode Command Line Tools (for linking)
```bash
xcode-select --install
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt install gcc libssl-dev
```

### Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/cargopete/sage/main/scripts/install.sh | bash
```

This downloads the pre-compiled toolchain (~100-230MB) — no Rust required.

### From Source (for contributors)

```bash
git clone https://github.com/cargopete/sage
cd sage
cargo build --release
./scripts/build-toolchain.sh  # Optional: for fast builds
```

## Usage

Run a Sage program:

```bash
sage run examples/hello.sg

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
| `SAGE_TOOLCHAIN` | Path to pre-compiled toolchain | Auto-detected |

## Architecture

Sage follows a traditional multi-pass compiler architecture:

```
Source (.sg) → Lexer → Parser → Type Checker → Rust Codegen → Native Binary
```

The compiler is written in ~7,600 lines of Rust, organised into focused crates:

| Crate | Purpose |
|-------|---------|
| `sage-lexer` | Tokenizer (logos-based) |
| `sage-parser` | Parser (chumsky-based) |
| `sage-checker` | Name resolution + type checker |
| `sage-codegen` | Rust code generator |
| `sage-runtime` | Async runtime, LLM integration |
| `sage-cli` | Command-line interface |

## Project Structure

```
sage/
├── crates/
│   ├── sage-types/        # Shared type definitions (Span, Ident, TypeExpr)
│   ├── sage-lexer/        # Tokenizer (logos-based)
│   ├── sage-parser/       # Parser (chumsky-based)
│   ├── sage-checker/      # Name resolution + type checker
│   ├── sage-codegen/      # Rust code generator
│   ├── sage-runtime/      # Runtime library (agents, LLM, etc.)
│   └── sage-cli/          # CLI entry point
├── scripts/
│   └── build-toolchain.sh # Build pre-compiled runtime
├── docs/
│   ├── RFC-0001-poc.md    # Language specification
│   └── VISION.md          # Roadmap and future direction
├── tests/
│   └── docker/            # Installation verification tests
├── assets/
│   └── ward.png           # Ward the Owl mascot
└── examples/              # Example .sg programs
```

## License

MIT
