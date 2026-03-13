# RFC-0008: Package Manager

- **Status:** Implemented
- **Created:** 2026-03-13
- **Author:** Pete Pavlovski
- **Depends on:** RFC-0002 (Multi-File Project Structure)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Package Concepts](#4-package-concepts)
5. [Manifest Changes (sage.toml)](#5-manifest-changes-sagetoml)
6. [The Lock File (sage.lock)](#6-the-lock-file-sagelock)
7. [Import Syntax](#7-import-syntax)
8. [Dependency Resolution](#8-dependency-resolution)
9. [Package Cache](#9-package-cache)
10. [CLI Commands](#10-cli-commands)
11. [Integration with Existing Commands](#11-integration-with-existing-commands)
12. [Implementation Plan](#12-implementation-plan)
13. [New Error Codes](#13-new-error-codes)
14. [Open Questions](#14-open-questions)
15. [Alternatives Considered](#15-alternatives-considered)
16. [Out of Scope](#16-out-of-scope)

---

## 1. Summary

This RFC introduces a **package manager** for Sage. Packages are Git repositories containing a Sage library project. Dependencies are declared in `sage.toml`, pinned in `sage.lock`, and cached in `~/.sage/packages/<name>/<version>/`. The package name becomes a top-level path prefix in import statements, consistent with how local modules work. The initial implementation is **Git-first** — no central registry is required.

The feature surface is four CLI commands: `sage add`, `sage remove`, `sage install`, and `sage update`.

---

## 2. Motivation

Sage's module system (RFC-0002) enables code organisation within a project but provides no mechanism for sharing code between projects. Every Sage project today must reimplement common patterns from scratch: prompt utilities, agent templates, domain-specific records, and reusable coordinators.

This creates two concrete problems:

**No standard library.** There is nowhere to put `sage_stdlib` — the collection of utility functions (string manipulation, list operations, math) that every language needs. Without a package system, stdlib must be baked into the compiler, which couples language releases to library evolution.

**No ecosystem.** The promise of agents-as-primitives is that the community builds a rich catalogue of reusable agents — a `ResearchAgent`, a `FactCheckerAgent`, a `TranslatorAgent` — that developers compose into applications. This catalogue cannot exist without a way to publish, version, and consume packages.

---

## 3. Design Goals

1. **Git-first.** No registry infrastructure required for v1. Any public Git repository is a valid package source.
2. **Reproducible builds.** A `sage.lock` file pins every dependency to an exact commit SHA. Two developers with the same lockfile get identical builds.
3. **Familiar manifest syntax.** Extend `sage.toml` with a `[dependencies]` table. Format follows Cargo conventions.
4. **Seamless imports.** Package names become path prefixes in `use` statements. No new import syntax required.
5. **Shared global cache.** Packages are downloaded once and reused across all projects on the machine.
6. **Path to a registry.** Every design decision should be reversible or extensible when a central registry is added later.

---

## 4. Package Concepts

### 4.1 What is a Package?

A Sage **package** is a Git repository that contains a valid Sage library project — a `sage.toml` with a `[project]` section and no `run` statement in its entry point. Packages export agents, records, enums, functions, and constants via the standard `pub` visibility system.

A package has:
- A **name** declared in its own `sage.toml`
- A **version** (SemVer string, e.g. `"1.2.0"`)
- A **Git URL** pointing to the repository
- An **entry point** (defaults to `src/main.sg`, configurable via `entry` in `sage.toml`)

### 4.2 Package Name as Path Prefix

When a dependency named `sage_stdlib` is declared, its public items become importable under the `sage_stdlib::` path prefix — exactly as if `sage_stdlib` were a local module declared via `mod sage_stdlib`. No new import syntax is introduced.

```sage
// Importing from a package works identically to importing from a local module
use sage_stdlib::strings::split;
use sage_stdlib::math::clamp;
use research_agents::Researcher;
```

The package name in `sage.toml` must match the `name` field in the dependency's own `sage.toml`. The loader enforces this at install time.

### 4.3 Library vs Executable

A package intended for use as a dependency must be a **library** — a Sage project with no `run` statement at its entry point. Attempting to depend on an executable project (one with `run`) is an error.

```toml
# sage.toml for a library package — no entry run statement
[project]
name = "research_agents"
version = "0.3.0"
entry = "src/lib.sg"
```

---

## 5. Manifest Changes (sage.toml)

### 5.1 The `[dependencies]` Table

Dependencies are declared under `[dependencies]`. Each key is the local package name (used as the import path prefix) and the value is a dependency specifier.

```toml
[project]
name = "my_project"
version = "0.1.0"
entry = "src/main.sg"

[dependencies]
sage_stdlib = { git = "https://github.com/sagelang/stdlib", tag = "v0.2.0" }
research_agents = { git = "https://github.com/acme/research-agents", tag = "v1.0.0" }
```

### 5.2 Dependency Specifier Forms

Three forms are supported, in order of preference:

**Tag** — resolves to the commit that tag points to. Recommended for production dependencies. The tag is resolved to a commit SHA at install time and pinned in `sage.lock`.

```toml
sage_stdlib = { git = "https://github.com/sagelang/stdlib", tag = "v0.2.0" }
```

**Branch** — resolves to the current HEAD of a branch. Useful during development. Non-reproducible without a lockfile.

```toml
my_lib = { git = "https://github.com/me/my-lib", branch = "main" }
```

**Rev** — resolves to an exact commit SHA. Maximally explicit. Recommended when a package has no release tags.

```toml
experimental = { git = "https://github.com/someone/experimental", rev = "a3f8c21" }
```

Exactly one of `tag`, `branch`, or `rev` must be specified. Specifying more than one is an error.

### 5.3 Name Matching

The key in `[dependencies]` (the local name) **must match** the `name` field in the dependency package's own `sage.toml`. This is enforced at `sage install` time. The constraint exists to prevent confusion — if you import `use research_agents::Researcher`, the package at that path must actually call itself `research_agents`.

---

## 6. The Lock File (sage.lock)

### 6.1 Purpose

`sage.lock` records the exact resolved state of every dependency — including transitive dependencies — so that builds are fully reproducible. It is always generated by `sage install` and `sage add`, and is always read by `sage run`, `sage check`, and `sage build`.

**`sage.lock` must be committed to version control** for executable projects. Library packages should add `sage.lock` to `.gitignore` (following Cargo's convention — consumers pin versions, not libraries).

### 6.2 Format

`sage.lock` is a TOML file. Each resolved package is a `[[package]]` entry:

```toml
# This file is generated by `sage install`. Do not edit manually.
# Regenerate with `sage install` or `sage update`.

[[package]]
name = "sage_stdlib"
version = "0.2.0"
git = "https://github.com/sagelang/stdlib"
rev = "d4e8f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8"

[[package]]
name = "research_agents"
version = "1.0.0"
git = "https://github.com/acme/research-agents"
rev = "9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e"

[[package]]
name = "string_utils"
version = "0.5.1"
git = "https://github.com/sagelang/string-utils"
rev = "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b"
# transitive dependency of research_agents
```

### 6.3 Lock File Staleness

The lock file is considered stale when:
- A dependency exists in `sage.toml` but not in `sage.lock`
- A dependency exists in `sage.lock` but not in `sage.toml`
- The resolved `rev` in `sage.lock` does not match what the specifier in `sage.toml` would resolve to (e.g., a tag has moved — this is only checked on `sage update`)

When stale, `sage run` and `sage check` print a warning and automatically re-run `sage install` before proceeding.

---

## 7. Import Syntax

No new syntax is introduced. Package names are resolved identically to top-level local modules. The loader's module resolution order becomes:

1. Local module (declared via `mod` in the current project)
2. Installed package (name matches a key in `[dependencies]`)
3. Prelude

```sage
// sage.toml has: research_agents = { git = "...", tag = "v1.0.0" }

use research_agents::Researcher;
use research_agents::FactChecker;
use research_agents::prompts::research_prompt;

agent Main {
    on start {
        let r = spawn Researcher { topic: "quantum computing" };
        let result = await r;
        print(result);
        emit(0);
    }
}

run Main;
```

If a local module and a package dependency share the same name, the local module takes precedence and a warning is emitted.

---

## 8. Dependency Resolution

### 8.1 Resolution Strategy

For v1, the resolver is intentionally simple: **breadth-first traversal with version conflict detection**. The algorithm is:

1. Parse `sage.toml` to get the direct dependency list.
2. For each dependency, fetch the package's `sage.toml` from Git (or from the local cache if already downloaded) to discover its own dependencies.
3. Recurse until the full transitive dependency graph is known.
4. Detect conflicts: if two packages in the graph require the same package name at **incompatible versions** (different major versions under SemVer), fail with error E030.
5. If no conflicts, write `sage.lock` with the resolved commit SHA for every package.

### 8.2 Version Compatibility

Two version requirements for the same package are compatible if they resolve to the same major version and the higher minor/patch satisfies both. The resolver picks the highest compatible version.

```
research_agents requires: sage_stdlib ^0.2.0
my_project requires:      sage_stdlib ^0.2.1

→ Resolves to: sage_stdlib 0.2.3 (latest 0.2.x)  ✓
```

```
package_a requires: utils ^1.0.0
package_b requires: utils ^2.0.0

→ Error E030: incompatible version requirements for "utils"  ✗
```

### 8.3 Resolution from Lock File

When `sage.lock` is present and up-to-date, the resolver skips version resolution entirely and uses the pinned commit SHAs directly. This makes `sage run` fast — no network calls, no resolution logic.

---

## 9. Package Cache

### 9.1 Cache Location

All downloaded packages are stored in the **global package cache**:

```
~/.sage/packages/<name>/<version>/
```

Examples:
```
~/.sage/packages/sage_stdlib/0.2.0/
~/.sage/packages/research_agents/1.0.0/
~/.sage/packages/string_utils/0.5.1/
```

Each directory is a complete checkout of the package at the resolved commit — a bare clone is made and then the specific revision is checked out. The cache is shared across all Sage projects on the machine.

### 9.2 Cache Integrity

Each cached package directory includes a `.sage-meta.toml` file written by the package manager:

```toml
name = "sage_stdlib"
version = "0.2.0"
git = "https://github.com/sagelang/stdlib"
rev = "d4e8f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8"
cached_at = "2026-03-13T10:22:00Z"
```

Before using a cached package, the loader verifies the `rev` in `.sage-meta.toml` matches the rev in `sage.lock`. If they differ (e.g., manual cache corruption), the package is re-downloaded.

### 9.3 Multiple Versions

Because packages are stored under `<name>/<version>/`, multiple versions of the same package can coexist in the cache without conflict. A project using `sage_stdlib 0.1.0` and another using `sage_stdlib 0.2.0` share the same machine cache but each gets their pinned version.

### 9.4 Cache Management

```bash
# Show cache contents and disk usage
sage cache list

# Remove a specific cached package
sage cache remove sage_stdlib@0.1.0

# Remove all cached packages (nuclear option)
sage cache clean
```

These commands are informational and maintenance utilities — they do not affect `sage.toml` or `sage.lock`.

---

## 10. CLI Commands

### 10.1 `sage add`

Adds a dependency to `sage.toml` and updates `sage.lock`.

```bash
sage add <name> --git <url> --tag <tag>
sage add <name> --git <url> --branch <branch>
sage add <name> --git <url> --rev <sha>
```

Examples:
```bash
sage add sage_stdlib --git https://github.com/sagelang/stdlib --tag v0.2.0
sage add research_agents --git https://github.com/acme/research-agents --tag v1.0.0
```

Behaviour:
1. Validates that the Git URL is reachable.
2. Fetches the package's `sage.toml` to confirm `name` matches the `<name>` argument (E031).
3. Confirms the package is a library, not an executable (E032).
4. Appends the dependency to `sage.toml`.
5. Runs the resolver to update `sage.lock`.
6. Downloads the package to the cache if not already present.

Output:
```
  Adding research_agents v1.0.0 (https://github.com/acme/research-agents)
Resolving dependencies...
    Fetching string_utils v0.5.1 (transitive)
     Locking 2 packages
    Updated sage.lock
```

### 10.2 `sage remove`

Removes a dependency from `sage.toml` and `sage.lock`.

```bash
sage remove <name>
```

Example:
```bash
sage remove research_agents
```

Behaviour:
1. Removes the entry from `[dependencies]` in `sage.toml`.
2. Re-runs the resolver to rebuild `sage.lock` (transitive deps no longer needed by other packages are also removed).
3. Does **not** remove anything from the local cache (use `sage cache remove` for that).

Output:
```
  Removing research_agents
   Removed 1 package (string_utils retained as dependency of sage_stdlib)
    Updated sage.lock
```

### 10.3 `sage install`

Resolves all dependencies in `sage.toml` and writes `sage.lock`. Downloads any packages not already in the cache.

```bash
sage install
```

Behaviour:
1. Reads `sage.toml`.
2. Runs the full resolver from scratch.
3. Downloads any packages not already in the cache.
4. Writes `sage.lock`.

This command is idempotent. Running it again when nothing has changed is a no-op (the lock file is already consistent). This is the command CI systems should call before `sage build`.

Output:
```
Resolving dependencies...
   Fetching sage_stdlib v0.2.0 (cached)
   Fetching research_agents v1.0.0 (downloading...)
   Fetching string_utils v0.5.1 (downloading...)
     Locking 3 packages
    Updated sage.lock
```

### 10.4 `sage update`

Re-resolves dependencies, allowing newer versions to be fetched within the constraints declared in `sage.toml`.

```bash
# Update all dependencies
sage update

# Update a single dependency
sage update sage_stdlib
```

For `tag` and `rev` specifiers, `sage update` is a no-op on that package (the version is already pinned to a specific commit). For `branch` specifiers, `sage update` fetches the latest HEAD of the branch and updates `sage.lock`.

To update a `tag` specifier to a new release, edit `sage.toml` manually and then run `sage install`.

Output:
```
   Updating research_agents v1.0.0 → v1.1.0 (branch: main advanced to a9f8e7d)
     Locking 3 packages
    Updated sage.lock
```

---

## 11. Integration with Existing Commands

### 11.1 `sage run` and `sage check`

Both commands check for lockfile staleness before proceeding. If the lock file is missing or stale, they automatically invoke `sage install`:

```
$ sage run src/main.sg
  Warning: sage.lock is out of date. Running sage install...
Resolving dependencies...
   Fetching sage_stdlib v0.2.0 (downloading...)
     Locking 1 package
    Updated sage.lock
  Running src/main.sg
```

If `sage install` fails (e.g., network unavailable), `sage run` exits with an error rather than proceeding with a potentially inconsistent environment.

### 11.2 `sage build`

`sage build` (future command) behaves identically to `sage run` with respect to dependency installation — it ensures the lock file is fresh before compiling.

### 11.3 The Module Loader (`sage-loader`)

The loader gains a new resolution phase before local module resolution:

1. Read `sage.lock` to get the list of installed packages and their cache paths.
2. For each `use` statement, check if the first path segment matches a package name. If so, resolve the path against the package's cache directory.
3. Proceed with local module resolution for non-package paths.

This change is entirely within `sage-loader` — the parser, checker, and codegen are unaware that some modules came from packages.

---

## 12. Implementation Plan

### Phase 1 — Manifest & Lock File (3–4 days)
- [x] Extend `ProjectManifest` in `sage-loader` to parse `[dependencies]`
- [x] Define `DependencySpec` type (`git`, `tag`/`branch`/`rev`)
- [x] Define `LockFile` type and TOML serialization/deserialization
- [x] Implement `sage.lock` staleness detection
- [x] Write unit tests for manifest and lock file parsing

### Phase 2 — Package Cache (2–3 days)
- [x] Implement `~/.sage/packages/<name>/<version>/` cache structure
- [x] Implement Git fetch + checkout to cache directory (using `git2` crate or shell `git`)
- [x] Implement `.sage-meta.toml` write and integrity check
- [x] Implement cache hit detection (skip download if rev matches)
- [x] Write tests with a local bare Git repo as fixture

### Phase 3 — Resolver (3–4 days)
- [x] Implement breadth-first transitive dependency traversal
- [x] Implement SemVer compatibility checking
- [x] Implement conflict detection (E030)
- [x] Implement name/type validation at install time (E031, E032)
- [x] Write resolver unit tests covering: no deps, single dep, transitive deps, version conflict

### Phase 4 — Loader Integration (2–3 days)
- [x] Extend `ModuleTree` loading to resolve package path prefixes from `sage.lock`
- [x] Add package paths to module resolution order (before local modules)
- [x] Warn on local module / package name collision
- [x] Write integration test: project that imports from a local package fixture

### Phase 5 — CLI Commands (3–4 days)
- [x] Implement `sage add` with Git validation and manifest update
- [x] Implement `sage remove` with manifest update and lock rebuild
- [x] Implement `sage install` with progress output
- [x] Implement `sage update` (all and single-package forms)
- [x] Implement `sage cache list`, `sage cache remove`, `sage cache clean`
- [x] Wire `sage run` and `sage check` to check lockfile staleness

### Phase 6 — Tests & Polish (2–3 days)
- [x] End-to-end test: `sage add`, import from package, `sage run`
- [x] End-to-end test: transitive dependencies
- [x] End-to-end test: version conflict produces E030
- [x] End-to-end test: lockfile ensures reproducible resolution
- [x] CI: add a minimal fixture package repository for tests
- [x] Update README with package manager documentation

**Total estimated effort:** ~3 weeks

---

## 13. New Error Codes

| Code | Name | Description |
|------|------|-------------|
| E030 | `IncompatibleVersions` | Two packages in the dependency graph require incompatible versions of the same package (different major versions). |
| E031 | `PackageNameMismatch` | The name specified in `sage add` or `[dependencies]` does not match the `name` field in the package's own `sage.toml`. |
| E032 | `DependencyIsExecutable` | The declared dependency is an executable project (has a `run` statement). Only library packages may be depended upon. |
| E033 | `LockFileMissing` | `sage run` or `sage check` was invoked without a `sage.lock` and `--offline` was passed (preventing automatic `sage install`). |
| E034 | `PackageNotFound` | A `use` statement references a package name that is not declared in `[dependencies]`. |
| E035 | `GitFetchFailed` | The Git repository could not be fetched. Includes the URL and the underlying error. |

---

## 14. Open Questions

**Q1: Should the local dependency name be allowed to differ from the package's declared name?**

Currently the RFC requires them to match. But there are valid reasons to alias: two packages with the same name from different authors, or a fork under a different name. A future `rename` key could handle this:

```toml
my_alias = { git = "...", tag = "v1.0.0", rename = "original_name" }
```

Deferred — the matching requirement is simpler and the aliasing need is rare.

**Q2: How should `sage add` behave when no `--tag`/`--branch`/`--rev` is specified?**

Options: default to `branch = "main"`, error and require explicit specifier, or query the repo for its latest tag. Defaulting to `main` is the most ergonomic but least reproducible. Requiring an explicit specifier is safer. For v1, require explicit specifier and provide a clear error message.

**Q3: Should packages be able to re-export items from their dependencies?**

`pub use dep::SomeAgent` already works in the module system. It should work for package items too, since package modules are just modules. No explicit design needed — test and confirm.

**Q4: What is the `SAGE_PATH` equivalent for local package overrides?**

For development workflows (e.g., working on `sage_stdlib` and an app simultaneously), developers need a way to override a registry/Git dependency with a local path:

```toml
sage_stdlib = { path = "../sage_stdlib" }
```

This is Cargo's `[patch]` / path dependency model. Useful enough that it should probably be in v1, but adds implementation complexity. Marking as a stretch goal for this RFC.

**Q5: Workspaces?**

Multiple related packages in one repository (like Cargo workspaces) is a common need but a significant feature. Deferred to a follow-up RFC.

---

## 15. Alternatives Considered

### 15.1 URL Imports (Deno-style)

```sage
use "https://sage-pkg.io/research_agents/v1/mod.sg"::Researcher;
```

**Rejected.** This embeds URLs into source code, making them hard to update, audit, or version-lock centrally. RFC-0002 considered and rejected this form.

### 15.2 Central Registry (Cargo/npm-style)

```toml
sage_stdlib = "0.2"
```

**Deferred, not rejected.** A registry is the right end state — discoverability, short names, centralized security scanning. But operating registry infrastructure is a meaningful commitment. Git-first lets the ecosystem start without that commitment and the manifest format is designed so registry support is purely additive: short-form version strings can be parsed as registry deps, Git specifiers remain valid alongside them.

### 15.3 Vendoring by Default

Copy all dependency source code into a `vendor/` directory and commit it. No network required at build time.

**Deferred.** A `sage vendor` command that populates `vendor/` from `sage.lock` is a clean addition for air-gapped or fully reproducible environments. Not needed for v1 — the lockfile already guarantees reproducibility for projects with network access.

---

## 16. Out of Scope

- **Central package registry** — future RFC
- **`sage publish`** — requires registry
- **Workspaces** — multiple packages in one repository
- **Private registries** — authentication for corporate artifact stores
- **`path` dependencies** — local path overrides (stretch goal for this RFC)
- **`sage vendor`** — copy deps into the repository
- **Dependency security auditing** — vulnerability scanning
- **Package signing / verification** — cryptographic integrity beyond commit SHA matching

---

*Packages let the community build the ecosystem that makes agents-as-primitives real. Ward expects many good agents.*
