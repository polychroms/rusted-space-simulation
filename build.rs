use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let shaders = [
        "clear_grid.comp",
        "build_grid_eos.comp",
        "continuum_forces.comp",
        "integrate_plasticity.comp",
        "grav_clear.comp",
        "grav_bin.comp",
        "grav_force.comp",
    ];

    for shader in shaders {
        let src = PathBuf::from("src/shaders").join(shader);
        println!("cargo:rerun-if-changed={}", src.display());
        let spv_out = out_dir.join(format!("{}.spv", shader));

        let status = Command::new("glslc")
            .arg("--target-env=vulkan1.2")
            .arg("-O")
            .arg(&src)
            .arg("-o")
            .arg(&spv_out)
            .status()
            .unwrap_or_else(|e| panic!("Failed to execute glslc on {}: {}", src.display(), e));

        if !status.success() {
            panic!("glslc compilation failed for {}", src.display());
        }
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon(&out_dir);
    }
}

fn embed_windows_icon(out_dir: &std::path::Path) {
    let png_path = std::path::Path::new("packaging/rssim.png");
    println!("cargo:rerun-if-changed={}", png_path.display());
    let Ok(png_bytes) = std::fs::read(png_path) else {
        return;
    };

    let ico_path = out_dir.join("rssim.ico");
    let mut ico = Vec::with_capacity(22 + png_bytes.len());
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]); // ICONDIR: type=1 (ICO), count=1
    ico.extend_from_slice(&[0, 0, 0, 0]); // 256x256, 0 palette colors
    ico.extend_from_slice(&1u16.to_le_bytes()); // color planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // 32 bpp RGBA
    ico.extend_from_slice(&(png_bytes.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes()); // offset right after 22-byte header
    ico.extend_from_slice(&png_bytes);
    std::fs::write(&ico_path, ico).expect("write rssim.ico");

    let rc_path = out_dir.join("rssim.rc");
    let res_path = out_dir.join("rssim.res");
    std::fs::write(&rc_path, "1 ICON \"rssim.ico\"\n").expect("write rssim.rc");

    let ok = Command::new("llvm-rc")
        .arg("/FO")
        .arg(&res_path)
        .arg(&rc_path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bin=rssim={}", res_path.display());
    }
}
