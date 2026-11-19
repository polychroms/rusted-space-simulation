# Rusted Space Simulation (`rssim`)

Vulkan 1.2 GPU and Rayon CPU 2D continuum shock, armor penetration, reactive detonation, and self-gravitating planetary collision simulator written in Rust.

## Key Capabilities (`v0.0.7`)

- **32 Calibrated Continuum Materials**: Metals (`Tungsten`, `RHA`, `HHA`, `Mild Steel`, `Depleted Uranium`, `Copper`, `Gold`, `Aluminum`, `Titanium`, `Magnesium`), Ceramics/Minerals (`Boron Carbide`, `Silicon Carbide`, `Tungsten Carbide`, `Armor Glass`, `Concrete`, `Granite`, `Basalt Rock`, `Planetary Fe-Ni`, `Water Ice`, `Liquid Water`), Polymers/NERA (`Aramid`, `UHMWPE`, `Rubber`, `Textolite`), and Reactive Energetics (`Comp-B`, `Octol`, `C4`, `Cast TNT`, condensed exothermic `Thermite`, `Piezo Impact Fuze`, `Base Delay Fuze`).
- **GPU-Accelerated Continuum Mechanics**: Vulkan 1.2 SPIR-V compute pipeline (`build_grid_eos`, `continuum_forces`, `integrate_plasticity`) with 131K spatial hash, Mie-Gruneisen / Tillotson vapor / JWL detonation EOS, Fourier thermal conduction, Stefan-Boltzmann radiative cooling, molten capillary surface tension, Johnson-Cook & Drucker-Prager plasticity, and 32x32 LDS N-body self-gravity.
- **Parallel Pixel-Art Rasterizer**: Single-pass Rayon staging, `256x256` FBM explosion plume LUT, camera-parallax deep-space starfield, Planck blackbody glow, and rounded molten droplets with zero-idle CPU usage on pause.
- **Live Interactive Physics Tools (`Q / B / E / L / G / X`)**: Non-overlapping honeycomb Matter Brush, Void Eraser (with Witness Gauge index remapping), front-surface Thermal Ablation Laser, softened 2D Gravity Vortex / Repulsor, and instant Shock Blast, plus Camera Tracking (`Free`, `Fastest Core`, `System CoM`), `Auto Bullet-Time`, and `Impact Camera Shake`.
- **Studio Workspace & 17 Built-In Scenarios**: Floating top-center viewport quick-tool dock, categorized scenario library (`HEAT`, `APFSDS`, `Tandem HEAT`, `EFP`, `APHE`, `HESH`, `Chobham NERA`, `Bunker-Buster`, `Roche-Limit Ring Disruption`, `Binary Proto-Planet Merger`, `Europa Ocean-Moon Hyper-Impact`, `Planetary Impact`, `Icy Comet`, `Whipple Shield`, `Sub-Hydrodynamic Plastic Dent`, `Empty Space`), curved N-body orbital trajectory preview with one-click `Orbit + / Orbit - / Flyby` velocity assists, Blueprint CAD (`.rsim`), and `.rscene` persistence.
- **Laboratory Analysis & Exports**: Line-of-Sight Caliper RHAe analyzer (`M`), Lagrangian Witness Gauges & oscilloscope (`Alt+Click`), Connected-Component Remnant Cluster & Escape Analyzer (`Analyze Remnants`), lossless PNG (`F12`), telemetry CSV, and native pure-Rust animated GIF89a capture (`Record GIF` / `Replay GIF`).

## Quick Install (Linux x86_64)

Download and run the installer from release `v0.0.7` to fetch release files from GitHub into your user `~/bin` directory (and desktop application menu):

```bash
curl -fsSL https://github.com/rappar222/rusted-space-simulation/releases/download/v0.0.7/install.sh | bash
```

Or run `Rusted_Space_Simulation-0.0.7-x86_64.AppImage` directly from the [GitHub Releases](https://github.com/rappar222/rusted-space-simulation/releases) page:

```bash
chmod +x Rusted_Space_Simulation-0.0.7-x86_64.AppImage
./Rusted_Space_Simulation-0.0.7-x86_64.AppImage
```

Installer options:
- `INSTALL_DIR=/custom/bin ./install.sh` or `./install.sh --dir /custom/bin` installs to a custom bin directory.
- `./install.sh --from-source` downloads the `v0.0.7` source archive from GitHub and compiles with `cargo build --release`.

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
