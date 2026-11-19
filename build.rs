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
}
