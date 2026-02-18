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
find "$SRC_DIR" -name "*.py" | while read -r pyfile; do
    # Get relative path from src/
    REL_PATH="${pyfile#$SRC_DIR/}"
    # Change extension to .mpy
    MPY_PATH="$BUILD_DIR/${REL_PATH%.py}.mpy"

    # Create subdirectory if needed
    mkdir -p "$(dirname "$MPY_PATH")"

    echo -n "  Compiling $REL_PATH..."
    if mpy-cross -march=xtensawin "$pyfile" -o "$MPY_PATH" 2>/dev/null; then
        echo -e " ${GREEN}✓${NC}"
    else
        echo -e " ${RED}✗${NC}"
        FAILED=1
    fi
done

if [ "$FAILED" -eq 1 ]; then
    echo -e "${RED}Compilation failed. Check mpy-cross output above.${NC}"
    echo -e "${YELLOW}Tip: For ESP32-C6 (RISC-V), you may need a different -march flag.${NC}"
    echo "Try running: mpremote exec \"import sys; print(sys.implementation)\""
    exit 1
fi

echo ""
echo -e "${GREEN}=== Compilation complete ===${NC}"
echo ""
echo -e "${YELLOW}=== Mounting and running ===${NC}"
mpremote mount "$BUILD_DIR" exec "import main; main.main()"
