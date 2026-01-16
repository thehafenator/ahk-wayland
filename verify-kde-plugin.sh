#!/bin/bash
# Verify KDE plugin installation and functionality

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  AHK-Wayland KDE Plugin Verification                      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check 1: Plugin file exists
echo -n "Checking plugin file... "
PLUGIN_PATH="$HOME/.local/lib/plugins/kwin/effects/plugins/ahk-wayland-activeclient.so"
if [ -f "$PLUGIN_PATH" ]; then
    echo -e "${GREEN}✓ Found${NC}"
    ls -lh "$PLUGIN_PATH"
else
    echo -e "${RED}✗ Not found${NC}"
    echo "Expected location: $PLUGIN_PATH"
    echo ""
    echo "To install, run:"
    echo "  cargo build --release --features kde"
    exit 1
fi
echo ""

# Check 2: KWin is running
echo -n "Checking KWin... "
if pgrep -x "kwin_wayland" > /dev/null; then
    echo -e "${GREEN}✓ kwin_wayland running${NC}"
    KWIN_CMD="kwin_wayland"
elif pgrep -x "kwin_x11" > /dev/null; then
    echo -e "${GREEN}✓ kwin_x11 running${NC}"
    KWIN_CMD="kwin_x11"
else
    echo -e "${RED}✗ Not running${NC}"
    echo "KWin is not running. Are you on KDE Plasma?"
    exit 1
fi
echo ""

# Check 3: D-Bus session bus
echo -n "Checking D-Bus session... "
if [ -n "$DBUS_SESSION_BUS_ADDRESS" ]; then
    echo -e "${GREEN}✓ Active${NC}"
    echo "  Address: $DBUS_SESSION_BUS_ADDRESS"
else
    echo -e "${RED}✗ Not found${NC}"
    echo "D-Bus session bus not available"
    exit 1
fi
echo ""

# Check 4: Listen for D-Bus signals (10 second test)
echo "Testing D-Bus signals (10 seconds)..."
echo -e "${YELLOW}Please switch between windows now!${NC}"
echo ""

SIGNAL_COUNT=0
timeout 10 dbus-monitor "type='signal',interface='org.ahkwayland.ActiveWindow'" 2>/dev/null | while read -r line; do
    if [[ "$line" == *"member="* ]]; then
        SIGNAL_COUNT=$((SIGNAL_COUNT + 1))
        echo -e "${GREEN}✓ Signal received:${NC} $line"
    elif [[ "$line" == *"string"* ]]; then
        echo "  $line"
    fi
done

echo ""

# Check 5: Suggest restart if needed
if [ $SIGNAL_COUNT -eq 0 ]; then
    echo -e "${YELLOW}⚠ No signals detected${NC}"
    echo ""
    echo "The plugin may need to be activated. Try restarting KWin:"
    echo "  $KWIN_CMD --replace &"
    echo ""
    echo "If that doesn't work, check KWin logs:"
    echo "  journalctl -f | grep kwin"
else
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ✓ KDE Plugin is working correctly!                       ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
fi

echo ""
