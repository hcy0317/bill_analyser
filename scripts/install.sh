#!/bin/bash
# Bill Analyser - Linux/macOS 安装脚本
# =====================================================

set -e

# 颜色定义
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

# 获取项目根目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

echo -e "${GRAY}Project root: $PROJECT_ROOT${NC}"

# ============================================
# Python 环境设置
# ============================================

echo ""
echo -e "${YELLOW}[1/4] Setting up Python environment...${NC}"

# 检查 Python 版本
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

# 创建虚拟环境
if [ ! -d ".venv" ] || [ "$1" == "--force" ]; then
    if [ -d ".venv" ]; then
        echo -e "${YELLOW}Removing existing virtual environment...${NC}"
        rm -rf .venv
    fi
    echo -e "${YELLOW}Creating virtual environment...${NC}"
    python3 -m venv .venv
fi

# 安装 Python 依赖
echo -e "${YELLOW}Installing Python dependencies...${NC}"
source .venv/bin/activate
pip install --upgrade pip -q
pip install -r requirements.txt -q

echo -e "${GREEN}Python environment setup complete!${NC}"

# ============================================
# Node.js 环境设置
# ============================================

echo ""
echo -e "${YELLOW}[2/4] Setting up Node.js environment...${NC}"

# 检查 Node.js 版本
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

# 安装前端依赖
cd src/web
if [ ! -d "node_modules" ] || [ "$1" == "--force" ]; then
    echo -e "${YELLOW}Installing frontend dependencies...${NC}"
    npm install --silent
fi
cd "$PROJECT_ROOT"

echo -e "${GREEN}Node.js environment setup complete!${NC}"

# ============================================
# 创建所需目录
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
# 初始化配置
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
# 摘要
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
