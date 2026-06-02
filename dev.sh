#!/bin/bash
# dev.sh - Compile and run with mpremote mount for development

set -e

LANG="en"
while [[ $# -gt 0 ]]; do
    case $1 in
        --lang) LANG="$2"; shift 2 ;;
        *)      shift ;;
    esac
done

SRC_DIR="src"
BUILD_DIR="build"
TRANSLATED_DIR="$BUILD_DIR/translated-$LANG"

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

echo -e "${YELLOW}=== Translating source (lang=$LANG) ===${NC}"
rm -rf "$BUILD_DIR"
python3 tools/translate.py --lang "$LANG" "$SRC_DIR" "$TRANSLATED_DIR"
echo -e "${GREEN}=== Translation complete ===${NC}"

echo ""
echo -e "${YELLOW}=== Compiling .py to .mpy ===${NC}"
echo "  (skipping assets/ — frozen into firmware, not needed on filesystem)"
mkdir -p "$BUILD_DIR"

# Find all .py files and compile them (assets are frozen in firmware)
FAILED=0
while read -r pyfile; do
    # Get relative path from translated dir
    REL_PATH="${pyfile#$TRANSLATED_DIR/}"
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
done < <(find "$TRANSLATED_DIR" -name "*.py" -not -path "$TRANSLATED_DIR/assets/*")

if [ "$FAILED" -eq 1 ]; then
    echo -e "${RED}Compilation failed. Fix the errors above and try again.${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}=== Compilation complete ===${NC}"

echo ""
echo -e "${YELLOW}=== Converting level files ===${NC}"
mkdir -p "$BUILD_DIR/platformer_levels"
for txt in levels/level_*.txt; do
    name=$(basename "${txt%.txt}")
    python3 tools/convert_level.py "$txt" "$name" "$BUILD_DIR/platformer_levels" --quiet
done

echo ""
echo -e "${YELLOW}=== Mounting and running ===${NC}"
echo "(If boot.py is installed, hold A+B or wait for the 1s interrupt window)"
mpremote mount "$BUILD_DIR" exec "import main; main.main()"
