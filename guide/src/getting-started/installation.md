# Installation

## Prerequisites

Sage requires a C linker and OpenSSL headers for compilation. Rust is **not** required.

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

## Install Sage

### Homebrew (macOS)

```bash
brew install cargopete/sage/sage
```

### Quick Install (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/cargopete/sage/main/scripts/install.sh | bash
```

After installation, add the toolchain to your shell profile:

```bash
export SAGE_TOOLCHAIN=/usr/local/sage/toolchain
```

### Cargo (if you have Rust)

```bash
cargo install sage-lang
```

### Nix

```bash
nix profile install github:cargopete/sage
```

## Verify Installation

```bash
sage --version
```

You should see output like:

```
sage 0.1.0
```

## Next Steps

Now that Sage is installed, let's write your first program: [Hello World](./hello-world.md).
