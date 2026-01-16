{
  description = "ahk-wayland - AutoHotkey for Wayland";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        
        # KWin plugin
        kwin-plugin = pkgs.stdenv.mkDerivation {
          pname = "ahk-wayland-kwin-plugin";
          version = "1.0.0";
          
          src = ./src/client/kdeactiveeventlistener;
          
          nativeBuildInputs = with pkgs; [
            cmake
            extra-cmake-modules
          ];
          
          buildInputs = with pkgs.kdePackages; [
            qtbase
            qtdeclarative
            kwindowsystem
            kcoreaddons
            kconfig
            kwin
          ];
          
          cmakeFlags = [
            "-DCMAKE_BUILD_TYPE=Release"
          ];
          
          meta = with pkgs.lib; {
            description = "KWin plugin for ahk-wayland window detection";
            license = licenses.gpl3;
            platforms = platforms.linux;
          };
        };
        
        # Main Rust application
        ahk-wayland = pkgs.rustPlatform.buildRustPackage {
          pname = "ahk-wayland";
          version = "0.14.5";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = with pkgs; [
            pkg-config
            cmake
            extra-cmake-modules
          ];
          
          buildInputs = with pkgs; [
            dbus
            xdotool
            ydotool
          ] ++ (with pkgs.kdePackages; [
            qtbase
            kwin
            kcoreaddons
          ]);
          
          buildFeatures = [ "kde" ];
          
          # Ensure KWin plugin builds during main build
          preBuild = ''
            export HOME=$TMPDIR
          '';
          
          meta = with pkgs.lib; {
            description = "AutoHotkey for Wayland";
            homepage = "https://github.com/phil294/ahk-wayland";
            license = licenses.gpl3;
            platforms = platforms.linux;
          };
        };
        
      in
      {
        packages = {
          default = ahk-wayland;
          kwin-plugin = kwin-plugin;
          ahk-wayland = ahk-wayland;
        };
        
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            pkg-config
            dbus
            cmake
            extra-cmake-modules
          ] ++ (with pkgs.kdePackages; [
            qtbase
            qtdeclarative
            kwin
            kcoreaddons
            kconfig
            kwindowsystem
          ]);
          
          shellHook = ''
            echo "ahk-wayland development environment"
            echo "Build with: cargo build --features kde"
            echo "Install KWin plugin: cd src/client/kdeactiveeventlistener/build && cmake --install ."
          '';
        };
        
        # NixOS module for easy system integration
        nixosModules.default = { config, lib, pkgs, ... }:
          with lib;
          let
            cfg = config.services.ahk-wayland;
          in {
            options.services.ahk-wayland = {
              enable = mkEnableOption "ahk-wayland AutoHotkey service";
              
              package = mkOption {
                type = types.package;
                default = self.packages.${system}.default;
                description = "The ahk-wayland package to use";
              };
              
              kdeSupport = mkOption {
                type = types.bool;
                default = true;
                description = "Install KWin plugin for KDE Plasma support";
              };
            };
            
            config = mkIf cfg.enable {
              environment.systemPackages = [ cfg.package ]
                ++ (if cfg.kdeSupport then [ self.packages.${system}.kwin-plugin ] else []);
            };
          };
      }
    );
}