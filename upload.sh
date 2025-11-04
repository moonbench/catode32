#!/bin/bash
# upload.sh - Quick upload script for virtual pet project

echo "=== Virtual Pet Upload Script ==="
echo ""

# Check if mpremote is available
if ! command -v mpremote &> /dev/null; then
    echo "Error: mpremote not found. Install with: pip install mpremote"
    exit 1
fi

# echo "Step 0: Resetting device..."
# mpremote reset
# sleep 2

echo ""
echo "Step 1: Installing SSD1306 library..."
mpremote mip install ssd1306

echo ""
echo "Step 2: Creating /virtual_pet directory..."
mpremote fs mkdir /virtual_pet 2>/dev/null || true

echo ""
echo "Step 3: Uploading files..."
mpremote fs cp config.py :/virtual_pet/
echo "  ✓ config.py uploaded"

mpremote fs cp input.py :/virtual_pet/
echo "  ✓ input.py uploaded"

mpremote fs cp character.py :/virtual_pet/
echo "  ✓ character.py uploaded"

mpremote fs cp renderer.py :/virtual_pet/
echo "  ✓ renderer.py uploaded"

mpremote fs cp main.py :/virtual_pet/
echo "  ✓ main.py uploaded"

# echo ""
# echo "Step 4: Uploading auto-start boot.py..."
# mpremote fs cp boot.py :/
# echo "  ✓ boot.py uploaded (game will auto-start on boot!)"

echo ""
echo "Step 5: Verifying upload..."
mpremote fs ls /virtual_pet

echo ""
echo "=== Upload Complete! ==="
echo ""
echo "To run the game:"
echo "  mpremote exec \"import sys; sys.path.append('/virtual_pet'); exec(open('/virtual_pet/main.py').read())\""
echo ""
echo "Or connect interactively:"
echo "  mpremote"
echo "  >>> import sys"
echo "  >>> sys.path.append('/virtual_pet')"
echo "  >>> exec(open('/virtual_pet/main.py').read())"
echo ""