#!/bin/bash
# Bill Analyser - Installation Script for Linux/macOS
# =====================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

echo ""
echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}  Bill Analyser - Installation Script${NC}"
echo -e "${CYAN}================================================${NC}"
echo ""

# Get project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

echo -e "${GRAY}Project root: $PROJECT_ROOT${NC}"

# ============================================
# Python Environment Setup
# ============================================

echo ""
echo -e "${YELLOW}[1/4] Setting up Python environment...${NC}"

# Check Python version
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Error: Python3 not found. Please install Python 3.10+${NC}"
    exit 1
fi

PYTHON_VERSION=$(python3 --version 2>&1 | grep -oP '\d+\.\d+' | head -1)
echo -e "${GRAY}Python version: $PYTHON_VERSION${NC}"

MAJOR=$(echo $PYTHON_VERSION | cut -d. -f1)
MINOR=$(echo $PYTHON_VERSION | cut -d. -f2)

if [ "$MAJOR" -lt 3 ] || ([ "$MAJOR" -eq 3 ] && [ "$MINOR" -lt 10 ]); then
    echo -e "${RED}Error: Python 3.10+ is required${NC}"
    exit 1
fi

# Create virtual environment
if [ ! -d ".venv" ] || [ "$1" == "--force" ]; then
    if [ -d ".venv" ]; then
        echo -e "${YELLOW}Removing existing virtual environment...${NC}"
        rm -rf .venv
    fi
    echo -e "${YELLOW}Creating virtual environment...${NC}"
    python3 -m venv .venv
fi

# Install Python dependencies
echo -e "${YELLOW}Installing Python dependencies...${NC}"
source .venv/bin/activate
pip install --upgrade pip -q
pip install -r requirements.txt -q

echo -e "${GREEN}Python environment setup complete!${NC}"

# ============================================
# Node.js Environment Setup
# ============================================

echo ""
echo -e "${YELLOW}[2/4] Setting up Node.js environment...${NC}"

# Check Node.js version
if ! command -v node &> /dev/null; then
    echo -e "${RED}Error: Node.js not found. Please install Node.js 18+${NC}"
    exit 1
fi

NODE_VERSION=$(node --version | grep -oP '\d+' | head -1)
echo -e "${GRAY}Node.js version: v$NODE_VERSION${NC}"

if [ "$NODE_VERSION" -lt 18 ]; then
    echo -e "${RED}Error: Node.js 18+ is required${NC}"
    exit 1
fi

# Install frontend dependencies
cd src/web
if [ ! -d "node_modules" ] || [ "$1" == "--force" ]; then
    echo -e "${YELLOW}Installing frontend dependencies...${NC}"
    npm install --silent
fi
cd "$PROJECT_ROOT"

echo -e "${GREEN}Node.js environment setup complete!${NC}"

# ============================================
# Create Required Directories
# ============================================

echo ""
echo -e "${YELLOW}[3/4] Creating required directories...${NC}"

DIRECTORIES=("data" "logs" "uploads" "output" "backup" "bills")

for dir in "${DIRECTORIES[@]}"; do
    if [ ! -d "$dir" ]; then
        mkdir -p "$dir"
        echo -e "${GRAY}  Created: $dir/${NC}"
    fi
done

echo -e "${GREEN}Directories setup complete!${NC}"

# ============================================
# Initialize Configuration
# ============================================

echo ""
echo -e "${YELLOW}[4/4] Checking configuration...${NC}"

mkdir -p config

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

echo -e "${GREEN}Configuration check complete!${NC}"

# ============================================
# Summary
# ============================================

echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Installation Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${CYAN}Next steps:${NC}"
echo "  1. Activate venv:   source .venv/bin/activate"
echo "  2. Start backend:   python src/api/app.py"
echo "  3. Start frontend:  cd src/web && npm run dev"
echo "  4. Open browser:    http://127.0.0.1:8081"
echo ""
echo -e "${GRAY}For more information, see README.md${NC}"
echo ""
