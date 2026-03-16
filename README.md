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
  <a href="https://sagelang.github.io/sage-book">Guide</a> •
  <a href="https://github.com/sagelang/rfcs">RFCs</a>
</p>

---

Sage is not a library or framework — agents are a **semantic primitive** baked into the compiler and runtime. It targets professional software developers building AI-native systems.

Instead of wrestling with Python frameworks like LangChain or CrewAI, you write agents as naturally as you write functions:

```sage
agent Researcher {
    topic: String

    on start {
        let summary = try infer(
            "Write a concise 2-sentence summary of: {self.topic}"
        );
        emit(summary);
    }

    on error(e) {
        emit("Research unavailable");
    }
}

agent Coordinator {
    on start {
        let r1 = spawn Researcher { topic: "quantum computing" };
        let r2 = spawn Researcher { topic: "CRISPR gene editing" };

        let s1 = try await r1;
        let s2 = try await r2;

        print(s1);
        print(s2);
        emit(0);
    }

    on error(e) {
        print("A researcher failed");
        emit(1);
    }
}

run Coordinator;
```

## Status

**v1.0.3 released** — Generics (parametric polymorphism) for functions, records, and enums.

| | |
|---|---|
| **Latest** | [v1.0.3](https://github.com/sagelang/sage/releases/tag/v1.0.3) |
| **Extension** | `.sg` |
| **Platforms** | macOS (ARM), Linux (x86_64, ARM) |
| **Build time** | ~0.5s |
| **Editors** | [Zed](https://zed.dev), [VS Code](https://code.visualstudio.com) |

See the [RFCs repository](https://github.com/sagelang/rfcs) for the language specification.

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
        let result = try await w;
        emit(result);
    }

    on error(e) {
        emit(0);
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

### Generics

Functions, records, and enums support type parameters:

```sage
// Generic functions
fn identity<T>(x: T) -> T {
    return x;
}

fn map<T, U>(list: List<T>, f: Fn(T) -> U) -> List<U> {
    let result: List<U> = [];
    for item in list {
        result = push(result, f(item));
    }
    return result;
}

// Generic records
record Pair<A, B> {
    first: A,
    second: B,
}

// Generic enums
enum Either<L, R> {
    Left(L),
    Right(R),
}

// Usage - types are inferred
let x = identity(42);                    // T = Int
let pair = Pair { first: 1, second: "hi" };  // Pair<Int, String>

// Turbofish syntax for explicit type arguments
let e = Either::<String, Int>::Left("error");
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
├── grove.toml          # Project manifest
└── src/
    ├── main.sg         # Entry point
    └── agents.sg       # Agent definitions
```

**grove.toml:**
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
        let result = try await w;
        print(result);
        emit(0);
    }

    on error(e) {
        emit(1);
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
        try send(w, Task);
        try send(w, Shutdown);
        try await w;
        emit(0);
    }

    on error(e) {
        emit(1);
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

### Error Handling

Fallible operations (`infer`, `await`, `send`, and functions marked `fails`) must be explicitly handled:

```sage
agent Main {
    on start {
        // try propagates errors to the agent's on error handler
        let result = try infer("What is 2+2?");
        print(result);
        emit(0);
    }

    on error(e) {
        print("Something went wrong");
        emit(1);
    }
}

run Main;
```

You can also use `catch` to handle errors inline:

```sage
let result = catch infer("prompt") {
    "fallback value"
};
```

Functions can be marked as fallible:

```sage
fn risky_operation() -> Int fails {
    let value = try infer("Give me a number");
    return parse_int(value);
}
```

### Built-in Tools

Agents can use built-in tools by declaring them with `use`:

```sage
agent Fetcher {
    use Http

    on start {
        let response = try Http.get("https://httpbin.org/get");
        print(response.body);
        emit(response.status);
    }

    on error(e) {
        emit(-1);
    }
}

run Fetcher;
```

**Available tools:**

| Tool | Methods | Description |
|------|---------|-------------|
| `Http` | `get(url)`, `post(url, body)` | HTTP client for web requests |

**HttpResponse fields:**

| Field | Type | Description |
|-------|------|-------------|
| `status` | `Int` | HTTP status code (e.g., 200, 404) |
| `body` | `String` | Response body as text |
| `headers` | `Map<String, String>` | Response headers |

Tool calls are fallible and must be wrapped in `try` or `catch`.

### Testing

Sage has a built-in testing framework with first-class LLM mocking:

**src/main_test.sg:**
```sage
test "addition works" {
    assert_eq(1 + 1, 2);
}

test "agent returns expected output" {
    mock infer -> "Mocked LLM response";

    let result = await spawn Summariser { topic: "test" };
    assert_eq(result, "Mocked LLM response");
}

@serial test "runs in isolation" {
    // This test won't run concurrently with others
    assert_true(true);
}
```

Run tests:
```bash
sage test .              # Run all tests in project
sage test . --filter add # Run tests matching "add"
sage test . --verbose    # Show failure details
```

**Test files** must end in `_test.sg` and are automatically discovered.

**Assertions available:**
- `assert(expr)` — assert expression is true
- `assert_eq(a, b)` — assert equality
- `assert_neq(a, b)` — assert inequality
- `assert_gt`, `assert_lt`, `assert_gte`, `assert_lte` — comparisons
- `assert_contains(str, substr)` — string contains
- `assert_starts_with`, `assert_ends_with` — string prefix/suffix
- `assert_empty`, `assert_not_empty` — collection checks
- `assert_fails(expr)` — assert expression returns an error

**Mock LLM responses** with `mock infer -> value;`. Mocks are consumed in order.

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
| `Http.get(url)` | HTTP GET request (requires `use Http`) |
| `Http.post(url, body)` | HTTP POST request (requires `use Http`) |

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

## Editor Support

Sage includes a Language Server Protocol (LSP) implementation for real-time diagnostics in your editor.

### Zed

Install the Sage extension from the Zed extension registry, or search for "Sage" in Extensions (`Cmd+Shift+X`).

Features:
- Syntax highlighting (tree-sitter based)
- Real-time error diagnostics
- Auto-indentation

### VS Code

Install the Sage extension from the VS Code marketplace, or search for "Sage" in Extensions.

Features:
- Syntax highlighting (TextMate grammar)
- Real-time error diagnostics

### Language Server

The language server is built into the `sage` CLI. Editors connect via:

```bash
sage sense
```

This starts the LSP server on stdin/stdout. Most editors handle this automatically when the Sage extension is installed.

## Usage

Create a new project:

```bash
sage new my_project
cd my_project
sage run .
```

Run a Sage program:

```bash
# Single file
sage run examples/hello.sg

# Project directory (looks for grove.toml)
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
| `sage-parser` | Lexer + Parser (logos + chumsky) |
| `sage-loader` | Module loading + project management |
| `sage-package` | Package management (git-based) |
| `sage-checker` | Name resolution + type checker |
| `sage-codegen` | Rust code generator |
| `sage-runtime` | Async runtime, LLM integration |
| `sage-sense` | Language Server Protocol (LSP) |
| `sage-cli` | Command-line interface |

## Project Structure

```
sage/
├── crates/
│   ├── sage-parser/       # Lexer + Parser (logos + chumsky)
│   ├── sage-loader/       # Module loading + project management
│   ├── sage-package/      # Package management (git-based)
│   ├── sage-checker/      # Name resolution + type checker
│   ├── sage-codegen/      # Rust code generator
│   ├── sage-runtime/      # Runtime library (agents, LLM, etc.)
│   ├── sage-sense/        # Language Server Protocol (LSP)
│   └── sage-cli/          # CLI entry point
├── scripts/
│   └── build-toolchain.sh # Build pre-compiled runtime
├── tests/
│   └── docker/            # Installation verification tests
├── assets/
│   └── ward.png           # Ward the Owl mascot
└── examples/              # Example .sg programs
```

## Related Repositories

| Repository | Description |
|------------|-------------|
| [sagelang/rfcs](https://github.com/sagelang/rfcs) | Language design RFCs |
| [sagelang/sage-book](https://github.com/sagelang/sage-book) | GitBook documentation |
| [sagelang/sage-vscode](https://github.com/sagelang/sage-vscode) | VS Code extension |
| [sagelang/sage-zed](https://github.com/sagelang/sage-zed) | Zed extension |
| [sagelang/tree-sitter-sage](https://github.com/sagelang/tree-sitter-sage) | Tree-sitter grammar |

## License

MIT
