{
  lib,
  stdenv,
  cmake,
  extra-cmake-modules,
  pkg-config,
  qt6,
  kdePackages,
  wayland,
  libepoxy,
  libdrm,
  dbus,
  writeShellScriptBin,
}:

let
  plugin = stdenv.mkDerivation {
    pname = "ahk-wayland-activeclient";
    version = "1.0";

    # Clean source to exclude build artifacts
    src = lib.cleanSourceWith {
      src = ./.;
      filter =
        path: type:
        let
          baseName = baseNameOf path;
        in
        !(lib.elem baseName [
          "build"
          "CMakeCache.txt"
          ".cache"
          "result"
        ]);
    };

    nativeBuildInputs = [
      cmake
      extra-cmake-modules
      pkg-config
      qt6.wrapQtAppsHook
    ];

    buildInputs = [
      kdePackages.kwin
      qt6.qtbase
      qt6.qtdeclarative
      qt6.qtwayland
      kdePackages.kcoreaddons
      kdePackages.ki18n
      wayland
      libepoxy
      libdrm
      dbus
    ];

    # Always clean before building
    preConfigure = ''
      if [ -d build ]; then
        echo "Removing stale build directory..."
        rm -rf build
      fi
    '';

    cmakeFlags = [
      "-DCMAKE_INSTALL_PREFIX=${placeholder "out"}"
    ];

    meta = with lib; {
      description = "KWin effect to monitor active window changes for AHK-Wayland";
      homepage = "https://github.com/thehafenator/ahk-wayland";
      license = licenses.gpl3;
      platforms = platforms.linux;
    };
  };

  # Helper script to manage plugin state
  kwin-plugin-manager = writeShellScriptBin "ahk-kwin-setup" ''
    #!/usr/bin/env bash

    PLUGIN_NAME="ahk-wayland-activeclient"
    CONFIG_FILE="$HOME/.config/kwinrc"

    check_enabled() {
        if [ -f "$CONFIG_FILE" ]; then
            kreadconfig6 --file kwinrc --group Plugins --key "''${PLUGIN_NAME}Enabled" 2>/dev/null
        else
            echo "false"
        fi
    }

    enable_plugin() {
        echo "Enabling $PLUGIN_NAME plugin..."
        kwriteconfig6 --file kwinrc --group Plugins --key "''${PLUGIN_NAME}Enabled" true
        
        # Try to reconfigure KWin if it's running
        if qdbus org.kde.KWin /KWin &>/dev/null; then
            echo "Reloading KWin configuration..."
            qdbus org.kde.KWin /KWin reconfigure
            
            # Try to load the effect
            if qdbus org.kde.KWin /Effects org.kde.kwin.Effects.loadEffect "$PLUGIN_NAME" 2>/dev/null; then
                echo "✓ Plugin loaded successfully!"
                echo ""
                echo "The $PLUGIN_NAME plugin is now active."
            else
                echo "⚠ Plugin enabled but requires KWin restart."
                echo ""
                echo "Please log out and log back in to activate the plugin."
            fi
        else
            echo "✓ Plugin will be enabled on next login."
            echo ""
            echo "Please log out and log back in to activate the plugin."
        fi
    }

    disable_plugin() {
        echo "Disabling $PLUGIN_NAME plugin..."
        kwriteconfig6 --file kwinrc --group Plugins --key "''${PLUGIN_NAME}Enabled" false
        
        if qdbus org.kde.KWin /KWin &>/dev/null; then
            qdbus org.kde.KWin /KWin reconfigure
            echo "✓ Plugin disabled."
        fi
    }

    status_plugin() {
        local enabled=$(check_enabled)
        echo "Plugin: $PLUGIN_NAME"
        echo "Status: $enabled"
        
        if [ "$enabled" = "true" ]; then
            # Check if actually loaded
            if qdbus org.kde.KWin /Effects org.kde.kwin.Effects.loadedEffects 2>/dev/null | grep -q "$PLUGIN_NAME"; then
                echo "Runtime: Loaded and running"
            else
                echo "Runtime: Enabled but not loaded (restart KWin)"
            fi
        fi
    }

    case "''${1:-status}" in
        enable)
            enable_plugin
            ;;
        disable)
            disable_plugin
            ;;
        status)
            status_plugin
            ;;
        *)
            echo "Usage: ahk-kwin-setup {enable|disable|status}"
            echo ""
            echo "Commands:"
            echo "  enable  - Enable the KWin plugin"
            echo "  disable - Disable the KWin plugin"
            echo "  status  - Check plugin status"
            exit 1
            ;;
    esac
  '';

in
stdenv.mkDerivation {
  pname = "ahk-wayland-activeclient-wrapped";
  version = plugin.version;

  dontUnpack = true;
  dontBuild = true;

  installPhase = ''
    mkdir -p $out
    cp -r ${plugin}/* $out/
    cp -r ${kwin-plugin-manager}/bin $out/
  '';

  passthru = {
    unwrapped = plugin;
  };

  meta = plugin.meta;
}
