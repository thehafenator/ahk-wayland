use std::process::Command;
use std::path::{Path, PathBuf};
use std::env;
use std::fs;

fn main() {
    #[cfg(feature = "kde")]
    {
        println!("cargo:warning=Building KWin plugin for KDE support...");
        
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let plugin_dir = Path::new(&manifest_dir)
            .join("src")
            .join("client")
            .join("kdeactiveeventlistener");
        let build_dir = plugin_dir.join("build");
        
        if !plugin_dir.exists() {
            println!("cargo:warning=KWin plugin directory not found");
            return;
        }
        
        // Always clean and rebuild
        if build_dir.exists() {
            println!("cargo:warning=Cleaning old build...");
            let _ = fs::remove_dir_all(&build_dir);
        }
        std::fs::create_dir_all(&build_dir).ok();
        
        let home = env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
        let install_prefix = format!("{}/.local", home);
        
        // Remove old plugin
        let plugin_path = format!("{}/lib/plugins/kwin/effects/plugins/ahk-wayland-activeclient.so", install_prefix);
        if Path::new(&plugin_path).exists() {
            println!("cargo:warning=Removing old plugin...");
            let _ = fs::remove_file(&plugin_path);
        }
        
        println!("cargo:warning=Configuring CMake...");
        
        let mut cmake_cmd = Command::new("cmake");
        cmake_cmd
            .current_dir(&build_dir)
            .arg("..")
            .arg(format!("-DCMAKE_INSTALL_PREFIX={}", install_prefix))
            .arg("-DCMAKE_BUILD_TYPE=Release")
            .arg("-DQT_NO_CREATE_VERSIONLESS_TARGETS=ON");
        
        // NixOS-specific configuration
        if is_nixos() {
            println!("cargo:warning=Configuring for NixOS...");
            configure_nixos_paths(&mut cmake_cmd);
        }
        
        let output = cmake_cmd.output();
        
        match output {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("cargo:warning=✗ CMake failed:");
                    for line in stderr.lines().take(20) {
                        println!("cargo:warning=  {}", line);
                    }
                    return;
                }
                println!("cargo:warning=✓ CMake configured");
            }
            Err(e) => {
                println!("cargo:warning=✗ CMake not found: {}", e);
                return;
            }
        }
        
        // Build
        println!("cargo:warning=Building plugin...");
        let build_result = Command::new("cmake")
            .current_dir(&build_dir)
            .arg("--build")
            .arg(".")
            .status();
        
        if !build_result.map(|s| s.success()).unwrap_or(false) {
            println!("cargo:warning=✗ Build failed");
            return;
        }
        
        // Install
        println!("cargo:warning=Installing plugin...");
        let install_result = Command::new("cmake")
            .current_dir(&build_dir)
            .arg("--install")
            .arg(".")
            .status();
        
        if install_result.map(|s| s.success()).unwrap_or(false) {
            println!("cargo:warning=");
            println!("cargo:warning=╔═══════════════════════════════════════════╗");
            println!("cargo:warning=║  ✓ KWin Plugin Installed Successfully!    ║");
            println!("cargo:warning=╚═══════════════════════════════════════════╝");
            println!("cargo:warning=");
            println!("cargo:warning=Location: {}", plugin_path);
            println!("cargo:warning=");
            println!("cargo:warning=To enable the plugin, run:");
            println!("cargo:warning=  kwriteconfig6 --file kwinrc --group Plugins --key ahk-wayland-activeclientEnabled true");
            println!("cargo:warning=  qdbus org.kde.KWin /KWin reconfigure");
            println!("cargo:warning=");
            println!("cargo:warning=Or restart KWin:");
            println!("cargo:warning=  kwin_wayland --replace &");
            println!("cargo:warning=");
        } else {
            println!("cargo:warning=✗ Install failed");
        }
        
        println!("cargo:rerun-if-changed=src/client/kdeactiveeventlistener/");
    }
}

fn is_nixos() -> bool {
    Path::new("/nix/store").exists()
}

fn configure_nixos_paths(cmake_cmd: &mut Command) {
    let mut paths = Vec::new();
    let mut pkg_config_paths = Vec::new();
    let mut opengl_include = None;
    let mut opengl_lib = None;
    let mut x11_include = None;
    let mut x11_lib = None;
    let mut qt6_dir = None;
    
    println!("cargo:warning=Searching for NixOS packages...");
    
    // Qt6 - need BOTH dev (for cmake) and base (for binaries)
    if let Some(qt6) = find_nix_package_with_suffix("qtbase-6", "-dev") {
        println!("cargo:warning=Found Qt6: {}", qt6);
        qt6_dir = Some(qt6.clone());
        paths.push(qt6.clone());
        add_pkgconfig_paths(&qt6, &mut pkg_config_paths);
    }
    
    if let Some(qt6_bin) = find_nix_package_without_suffix("qtbase-6", &["-dev", "-bin"]) {
        println!("cargo:warning=Found Qt6 binaries: {}", qt6_bin);
        paths.push(qt6_bin);
    }
    
    // Qt6 declarative - need BOTH for cmake configs
    if let Some(qtdeclarative) = find_nix_package_with_suffix("qtdeclarative-6", "-dev") {
        println!("cargo:warning=Found Qt6 Declarative (dev): {}", qtdeclarative);
        paths.push(qtdeclarative.clone());
        add_pkgconfig_paths(&qtdeclarative, &mut pkg_config_paths);
    }
    
    if let Some(qtdeclarative_base) = find_nix_package_without_suffix("qtdeclarative-6", &["-dev", "-bin"]) {
        println!("cargo:warning=Found Qt6 Declarative (base): {}", qtdeclarative_base);
        paths.push(qtdeclarative_base);
    }
    
    // KDE Frameworks
    if let Some(kwin) = find_nix_package_with_suffix("kwin-6", "-dev") {
        println!("cargo:warning=Found KWin: {}", kwin);
        paths.push(kwin.clone());
        add_pkgconfig_paths(&kwin, &mut pkg_config_paths);
    }
    
    if let Some(ecm) = find_nix_package_without_suffix("extra-cmake-modules", &[]) {
        println!("cargo:warning=Found ECM: {}", ecm);
        paths.push(ecm);
    }
    
    if let Some(kcoreaddons) = find_nix_package_with_suffix("kcoreaddons-6", "-dev") {
        println!("cargo:warning=Found KCoreAddons: {}", kcoreaddons);
        paths.push(kcoreaddons.clone());
        add_pkgconfig_paths(&kcoreaddons, &mut pkg_config_paths);
    }
    
    if let Some(kconfig) = find_nix_package_with_suffix("kconfig-6", "-dev") {
        println!("cargo:warning=Found KConfig: {}", kconfig);
        paths.push(kconfig.clone());
        add_pkgconfig_paths(&kconfig, &mut pkg_config_paths);
    }
    
    if let Some(kwindowsystem) = find_nix_package_with_suffix("kwindowsystem-6", "-dev") {
        println!("cargo:warning=Found KWindowSystem: {}", kwindowsystem);
        paths.push(kwindowsystem.clone());
        add_pkgconfig_paths(&kwindowsystem, &mut pkg_config_paths);
    }
    
    // X11 libraries - need BOTH dev (headers) and base (.so files)
    if let Some(libx11_dev) = find_nix_package_with_suffix("libx11-", "-dev") {
        println!("cargo:warning=Found libX11 (dev): {}", libx11_dev);
        let x11_inc = format!("{}/include", libx11_dev);
        if Path::new(&x11_inc).exists() {
            x11_include = Some(x11_inc);
        }
        paths.push(libx11_dev.clone());
        add_pkgconfig_paths(&libx11_dev, &mut pkg_config_paths);
    }
    
    if let Some(libx11_base) = find_nix_package_without_suffix("libx11-", &["-dev", "-man"]) {
        println!("cargo:warning=Found libX11 (base): {}", libx11_base);
        let x11_l = format!("{}/lib", libx11_base);
        if Path::new(&x11_l).exists() {
            x11_lib = Some(x11_l);
        }
        paths.push(libx11_base);
    }
    
    if let Some(libxcb) = find_nix_package_with_suffix("libxcb-1", "-dev") {
        println!("cargo:warning=Found libxcb: {}", libxcb);
        paths.push(libxcb.clone());
        add_pkgconfig_paths(&libxcb, &mut pkg_config_paths);
    }
    
    if let Some(libxau) = find_nix_package_with_suffix("libxau-", "-dev") {
        println!("cargo:warning=Found libxau: {}", libxau);
        paths.push(libxau.clone());
        add_pkgconfig_paths(&libxau, &mut pkg_config_paths);
    }
    
    if let Some(libxdmcp) = find_nix_package_with_suffix("libxdmcp-", "-dev") {
        println!("cargo:warning=Found libxdmcp: {}", libxdmcp);
        paths.push(libxdmcp.clone());
        add_pkgconfig_paths(&libxdmcp, &mut pkg_config_paths);
    }
    
    if let Some(libdrm) = find_nix_package_with_suffix("libdrm-", "-dev") {
        println!("cargo:warning=Found libdrm: {}", libdrm);
        paths.push(libdrm.clone());
        add_pkgconfig_paths(&libdrm, &mut pkg_config_paths);
    }
    
    // Wayland
    if let Some(wayland) = find_nix_package_with_suffix("wayland-", "-dev") {
        println!("cargo:warning=Found Wayland: {}", wayland);
        paths.push(wayland.clone());
        add_pkgconfig_paths(&wayland, &mut pkg_config_paths);
    }
    
    if let Some(libffi) = find_nix_package_with_suffix("libffi-", "-dev") {
        println!("cargo:warning=Found libffi: {}", libffi);
        paths.push(libffi.clone());
        add_pkgconfig_paths(&libffi, &mut pkg_config_paths);
    }
    
    if let Some(scanner) = find_nix_package_with_suffix("wayland-scanner-1", "-dev") {
        println!("cargo:warning=Found wayland-scanner: {}", scanner);
        paths.push(scanner.clone());
        add_pkgconfig_paths(&scanner, &mut pkg_config_paths);
    }
    
    if let Some(protocols) = find_nix_package_without_suffix("wayland-protocols-", &[]) {
        println!("cargo:warning=Found wayland-protocols: {}", protocols);
        paths.push(protocols.clone());
        add_pkgconfig_paths(&protocols, &mut pkg_config_paths);
    }
    
    if let Some(xproto) = find_nix_package_without_suffix("xorgproto-", &[]) {
        println!("cargo:warning=Found xorgproto: {}", xproto);
        paths.push(xproto.clone());
        add_pkgconfig_paths(&xproto, &mut pkg_config_paths);
    }
    
    if let Some(epoxy) = find_nix_package_with_suffix("libepoxy-", "-dev") {
        println!("cargo:warning=Found libepoxy: {}", epoxy);
        paths.push(epoxy.clone());
        add_pkgconfig_paths(&epoxy, &mut pkg_config_paths);
    }
    
    // OpenGL
    if let Some(libglvnd) = find_nix_package_with_suffix("libglvnd-", "-dev") {
        println!("cargo:warning=Found libglvnd: {}", libglvnd);
        let gl_inc = format!("{}/include", libglvnd);
        let gl_lib = format!("{}/lib", libglvnd);
        
        if Path::new(&gl_inc).exists() {
            opengl_include = Some(gl_inc);
        }
        if Path::new(&gl_lib).exists() {
            opengl_lib = Some(gl_lib);
        }
        
        paths.push(libglvnd);
    }
    
    // Set CMAKE_PREFIX_PATH FIRST
    if !paths.is_empty() {
        let prefix_path = paths.join(";");
        cmake_cmd.arg(format!("-DCMAKE_PREFIX_PATH={}", prefix_path));
        println!("cargo:warning=CMAKE_PREFIX_PATH set with {} paths", paths.len());
    }
    
    // Explicitly set Qt6Quick_DIR
    for path in &paths {
        if path.contains("qtdeclarative") && !path.ends_with("-dev") {
            let qt6quick_cmake = format!("{}/lib/cmake/Qt6Quick", path);
            if Path::new(&qt6quick_cmake).exists() {
                cmake_cmd.arg(format!("-DQt6Quick_DIR={}", qt6quick_cmake));
                println!("cargo:warning=Set Qt6Quick_DIR: {}", qt6quick_cmake);
            }
        }
    }
    
    // Set Qt6_ROOT and Qt6_DIR
    if let Some(qt6) = qt6_dir {
        let qt6_cmake = format!("{}/lib/cmake/Qt6", qt6);
        if Path::new(&qt6_cmake).exists() {
            cmake_cmd.env("Qt6_ROOT", &qt6);
            cmake_cmd.arg(format!("-DQt6_DIR={}", qt6_cmake));
            cmake_cmd.arg(format!("-DQt6Core_DIR={}/lib/cmake/Qt6Core", qt6));
            println!("cargo:warning=Set Qt6_ROOT: {}", qt6);
            println!("cargo:warning=Set Qt6_DIR: {}", qt6_cmake);
        }
        
        // Add Qt6 bin to PATH
        let qt6_bin = format!("{}/bin", qt6);
        if Path::new(&qt6_bin).exists() {
            if let Ok(current_path) = env::var("PATH") {
                cmake_cmd.env("PATH", format!("{}:{}", qt6_bin, current_path));
            } else {
                cmake_cmd.env("PATH", &qt6_bin);
            }
            println!("cargo:warning=Added Qt6 bin to PATH: {}", qt6_bin);
        }
    }
    
    // Set PKG_CONFIG_PATH
    if !pkg_config_paths.is_empty() {
        cmake_cmd.env("PKG_CONFIG_PATH", pkg_config_paths.join(":"));
        println!("cargo:warning=Set PKG_CONFIG_PATH with {} paths", pkg_config_paths.len());
    }
    
    // Explicitly set OpenGL paths
    if let Some(inc) = opengl_include {
        cmake_cmd.arg(format!("-DOPENGL_INCLUDE_DIR={}", inc));
        println!("cargo:warning=Set OPENGL_INCLUDE_DIR: {}", inc);
    }
    
    if let Some(lib) = opengl_lib {
        cmake_cmd.arg(format!("-DOPENGL_gl_LIBRARY={}/libGL.so", lib));
        println!("cargo:warning=Set OPENGL_gl_LIBRARY: {}/libGL.so", lib);
    }
    
    // Explicitly set X11 paths
    if let Some(inc) = x11_include {
        cmake_cmd.arg(format!("-DX11_X11_INCLUDE_PATH={}", inc));
        println!("cargo:warning=Set X11_X11_INCLUDE_PATH: {}", inc);
    }
    
    if let Some(lib) = x11_lib {
        cmake_cmd.arg(format!("-DX11_X11_LIB={}/libX11.so", lib));
        println!("cargo:warning=Set X11_X11_LIB: {}/libX11.so", lib);
    }
}

/// Find Nix package WITH a specific suffix (e.g., "-dev")
fn find_nix_package_with_suffix(pattern: &str, suffix: &str) -> Option<String> {
    let nix_store = Path::new("/nix/store");
    if !nix_store.exists() {
        return None;
    }
    
    let entries = fs::read_dir(nix_store).ok()?;
    
    let mut matches: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".drv") {
                    return false;
                }
                
                if let Some(pkg_name) = name.split_once('-').map(|(_, rest)| rest) {
                    return pkg_name.starts_with(pattern) && pkg_name.ends_with(suffix);
                }
            }
            false
        })
        .collect();
    
    matches.sort();
    matches.last().and_then(|p| p.to_str().map(String::from))
}

/// Find Nix package WITHOUT specific suffixes (e.g., not "-dev", not "-man")
fn find_nix_package_without_suffix(pattern: &str, exclude_suffixes: &[&str]) -> Option<String> {
    let nix_store = Path::new("/nix/store");
    if !nix_store.exists() {
        return None;
    }
    
    let entries = fs::read_dir(nix_store).ok()?;
    
    let mut matches: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".drv") {
                    return false;
                }
                
                if let Some(pkg_name) = name.split_once('-').map(|(_, rest)| rest) {
                    if !pkg_name.starts_with(pattern) {
                        return false;
                    }
                    
                    // Exclude packages with unwanted suffixes
                    for suffix in exclude_suffixes {
                        if pkg_name.ends_with(suffix) {
                            return false;
                        }
                    }
                    
                    return true;
                }
            }
            false
        })
        .collect();
    
    matches.sort();
    matches.last().and_then(|p| p.to_str().map(String::from))
}

/// Helper function to add pkgconfig paths from a package
fn add_pkgconfig_paths(package_path: &str, pkg_config_paths: &mut Vec<String>) {
    for subpath in &["lib/pkgconfig", "share/pkgconfig", "lib64/pkgconfig"] {
        let pc_path = format!("{}/{}", package_path, subpath);
        if Path::new(&pc_path).exists() {
            pkg_config_paths.push(pc_path);
        }
    }
}