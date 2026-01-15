{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  # Tools and hooks go here
  nativeBuildInputs = with pkgs; [
    cmake
    extra-cmake-modules
    pkg-config
    gcc
    gnumake
    qt6.wrapQtAppsHook  # <--- MOVED HERE (Correct location)
    dbus
  ];

  # Libraries go here
  buildInputs = with pkgs; [
    kdePackages.kwin
    qt6.qtbase
    qt6.qtdeclarative
    kdePackages.kcoreaddons
    kdePackages.ki18n
    wayland
    libepoxy
    libdrm
    kdePackages.kwindowsystem
  ];

  shellHook = ''
    echo "KWin effect dev shell loaded"
    echo "Run: rm -rf build && mkdir build && cd build"
    echo "  cmake .. -DCMAKE_INSTALL_PREFIX=~/.local"
    echo "  make -j$(nproc)"
    echo "  make install"
  '';
}