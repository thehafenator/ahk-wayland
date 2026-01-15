cmake_minimum_required(VERSION 3.20)
project(ahk-wayland-activeclient)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(QT_MAJOR_VERSION 6)
set(KF_MAJOR_VERSION 6)

find_package(ECM ${KF_MAJOR_VERSION}.0 REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH})

include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDECompilerSettings NO_POLICY_SCOPE)

find_package(Qt${QT_MAJOR_VERSION} REQUIRED COMPONENTS Core DBus Widgets)
find_package(KF${KF_MAJOR_VERSION} REQUIRED COMPONENTS CoreAddons)
find_package(KWin REQUIRED)

# Get KWin package prefix
get_filename_component(KWIN_PREFIX "${KWin_DIR}/../../.." ABSOLUTE)
message(STATUS "KWin prefix: ${KWIN_PREFIX}")

# The Effect - INSTALL TO effects/plugins NOT effects
kcoreaddons_add_plugin(ahk-wayland-activeclient
    SOURCES
        main.cpp
        activeclienteffect.cpp
    INSTALL_NAMESPACE "kwin/effects/plugins"
)

target_link_libraries(ahk-wayland-activeclient
    PRIVATE
        Qt${QT_MAJOR_VERSION}::Core
        Qt${QT_MAJOR_VERSION}::DBus
        Qt${QT_MAJOR_VERSION}::Widgets
        KF${KF_MAJOR_VERSION}::CoreAddons
        KWin::kwin
)

# Manually add KWin include directory
target_include_directories(ahk-wayland-activeclient
    PRIVATE
        ${KWIN_PREFIX}/include
)
