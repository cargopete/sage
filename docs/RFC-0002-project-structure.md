# RFC-0002: Multi-File Project Structure

- **Status:** Implemented
- **Created:** 2026-03-11
- **Updated:** 2026-03-13
- **Author:** Sage Contributors

## Summary

This RFC proposes a module system and project structure for Sage, enabling programs to span multiple files. This is essential for any non-trivial application.

## Motivation

Currently, Sage programs must be single files. This is fine for examples but impractical for real projects where you want to:

- Organize code logically (agents in one place, utilities in another)
- Reuse agents across projects
- Collaborate with multiple developers
- Build libraries of reusable components

## Design Goals

1. **Simple to start** - A single file should still "just work"
2. **Familiar** - Draw from Rust/Go/Python conventions
3. **Agent-centric** - The module system should feel natural for agent-based code
4. **Growable** - Can evolve into a package ecosystem later

---

## 1. Project Structure

### 1.1 Minimal Project

A single `.sg` file remains valid:

```
my_program.sg
```

### 1.2 Standard Project Layout

```
my_project/
├── sage.toml           # Project manifest
├── src/
│   ├── main.sg         # Entry point (contains `run Agent`)
│   ├── agents/
│   │   ├── mod.sg      # Agent module declarations
│   │   ├── researcher.sg
│   │   └── coordinator.sg
│   ├── lib/
│   │   ├── mod.sg
│   │   └── utils.sg
│   └── tools/          # Future: tool definitions
│       └── mod.sg
└── examples/
    └── demo.sg
```

### 1.3 Project Manifest: `sage.toml`

```toml
[project]
name = "my_project"
version = "0.1.0"
entry = "src/main.sg"

[dependencies]
# Future: external packages
# sage_stdlib = "0.1"
```

---

## 2. Module System

### 2.1 Declaring Modules

A `mod.sg` file declares what's in a directory:

```sage
// src/agents/mod.sg

pub mod researcher    // loads researcher.sg
pub mod coordinator   // loads coordinator.sg
```

### 2.2 Importing

Use `use` statements to bring items into scope:

```sage
// src/main.sg

use agents::Researcher
use agents::Coordinator
use lib::utils::format_result

agent Main {
    on start {
        let r = spawn Researcher { topic: "quantum computing" }
        let result = await r
        print(format_result(result))
        emit(result)
    }
}

run Main
```

### 2.3 Import Variations

```sage
// Import a single item
use agents::Researcher

// Import multiple items
use agents::{Researcher, Coordinator}

// Import all public items (use sparingly)
use agents::*

// Aliased import
use agents::Researcher as ResearchAgent

// Relative imports within same module
use super::utils
use self::helper
```

### 2.4 Visibility

By default, items are **private** to their module. Use `pub` to export:

```sage
// src/agents/researcher.sg

// Public - can be imported by other modules
pub agent Researcher {
    belief topic: String

    on start {
        let result = do_research(self.topic)
        emit(result)
    }
}

// Private - only visible within this file
fn do_research(topic: String) -> String {
    infer("Research: {topic}")
}

// Public function
pub fn create_researcher(topic: String) -> Agent<String> {
    spawn Researcher { topic: topic }
}
```

### 2.5 Re-exports

Modules can re-export items from submodules:

```sage
// src/agents/mod.sg

pub mod researcher
pub mod coordinator

// Re-export for convenience
pub use researcher::Researcher
pub use coordinator::Coordinator
```

This allows users to write `use agents::Researcher` instead of `use agents::researcher::Researcher`.

---

## 3. Name Resolution

### 3.1 Resolution Order

When resolving a name, the compiler looks in this order:

1. Local scope (variables, parameters)
2. Current module scope (agents, functions in same file)
3. Explicitly imported items (`use` statements)
4. Prelude (built-in functions: `print`, `len`, etc.)

### 3.2 Fully Qualified Names

Any item can be referenced by its full path:

```sage
let r = spawn agents::researcher::Researcher { topic: "test" }
```

### 3.3 Circular Imports

Circular imports are **not allowed**. The compiler will detect and report cycles:

```
error: circular import detected
  ┌─ src/agents/a.sg:1:1
  │
1 │ use agents::b::B
  │ ^^^^^^^^^^^^^^^^
  │
  = note: a.sg → b.sg → a.sg
```

---

## 4. Entry Point

### 4.1 The `run` Statement

Every executable Sage program must have exactly one `run` statement. In a multi-file project, this must be in the entry file specified in `sage.toml`.

```sage
// src/main.sg

use agents::Coordinator

run Coordinator  // Entry point
```

### 4.2 Libraries

A Sage project without a `run` statement is a **library** - it exports agents and functions but cannot be executed directly.

```toml
# sage.toml for a library
[project]
name = "sage_research_agents"
version = "0.1.0"
# No entry point - this is a library
```

---

## 5. CLI Changes

### 5.1 Running a Project

```bash
# Run a single file (unchanged)
sage run program.sg

# Run a project (looks for sage.toml)
sage run

# Run a project in a specific directory
sage run --project ./my_project
```

### 5.2 Checking a Project

```bash
# Check all files in project
sage check

# Check a specific file
sage check src/agents/researcher.sg
```

### 5.3 Creating a New Project

```bash
# Create a new project
sage new my_project

# Creates:
# my_project/
# ├── sage.toml
# └── src/
#     └── main.sg
```

---

## 6. Compilation Model

### 6.1 Two-Phase Compilation

**Phase 1: Discovery & Parsing**
1. Read `sage.toml` to find entry point
2. Parse entry file
3. Follow `mod` and `use` declarations to discover all files
4. Parse all discovered files
5. Build module tree

**Phase 2: Analysis & Checking**
1. Resolve all imports (detect cycles, missing modules)
2. Build global symbol table across all modules
3. Type check all modules
4. Validate entry agent

### 6.2 Incremental Compilation (Future)

For large projects, only recompile changed files. Track dependencies to know what's affected.

---

## 7. Example: Multi-File Research Project

### Project Structure

```
research_project/
├── sage.toml
└── src/
    ├── main.sg
    ├── agents/
    │   ├── mod.sg
    │   ├── researcher.sg
    │   └── fact_checker.sg
    └── lib/
        ├── mod.sg
        └── prompts.sg
```

### sage.toml

```toml
[project]
name = "research_project"
version = "0.1.0"
entry = "src/main.sg"
```

### src/agents/mod.sg

```sage
pub mod researcher
pub mod fact_checker

pub use researcher::Researcher
pub use fact_checker::FactChecker
```

### src/agents/researcher.sg

```sage
use lib::prompts::research_prompt

pub agent Researcher {
    belief topic: String

    on start {
        let findings = infer(research_prompt(self.topic))
        emit(findings)
    }
}
```

### src/agents/fact_checker.sg

```sage
use lib::prompts::fact_check_prompt

pub agent FactChecker {
    belief claim: String

    on start {
        let verification = infer(fact_check_prompt(self.claim))
        emit(verification)
    }
}
```

### src/lib/mod.sg

```sage
pub mod prompts
```

### src/lib/prompts.sg

```sage
pub fn research_prompt(topic: String) -> String {
    "Research the following topic thoroughly: " ++ topic
}

pub fn fact_check_prompt(claim: String) -> String {
    "Verify the following claim. Is it accurate? " ++ claim
}
```

### src/main.sg

```sage
use agents::{Researcher, FactChecker}

agent Coordinator {
    on start {
        // Research phase
        let r = spawn Researcher { topic: "quantum computing applications" }
        let findings = await r

        // Verification phase
        let fc = spawn FactChecker { claim: findings }
        let verified = await fc

        print("Findings: " ++ findings)
        print("Verification: " ++ verified)
        emit(verified)
    }
}

run Coordinator
```

---

## 8. Implementation Plan

### Phase 1: Module Parsing ✓
- [x] Parse `mod` declarations
- [x] Parse `use` statements
- [x] Build module tree from file system

### Phase 2: Name Resolution ✓
- [x] Resolve imports across modules
- [x] Detect circular dependencies
- [x] Handle visibility (`pub`)

### Phase 3: Cross-Module Type Checking ✓
- [x] Build global symbol table
- [x] Type check across module boundaries
- [x] Validate spawn/await types across modules

### Phase 4: CLI & Project Support (Partial)
- [x] Parse `sage.toml`
- [x] Implement `sage new`
- [x] Update `sage run` for projects
- [x] Update `sage check` for projects

### Phase 5: Polish
- [x] Good error messages for import failures
- [x] IDE support considerations (future)
- [x] Documentation generation (future)

---

## 9. Open Questions

1. **Prelude customization?** Should projects be able to extend/modify the prelude?

2. **Conditional compilation?** `#[cfg(test)]` style attributes for test-only code?

3. **Build caching?** Where to store intermediate artifacts?

4. **Workspaces?** Multiple related projects in one repo (like Cargo workspaces)?

5. **External dependencies?** Package registry? Git dependencies? Local paths?

---

## 10. Alternatives Considered

### 10.1 Single-File with Inline Modules

```sage
mod agents {
    pub agent Researcher { ... }
}
```

**Rejected:** Doesn't solve the "organize across files" problem.

### 10.2 Include-Style Imports

```sage
include "agents/researcher.sg"
```

**Rejected:** No namespacing, prone to conflicts, feels dated.

### 10.3 URL Imports (Deno-style)

```sage
use "https://sage-pkg.io/research-agents/v1"
```

**Deferred:** Interesting for future package management, but adds complexity. Start with local modules.

---

## 11. References

- [Rust Module System](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)
- [Go Package System](https://go.dev/doc/code)
- [Python Import System](https://docs.python.org/3/reference/import.html)

---

*This RFC enables Sage to grow from a scripting language into a tool for building real applications.*
