#!/bin/bash
# Build and run the Ubuntu installation test
#
# Usage:
#   ./tests/docker/test-install.sh                    # Test latest GitHub release
#   ./tests/docker/test-install.sh v0.1.0             # Test specific version
#   ./tests/docker/test-install.sh --local TARBALL   # Test local tarball
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_ROOT"

if [[ "${1:-}" == "--local" ]]; then
    TARBALL="${2:?Usage: $0 --local <tarball-path>}"
    if [[ ! -f "$TARBALL" ]]; then
        echo "Error: Tarball not found: $TARBALL" >&2
        exit 1
    fi

    echo "🐳 Building Ubuntu test container with local tarball..."
    docker build \
        -f tests/docker/Dockerfile.local \
        --build-arg TARBALL="$TARBALL" \
        -t sage-test-local \
        .

    echo
    echo "🧪 Running installation tests..."
    docker run --rm sage-test-local
else
    VERSION="${1:-}"
    BUILD_ARGS=""
    if [[ -n "$VERSION" ]]; then
        BUILD_ARGS="--build-arg SAGE_VERSION=$VERSION"
    fi

    echo "🐳 Building Ubuntu test container..."
    docker build \
        -f tests/docker/Dockerfile.ubuntu-clean \
        $BUILD_ARGS \
        -t sage-test-ubuntu \
        .

    echo
    echo "🧪 Running installation tests..."
    docker run --rm sage-test-ubuntu
fi

echo
echo "🎉 All tests passed! Sage installs and runs correctly on clean Ubuntu."
