# RFC-0004: Pre-compiled Runtime for Zero-Dependency Builds

- **Status:** Draft
- **Created:** 2026-03-12
- **Author:** Sage Team
- **Depends on:** RFC-0003 (Compile to Rust)

## Summary

Eliminate the Rust toolchain requirement for Sage users by shipping pre-compiled runtime libraries (`.rlib` files) and invoking `rustc` directly instead of `cargo`. This reduces build times from ~10s to ~0.6s and removes the need for users to install Rust.

## Motivation

Currently, `sage build` shells out to `cargo build`, which requires:
1. A full Rust toolchain installed (`rustup`, `cargo`, `rustc`)
2. Compiling all dependencies on first build (~10s)
3. ~300MB of toolchain on disk

This creates friction for users who just want to write Sage programs.

### Goals

1. **Zero dependencies** — Users download Sage, it works
2. **Fast builds** — Sub-second compilation for typical programs
3. **Small distribution** — Keep installer under 100MB

## Design Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Sage Distribution                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   bin/                                                          │
│   └── sage                 CLI binary                           │
│                                                                 │
│   toolchain/                                                    │
│   ├── rustc                Rust compiler (platform-specific)   │
│   └── libs/                                                     │
│       ├── libstd-*.rlib    Rust standard library               │
│       ├── libcore-*.rlib   Core library                        │
│       ├── libtokio-*.rlib  Async runtime                       │
│       ├── libreqwest-*.rlib HTTP client                        │
│       ├── libserde-*.rlib  Serialization                       │
│       └── libsage_runtime-*.rlib  Sage runtime                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Build Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Sage Source │ ──▶ │   Codegen    │ ──▶ │  main.rs     │
│   (.sg)      │     │              │     │  (generated) │
└──────────────┘     └──────────────┘     └──────────────┘
                                                 │
                                                 ▼
                     ┌───────────────────────────────────────┐
                     │              rustc                     │
                     │                                        │
                     │  rustc main.rs                         │
                     │    --edition 2021                      │
                     │    --crate-type bin                    │
                     │    -L dependency=toolchain/libs        │
                     │    --extern sage_runtime=...           │
                     │    --extern tokio=...                  │
                     │    -o output/program                   │
                     │                                        │
                     └───────────────────────────────────────┘
                                                 │
                                                 ▼
                     ┌──────────────────────────────────────┐
                     │           Native Binary              │
                     │  (linked against pre-compiled libs)  │
                     └──────────────────────────────────────┘
```

## Implementation

### Phase 1: Build Infrastructure

Create a build script that pre-compiles all dependencies:

```bash
# scripts/build-toolchain.sh

TARGET="aarch64-apple-darwin"  # or x86_64-unknown-linux-gnu, etc.

# Build sage-runtime and all dependencies as rlibs
cargo build --release \
    --target $TARGET \
    -p sage-runtime \
    --message-format=json \
    | jq -r 'select(.reason=="compiler-artifact") | .filenames[]' \
    | grep '\.rlib$' \
    > rlib-list.txt

# Copy rlibs to distribution
mkdir -p dist/toolchain/libs
cat rlib-list.txt | xargs -I{} cp {} dist/toolchain/libs/

# Copy rustc
cp $(rustup which rustc) dist/toolchain/rustc
```

### Phase 2: Modify Codegen

Update `sage-codegen` to NOT generate a `Cargo.toml`. Instead, just generate `main.rs`.

```rust
// sage-codegen/src/generator.rs

pub struct GeneratedProject {
    pub main_rs: String,
    // Remove: pub cargo_toml: String,
}
```

### Phase 3: Direct rustc Invocation

Replace `cargo build` with direct `rustc` invocation:

```rust
// sage-cli/src/main.rs

fn compile_with_rustc(main_rs: &Path, output: &Path, release: bool) -> Result<()> {
    let toolchain_dir = get_toolchain_dir()?;
    let rustc = toolchain_dir.join("rustc");
    let libs_dir = toolchain_dir.join("libs");

    let mut cmd = Command::new(&rustc);
    cmd.arg(main_rs)
        .arg("--edition").arg("2021")
        .arg("--crate-type").arg("bin")
        .arg("-L").arg(format!("dependency={}", libs_dir.display()))
        .arg("-o").arg(output);

    // Add --extern for each dependency
    for rlib in std::fs::read_dir(&libs_dir)? {
        let rlib = rlib?.path();
        if let Some(name) = parse_rlib_name(&rlib) {
            cmd.arg("--extern").arg(format!("{}={}", name, rlib.display()));
        }
    }

    if release {
        cmd.arg("-O");  // Optimization
    }

    let status = cmd.status()?;
    if !status.success() {
        bail!("rustc failed");
    }

    Ok(())
}

fn parse_rlib_name(path: &Path) -> Option<String> {
    // libfoo-abc123.rlib -> foo
    let stem = path.file_stem()?.to_str()?;
    let name = stem.strip_prefix("lib")?;
    let name = name.split('-').next()?;
    Some(name.replace('_', "_"))  // Keep underscores
}
```

### Phase 4: Toolchain Discovery

```rust
fn get_toolchain_dir() -> Result<PathBuf> {
    // 1. Check relative to sage binary (for distribution)
    let exe = std::env::current_exe()?;
    let bundled = exe.parent()?.parent()?.join("toolchain");
    if bundled.exists() {
        return Ok(bundled);
    }

    // 2. Check SAGE_TOOLCHAIN env var (for development)
    if let Ok(path) = std::env::var("SAGE_TOOLCHAIN") {
        return Ok(PathBuf::from(path));
    }

    // 3. Fall back to system rustc (development mode)
    bail!("Sage toolchain not found. Set SAGE_TOOLCHAIN or reinstall Sage.")
}
```

## Platform Targets

### Tier 1 (Must Have)

| Target | OS | Arch | Notes |
|--------|-----|------|-------|
| `aarch64-apple-darwin` | macOS | ARM64 | M1/M2/M3 Macs |
| `x86_64-apple-darwin` | macOS | x86_64 | Intel Macs |
| `x86_64-unknown-linux-gnu` | Linux | x86_64 | Most servers |

### Tier 2 (Should Have)

| Target | OS | Arch | Notes |
|--------|-----|------|-------|
| `aarch64-unknown-linux-gnu` | Linux | ARM64 | AWS Graviton, etc. |
| `x86_64-pc-windows-msvc` | Windows | x86_64 | Windows users |

### Tier 3 (Nice to Have)

| Target | OS | Arch | Notes |
|--------|-----|------|-------|
| `x86_64-unknown-linux-musl` | Linux | x86_64 | Fully static binaries |
| `aarch64-unknown-linux-musl` | Linux | ARM64 | Alpine, etc. |

## Distribution Size

### Estimated Sizes (per platform)

| Component | Size |
|-----------|------|
| `rustc` | ~50MB |
| `libstd` + `libcore` | ~20MB |
| `libtokio` | ~5MB |
| `libreqwest` + deps | ~10MB |
| `libsage_runtime` | ~1MB |
| **Total** | **~86MB** |

Compressed (`.tar.gz`): ~30-40MB

### Comparison

| Distribution | Size |
|--------------|------|
| Full Rust toolchain | ~300MB |
| Sage with pre-compiled | ~40MB |
| Sage with bundled cargo | ~250MB |

## Release Process

### GitHub Actions Workflow

```yaml
# .github/workflows/release.yml

name: Release

on:
  push:
    tags: ['v*']

jobs:
  build-toolchain:
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build rlibs
        run: ./scripts/build-toolchain.sh ${{ matrix.target }}

      - name: Package
        run: |
          tar -czvf sage-${{ matrix.target }}.tar.gz \
            -C dist .

      - name: Upload
        uses: actions/upload-artifact@v4
        with:
          name: sage-${{ matrix.target }}
          path: sage-${{ matrix.target }}.tar.gz

  release:
    needs: build-toolchain
    runs-on: ubuntu-latest
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v4

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: '*/sage-*.tar.gz'
```

## Migration Path

### Phase 1: Parallel Support
- Keep `cargo build` path working
- Add `--use-toolchain` flag to use pre-compiled libs
- Test thoroughly

### Phase 2: Default Switch
- Pre-compiled becomes default
- `--use-cargo` flag for fallback
- Update documentation

### Phase 3: Remove Cargo Path
- Delete cargo-based compilation code
- Simplify codebase

## Open Questions

### 1. Cross-Compilation

Can users compile for a different target than their host?

**Options:**
- A: Ship all target rlibs with every distribution (~400MB)
- B: Download target rlibs on demand
- C: Don't support cross-compilation (use cargo for that)

**Recommendation:** C for now. Cross-compilation is an advanced use case.

### 2. Rust Version Pinning

Which Rust version do we pin to?

**Recommendation:** Latest stable at release time. Document in release notes.

### 3. rustc Updates

How do we handle rustc security updates?

**Recommendation:** Ship new Sage releases with updated rustc. Security-critical updates get expedited releases.

### 4. Debug Info

Should we include debug symbols in pre-compiled libs?

**Recommendation:** No for release builds (smaller). Yes for debug builds (better stack traces).

### 5. Incremental Compilation

rustc supports incremental compilation. Do we use it?

**Recommendation:** No initially. User code is small; incremental overhead may not be worth it.

## Performance Expectations

### Current (with cargo)

```
$ time sage build hello.sg
  Compiling tokio v1.50...
  Compiling reqwest v0.12...
  Compiling sage-runtime v0.1...
  Compiling hello v0.1...

real    0m10.2s
```

### With Pre-compiled Libs

```
$ time sage build hello.sg
  Compiling hello.sg...

real    0m0.6s
```

### Subsequent Builds (both)

With cargo's incremental: ~1-2s
With pre-compiled: ~0.4s (only user code changes)

## Risks

| Risk | Mitigation |
|------|------------|
| ABI incompatibility between rustc versions | Pin rustc version, test thoroughly |
| Platform-specific linking issues | CI testing on all platforms |
| Large distribution size | Compression, strip symbols |
| Complex release process | Automate with GitHub Actions |
| rustc licensing (MIT/Apache) | Compatible with MIT, include notices |

## Timeline

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Build scripts | 1 week | `build-toolchain.sh` working |
| rustc invocation | 1 week | Direct rustc compilation working |
| CI pipeline | 1 week | Automated builds for all platforms |
| Testing | 1 week | All examples work on all platforms |
| Documentation | 3 days | Install guide, release notes |
| **Total** | ~5 weeks | Production-ready |

## References

- [RFC-0003: Compile to Rust](./RFC-0003-compile-to-rust.md)
- [rustc Book: Linking](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker)
- [Cargo: Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
