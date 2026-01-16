{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    cmake
    kdePackages.extra-cmake-modules
    pkg-config
    rustc
    cargo
  ];

  buildInputs = with pkgs; [
    dbus.dev
    kdePackages.kwin
    kdePackages.kcoreaddons
    kdePackages.ki18n
    kdePackages.kwindowsystem
    qt6.qtbase
    qt6.qtdeclarative
    qt6.wrapQtAppsHook
    wayland
    libepoxy
    libdrm
  ];
}
