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
  <a href="https://sagelang.github.io/sage">Guide</a> •
  <a href="docs/RFC-0001-poc.md">Specification</a> •
  <a href="docs/VISION.md">Roadmap</a>
</p>

---

Sage is not a library or framework — agents are a **semantic primitive** baked into the compiler and runtime. It targets professional software developers building AI-native systems.

Instead of wrestling with Python frameworks like LangChain or CrewAI, you write agents as naturally as you write functions:

```sage
agent Researcher {
    topic: String

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

**v0.3.0 released** — First-class functions, closures, and package management.

| | |
|---|---|
| **Latest** | [v0.3.0](https://github.com/sagelang/sage/releases/tag/v0.3.0) |
| **Extension** | `.sg` |
| **Platforms** | macOS (ARM), Linux (x86_64, ARM) |
| **Build time** | ~0.5s |

See [docs/RFC-0001-poc.md](docs/RFC-0001-poc.md) for the language specification.

## Language Syntax

### Agents & State

Agents are the core abstraction — autonomous units with state and event handlers:

```sage
agent Worker {
    value: Int
    multiplier: Int

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

### Closures

Sage supports first-class functions and closures:

```sage
// Closure with typed parameters
let add = |x: Int, y: Int| x + y;

// Empty parameter closure
let get_value = || 42;

// Function taking a closure parameter
fn apply(f: Fn(Int) -> Int, x: Int) -> Int {
    return f(x);
}

// Usage
let double = |x: Int| x * 2;
let result = apply(double, 21);  // 42
```

Closure parameters currently require explicit type annotations.

### Modules & Multi-File Projects

Sage supports multi-file projects with a familiar module system:

```
my_project/
├── sage.toml           # Project manifest
└── src/
    ├── main.sg         # Entry point
    └── agents.sg       # Agent definitions
```

**sage.toml:**
```toml
[project]
name = "my_project"
entry = "src/main.sg"
```

**src/agents.sg:**
```sage
pub agent Worker {
    task: String

    on start {
        emit(self.task ++ " completed");
    }
}
```

**src/main.sg:**
```sage
mod agents;
use agents::Worker;

agent Main {
    on start {
        let w = spawn Worker { task: "Processing" };
        let result = await w;
        print(result);
        emit(0);
    }
}
run Main;
```

**Visibility:** Items are private by default. Use `pub` to export agents, functions, records, enums, and constants.

**Import styles:**
```sage
use agents::Worker;              // Single import
use agents::{Worker, Helper};    // Multiple imports
use agents::*;                   // Glob import
use agents::Worker as W;         // Aliased import
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

// Iterate over maps with tuple destructuring
let scores = {"alice": 100, "bob": 85};
for (name, score) in scores {
    print(name ++ ": " ++ str(score));
}

while count < 10 {
    count = count + 1;
}

loop {
    // runs indefinitely until break
    if done {
        break;
    }
}
```

### Agent Message Passing

Agents can receive typed messages from their mailbox, enabling actor-model patterns:

```sage
enum WorkerMsg {
    Task,
    Ping,
    Shutdown,
}

agent Worker receives WorkerMsg {
    id: Int

    on start {
        loop {
            let msg: WorkerMsg = receive();
            match msg {
                Task => print("Processing task"),
                Ping => print("Worker alive"),
                Shutdown => break,
            }
        }
        emit(0);
    }
}

agent Coordinator {
    on start {
        let w = spawn Worker { id: 1 };
        send(w, Task);
        send(w, Shutdown);
        await w;
        emit(0);
    }
}

run Coordinator;
```

The `receives` clause declares the message type an agent accepts. `receive()` blocks until a message arrives. Agents without `receives` are pure spawn/await agents.

### Types

| Type | Description |
|------|-------------|
| `Int` | Integer numbers |
| `Float` | Floating-point numbers |
| `Bool` | `true` or `false` |
| `String` | Text strings |
| `Unit` | No value (like Rust's `()`) |
| `List<T>` | Lists, e.g., `[1, 2, 3]` |
| `Map<K, V>` | Key-value maps, e.g., `{"a": 1, "b": 2}` |
| `(A, B, C)` | Tuples, e.g., `(1, "hello", true)` |
| `Option<T>` | Optional values (`Some(x)` or `None`) |
| `Result<T, E>` | Success or error (`Ok(x)` or `Err(e)`) |
| `Inferred<T>` | LLM inference results |
| `Fn(A, B) -> C` | Function types |

### Records & Enums

Define custom data types:

```sage
record Point {
    x: Int,
    y: Int,
}

enum Status {
    Active,
    Inactive,
    Pending,
}

// Enums can carry payloads
enum Result {
    Ok(Int),
    Err(String),
}

const MAX_RETRIES: Int = 3;
```

Construct records and access fields:

```sage
let p = Point { x: 10, y: 20 };
let sum = p.x + p.y;

// Construct enum variants with payloads
let success = Result::Ok(42);
let failure = Result::Err("not found");
```

### Match Expressions

Pattern matching with exhaustiveness checking:

```sage
fn describe(s: Status) -> String {
    return match s {
        Active => "running",
        Inactive => "stopped",
        Pending => "waiting",
    };
}

fn classify(n: Int) -> String {
    return match n {
        0 => "zero",
        1 => "one",
        _ => "many",
    };
}

// Pattern matching with payload binding
fn unwrap_result(r: Result) -> String {
    return match r {
        Ok(value) => str(value),
        Err(msg) => msg,
    };
}
```

### Expressions

| Operator | Description |
|----------|-------------|
| `+`, `-`, `*`, `/` | Arithmetic |
| `==`, `!=`, `<`, `>`, `<=`, `>=` | Comparison |
| `&&`, `\|\|`, `!` | Logical |
| `++` | String concatenation |
| `"Hello, {name}!"` | String interpolation |

### Maps & Tuples

Maps are key-value collections:

```sage
let ages = {"alice": 30, "bob": 25};
let alice_age = map_get(ages, "alice");  // Option<Int>

map_set(ages, "charlie", 35);
let has_bob = map_has(ages, "bob");      // true
let keys = map_keys(ages);               // List<String>
```

Tuples are fixed-size heterogeneous collections:

```sage
let pair = (42, "hello");
let first = pair.0;   // 42
let second = pair.1;  // "hello"

// Tuple destructuring
let (x, y) = pair;
```

### Built-in Functions

| Function | Description |
|----------|-------------|
| `print(msg)` | Output to console |
| `str(value)` | Convert any type to string |
| `len(list)` | Get list or map length |
| `push(list, item)` | Add item to list |
| `infer(prompt)` | LLM inference |
| `receive()` | Receive message from mailbox (agents only) |
| `send(handle, msg)` | Send message to agent |
| `map_get(map, key)` | Get value from map (returns `Option<V>`) |
| `map_set(map, key, value)` | Set key-value in map |
| `map_has(map, key)` | Check if key exists |
| `map_delete(map, key)` | Remove key from map |
| `map_keys(map)` | Get all keys as list |
| `map_values(map)` | Get all values as list |

### Semicolons

Following Rust conventions:
- **Required** after: `let`, `return`, assignments, expression statements, `run`
- **Not required** after block statements: `if`/`else`, `for`

## Installation

### Prerequisites

Sage requires a C linker and OpenSSL headers (Rust is **not** required).

**macOS:**
```bash
xcode-select --install
```

**Debian/Ubuntu:**
```bash
sudo apt install gcc libssl-dev
```

**Fedora/RHEL:**
```bash
sudo dnf install gcc openssl-devel
```

**Arch:**
```bash
sudo pacman -S gcc openssl
```

### Homebrew (macOS)

```bash
brew install sagelang/sage/sage
```

### Cargo (if you have Rust)

```bash
cargo install sage-lang
```

### Nix

```bash
nix profile install github:sagelang/sage
```

Or add to your flake inputs.

### Quick Install (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/sagelang/sage/main/scripts/install.sh | bash
```

Homebrew and quick install download the pre-compiled toolchain (~100-230MB) — no Rust required.

### From Source

```bash
git clone https://github.com/sagelang/sage
cd sage
cargo build --release
```

## Usage

Run a Sage program:

```bash
# Single file
sage run examples/hello.sg

# Project directory (looks for sage.toml)
sage run my_project/

# With real LLM (requires SAGE_API_KEY)
export SAGE_API_KEY="your-openai-api-key"
sage run examples/research.sg
```

Check a program for errors without running:

```bash
# Single file
sage check examples/hello.sg

# Project directory
sage check my_project/
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SAGE_API_KEY` | OpenAI API key for LLM inference | Required for `infer` |
| `SAGE_LLM_URL` | Base URL for OpenAI-compatible API | `https://api.openai.com/v1` |
| `SAGE_MODEL` | Model to use | `gpt-4o-mini` |
| `SAGE_INFER_RETRIES` | Max retries for structured inference | `3` |
| `SAGE_TOOLCHAIN` | Path to pre-compiled toolchain | Auto-detected |

## Architecture

Sage follows a traditional multi-pass compiler architecture:

```
Source (.sg) → Lexer → Parser → Loader → Type Checker → Rust Codegen → Native Binary
```

The compiler is written in ~9,000 lines of Rust, organised into focused crates:

| Crate | Purpose |
|-------|---------|
| `sage-lexer` | Tokenizer (logos-based) |
| `sage-parser` | Parser (chumsky-based) |
| `sage-loader` | Module loading + project management |
| `sage-package` | Package management (git-based) |
| `sage-checker` | Name resolution + type checker |
| `sage-codegen` | Rust code generator |
| `sage-runtime` | Async runtime, LLM integration |
| `sage-lang` | Command-line interface |

## Project Structure

```
sage/
├── crates/
│   ├── sage-types/        # Shared type definitions (Span, Ident, TypeExpr)
│   ├── sage-lexer/        # Tokenizer (logos-based)
│   ├── sage-parser/       # Parser (chumsky-based)
│   ├── sage-loader/       # Module loading + project management
│   ├── sage-package/      # Package management (git-based)
│   ├── sage-checker/      # Name resolution + type checker
│   ├── sage-codegen/      # Rust code generator
│   ├── sage-runtime/      # Runtime library (agents, LLM, etc.)
│   └── sage-cli/          # CLI entry point
├── scripts/
│   └── build-toolchain.sh # Build pre-compiled runtime
├── docs/
│   ├── RFC-0001-poc.md    # Language specification
│   ├── RFC-0002-*.md      # Multi-file project structure
│   ├── RFC-0005-*.md      # User-defined types
│   ├── RFC-0006-*.md      # Agent message passing
│   ├── RFC-0007-*.md      # Error handling
│   ├── RFC-0009-*.md      # First-class functions
│   ├── RFC-0010-*.md      # Maps, tuples, enum payloads
│   └── VISION.md          # Roadmap and future direction
├── tests/
│   └── docker/            # Installation verification tests
├── assets/
│   └── ward.png           # Ward the Owl mascot
└── examples/              # Example .sg programs
```

## License

MIT
