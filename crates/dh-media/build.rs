fn main() {
    println!("cargo:rerun-if-changed=native/vpx_bridge.c");
    for name in ["VPX_INCLUDE_DIR", "VPX_LIB_DIR", "VPX_LIB_NAME"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if std::env::var_os("CARGO_FEATURE_VP8").is_none() {
        return;
    }
    let include = std::env::var_os("VPX_INCLUDE_DIR")
        .expect("VPX_INCLUDE_DIR must name the pinned libvpx header directory");
    let library = std::env::var_os("VPX_LIB_DIR")
        .expect("VPX_LIB_DIR must name the pinned libvpx library directory");
    let name = std::env::var("VPX_LIB_NAME").unwrap_or_else(|_| "vpx".into());
    assert!(
        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "invalid VPX library name"
    );
    cc::Build::new()
        .file("native/vpx_bridge.c")
        .include(include)
        .warnings(true)
        .compile("dh_vpx_bridge");
    println!(
        "cargo:rustc-link-search=native={}",
        std::path::Path::new(&library).display()
    );
    println!("cargo:rustc-link-lib=static={name}");
    let filename = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        format!("{name}.lib")
    } else {
        format!("lib{name}.a")
    };
    println!(
        "cargo:rerun-if-changed={}",
        std::path::Path::new(&library).join(filename).display()
    );
    if matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("linux" | "android")
    ) {
        println!("cargo:rustc-link-lib=m");
    }
}
