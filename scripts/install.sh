#!/bin/bash
# Bill Analyser Rust/Frontend development environment setup.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m'

echo ""
echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}  Bill Analyser - Installation Script${NC}"
echo -e "${CYAN}================================================${NC}"
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

echo -e "${GRAY}Project root: $PROJECT_ROOT${NC}"

echo ""
echo -e "${YELLOW}[1/3] Checking Rust toolchain...${NC}"
if ! command -v cargo >/dev/null 2>&1; then
    echo -e "${RED}Error: Rust toolchain not found. Install Rust stable first.${NC}"
    exit 1
fi
cargo --version
cargo build -p bill-analyser-http --bin bill_http_server
echo -e "${GREEN}Rust setup complete!${NC}"

echo ""
echo -e "${YELLOW}[2/3] Setting up Node.js environment...${NC}"
if ! command -v node >/dev/null 2>&1; then
    echo -e "${RED}Error: Node.js not found. Please install Node.js 22+.${NC}"
    exit 1
fi
NODE_MAJOR="$(node --version | grep -oE '[0-9]+' | head -1)"
echo -e "${GRAY}Node.js version: $(node --version)${NC}"
if [ "$NODE_MAJOR" -lt 22 ]; then
    echo -e "${RED}Error: Node.js 22+ is required${NC}"
    exit 1
fi

cd src/web
if [ ! -d "node_modules" ] || [ "${1:-}" = "--force" ]; then
    npm install --silent
fi
cd "$PROJECT_ROOT"
echo -e "${GREEN}Node.js setup complete!${NC}"

echo ""
echo -e "${YELLOW}[3/3] Creating required directories...${NC}"
for dir in data logs uploads output backup bills config; do
    if [ ! -d "$dir" ]; then
        mkdir -p "$dir"
        echo -e "${GRAY}  Created: $dir/${NC}"
    fi
done

if [ ! -f "config/server_config.json" ]; then
    cat > config/server_config.json << 'EOF'
{
  "host": "127.0.0.1",
  "port": 5000,
  "debug": true
}
EOF
    echo -e "${GRAY}  Created default server_config.json${NC}"
fi

echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Installation Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${CYAN}Next steps:${NC}"
echo "  1. Start backend:   ./start_backend.ps1 or cargo run -p bill-analyser-http --bin bill_http_server"
echo "  2. Start frontend:  cd src/web && npm run dev"
echo "  3. Open browser:    http://127.0.0.1:8081"
echo ""
