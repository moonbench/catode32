#!/bin/bash
# upload.sh - Upload script for virtual pet project

# Note, this installs boot.py on the device which is required for standalone operation
# However, when boot.py is installed, mpremote will struggl to get a REPL shell, which makes development difficult
# There are instructions in the README for how to remove boot.py so you can develop again

set -e  # Exit on any error

echo "=== Virtual Pet Upload Script ==="
echo ""

# Configuration
DEVICE_PORT="${1:-}"  # Optional: pass port as first argument
SRC_DIR="src"
BUILD_DIR="build"

# Color codes for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if source directory exists
if [ ! -d "$SRC_DIR" ]; then
    echo -e "${RED}Error: $SRC_DIR directory not found!${NC}"
    exit 1
fi

# Check if mpremote is available
if ! command -v mpremote &> /dev/null; then
    echo -e "${RED}Error: mpremote not found. Install with: pip install mpremote${NC}"
    exit 1
fi

# Check for mpy-cross
if ! command -v mpy-cross &> /dev/null; then
    echo -e "${RED}Error: mpy-cross not found. Install with: pip install mpy-cross${NC}"
    exit 1
fi

# Function to run mpremote with optional port
mp() {
    if [ -n "$DEVICE_PORT" ]; then
        mpremote connect "$DEVICE_PORT" "$@"
    else
        mpremote "$@"
    fi
}

echo "Step 1: Checking connection..."
if mp fs ls / > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Connected to device${NC}"
else
    echo -e "${RED}✗ Failed to connect to device${NC}"
    exit 1
fi

echo ""
echo "Step 2: Installing dependencies..."
mp mip install ssd1306
echo -e "${GREEN}✓ SSD1306 library installed${NC}"

echo ""
echo -e "${YELLOW}Step 3: Compiling .py to .mpy...${NC}"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

find "$SRC_DIR" -name "*.py" | while read -r pyfile; do
    REL_PATH="${pyfile#$SRC_DIR/}"
    MPY_PATH="$BUILD_DIR/${REL_PATH%.py}.mpy"
    mkdir -p "$(dirname "$MPY_PATH")"
    echo -n "  Compiling $REL_PATH..."
    if mpy-cross -march=xtensawin "$pyfile" -o "$MPY_PATH" 2>/dev/null; then
        echo -e " ${GREEN}✓${NC}"
    else
        echo -e " ${RED}✗${NC}"
        echo -e "${RED}Compilation failed for $pyfile${NC}"
        exit 1
    fi
done
echo -e "${GREEN}✓ Compilation complete${NC}"

echo ""
echo -e "${YELLOW}Step 4: Cleaning ALL files from device...${NC}"
echo "  (keeping boot.py and lib/ directory for safety)"

# Function to recursively delete files and directories
clean_device() {
    # Get list of all items in root
    mp fs ls / | while read -r line; do
        # Parse the ls output (format: "size filename" or "directory/")
        if [[ $line =~ ^[0-9]+[[:space:]]+(.+)$ ]]; then
            # It's a file
            filename="${BASH_REMATCH[1]}"
            # Skip boot.py (system file) and main.py might be running
            if [[ "$filename" != "boot.py" ]] && [[ "$filename" != "webrepl_cfg.py" ]]; then
                echo "    Removing file: /$filename"
                mp fs rm "/$filename" 2>/dev/null || true
            fi
        elif [[ $line =~ ^(.+)\/$ ]]; then
            # It's a directory
            dirname="${BASH_REMATCH[1]}"
            # Skip lib directory (contains installed packages)
            if [[ "$dirname" != "lib" ]]; then
                echo "    Removing directory: /$dirname/"
                mp fs rm -r "/$dirname" 2>/dev/null || true
            fi
        fi
    done
}

# Clean the device
clean_device
echo -e "${GREEN}✓ Device cleaned${NC}"

echo ""
echo "Step 5: Uploading compiled files..."
# Count files for progress
TOTAL_FILES=$(find $BUILD_DIR -type f -name "*.mpy" | wc -l)
CURRENT=0

# Upload all compiled .mpy files
find $BUILD_DIR -type f -name "*.mpy" | while read -r file; do
    CURRENT=$((CURRENT + 1))
    # Get relative path and remove build/ prefix
    REL_PATH="${file#$BUILD_DIR/}"
    echo -n "  [$CURRENT/$TOTAL_FILES] Uploading $REL_PATH..."

    # Create directory if needed
    DIR_PATH=$(dirname "/$REL_PATH")
    if [ "$DIR_PATH" != "/" ]; then
        mp fs mkdir "$DIR_PATH" 2>/dev/null || true
    fi

    # Upload file
    mp fs cp "$file" ":/$REL_PATH"
    echo -e " ${GREEN}✓${NC}"
done

echo ""
echo "Step 6: Uploading boot.py..."
mp fs cp boot.py :boot.py
echo -e "${GREEN}✓ boot.py uploaded${NC}"

echo ""
echo "Step 7: Verifying upload..."
echo "Root files:"
mp fs ls /
echo ""
if mp fs ls /scenes > /dev/null 2>&1; then
    echo "Scenes directory:"
    mp fs ls /scenes
fi

echo ""
echo -e "${GREEN}=== Upload Complete! ===${NC}"
echo ""
echo "To run the game:"
echo "  mpremote exec 'import main; main.main()'"
echo ""
echo "To connect interactively:"
echo "  mpremote"
echo "  >>> import main"
echo "  >>> main.main()"
echo ""
echo "To reset the device:"
echo "  mpremote reset"