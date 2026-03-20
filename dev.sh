#!/bin/bash
# dev.sh - Compile and run with mpremote mount for development

set -e

SRC_DIR="src"
BUILD_DIR="build"

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check for mpy-cross
if ! command -v mpy-cross &> /dev/null; then
    echo -e "${RED}Error: mpy-cross not found. Install with: pip install mpy-cross${NC}"
    exit 1
fi

# Check for mpremote
if ! command -v mpremote &> /dev/null; then
    echo -e "${RED}Error: mpremote not found. Install with: pip install mpremote${NC}"
    exit 1
fi

echo -e "${YELLOW}=== Compiling .py to .mpy ===${NC}"

# Clean and create build directory
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# Find all .py files and compile them
FAILED=0
while read -r pyfile; do
    # Get relative path from src/
    REL_PATH="${pyfile#$SRC_DIR/}"
    # Change extension to .mpy
    MPY_PATH="$BUILD_DIR/${REL_PATH%.py}.mpy"

    # Create subdirectory if needed
    mkdir -p "$(dirname "$MPY_PATH")"

    echo -n "  Compiling $REL_PATH..."
    if mpy-cross -march=xtensawin "$pyfile" -o "$MPY_PATH" 2>/tmp/mpy_cross_err; then
        echo -e " ${GREEN}✓${NC}"
    else
        echo -e " ${RED}✗${NC}"
        cat /tmp/mpy_cross_err
        FAILED=1
    fi
done < <(find "$SRC_DIR" -name "*.py")

if [ "$FAILED" -eq 1 ]; then
    echo -e "${RED}Compilation failed. Fix the errors above and try again.${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}=== Compilation complete ===${NC}"
echo ""
echo -e "${YELLOW}=== Mounting and running ===${NC}"
echo "(If boot.py is installed, hold A+B or wait for the 1s interrupt window)"
if ! mpremote mount "$BUILD_DIR" exec "import main; main.main()"; then
    echo ""
    echo -e "${RED}✗ Failed to connect${NC}"
    echo "  If boot.py is installed, hold A+B while running this script"
    echo "  to stay in REPL mode during the 1s startup window."
    exit 1
fi
