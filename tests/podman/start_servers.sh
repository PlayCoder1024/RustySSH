#!/bin/bash
# Start SSH test servers for RustySSH integration tests (Podman)
#
# Usage:
#   ./tests/podman/start_servers.sh      # Start servers
#   ./tests/podman/start_servers.sh stop # Stop servers

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/compose.yaml"

COMPOSE_CMD=""
if command -v podman >/dev/null 2>&1 && podman compose version >/dev/null 2>&1; then
    COMPOSE_CMD="podman compose"
elif command -v podman-compose >/dev/null 2>&1; then
    COMPOSE_CMD="podman-compose"
else
    echo "Error: podman compose or podman-compose not found in PATH." >&2
    exit 1
fi

if [ "$1" = "stop" ]; then
    echo "Stopping SSH test servers..."
    $COMPOSE_CMD -f "$COMPOSE_FILE" down
    echo "Servers stopped."
    exit 0
fi

echo "Starting SSH test servers..."
$COMPOSE_CMD -f "$COMPOSE_FILE" up -d

# Wait for servers to be ready
echo "Waiting for servers to be ready..."
sleep 3

# Check health
for port in 2201 2202 2203 2204 2205 2222; do
    if nc -z localhost "$port" 2>/dev/null; then
        echo "  [OK] Server on port $port is ready"
    else
        echo "  [FAIL] Server on port $port is not responding"
    fi
done

echo ""
echo "Test servers are ready!"
echo "  direct-01: localhost:2201 (root/password123)"
echo "  direct-02: localhost:2202 (root/password123)"
echo "  direct-03: localhost:2203 (root/password123)"
echo "  direct-04: localhost:2204 (root/password123)"
echo "  direct-05: localhost:2205 (root/password123)"
echo "  jump-host: localhost:2222 (root/password123)"
echo ""
echo "Run tests with: cargo test --test main"
echo "Stop servers with: $0 stop"
