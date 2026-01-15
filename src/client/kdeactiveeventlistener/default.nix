{ pkgs ? import <nixpkgs> {} }:

pkgs.stdenv.mkDerivation {
  pname = "ahk-wayland-activeclient";
  version = "1.0";

  src = ./.;

  nativeBuildInputs = with pkgs; [
    cmake
    extra-cmake-modules
    pkg-config
    qt6.wrapQtAppsHook
  ];

  buildInputs = with pkgs; [
    kdePackages.kwin
    qt6.qtbase
    qt6.qtdeclarative
    qt6.qtwayland        # Added
    kdePackages.kcoreaddons
    kdePackages.ki18n
    wayland
    libepoxy
    libdrm
    dbus
  ];

  cmakeFlags = [
    "-DCMAKE_INSTALL_PREFIX=${placeholder "out"}"
  ];
}