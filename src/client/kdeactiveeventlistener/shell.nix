{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    cmake
    extra-cmake-modules
    pkg-config
    gcc
    gnumake
    qt6.wrapQtAppsHook
    dbus
  ];

  buildInputs = with pkgs; [
    kdePackages.kwin
    qt6.qtbase
    qt6.qtdeclarative
    qt6.qtwayland
    kdePackages.kcoreaddons
    kdePackages.ki18n
    wayland
    libepoxy
    libdrm
    kdePackages.kwindowsystem
  ];

  shellHook = ''
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  KWin Active Client Effect - Development Shell"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "📦 Quick commands:"
    echo "  clean         - Remove build directory"
    echo "  build         - Clean, configure, and build"
    echo "  install       - Install to ~/.local (preserves enabled state)"
    echo "  force-install - Install and force-enable plugin"
    echo "  test          - Test the plugin"
    echo "  status        - Check plugin status"
    echo ""

    # Helper functions
    clean() {
      echo "🧹 Cleaning build artifacts..."
      rm -rf build
      echo "✓ Done"
    }

    build() {
      clean
      echo "🔧 Configuring..."
      mkdir -p build && cd build
      cmake .. -DCMAKE_INSTALL_PREFIX=~/.local
      echo "🔨 Building..."
      make -j$(nproc)
      cd ..
      echo "✓ Build complete"
    }

    install() {
      local plugin_path="$HOME/.local/lib/qt-6/plugins/kwin/effects/plugins/ahk-wayland-activeclient.so"
      
      if [ -f "$plugin_path" ]; then
        echo "✓ Plugin already installed in ~/.local - skipping build/install"
        
        # Check if plugin is currently enabled
        local was_enabled=$(kreadconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled 2>/dev/null || echo "false")
        
        if [ "$was_enabled" = "true" ]; then
          echo "🔄 Reloading KWin..."
          qdbus org.kde.KWin /KWin reconfigure 2>/dev/null || echo "⚠ KWin not running"
        else
          echo "Plugin disabled - run 'force-install' to enable"
        fi
        return 0
      fi
      
      if [ ! -d "build" ]; then
        echo "❌ No build directory found. Run 'build' first."
        return 1
      fi
      
      # Check if plugin is currently enabled
      local was_enabled=$(kreadconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled 2>/dev/null || echo "false")
      
      echo "📦 Installing to ~/.local..."
      cd build && make install && cd ..
      
      # Preserve enabled state
      if [ "$was_enabled" = "true" ]; then
        echo "✓ Installed (plugin remains enabled)"
        echo "🔄 Reloading KWin..."
        qdbus org.kde.KWin /KWin reconfigure 2>/dev/null || echo "⚠ KWin not running"
      else
        echo "✓ Installed (plugin disabled - run 'force-install' to enable)"
      fi
    }

      force-install() {
      local plugin_path="$HOME/.local/lib/qt-6/plugins/kwin/effects/plugins/ahk-wayland-activeclient.so"
      
      if [ -f "$plugin_path" ]; then
        echo "✓ Plugin already installed in ~/.local - skipping build/install"
      else
        if [ ! -d "build" ]; then
          build
        fi
        
        echo "📦 Force installing to ~/.local..."
        cd build && make install && cd ..
      fi
      
      echo "🔧 Enabling plugin..."
      kwriteconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled true
      
      if qdbus org.kde.KWin /KWin &>/dev/null; then
        echo "🔄 Reloading KWin..."
        qdbus org.kde.KWin /KWin reconfigure
        qdbus org.kde.KWin /Effects org.kde.kwin.Effects.loadEffect ahk-wayland-activeclient 2>/dev/null && \
          echo "✓ Plugin loaded!" || \
          echo "⚠ Plugin enabled - log out/in to activate"
      else
        echo "✓ Plugin enabled - log out/in to activate"
      fi
    }

    status() {
      echo "🔍 Plugin Status:"
      local enabled=$(kreadconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled 2>/dev/null || echo "false")
      echo "  Enabled in config: $enabled"
      
      if qdbus org.kde.KWin /Effects org.kde.kwin.Effects.loadedEffects 2>/dev/null | grep -q "ahk-wayland-activeclient"; then
        echo "  Runtime status: ✓ Loaded and running"
      else
        echo "  Runtime status: ✗ Not loaded"
      fi
      
      if [ -f ~/.local/lib/qt-6/plugins/kwin/effects/plugins/ahk-wayland-activeclient.so ]; then
        echo "  Local install: ✓ Found in ~/.local"
      fi
    }

    test() {
      echo "🧪 Testing plugin..."
      if qdbus org.kde.KWin /Effects org.kde.kwin.Effects.loadedEffects 2>/dev/null | grep -q "ahk-wayland-activeclient"; then
        echo "✓ Plugin is loaded"
        echo "Testing DBus signal..."
        dbus-monitor "type='signal',interface='org.ahkwayland.ActiveWindow'" &
        local MONITOR_PID=$!
        sleep 1
        echo "Switch windows to test (Ctrl+C to stop monitoring)"
        wait $MONITOR_PID 2>/dev/null || true
      else
        echo "❌ Plugin not loaded. Run 'force-install' first."
      fi
    }

    export -f clean build install force-install status test
  '';
}
