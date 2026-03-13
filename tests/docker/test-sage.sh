#!/bin/bash
# Test script for verifying Sage works on a clean system
set -euo pipefail

echo "========================================"
echo "Sage Installation Verification Test"
echo "========================================"
echo

# Test 1: Check sage binary exists
echo "Test 1: Checking sage binary..."
if command -v sage &>/dev/null; then
    echo "  ✅ sage binary found"
    sage --version
else
    echo "  ❌ sage binary not found"
    exit 1
fi
echo

# Test 2: Verify NO Rust is installed
echo "Test 2: Confirming Rust is NOT installed..."
if command -v rustc &>/dev/null; then
    echo "  ❌ rustc found - this test requires a Rust-free environment"
    exit 1
else
    echo "  ✅ rustc not found (as expected)"
fi

if command -v cargo &>/dev/null; then
    echo "  ❌ cargo found - this test requires a Rust-free environment"
    exit 1
else
    echo "  ✅ cargo not found (as expected)"
fi
echo

# Test 3: Check toolchain is set up
echo "Test 3: Checking pre-compiled toolchain..."
if [ -d "/usr/local/sage/toolchain" ]; then
    echo "  ✅ Toolchain directory exists"
    export SAGE_TOOLCHAIN=/usr/local/sage/toolchain
else
    echo "  ⚠️  Toolchain directory not found, sage may fall back to system Rust"
fi
echo

# Test 4: Type-check a program
echo "Test 4: Type-checking simple.sg..."
if sage check ~/test/simple.sg; then
    echo "  ✅ Type check passed"
else
    echo "  ❌ Type check failed"
    exit 1
fi
echo

# Test 5: Compile and run
echo "Test 5: Compiling and running simple.sg..."
if output=$(sage run ~/test/simple.sg 2>&1); then
    echo "  Output: $output"
    if echo "$output" | grep -q "Hello from Sage"; then
        echo "  ✅ Program executed successfully"
    else
        echo "  ❌ Unexpected output"
        exit 1
    fi
else
    echo "  ❌ Compilation/execution failed"
    echo "  Error: $output"
    exit 1
fi
echo

# Test 6: Build to binary
echo "Test 6: Building to standalone binary..."
if sage build ~/test/simple.sg -o ~/test/out; then
    if [ -f ~/test/out/simple ]; then
        echo "  ✅ Binary created"
        # Run the compiled binary
        if binary_output=$(~/test/out/simple 2>&1); then
            echo "  Output: $binary_output"
            echo "  ✅ Binary executed successfully"
        else
            echo "  ❌ Binary execution failed"
            exit 1
        fi
    else
        echo "  ❌ Binary not found"
        exit 1
    fi
else
    echo "  ❌ Build failed"
    exit 1
fi
echo

echo "========================================"
echo "✅ All tests passed!"
echo "========================================"
echo "Sage works correctly on Ubuntu without Rust."
