# Rusted Space Simulation (`rssim`)

Vulkan 1.2 GPU and Rayon CPU 2D continuum shock, armor penetration, reactive detonation, and self-gravitating planetary collision simulator written in Rust.

## Key Capabilities (`v0.10.0`)

- **Shaped-Charge Penetration & Explosion Physics (`v0.10.0`)**: Fixed HEAT copper cone liner thickness (`paint_wedge` + `92x67` high-confinement `HeatWarhead`) and added hypervelocity adiabatic thermal-shear plugging (`hydro_shear` across CPU + Vulkan GPU) for deep RHA armor cratering, paired with bilinear additive HDR fireball plasma over dark turbulent smoke and bright spark streaks without camera shake or ring overlays.
- **Stability, Multi-Density & Studio Ergonomics (`v0.10.0`)**: Hardened adaptive Verlet speed-ramp stability, fixed `19:1` multi-density projectile internal bond collapse, added `Command-Detonate (D)`, `Pinned Particle Inspector (I)`, `155mm Octol HE-Frag Shell` (`OctolFragDemolition`), `GravitySlingshotOrbits`, speed preset pills, `Y=0` centering, Gurney estimator, and Wayland `.with_app_id("rssim")` icon binding.
- **Attractor & Repulsor Gravity Materials**: Dedicated `Attractor` (`+3.5×` cyan core) and `Repulsor` (`-3.5×` violet anti-gravity field) continuum materials with momentum-conserving signed 32×32 gravity grids (CPU + Vulkan GPU + predictor parity), automatic gravity activation on placement/painting, and same-sign internal cohesion so Repulsor bodies never self-explode.
- **Exact Swept Orbital & Ballistic Impact Predictor**: Multi-node 2D extended-body mass quadrature with dynamic spatial softening parity (`ContinuumWorld` 32×32 gravity match), adaptive symplectic Velocity-Verlet integration, exact shape-vs-shape swept collision (`Circle`, `Annulus` cavity/wall, oriented `Rect`, `Ellipse`, `Polygon` incl. `LongRodDart` fins & `TriangleWedge`, and `Blueprint` cell grids) with `0` tunneling up to `100 km/s`, surface-accurate contact markers (`impact`), and translucent ghost silhouettes at impact contact.
- **Modern Obsidian Glass Studio UI**: Dedicated [`ui_theme.rs`](src/ui_theme.rs) design system with obsidian canvas hierarchy, frosted-glass floating dock & HUD cards, segmented top workspace rail & status strip, interactive Empty-Space Welcome launcher, categorized 2-column Scenario Browser, keycap shortcut reference, and tofu-free proportional icon coverage.
- **34 Calibrated Continuum Materials**: Metals (`Tungsten`, `RHA`, `HHA`, `Mild Steel`, `Depleted Uranium`, `Copper`, `Gold`, `Aluminum`, `Titanium`, `Magnesium`), Ceramics/Minerals (`Boron Carbide`, `Silicon Carbide`, `Tungsten Carbide`, `Armor Glass`, `Concrete`, `Granite`, `Basalt Rock`, `Planetary Fe-Ni`, `Water Ice`, `Liquid Water`), Polymers/NERA (`Aramid`, `UHMWPE`, `Rubber`, `Textolite`), Reactive Energetics (`Comp-B`, `Octol`, `C4`, `Cast TNT`, condensed exothermic `Thermite`, `Piezo Impact Fuze`, `Base Delay Fuze`), and Gravity & Space (`Attractor`, `Repulsor`).
- **GPU-Accelerated Continuum Mechanics**: Vulkan 1.2 SPIR-V compute pipeline (`build_grid_eos`, `continuum_forces`, `integrate_plasticity`) with 131K spatial hash, Mie-Gruneisen / Tillotson vapor / JWL detonation EOS, Fourier thermal conduction, Stefan-Boltzmann radiative cooling, molten capillary surface tension, Johnson-Cook & Drucker-Prager plasticity, and 32x32 LDS N-body self-gravity.
- **Parallel Pixel-Art Rasterizer**: Single-pass Rayon staging, bilinear `256x256` FBM fireball/smoke LUT, additive HDR plasma pass, camera-parallax deep-space starfield, Planck blackbody glow, and rounded molten droplets with zero-idle CPU usage on pause.
- **Live Interactive Physics Tools (`Q / B / E / L / G / X / D`)**: Non-overlapping honeycomb Matter Brush, Void Eraser (with Witness Gauge index remapping), front-surface Thermal Ablation Laser, softened 2D Gravity Vortex / Repulsor, instant Shock Blast, and Command-Detonate (`D`), plus Camera Tracking (`Free`, `Fastest Core`, `System CoM`) and `Auto Bullet-Time`.
- **Studio Workspace & 19 Built-In Scenarios**: Floating top-center viewport quick-tool dock, categorized scenario library (`HEAT`, `APFSDS`, `Tandem HEAT`, `EFP`, `APHE`, `HESH`, `Chobham NERA`, `Bunker-Buster`, `Roche-Limit Ring Disruption`, `Binary Proto-Planet Merger`, `Europa Ocean-Moon Hyper-Impact`, `Planetary Impact`, `Icy Comet`, `Whipple Shield`, `Sub-Hydrodynamic Plastic Dent`, `Octol HE-Frag Shell`, `Attractor-Repulsor Orbital Slingshot`, `Empty Space`), curved N-body orbital trajectory preview with one-click `Orbit + / Orbit - / Flyby` velocity assists, Blueprint CAD (`.rsim`), and `.rscene` persistence.
- **Laboratory Analysis & Exports**: Line-of-Sight Caliper RHAe analyzer (`M`), Hover & Pinned Continuum Particle Probe (`I`), Lagrangian Witness Gauges & oscilloscope (`Alt+Click`), Connected-Component Remnant Cluster & Escape Analyzer (`Analyze Remnants`), lossless PNG (`F12`), telemetry CSV, and native pure-Rust animated GIF89a capture (`Record GIF` / `Replay GIF`).

## Quick Install (Linux x86_64)

Download and run the installer from release `v0.10.0` to fetch release files from GitHub into your user `~/bin` directory (and desktop application menu):

```bash
curl -fsSL https://github.com/polychroms/rusted-space-simulation/releases/download/v0.10.0/install.sh | bash
```

Or run `Rusted_Space_Simulation-0.10.0-x86_64.AppImage` directly from [GitHub Releases](https://github.com/polychroms/rusted-space-simulation/releases) or [GitLab Releases](https://gitlab.com/polychroms/rssim/-/releases):

```bash
chmod +x Rusted_Space_Simulation-0.10.0-x86_64.AppImage
./Rusted_Space_Simulation-0.10.0-x86_64.AppImage
```

Installer options:
- `INSTALL_DIR=/custom/bin ./install.sh` or `./install.sh --dir /custom/bin` installs to a custom bin directory.
- `./install.sh --from-source` downloads the `v0.10.0` source archive from GitHub and compiles with `cargo build --release`.

## Windows x86_64 Standalone App

Download `rssim.exe` or `rssim-v0.10.0-x86_64-windows.zip` from [GitHub Releases](https://github.com/polychroms/rusted-space-simulation/releases) or [GitLab Releases](https://gitlab.com/polychroms/rssim/-/releases) and double-click `rssim.exe`. No installer or Visual C++ Redistributable is required (static CRT, Vulkan 1.2 compute with automatic CPU fallback). Saved blueprints, scenes, and captures live in `%LOCALAPPDATA%\rssim\`.

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
