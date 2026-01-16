#!/usr/bin/env bash
set -e

echo "======================================"
echo "AHK-Wayland KWin Plugin Installer"
echo "======================================"
echo

# 1. Uninstall existing plugin
echo "Step 1: Removing existing plugin..."
PLUGIN_DIR="$HOME/.local/lib/plugins/kwin/effects"
PLUGIN_PLUGINS_DIR="$HOME/.local/lib/plugins/kwin/plugins"

if [ -f "$PLUGIN_DIR/ahk-wayland-activeclient.so" ]; then
    echo "  Removing $PLUGIN_DIR/ahk-wayland-activeclient.so"
    rm -f "$PLUGIN_DIR/ahk-wayland-activeclient.so"
fi

if [ -f "$PLUGIN_DIR/ahk-wayland-activeclient.json" ]; then
    echo "  Removing $PLUGIN_DIR/ahk-wayland-activeclient.json"
    rm -f "$PLUGIN_DIR/ahk-wayland-activeclient.json"
fi

if [ -f "$PLUGIN_PLUGINS_DIR/ahk-wayland-activeclient.so" ]; then
    echo "  Removing $PLUGIN_PLUGINS_DIR/ahk-wayland-activeclient.so"
    rm -f "$PLUGIN_PLUGINS_DIR/ahk-wayland-activeclient.so"
fi

# Also disable in config
echo "  Disabling plugin in KWin config..."
kwriteconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled false 2>/dev/null || true

echo "  ✓ Cleanup complete"
echo

# 2. Build and install
echo "Step 2: Building KWin plugin..."
cd "$(dirname "$0")"
cargo build --release --features kde

if [ $? -ne 0 ]; then
    echo "  ✗ Build failed!"
    exit 1
fi

echo "  ✓ Build complete"
echo

# 3. Verify installation
echo "Step 3: Verifying installation..."
INSTALLED_PLUGIN="$HOME/.local/lib/plugins/kwin/effects/plugins/ahk-wayland-activeclient.so"

if [ -f "$INSTALLED_PLUGIN" ]; then
    echo "  ✓ Plugin installed at: $INSTALLED_PLUGIN"
else
    echo "  ✗ Plugin not found at expected location!"
    exit 1
fi
echo

# 4. Enable plugin
echo "Step 4: Enabling plugin..."
kwriteconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled true
echo "  ✓ Plugin enabled in config"
echo

# 5. Reload KWin
echo "Step 5: Reloading KWin configuration..."
qdbus org.kde.KWin /KWin reconfigure 2>/dev/null || {
    echo "  Note: Could not reload via DBus. You may need to restart KWin manually:"
    echo "    kwin_wayland --replace &"
}
echo "  ✓ Configuration reloaded"
echo

echo "======================================"
echo "✓ Installation Complete!"
echo "======================================"
echo
echo "The KWin plugin is now installed and enabled."
echo
echo "To test if it's working, run:"
echo "  dbus-monitor --session \"interface='org.ahkwayland.ActiveWindow'\""
echo
echo "Then switch between windows to see DBus signals."
echo