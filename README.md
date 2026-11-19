# Rusted Space Simulation (`rssim`)

Vulkan 1.2 GPU and Rayon CPU 2D continuum shock, armor penetration, reactive detonation, and self-gravitating planetary collision simulator written in Rust.

## Key Capabilities

- **GPU-Accelerated Continuum Mechanics**: 7-stage Vulkan compute pipeline (spatial hash grid, Mie-Gruneisen / Tillotson cold+vapor & JWL detonation EOS, Acoustic-Riemann solid contact with compact sub-visual hitboxes, Johnson-Cook & Drucker-Prager viscoplastic radial return, tensile spall / ductile damage, and 32x32 PM self-gravity).
- **CPU/GPU Parity**: Matching Rayon CPU solver (`ContinuumWorld`) and GPU solver (`VulkanPhysicsEngine`).
- **Differentiated Planets & Wide Velocity Range**: Hydrostatic self-gravity pre-equilibration (`equilibrate_self_gravity`) keeps idle worlds stable while projectiles and impactors launch from `0.05` up to `100 km/s`.
- **Interactive Scenario & Warhead Blueprint CAD**: Built-in presets (`HEAT`, `EFP`, `APHE`, `APFSDS`, `HESH`, `ERA`, `Planetary Impact`, `Icy Comet`, `Empty Space`), zoom-invariant pixelated crumb rendering, timeline scrubbing, caliper RHAe armor analyzer, and CSV/PNG exports.

## Quick Install (Linux x86_64)

Download and run the installer from release `v0.0.3` to fetch all release files from GitHub into your user `~/bin` directory (and desktop application menu):

```bash
curl -fsSL https://github.com/rappar222/rusted-space-simulation/releases/download/v0.0.3/install.sh | bash
```

Or run `Rusted_Space_Simulation-0.0.3-x86_64.AppImage` directly from the [GitHub Releases](https://github.com/rappar222/rusted-space-simulation/releases) page:

```bash
chmod +x Rusted_Space_Simulation-0.0.3-x86_64.AppImage
./Rusted_Space_Simulation-0.0.3-x86_64.AppImage
```

Installer options:
- `INSTALL_DIR=/custom/bin ./install.sh` or `./install.sh --dir /custom/bin` installs to a custom bin directory.
- `./install.sh --from-source` downloads the `v0.0.3` source archive from GitHub and compiles with `cargo build --release`.

## Building from Source

Dependencies:
- Rust toolchain (`cargo`, `rustc`)
- `glslc` (from `shaderc` / Vulkan SDK) for `build.rs` SPIR-V compilation
- Vulkan runtime loader and GPU Vulkan 1.2 driver

```bash
cargo build --release
cargo test --release
./target/release/rssim --bench
```
