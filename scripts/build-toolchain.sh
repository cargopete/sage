#!/bin/bash
# Build pre-compiled rlibs for Sage distribution
set -euo pipefail

HOST_TARGET=$(rustc -vV | grep host | cut -d' ' -f2)
TARGET="${1:-$HOST_TARGET}"
DIST_DIR="dist/$TARGET"
IS_CROSS_COMPILE=false

if [[ "$TARGET" != "$HOST_TARGET" ]]; then
    IS_CROSS_COMPILE=true
    echo "Cross-compiling toolchain for $TARGET (host: $HOST_TARGET)"
else
    echo "Building toolchain for $TARGET..."
fi

# Clean and create output directory
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR/libs"

# Build sage-runtime
echo "Compiling sage-runtime and dependencies..."
if $IS_CROSS_COMPILE; then
    # For cross-compilation, we must use --target
    cargo build --release \
        -p sage-runtime \
        --target "$TARGET" \
        --message-format=json 2>/dev/null \
        | jq -r 'select(.reason=="compiler-artifact") | .filenames[]' \
        | grep -E '\.(rlib|dylib|so|a)$' \
        | while read -r lib; do
            cp "$lib" "$DIST_DIR/libs/"
            echo "  Copied $(basename "$lib")"
        done
else
    # Native build - don't use --target for hash compatibility
    cargo build --release \
        -p sage-runtime \
        --message-format=json 2>/dev/null \
        | jq -r 'select(.reason=="compiler-artifact") | .filenames[]' \
        | grep -E '\.(rlib|dylib|so)$' \
        | while read -r lib; do
            cp "$lib" "$DIST_DIR/libs/"
            echo "  Copied $(basename "$lib")"
        done
fi

# Copy Rust sysroot libs (std, core, alloc, etc.)
SYSROOT=$(rustc --print sysroot)
SYSROOT_LIBS="$SYSROOT/lib/rustlib/$TARGET/lib"

if [ -d "$SYSROOT_LIBS" ]; then
    echo "Copying sysroot libraries for $TARGET..."
    for lib in "$SYSROOT_LIBS"/lib*.rlib "$SYSROOT_LIBS"/lib*.a; do
        if [ -f "$lib" ]; then
            cp "$lib" "$DIST_DIR/libs/"
            echo "  Copied $(basename "$lib")"
        fi
    done
else
    echo "Warning: Sysroot libraries not found at $SYSROOT_LIBS"
    echo "Ensure target is installed: rustup target add $TARGET"
fi

# Copy rustc binary
RUSTC_PATH=$(rustup which rustc)
mkdir -p "$DIST_DIR/bin"
cp "$RUSTC_PATH" "$DIST_DIR/bin/rustc"
echo "Copied rustc"

# Copy required shared libraries for rustc
RUSTC_DIR=$(dirname "$RUSTC_PATH")
if [ -d "$RUSTC_DIR/../lib" ]; then
    mkdir -p "$DIST_DIR/lib"
    cp -R "$RUSTC_DIR/../lib/"* "$DIST_DIR/lib/" 2>/dev/null || true
    echo "Copied rustc libraries"
fi

# Set up sysroot structure for bundled rustc
# rustc expects: $SYSROOT/lib/rustlib/$TARGET/lib/*.rlib
SYSROOT_TARGET="$DIST_DIR/lib/rustlib/$TARGET/lib"
mkdir -p "$SYSROOT_TARGET"
cp "$DIST_DIR/libs/"* "$SYSROOT_TARGET/" 2>/dev/null || true
echo "Set up sysroot structure"

# Create manifest
echo "Creating manifest..."
cat > "$DIST_DIR/manifest.json" << EOF
{
    "target": "$TARGET",
    "rust_version": "$(rustc --version)",
    "sage_version": "0.1.0",
    "created": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

# Calculate size
SIZE=$(du -sh "$DIST_DIR" | cut -f1)
echo ""
echo "Done! Toolchain built in $DIST_DIR ($SIZE)"
echo ""
echo "Contents:"
ls "$DIST_DIR/libs" | wc -l | xargs echo "  Libraries:"
