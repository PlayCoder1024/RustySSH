#!/bin/bash
# Start SSH test servers for RustySSH integration tests
#
# Usage:
#   ./tests/docker/start_servers.sh      # Start servers
#   ./tests/docker/start_servers.sh stop # Stop servers

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"

if [ "$1" = "stop" ]; then
    echo "Stopping SSH test servers..."
    docker-compose -f "$COMPOSE_FILE" down
    echo "Servers stopped."
    exit 0
fi

echo "Starting SSH test servers..."
docker-compose -f "$COMPOSE_FILE" up -d

# Wait for servers to be ready
echo "Waiting for servers to be ready..."
sleep 3

# Check health
for port in 2222 2223 2224; do
    if nc -z localhost $port 2>/dev/null; then
        echo "  ✓ Server on port $port is ready"
    else
        echo "  ✗ Server on port $port is not responding"
    fi
done

echo ""
echo "Test servers are ready!"
echo "  Server 1: localhost:2222 (testuser/testpass)"
echo "  Server 2: localhost:2223 (testuser/testpass)"
echo "  Server 3: localhost:2224 (altuser/altpass)"
echo ""
echo "Run tests with: cargo test --test main"
echo "Stop servers with: $0 stop"
