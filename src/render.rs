use std::sync::OnceLock;

use eframe::egui::Color32;
use glam::Vec2;
use rayon::prelude::*;

use crate::materials::MaterialId;
use crate::physics::ContinuumWorld;

const MAX_FACETS: usize = 6;
const BAND_H: usize = 32;

#[derive(Clone, Copy)]
struct CrumbTemplate {
    n_facets: usize,
    nx: [f32; MAX_FACETS],
    ny: [f32; MAX_FACETS],
    dist: [f32; MAX_FACETS],
    bound_r: f32,
}

fn crumb_templates() -> &'static [CrumbTemplate; 128] {
    static TABLE: OnceLock<[CrumbTemplate; 128]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut out = [CrumbTemplate {
            n_facets: 6,
            nx: [0.0; MAX_FACETS],
            ny: [0.0; MAX_FACETS],
            dist: [1.0; MAX_FACETS],
            bound_r: 1.25,
        }; 128];

        let mut rng = 0xD1B5_4A32_D192_ED03_u64;
        let mut next_f32 = || -> f32 {
            rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            (z >> 40) as f32 / ((1u64 << 24) as f32)
        };

        for (idx, tpl) in out.iter_mut().enumerate() {
            // 0..64: 6-sided interlocking polycrystalline metallic/polymer Voronoi grains
            // 64..128: 5-sided jagged angular ceramic/mineral/spall shards & splinters
            let sharp_shard = idx >= 64;
            let nf = if sharp_shard { 5 } else { 6 };
            tpl.n_facets = nf;

            let stretch_x = if sharp_shard {
                0.78 + 0.44 * next_f32()
            } else {
                0.86 + 0.30 * next_f32()
            };
            let stretch_y = 1.0 / stretch_x.sqrt();

            let mut angles = [0.0f32; MAX_FACETS];
            let step = std::f32::consts::TAU / nf as f32;
            let jitter_ang = if sharp_shard { 0.22 } else { 0.16 };
            for (k, ang_slot) in angles.iter_mut().enumerate().take(nf) {
                *ang_slot = k as f32 * step + (next_f32() - 0.5) * 2.0 * jitter_ang;
            }

            for (k, &ang) in angles.iter().enumerate().take(nf) {
                let (s, c) = ang.sin_cos();
                // Scale facet normal inversely with stretch to produce elongated angular shards/grains
                let ux = c / stretch_x;
                let uy = s / stretch_y;
                let len = (ux * ux + uy * uy).sqrt().max(1e-5);
                tpl.nx[k] = ux / len;
                tpl.ny[k] = uy / len;
                tpl.dist[k] = if sharp_shard {
                    0.78 + 0.28 * next_f32()
                } else {
                    0.85 + 0.22 * next_f32()
                };
            }

            // Compute exact circumradius across adjacent facet intersections
            let mut max_vr = 1.05f32;
            for k in 0..nf {
                let m = (k + 1) % nf;
                let det = tpl.nx[k] * tpl.ny[m] - tpl.ny[k] * tpl.nx[m];
                if det.abs() > 0.15 {
                    let vx = (tpl.dist[k] * tpl.ny[m] - tpl.dist[m] * tpl.ny[k]) / det;
                    let vy = (tpl.nx[k] * tpl.dist[m] - tpl.nx[m] * tpl.dist[k]) / det;
                    max_vr = max_vr.max((vx * vx + vy * vy).sqrt());
                }
            }
            tpl.bound_r = max_vr.min(1.65);
        }
        out
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisMode {
    MaterialThermal,
    ShockPressure,
    PlasticStrain,
    DamageAndPhase,
    VelocityField,
    TemperatureFlir,
    BondNetworkXray,
    PenetrationXray,
}

impl VisMode {
    pub const ALL: [VisMode; 8] = [
        VisMode::MaterialThermal,
        VisMode::ShockPressure,
        VisMode::PlasticStrain,
        VisMode::DamageAndPhase,
        VisMode::VelocityField,
        VisMode::TemperatureFlir,
        VisMode::BondNetworkXray,
        VisMode::PenetrationXray,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VisMode::MaterialThermal => "1: Material + Detonation + Cracks",
            VisMode::ShockPressure => "2: Shockwave & Spall Tension (GPa)",
            VisMode::PlasticStrain => "3: Plastic Deformation (Strain)",
            VisMode::DamageAndPhase => "4: Fracture, Spall & Gas Phase",
            VisMode::VelocityField => "5: Velocity Magnitude (km/s)",
            VisMode::TemperatureFlir => "6: Temperature FLIR (Kelvin)",
            VisMode::BondNetworkXray => "7: Von Mises Shear Stress (GPa)",
            VisMode::PenetrationXray => "8: Ballistic X-Ray / Jet Trace",
        }
    }
}

pub struct Camera2D {
    /// World coordinates (cm) at center of viewport
    pub center: Vec2,
    /// Pixels per cm
    pub zoom: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            center: Vec2::new(4.0, 0.0),
            zoom: 12.5,
        }
    }
}

impl Camera2D {
    #[inline]
    pub fn world_to_screen(&self, p: Vec2, width: f32, height: f32) -> Vec2 {
        Vec2::new(
            0.5 * width + (p.x - self.center.x) * self.zoom,
            0.5 * height - (p.y - self.center.y) * self.zoom,
        )
    }

    #[inline]
    pub fn screen_to_world(&self, s: Vec2, width: f32, height: f32) -> Vec2 {
        Vec2::new(
            self.center.x + (s.x - 0.5 * width) / self.zoom,
            self.center.y - (s.y - 0.5 * height) / self.zoom,
        )
    }
}

#[inline]
fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Physically inspired blackbody incandescence RGB (0..1+) for T > 920 K
#[inline]
fn blackbody_glow(temp_k: f32) -> ([f32; 3], f32) {
    if temp_k < 920.0 {
        return ([0.0, 0.0, 0.0], 0.0);
    }
    let t = ((temp_k - 920.0) / 5200.0).clamp(0.0, 1.0);
    let rgb = if t < 0.30 {
        lerp_rgb([0.62, 0.24, 0.06], [0.97, 0.56, 0.16], t / 0.30)
    } else if t < 0.65 {
        lerp_rgb([0.97, 0.56, 0.16], [1.0, 0.85, 0.42], (t - 0.30) / 0.35)
    } else if t < 0.90 {
        lerp_rgb([1.0, 0.85, 0.42], [1.0, 0.94, 0.76], (t - 0.65) / 0.25)
    } else {
        lerp_rgb([1.0, 0.94, 0.76], [0.95, 0.97, 1.0], (t - 0.90) / 0.10)
    };
    let strength = ((temp_k - 920.0) / 2800.0).clamp(0.0, 0.84);
    (rgb, strength)
}

/// Inferno / Ironbow colormap for scalar in [0, 1]
#[inline]
fn inferno_map(x: f32) -> [u8; 3] {
    let t = x.clamp(0.0, 1.0);
    let c = if t < 0.25 {
        lerp_rgb([0.08, 0.06, 0.22], [0.48, 0.11, 0.43], t / 0.25)
    } else if t < 0.55 {
        lerp_rgb([0.48, 0.11, 0.43], [0.90, 0.36, 0.16], (t - 0.25) / 0.30)
    } else if t < 0.82 {
        lerp_rgb([0.90, 0.36, 0.16], [0.99, 0.80, 0.22], (t - 0.55) / 0.27)
    } else {
        lerp_rgb([0.99, 0.80, 0.22], [1.00, 0.99, 0.88], (t - 0.82) / 0.18)
    };
    [
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    ]
}

/// Turbo-like colormap for velocity in [0, 1]
#[inline]
fn turbo_map(x: f32) -> [u8; 3] {
    let t = x.clamp(0.0, 1.0);
    let c = if t < 0.25 {
        lerp_rgb([0.14, 0.18, 0.48], [0.10, 0.68, 0.88], t / 0.25)
    } else if t < 0.50 {
        lerp_rgb([0.10, 0.68, 0.88], [0.32, 0.92, 0.42], (t - 0.25) / 0.25)
    } else if t < 0.78 {
        lerp_rgb([0.32, 0.92, 0.42], [0.99, 0.72, 0.15], (t - 0.50) / 0.28)
    } else {
        lerp_rgb([0.99, 0.72, 0.15], [1.00, 0.22, 0.18], (t - 0.78) / 0.22)
    };
    [
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    ]
}

#[inline]
fn particle_color(world: &ContinuumWorld, i: usize, mode: VisMode) -> ([u8; 3], bool) {
    let props = world.mat[i].props();
    let temp = world.temp[i];
    let dmg = world.damage[i];
    let det = world.det_state[i];
    let det_age = world.det_age[i];
    let is_melted = temp >= props.melt_temp;

    match mode {
        VisMode::MaterialThermal => {
            if det == 2 {
                // Expanding reactive detonation products: white-gold plasma front -> fiery fireball -> atmospheric charcoal smoke
                let (c, halo) = if det_age < 2.2 {
                    (
                        lerp_rgb([1.0, 0.99, 0.94], [1.0, 0.84, 0.32], det_age / 2.2),
                        true,
                    )
                } else if det_age < 13.0 {
                    (
                        lerp_rgb(
                            [1.0, 0.84, 0.32],
                            [0.92, 0.34, 0.08],
                            (det_age - 2.2) / 10.8,
                        ),
                        det_age < 11.0,
                    )
                } else {
                    (
                        lerp_rgb(
                            [0.92, 0.34, 0.08],
                            [0.21, 0.22, 0.25],
                            ((det_age - 13.0) / 18.0).clamp(0.0, 1.0),
                        ),
                        false,
                    )
                };
                return (
                    [
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                    ],
                    halo,
                );
            } else if det == 1 {
                // Ignited pyrotechnic delay / piezo trace
                return ([255, 168, 52], true);
            }

            // Polycrystalline / mineral crumb grain variation + tumbling shard specular catch-light!
            let seed = world.crumb_seed[i];
            let grain_u = (((seed >> 8) & 0xFF) as f32 / 255.0) - 0.5; // -0.5..+0.5
            let tint_u = (((seed >> 16) & 0xFF) as f32 / 255.0) - 0.5; // -0.5..+0.5
            let grain_amp = if props.is_ductile() { 0.11 } else { 0.17 };
            let shard_glint = if dmg >= 0.65 {
                0.06 * (world.crumb_angle[i] * 2.0).cos()
            } else {
                0.0
            };
            let grain_mult = (1.0 + grain_amp * grain_u + shard_glint).clamp(0.80, 1.20);

            let base = [
                (props.color[0] as f32 / 255.0 * grain_mult + 0.015 * tint_u).clamp(0.0, 1.0),
                (props.color[1] as f32 / 255.0 * grain_mult + 0.004 * tint_u).clamp(0.0, 1.0),
                (props.color[2] as f32 / 255.0 * grain_mult - 0.015 * tint_u).clamp(0.0, 1.0),
            ];
            // Etch internal fracture & spall crack lines visibly without shrinking solids into black voids
            let crack_shade = if !is_melted && dmg > 0.14 && dmg < 0.92 {
                1.0 - 0.42 * ((dmg - 0.14) / 0.70).clamp(0.0, 1.0)
            } else if !is_melted && dmg >= 0.92 {
                0.86
            } else {
                1.0
            };
            // Subtle metallic shock compression sheen (avoids oversaturated cyan tinting)
            let comp_boost = ((world.rho[i] / world.rho0[i] - 1.0) * 0.95).clamp(0.0, 0.24);
            let shock_flash = (world.shock_rate[i] * 1.4).clamp(0.0, 0.26);
            let mut rgb = [
                (base[0] * crack_shade + 0.28 * comp_boost + 0.36 * shock_flash).clamp(0.0, 1.0),
                (base[1] * crack_shade + 0.32 * comp_boost + 0.42 * shock_flash).clamp(0.0, 1.0),
                (base[2] * crack_shade + 0.38 * comp_boost + 0.50 * shock_flash).clamp(0.0, 1.0),
            ];
            let (glow, g_amt) = blackbody_glow(temp);
            if g_amt > 0.0 {
                rgb = lerp_rgb(rgb, glow, g_amt);
            }
            (
                [
                    (rgb[0] * 255.0) as u8,
                    (rgb[1] * 255.0) as u8,
                    (rgb[2] * 255.0) as u8,
                ],
                temp > 1750.0,
            )
        }

        VisMode::ShockPressure => {
            // Net pressure in GPa: positive = shock compression, negative = spall tension!
            let p_gpa = world.net_pressure[i] * 100.0;
            if p_gpa >= 0.0 {
                // nonlinear sqrt compression scale so both 1 GPa elastic precursors and 60 GPa shocks shine!
                let t = (p_gpa / 38.0).max(0.0).sqrt().clamp(0.0, 1.0);
                let shock_boost = (world.shock_rate[i] * 2.2).clamp(0.0, 0.35);
                let s = (t + shock_boost).clamp(0.0, 1.0);
                let c = if s < 0.35 {
                    lerp_rgb([0.13, 0.16, 0.23], [0.08, 0.58, 0.92], s / 0.35)
                } else if s < 0.75 {
                    lerp_rgb([0.08, 0.58, 0.92], [0.42, 0.98, 0.96], (s - 0.35) / 0.40)
                } else {
                    lerp_rgb([0.42, 0.98, 0.96], [1.00, 0.98, 0.76], (s - 0.75) / 0.25)
                };
                (
                    [
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                    ],
                    s > 0.72,
                )
            } else {
                // Tensile rarefaction wave (spallation zone!) -> vivid magenta/crimson
                let t = ((-p_gpa) / 3.2).sqrt().clamp(0.0, 1.0);
                let c = lerp_rgb([0.13, 0.16, 0.23], [1.00, 0.14, 0.58], t);
                (
                    [
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                    ],
                    t > 0.65,
                )
            }
        }

        VisMode::PlasticStrain => {
            let s = (world.plastic_strain[i] / 0.65).sqrt().clamp(0.0, 1.0);
            (inferno_map(s), s > 0.80)
        }

        VisMode::DamageAndPhase => {
            if det == 2 {
                // Detonation expanding gas products
                ([255, 232, 92], det_age < 3.5)
            } else if det == 1 {
                ([255, 150, 40], true)
            } else if is_melted {
                // Hydrodynamic molten fluid phase
                ([238, 120, 255], true)
            } else if dmg >= 0.85 {
                // Detached spall fragment / comminuted rubble
                ([250, 74, 58], false)
            } else if dmg > 0.12 {
                // Propagating crack / partial spall damage
                let t = ((dmg - 0.12) / 0.73).clamp(0.0, 1.0);
                let c = lerp_rgb([0.95, 0.78, 0.18], [0.98, 0.29, 0.22], t);
                (
                    [
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                    ],
                    false,
                )
            } else if world.plastic_strain[i] > 0.015 {
                // Plastically deformed intact solid
                let t = (world.plastic_strain[i] / 0.35).clamp(0.0, 1.0);
                let c = lerp_rgb([0.16, 0.56, 0.72], [0.18, 0.88, 0.56], t);
                (
                    [
                        (c[0] * 255.0) as u8,
                        (c[1] * 255.0) as u8,
                        (c[2] * 255.0) as u8,
                    ],
                    false,
                )
            } else {
                // Intact elastic solid
                ([52, 108, 176], false)
            }
        }

        VisMode::VelocityField => {
            let v_kms = world.vel[i].length() * 10.0;
            let t = (v_kms / 5.5).sqrt().clamp(0.0, 1.0);
            (turbo_map(t), v_kms > 4.5)
        }

        VisMode::TemperatureFlir => {
            let t = ((temp - 240.0) / 3400.0).max(0.0).sqrt().clamp(0.0, 1.0);
            (inferno_map(t), temp > 2200.0)
        }

        VisMode::BondNetworkXray => {
            if det == 2 {
                ([255, 215, 85], false)
            } else {
                let vm_gpa = world.von_mises_gpa(i);
                let t = (vm_gpa / 2.6).sqrt().clamp(0.0, 1.0);
                (turbo_map(t), vm_gpa > 2.2)
            }
        }

        VisMode::PenetrationXray => {
            // Ballistic X-Ray cutaway view: stationary intact structure is cool dark blueprint slate
            // while high-speed penetrators, Munroe jets, ERA flyers, spall & detonations glow vividly!
            let v_kms = world.vel[i].length() * 10.0;
            if det == 2 {
                let fade = (1.0 - (det_age / 12.0).clamp(0.0, 0.78)).clamp(0.22, 1.0);
                (
                    [
                        (255.0 * fade) as u8,
                        (218.0 * fade) as u8,
                        (98.0 * fade) as u8,
                    ],
                    det_age < 3.0,
                )
            } else if det == 1 {
                ([255, 130, 35], true)
            } else if v_kms < 0.22 && dmg < 0.22 {
                if props.is_energetic {
                    // Subtle warm amber silhouette for undetonated ERA / internal ammo racks
                    ([108, 72, 42], false)
                } else {
                    let dens_t = (props.rho0 / 18.0).clamp(0.0, 1.0);
                    let c = lerp_rgb([0.13, 0.18, 0.27], [0.26, 0.36, 0.52], dens_t);
                    (
                        [
                            (c[0] * 255.0) as u8,
                            (c[1] * 255.0) as u8,
                            (c[2] * 255.0) as u8,
                        ],
                        false,
                    )
                }
            } else {
                let vt = (v_kms / 6.2).sqrt().clamp(0.15, 1.0);
                (turbo_map(vt), v_kms > 3.2)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct DrawItem {
    cx: f32,
    cy: f32,
    tx_off: f32,
    ty_off: f32,
    r: f32,
    angle: f32,
    tpl_idx: u8,
    mat: MaterialId,
    seed: u32,
    dmg: f32,
    col: [u8; 3],
    bevel: i16,
    render_kind: u8, // 0 = solid/shard crumb, 1 = volumetric gas/fireball plume, 2 = additive blast glow
    halo_r: f32,
}

pub struct PixelCanvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Color32>,
    draw_items: Vec<DrawItem>,
    band_buckets: Vec<Vec<usize>>,
}

impl PixelCanvas {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            draw_items: Vec::new(),
            band_buckets: Vec::new(),
        }
    }

    pub fn render(
        &mut self,
        world: &ContinuumWorld,
        cam: &Camera2D,
        width: usize,
        height: usize,
        mode: VisMode,
        motion_trails: bool,
    ) {
        let width = width.max(64);
        let height = height.max(64);
        let total = width * height;
        let resized = self.width != width || self.height != height || self.pixels.len() != total;
        if resized {
            self.width = width;
            self.height = height;
            self.pixels.assign_bg(total);
        } else if motion_trails {
            // Subtle high-speed camera persistence fade toward dark lab background (parallel!)
            self.pixels.par_chunks_mut(16384).for_each(|chunk| {
                for px in chunk {
                    let [r, g, b, _] = px.to_array();
                    *px = Color32::from_rgb(
                        ((r as u16 * 185 + 11 * 71) >> 8) as u8,
                        ((g as u16 * 185 + 14 * 71) >> 8) as u8,
                        ((b as u16 * 185 + 20 * 71) >> 8) as u8,
                    );
                }
            });
        } else {
            self.pixels.assign_bg(total);
        }

        // Draw subtle metric background grid (10 cm major, 2 cm minor, 5 mm microscope)
        if !motion_trails || resized {
            self.draw_metric_grid(cam);
        }

        let n_pts = world.len();
        if n_pts == 0 {
            return;
        }

        let wf = width as f32;
        let hf = height as f32;
        let w_max_f = (width - 1) as f32;
        let h_max_f = (height - 1) as f32;

        let templates = crumb_templates();
        // World-invariant cohesive continuum radius: 0.87 * dx * zoom seals neighbors cleanly
        let intact_r = 0.87 * world.dx * cam.zoom;
        let rich_tex = mode == VisMode::MaterialThermal;

        let num_bands = height.div_ceil(BAND_H);
        if self.band_buckets.len() < num_bands {
            self.band_buckets.resize_with(num_bands, Vec::new);
        }
        for b in &mut self.band_buckets[..num_bands] {
            b.clear();
        }
        self.draw_items.clear();

        // Four-pass depth compositing:
        // Pass 0: static intact armor/targets
        // Pass 1: volumetric detonation plumes & smoke clouds (mass-conserving overlapping radii!)
        // Pass 2: dynamic moving/damaged solids, jets & fragments in front of smoke
        // Pass 3: pure additive luminous fireball/plasma flash on top
        for pass in 0..4 {
            for i in 0..n_pts {
                let det = world.det_state[i];
                let is_moving = det == 1 || world.vel[i].length_squared() > 0.0025;
                let keep = match pass {
                    0 => det == 0 && !is_moving,
                    1 => det == 2,
                    2 => det != 2 && is_moving,
                    _ => det == 2,
                };
                if !keep {
                    continue;
                }
                let sp = cam.world_to_screen(world.pos[i], wf, hf);
                if !sp.x.is_finite() || !sp.y.is_finite() {
                    continue;
                }

                let (col, hot_halo) = particle_color(world, i, mode);

                let (render_kind, r, angle, tpl_idx, mat, seed, dmg, bevel, halo_r, bound_r, tx_off, ty_off) =
                    if det == 2 {
                        let mat = world.mat[i];
                        let det_power = if mat == MaterialId::ImpactFuze {
                            0.18
                        } else {
                            (mat.props().detonation_energy / 0.056)
                                .clamp(0.16, 1.25)
                                .sqrt()
                                .clamp(0.42, 1.15)
                        };
                        let rho_rel =
                            (world.rho[i] / world.rho0[i].max(1e-4)).clamp(0.045, 2.4);
                        let age = world.det_age[i].max(0.0);
                        // 2D volume expansion: parcel radius grows ~1/sqrt(rho_rel) scaled by explosive yield
                        let exp_scale = (1.0 / rho_rel).sqrt().clamp(1.0, 4.4);
                        let age_swell = (1.0 + 0.14 * age).clamp(1.0, 2.4);
                        let pr = intact_r
                            * det_power
                            * (1.42 * exp_scale.max(age_swell)).clamp(1.42, 5.2);
                        let eff_temp = if mat == MaterialId::ImpactFuze {
                            (world.temp[i] * 0.25).min(950.0)
                        } else {
                            world.temp[i]
                        };
                        let temp_c = (eff_temp * 0.1).clamp(40.0, 3000.0) as i16;
                        let v_code = (world.vel[i].length() * 35.0).clamp(0.0, 255.0) as u8;
                        let hr = if pass == 3 && hot_halo && mat != MaterialId::ImpactFuze {
                            (pr * (1.65 - 0.04 * age).max(1.0)).min(64.0)
                        } else {
                            0.0
                        };
                        if pass == 3 && hr < 0.45 {
                            continue;
                        }
                        (
                            if pass == 3 { 2u8 } else { 1u8 },
                            pr,
                            age,
                            v_code,
                            mat,
                            world.crumb_seed[i],
                            rho_rel,
                            temp_c,
                            hr,
                            1.38f32,
                            0.0f32,
                            0.0f32,
                        )
                    } else {
                        let dmg = world.damage[i].clamp(0.0, 1.0);
                        let mat = world.mat[i];
                        let props = mat.props();
                        let seed = world.crumb_seed[i];
                        let shard_bank = usize::from(!props.is_ductile() || dmg >= 0.55) * 64;
                        let tpl_idx = (shard_bank + ((seed as usize) & 63)) as u8;
                        let rho_rel =
                            (world.rho[i] / world.rho0[i].max(1e-4)).clamp(0.45, 2.0);
                        let melt_swell = if world.temp[i] >= props.melt_temp {
                            (1.0 / rho_rel).sqrt().clamp(1.0, 1.26)
                        } else {
                            1.0
                        };
                        let crumb_r = intact_r * (1.0 - 0.04 * dmg) * melt_swell;
                        let bevel = if dmg >= 0.45 { 20i16 } else { 13i16 };
                        let hr = if hot_halo {
                            (crumb_r * 1.55).min(28.0)
                        } else {
                            0.0
                        };
                        // World-invariant incandescent ember trail ONLY on glowing hot fragments & jets
                        let vel = world.vel[i];
                        let spd = vel.length();
                        let (tx_off, ty_off) = if pass == 2
                            && rich_tex
                            && world.temp[i] >= 1100.0
                            && spd >= 0.12
                        {
                            let raw_len =
                                (crumb_r * 1.20 + spd * 2.6 * cam.zoom).min(7.5 * intact_r);
                            let inv_spd = 1.0 / spd;
                            (-vel.x * inv_spd * raw_len, vel.y * inv_spd * raw_len)
                        } else {
                            (0.0, 0.0)
                        };
                        let br = templates[tpl_idx as usize].bound_r + 0.12;
                        (
                            0u8,
                            crumb_r,
                            world.crumb_angle[i],
                            tpl_idx,
                            mat,
                            seed,
                            dmg,
                            bevel,
                            hr,
                            br,
                            tx_off,
                            ty_off,
                        )
                    };

                if !r.is_finite() || !angle.is_finite() || r <= 0.02 {
                    continue;
                }

                let body_ext = if render_kind == 2 {
                    halo_r
                } else if r < 0.72 {
                    1.0
                } else {
                    r * bound_r + 0.85
                };
                let max_ext = body_ext.max(if halo_r >= 0.45 { halo_r } else { 0.0 });
                let min_x = sp.x.min(sp.x + tx_off) - max_ext;
                let max_x = sp.x.max(sp.x + tx_off) + max_ext;
                let min_y = sp.y.min(sp.y + ty_off) - max_ext;
                let max_y = sp.y.max(sp.y + ty_off) + max_ext;
                if max_x < 0.0 || min_x > w_max_f || max_y < 0.0 || min_y > h_max_f {
                    continue;
                }

                let y0 = min_y.floor().clamp(0.0, h_max_f) as usize;
                let y1 = max_y.ceil().clamp(0.0, h_max_f) as usize;
                if y0 > y1 {
                    continue;
                }

                let item_idx = self.draw_items.len();
                self.draw_items.push(DrawItem {
                    cx: sp.x,
                    cy: sp.y,
                    tx_off,
                    ty_off,
                    r,
                    angle,
                    tpl_idx,
                    mat,
                    seed,
                    dmg,
                    col,
                    bevel,
                    render_kind,
                    halo_r,
                });

                let b0 = y0 / BAND_H;
                let b1 = (y1 / BAND_H).min(num_bands - 1);
                for bucket in &mut self.band_buckets[b0..=b1] {
                    bucket.push(item_idx);
                }
            }
        }

        // Rasterize all horizontal screen bands concurrently across all CPU cores!
        let band_len = BAND_H * width;
        let draw_items = &self.draw_items;
        let band_buckets = &self.band_buckets[..num_bands];

        self.pixels
            .par_chunks_mut(band_len)
            .enumerate()
            .for_each(|(b, band_px)| {
                let band_y0 = b * BAND_H;
                let band_rows = band_px.len() / width;
                if band_rows == 0 {
                    return;
                }
                let band_y1 = band_y0 + band_rows - 1;
                for &idx in &band_buckets[b] {
                    let it = &draw_items[idx];
                    match it.render_kind {
                        2 => {
                            let hot_fade = (1.0 - (it.angle / 16.0).clamp(0.0, 1.0)).max(0.15);
                            splat_additive_halo_band(
                                band_px,
                                width,
                                band_y0,
                                band_y1,
                                it.cx,
                                it.cy,
                                it.halo_r,
                                it.col,
                                0.14 * hot_fade,
                            );
                        }
                        1 => {
                            splat_explosion_plume_band(
                                band_px,
                                width,
                                band_y0,
                                band_y1,
                                it.cx,
                                it.cy,
                                it.r,
                                it.angle,
                                it.dmg,
                                it.bevel as f32 * 10.0,
                                it.seed,
                                it.col,
                                rich_tex,
                            );
                        }
                        _ => {
                            if it.tx_off * it.tx_off + it.ty_off * it.ty_off > 0.64 {
                                splat_ember_streak_band(
                                    band_px, width, band_y0, band_y1, it.cx, it.cy,
                                    it.cx + it.tx_off, it.cy + it.ty_off, it.r * 0.78, it.col,
                                );
                            }
                            if it.halo_r >= 0.45 {
                                splat_additive_halo_band(
                                    band_px, width, band_y0, band_y1, it.cx, it.cy, it.halo_r,
                                    it.col, 0.11,
                                );
                            }
                            splat_crumb_polygon_band(
                                band_px,
                                width,
                                band_y0,
                                band_y1,
                                it.cx,
                                it.cy,
                                it.r,
                                it.angle,
                                &templates[it.tpl_idx as usize],
                                it.mat,
                                it.seed,
                                it.dmg,
                                rich_tex,
                                it.col,
                                it.bevel,
                            );
                        }
                    }
                }
            });
    }

    fn draw_metric_grid(&mut self, cam: &Camera2D) {
        let wf = self.width as f32;
        let hf = self.height as f32;
        let w_min = cam.screen_to_world(Vec2::new(0.0, hf), wf, hf);
        let w_max = cam.screen_to_world(Vec2::new(wf, 0.0), wf, hf);

        // Fine 0.5 cm (5 mm) microscope grid when zoomed way in
        if cam.zoom >= 36.0 {
            let step = 0.5f32;
            let gx0 = (w_min.x / step).floor() as i32;
            let gx1 = (w_max.x / step).ceil() as i32;
            for gx in gx0..=gx1 {
                if gx % 4 == 0 {
                    continue;
                }
                let sx = (0.5 * wf + (gx as f32 * step - cam.center.x) * cam.zoom).round() as isize;
                if sx >= 0 && (sx as usize) < self.width {
                    let x = sx as usize;
                    for y in 0..self.height {
                        self.pixels[y * self.width + x] = Color32::from_rgb(14, 18, 25);
                    }
                }
            }
            let gy0 = (w_min.y / step).floor() as i32;
            let gy1 = (w_max.y / step).ceil() as i32;
            for gy in gy0..=gy1 {
                if gy % 4 == 0 {
                    continue;
                }
                let sy = (0.5 * hf - (gy as f32 * step - cam.center.y) * cam.zoom).round() as isize;
                if sy >= 0 && (sy as usize) < self.height {
                    let row = sy as usize * self.width;
                    for x in 0..self.width {
                        self.pixels[row + x] = Color32::from_rgb(14, 18, 25);
                    }
                }
            }
        }

        // Minor 2 cm grid if zoomed in enough
        if cam.zoom >= 10.0 {
            let step = 2.0f32;
            let gx0 = (w_min.x / step).floor() as i32;
            let gx1 = (w_max.x / step).ceil() as i32;
            for gx in gx0..=gx1 {
                if gx % 5 == 0 {
                    continue;
                }
                let sx = (0.5 * wf + (gx as f32 * step - cam.center.x) * cam.zoom).round() as isize;
                if sx >= 0 && (sx as usize) < self.width {
                    let x = sx as usize;
                    for y in 0..self.height {
                        self.pixels[y * self.width + x] = Color32::from_rgb(16, 20, 28);
                    }
                }
            }
            let gy0 = (w_min.y / step).floor() as i32;
            let gy1 = (w_max.y / step).ceil() as i32;
            for gy in gy0..=gy1 {
                if gy % 5 == 0 {
                    continue;
                }
                let sy = (0.5 * hf - (gy as f32 * step - cam.center.y) * cam.zoom).round() as isize;
                if sy >= 0 && (sy as usize) < self.height {
                    let row = sy as usize * self.width;
                    for x in 0..self.width {
                        self.pixels[row + x] = Color32::from_rgb(16, 20, 28);
                    }
                }
            }
        }

        // Major 10 cm grid (or 100 cm when zoomed far out)
        let major = if cam.zoom < 3.0 { 100.0f32 } else { 10.0f32 };
        let gx0 = (w_min.x / major).floor() as i32;
        let gx1 = (w_max.x / major).ceil() as i32;
        for gx in gx0..=gx1 {
            let sx = (0.5 * wf + (gx as f32 * major - cam.center.x) * cam.zoom).round() as isize;
            if sx >= 0 && (sx as usize) < self.width {
                let x = sx as usize;
                let col = if gx == 0 {
                    Color32::from_rgb(34, 44, 62)
                } else {
                    Color32::from_rgb(23, 29, 40)
                };
                for y in 0..self.height {
                    self.pixels[y * self.width + x] = col;
                }
            }
        }
        let gy0 = (w_min.y / major).floor() as i32;
        let gy1 = (w_max.y / major).ceil() as i32;
        for gy in gy0..=gy1 {
            let sy = (0.5 * hf - (gy as f32 * major - cam.center.y) * cam.zoom).round() as isize;
            if sy >= 0 && (sy as usize) < self.height {
                let row = sy as usize * self.width;
                let col = if gy == 0 {
                    Color32::from_rgb(34, 44, 62)
                } else {
                    Color32::from_rgb(23, 29, 40)
                };
                for x in 0..self.width {
                    self.pixels[row + x] = col;
                }
            }
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn splat_crumb_polygon(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        angle: f32,
        tpl: &CrumbTemplate,
        mat: MaterialId,
        seed: u32,
        dmg: f32,
        rich_tex: bool,
        col: [u8; 3],
        bevel: i16,
    ) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let h_max = self.height - 1;
        splat_crumb_polygon_band(
            &mut self.pixels,
            self.width,
            0,
            h_max,
            cx,
            cy,
            r,
            angle,
            tpl,
            mat,
            seed,
            dmg,
            rich_tex,
            col,
            bevel,
        );
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn splat_explosion_plume(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        det_age: f32,
        rho_rel: f32,
        temp_k: f32,
        seed: u32,
        tint_col: [u8; 3],
        rich_tex: bool,
    ) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        splat_explosion_plume_band(
            &mut self.pixels,
            self.width,
            0,
            self.height - 1,
            cx,
            cy,
            r,
            det_age,
            rho_rel,
            temp_k,
            seed,
            tint_col,
            rich_tex,
        );
    }

    #[cfg(test)]
    fn splat_aa_disk(&mut self, cx: f32, cy: f32, r: f32, col: [u8; 3]) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        splat_aa_disk_band(&mut self.pixels, self.width, 0, self.height - 1, cx, cy, r, col);
    }

    #[cfg(test)]
    fn splat_additive_halo(&mut self, cx: f32, cy: f32, r: f32, col: [u8; 3], intensity: f32) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        splat_additive_halo_band(
            &mut self.pixels,
            self.width,
            0,
            self.height - 1,
            cx,
            cy,
            r,
            col,
            intensity,
        );
    }

    #[cfg(test)]
    fn draw_line(&mut self, p0: Vec2, p1: Vec2, col: [u8; 3], alpha: u16) {
        if !p0.is_finite() || !p1.is_finite() || self.width == 0 || self.height == 0 {
            return;
        }
        let wf = self.width as f32;
        let hf = self.height as f32;
        if (p0.x < 0.0 && p1.x < 0.0)
            || (p0.x >= wf && p1.x >= wf)
            || (p0.y < 0.0 && p1.y < 0.0)
            || (p0.y >= hf && p1.y >= hf)
        {
            return;
        }
        let delta = p1 - p0;
        let steps = (delta.abs().max_element().ceil() as i32).min(4096);
        if steps <= 0 {
            return;
        }
        let inv_a = 256 - alpha;
        let step_v = delta / steps as f32;
        let mut cur = p0;
        for _ in 0..=steps {
            let x = cur.x as isize;
            let y = cur.y as isize;
            if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
                let idx = y as usize * self.width + x as usize;
                let bg = self.pixels[idx].to_array();
                self.pixels[idx] = Color32::from_rgb(
                    ((col[0] as u16 * alpha + bg[0] as u16 * inv_a) >> 8) as u8,
                    ((col[1] as u16 * alpha + bg[1] as u16 * inv_a) >> 8) as u8,
                    ((col[2] as u16 * alpha + bg[2] as u16 * inv_a) >> 8) as u8,
                );
            }
            cur += step_v;
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn splat_crumb_polygon_band(
    band_px: &mut [Color32],
    width: usize,
    band_y0: usize,
    band_y1: usize,
    cx: f32,
    cy: f32,
    r: f32,
    angle: f32,
    tpl: &CrumbTemplate,
    mat: MaterialId,
    seed: u32,
    dmg: f32,
    rich_tex: bool,
    col: [u8; 3],
    bevel: i16,
) {
    if !cx.is_finite()
        || !cy.is_finite()
        || !r.is_finite()
        || !angle.is_finite()
        || r <= 0.02
        || width == 0
        || band_y0 > band_y1
    {
        return;
    }

    // Subpixel zoom-out path: preserve exact world area without bloating to multi-pixel blobs!
    if r < 0.72 {
        let ix = cx.floor() as isize;
        let iy = cy.floor() as isize;
        if ix >= 0
            && (ix as usize) < width
            && iy >= band_y0 as isize
            && iy <= band_y1 as isize
        {
            let local_y = (iy - band_y0 as isize) as usize;
            let idx = local_y * width + ix as usize;
            let coverage = (std::f32::consts::PI * r * r).clamp(0.14, 1.0);
            let a_u16 = (coverage * 256.0) as u16;
            let inv_a = 256 - a_u16;
            let bg = band_px[idx].to_array();
            band_px[idx] = Color32::from_rgb(
                ((col[0] as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                ((col[1] as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                ((col[2] as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
            );
        }
        return;
    }

    let r_bound = r * (tpl.bound_r + 0.12) + 0.85;
    let x0_i = (cx - r_bound).floor() as isize;
    let x1_i = (cx + r_bound).ceil() as isize;
    let y0_i = (cy - r_bound).floor() as isize;
    let y1_i = (cy + r_bound).ceil() as isize;
    let w_max = width as isize - 1;
    if x1_i < 0 || x0_i > w_max || y1_i < band_y0 as isize || y0_i > band_y1 as isize {
        return;
    }
    let x0 = x0_i.max(0) as usize;
    let x1 = x1_i.min(w_max) as usize;
    let y0 = (y0_i.max(band_y0 as isize)) as usize;
    let y1 = (y1_i.min(band_y1 as isize)) as usize;

    let nf = tpl.n_facets;
    let (sin_a, cos_a) = angle.sin_cos();
    let mut rx = [0.0f32; MAX_FACETS];
    let mut ry = [0.0f32; MAX_FACETS];
    let mut dk = [0.0f32; MAX_FACETS];
    let mut f_light = [0i16; MAX_FACETS];

    // Smooth MIP weight so tiny <1.5px crumbs don't alias while r >= 2.2px gets full crisp pixel texels
    let tex_w = (((r - 0.90) * 0.72).clamp(0.0, 1.0) * 256.0) as i32;

    for k in 0..nf {
        let nx = tpl.nx[k] * cos_a - tpl.ny[k] * sin_a;
        let ny = tpl.nx[k] * sin_a + tpl.ny[k] * cos_a;
        rx[k] = nx;
        ry[k] = ny;
        dk[k] = tpl.dist[k] * r;
        let dot_l = -std::f32::consts::FRAC_1_SQRT_2 * (nx + ny);
        f_light[k] = (((dot_l * bevel as f32) as i32 * tex_w) >> 8) as i16;
    }

    // Body-locked 16.16 fixed-point coordinates: 8x8 crisp square pixel-art texels across crumb diameter 2r!
    let inv_r = 1.0 / r;
    let tex_scale = (4.0 * 65536.0) * inv_r;
    let du_dx = (cos_a * tex_scale).round() as i32;
    let dv_dx = (-sin_a * tex_scale).round() as i32;
    let u_off = (((seed & 0xFF) as i32) << 12).wrapping_add(16 * 65536);
    let v_off = ((((seed >> 8) & 0xFF) as i32) << 12).wrapping_add(16 * 65536);

    let rough_amp = if !mat.props().is_ductile() || dmg >= 0.45 {
        0.088 * r
    } else {
        0.036 * r
    };
    let serr_scale = (rough_amp * (tex_w as f32 * (1.0 / 256.0))) * (1.0 / 128.0);
    let serr_gate = -0.32 * r - 0.65;
    let solid = Color32::from_rgb(col[0], col[1], col[2]);
    let start_dx = (x0 as f32 + 0.5) - cx;

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5) - cy;
        let row = (y - band_y0) * width;

        let mut p = [0.0f32; MAX_FACETS];
        for k in 0..nf {
            p[k] = start_dx * rx[k] + dy * ry[k] - dk[k];
        }

        let mut u_fp = ((start_dx * cos_a + dy * sin_a) * tex_scale).round() as i32 + u_off;
        let mut v_fp = ((-start_dx * sin_a + dy * cos_a) * tex_scale).round() as i32 + v_off;
        let mut prev_tx = i32::MIN;
        let mut prev_ty = i32::MIN;
        let mut cached_tex_h = 0u32;
        let mut cached_dt = [0i16; 3];
        let mut cached_dt_valid = false;

        for x in x0..=x1 {
            let mut max_e = p[0];
            let mut max_k = 0usize;
            p[0] += rx[0];
            for k in 1..nf {
                let v = p[k];
                p[k] += rx[k];
                if v > max_e {
                    max_e = v;
                    max_k = k;
                }
            }

            let tx = u_fp >> 16;
            let ty = v_fp >> 16;
            u_fp = u_fp.wrapping_add(du_dx);
            v_fp = v_fp.wrapping_add(dv_dx);

            // Fast early reject far outside the polygon before hashing
            if max_e > rough_amp + 0.65 {
                continue;
            }

            if tex_w == 0 {
                if max_e <= -0.50 {
                    band_px[row + x] = solid;
                } else if max_e < 0.50 {
                    let alpha = (0.50 - max_e).clamp(0.0, 1.0);
                    let a_u16 = (alpha * 256.0) as u16;
                    let inv_a = 256 - a_u16;
                    let bg = band_px[row + x].to_array();
                    band_px[row + x] = Color32::from_rgb(
                        ((col[0] as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                        ((col[1] as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                        ((col[2] as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
                    );
                }
                continue;
            }

            // Scanline texel-run register cache: skips ~80-90% of hash + texture calls when zoomed in!
            if tx != prev_tx || ty != prev_ty {
                prev_tx = tx;
                prev_ty = ty;
                cached_tex_h = cell_hash(tx, ty, seed);
                cached_dt_valid = false;
            }

            let max_e_s = if max_e > serr_gate {
                let serr_n = ((cached_tex_h >> 24) & 0xFF) as i32 - 128;
                max_e + serr_n as f32 * serr_scale
            } else {
                max_e
            };

            if max_e_s >= 0.50 {
                continue;
            }

            if !cached_dt_valid {
                cached_dt =
                    crumb_pixel_tex_rgb(mat, tx, ty, cached_tex_h, seed, dmg, tex_w, rich_tex);
                cached_dt_valid = true;
            }
            let mut dt = cached_dt;

            // World-proportional facet bevel & dark grain-boundary outline
            let e_norm = max_e_s * inv_r;
            if e_norm > -0.22 {
                let dl = f_light[max_k];
                dt[0] += dl;
                dt[1] += dl;
                dt[2] += dl;
                if rich_tex && e_norm > -0.09 {
                    let seam = ((12 * tex_w) >> 8) as i16;
                    dt[0] -= seam;
                    dt[1] -= seam;
                    dt[2] -= seam;
                }
            }

            let rc = (col[0] as i16 + dt[0]).clamp(0, 255) as u8;
            let gc = (col[1] as i16 + dt[1]).clamp(0, 255) as u8;
            let bc = (col[2] as i16 + dt[2]).clamp(0, 255) as u8;

            if max_e_s <= -0.50 {
                band_px[row + x] = Color32::from_rgb(rc, gc, bc);
            } else {
                let alpha = (0.50 - max_e_s).clamp(0.0, 1.0);
                let a_u16 = (alpha * 256.0) as u16;
                let inv_a = 256 - a_u16;
                let bg = band_px[row + x].to_array();
                band_px[row + x] = Color32::from_rgb(
                    ((rc as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                    ((gc as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                    ((bc as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
                );
            }
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn splat_explosion_plume_band(
    band_px: &mut [Color32],
    width: usize,
    band_y0: usize,
    band_y1: usize,
    cx: f32,
    cy: f32,
    r: f32,
    det_age: f32,
    rho_rel: f32,
    temp_k: f32,
    seed: u32,
    tint_col: [u8; 3],
    rich_tex: bool,
) {
    if !cx.is_finite()
        || !cy.is_finite()
        || !r.is_finite()
        || r <= 0.02
        || width == 0
        || band_y0 > band_y1
    {
        return;
    }
    if r < 0.72 {
        let ix = cx.floor() as isize;
        let iy = cy.floor() as isize;
        if ix >= 0
            && (ix as usize) < width
            && iy >= band_y0 as isize
            && iy <= band_y1 as isize
        {
            let local_y = (iy - band_y0 as isize) as usize;
            let idx = local_y * width + ix as usize;
            let fade = (0.35 + 0.65 * (1.0 - (det_age / 14.0).clamp(0.0, 1.0))).clamp(0.28, 1.0);
            let coverage = (std::f32::consts::PI * r * r * fade).clamp(0.14, 0.95);
            let a_u16 = (coverage * 256.0) as u16;
            let inv_a = 256 - a_u16;
            let bg = band_px[idx].to_array();
            band_px[idx] = Color32::from_rgb(
                ((tint_col[0] as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                ((tint_col[1] as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                ((tint_col[2] as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
            );
        }
        return;
    }

    let r_bound = r * 1.38 + 0.75;
    let x0_i = (cx - r_bound).floor() as isize;
    let x1_i = (cx + r_bound).ceil() as isize;
    let y0_i = (cy - r_bound).floor() as isize;
    let y1_i = (cy + r_bound).ceil() as isize;
    let w_max = width as isize - 1;
    if x1_i < 0 || x0_i > w_max || y1_i < band_y0 as isize || y0_i > band_y1 as isize {
        return;
    }
    let x0 = x0_i.max(0) as usize;
    let x1 = x1_i.min(w_max) as usize;
    let y0 = (y0_i.max(band_y0 as isize)) as usize;
    let y1 = (y1_i.min(band_y1 as isize)) as usize;

    let inv_r = 1.0 / r.max(0.25);
    let swirl = (seed as f32 * 0.00173) + det_age * 0.14 * if (seed & 1) == 0 { 1.0 } else { -1.0 };
    let (sin_s, cos_s) = swirl.sin_cos();

    // Smooth scale-invariant billowing fBm turbulence (soft cloud tendrils, zero circular borders!)
    let tex_scale = (2.4 * 65536.0) * inv_r;
    let du_dx = (cos_s * tex_scale).round() as i32;
    let dv_dx = (-sin_s * tex_scale).round() as i32;
    let age_shift = (det_age * 0.85).round() as i32;
    let u_off = (((seed & 0xFF) as i32) << 12).wrapping_add(age_shift << 15);
    let v_off = ((((seed >> 8) & 0xFF) as i32) << 12).wrapping_sub(age_shift << 14);

    // Incandescent plasma flash -> bright fireball -> warm ember-lit atmospheric smoke veil
    let young_flash = (1.0 - (det_age / 3.2).clamp(0.0, 1.0)).powf(1.3);
    let fire_life = (1.0 - (det_age / 42.0).clamp(0.0, 1.0)).powf(1.10);
    let heat_norm = ((temp_k - 700.0) / 3400.0).clamp(0.0, 1.25);
    let hot_drive = (heat_norm * (0.20 + 0.80 * fire_life)).max(0.85 * fire_life);
    let gas_optical = (0.14 + 0.34 * fire_life) * (0.64 + 0.36 * rho_rel.clamp(0.0, 1.0));

    // Precomputed smooth value-noise helper
    #[inline(always)]
    fn value_noise(ux: i32, vy: i32, seed: u32) -> f32 {
        let tx = ux >> 16;
        let ty = vy >> 16;
        let fx = (ux & 0xFFFF) as f32 * (1.0 / 65536.0);
        let fy = (vy & 0xFFFF) as f32 * (1.0 / 65536.0);
        let sx = fx * fx * (3.0 - 2.0 * fx);
        let sy = fy * fy * (3.0 - 2.0 * fy);
        let h00 = (cell_hash(tx, ty, seed) & 0xFF) as f32 * (1.0 / 255.0);
        let h10 = (cell_hash(tx + 1, ty, seed) & 0xFF) as f32 * (1.0 / 255.0);
        let h01 = (cell_hash(tx, ty + 1, seed) & 0xFF) as f32 * (1.0 / 255.0);
        let h11 = (cell_hash(tx + 1, ty + 1, seed) & 0xFF) as f32 * (1.0 / 255.0);
        let a = h00 + (h10 - h00) * sx;
        let b = h01 + (h11 - h01) * sx;
        a + (b - a) * sy
    }

    let start_dx = (x0 as f32 + 0.5) - cx;
    let r_bound_sq = r_bound * r_bound;
    let inv_rb = 1.0 / r_bound.max(0.25);

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5) - cy;
        let dy2 = dy * dy;
        let row = (y - band_y0) * width;
        let mut u_fp = ((start_dx * cos_s + dy * sin_s) * tex_scale).round() as i32 + u_off;
        let mut v_fp = ((-start_dx * sin_s + dy * cos_s) * tex_scale).round() as i32 + v_off;

        for x in x0..=x1 {
            let dx = (x as f32 + 0.5) - cx;
            let d2 = dx * dx + dy2;
            let cu = u_fp;
            let cv = v_fp;
            u_fp = u_fp.wrapping_add(du_dx);
            v_fp = v_fp.wrapping_add(dv_dx);

            if d2 >= r_bound_sq {
                continue;
            }
            let r_lin = d2.sqrt() * inv_rb;

            // Smooth 2-octave fBm cloud turbulence warps the silhouette organically!
            let n1 = value_noise(cu, cv, seed);
            let n2 = value_noise(cu << 1, cv << 1, seed ^ 0x9E37_79B9);
            let billow = 0.65 * (1.0 - (2.0 * n1 - 1.0).abs()) + 0.35 * n2;
            let r_warped = (r_lin + 0.42 * (n1 - 0.5) + 0.22 * (n2 - 0.5)).clamp(0.0, 1.0);
            let omr = 1.0 - r_warped;
            let cloud_env = omr * omr * (1.0 + 2.0 * r_warped);

            let plume_density = (cloud_env * (0.68 + 0.52 * billow)).clamp(0.0, 1.0);
            if plume_density <= 0.025 {
                continue;
            }

            let alpha = (plume_density * gas_optical).clamp(0.0, 0.72);
            let a_u16 = (alpha * 256.0) as u16;
            let inv_a = 256 - a_u16;

            let out_rgb = if !rich_tex {
                let mod_f = 0.80 + 0.30 * billow;
                [
                    (tint_col[0] as f32 * mod_f).clamp(0.0, 255.0) as u8,
                    (tint_col[1] as f32 * mod_f).clamp(0.0, 255.0) as u8,
                    (tint_col[2] as f32 * mod_f).clamp(0.0, 255.0) as u8,
                ]
            } else {
                let t_local = (hot_drive * (0.35 + 0.82 * plume_density) * (0.40 + 0.60 * billow)
                    + 0.45 * young_flash * (1.0 - r_lin * r_lin))
                    .clamp(0.0, 1.35);

                let c = if t_local > 1.02 {
                    // Actinic plasma detonation front
                    lerp_rgb([1.00, 0.98, 0.88], [0.92, 0.97, 1.00], (t_local - 1.02) / 0.33)
                } else if t_local > 0.68 {
                    // Luminous gold-white fireball core
                    lerp_rgb([1.00, 0.70, 0.14], [1.00, 0.98, 0.88], (t_local - 0.68) / 0.34)
                } else if t_local > 0.38 {
                    // Fiery orange-crimson combustion waves
                    lerp_rgb([0.82, 0.20, 0.05], [1.00, 0.70, 0.14], (t_local - 0.38) / 0.30)
                } else if t_local > 0.18 {
                    // Ember-lit warm soot gradient
                    lerp_rgb([0.21, 0.19, 0.21], [0.82, 0.20, 0.05], (t_local - 0.18) / 0.20)
                } else {
                    // Soft translucent charcoal-slate smoke billow
                    let soot_l = 0.14 + 0.12 * billow;
                    [soot_l * 0.98, soot_l, soot_l * 1.05]
                };
                [
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                ]
            };

            let bg = band_px[row + x].to_array();
            band_px[row + x] = Color32::from_rgb(
                ((out_rgb[0] as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                ((out_rgb[1] as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                ((out_rgb[2] as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
            );
        }
    }
}

#[cfg(test)]
#[inline]
#[allow(clippy::too_many_arguments)]
fn splat_aa_disk_band(
    band_px: &mut [Color32],
    width: usize,
    band_y0: usize,
    band_y1: usize,
    cx: f32,
    cy: f32,
    r: f32,
    col: [u8; 3],
) {
    if !cx.is_finite()
        || !cy.is_finite()
        || !r.is_finite()
        || r <= 0.02
        || width == 0
        || band_y0 > band_y1
    {
        return;
    }
    if r < 0.72 {
        let ix = cx.floor() as isize;
        let iy = cy.floor() as isize;
        if ix >= 0
            && (ix as usize) < width
            && iy >= band_y0 as isize
            && iy <= band_y1 as isize
        {
            let local_y = (iy - band_y0 as isize) as usize;
            let idx = local_y * width + ix as usize;
            let coverage = (std::f32::consts::PI * r * r).clamp(0.14, 1.0);
            let a_u16 = (coverage * 256.0) as u16;
            let inv_a = 256 - a_u16;
            let bg = band_px[idx].to_array();
            band_px[idx] = Color32::from_rgb(
                ((col[0] as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                ((col[1] as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                ((col[2] as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
            );
        }
        return;
    }
    let r_pad = r + 0.75;
    let x0_i = (cx - r_pad).floor() as isize;
    let x1_i = (cx + r_pad).ceil() as isize;
    let y0_i = (cy - r_pad).floor() as isize;
    let y1_i = (cy + r_pad).ceil() as isize;
    let w_max = width as isize - 1;
    if x1_i < 0 || x0_i > w_max || y1_i < band_y0 as isize || y0_i > band_y1 as isize {
        return;
    }
    let x0 = x0_i.max(0) as usize;
    let x1 = x1_i.min(w_max) as usize;
    let y0 = (y0_i.max(band_y0 as isize)) as usize;
    let y1 = (y1_i.min(band_y1 as isize)) as usize;

    let r_inner = (r - 0.55).max(0.0);
    let r_inner_sq = r_inner * r_inner;
    let r_outer_sq = r_pad * r_pad;
    let solid = Color32::from_rgb(col[0], col[1], col[2]);

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5) - cy;
        let dy2 = dy * dy;
        let row = (y - band_y0) * width;
        for x in x0..=x1 {
            let dx = (x as f32 + 0.5) - cx;
            let d2 = dx * dx + dy2;
            if d2 <= r_inner_sq {
                band_px[row + x] = solid;
            } else if d2 < r_outer_sq {
                let d = d2.sqrt();
                let alpha = ((r_pad - d) / (r_pad - r_inner).max(0.5)).clamp(0.0, 1.0);
                let a_u16 = (alpha * 256.0) as u16;
                let inv_a = 256 - a_u16;
                let bg = band_px[row + x].to_array();
                band_px[row + x] = Color32::from_rgb(
                    ((col[0] as u16 * a_u16 + bg[0] as u16 * inv_a) >> 8) as u8,
                    ((col[1] as u16 * a_u16 + bg[1] as u16 * inv_a) >> 8) as u8,
                    ((col[2] as u16 * a_u16 + bg[2] as u16 * inv_a) >> 8) as u8,
                );
            }
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn splat_ember_streak_band(
    band_px: &mut [Color32],
    width: usize,
    band_y0: usize,
    band_y1: usize,
    hx: f32,
    hy: f32,
    tx: f32,
    ty: f32,
    head_r: f32,
    head_col: [u8; 3],
) {
    if !hx.is_finite()
        || !hy.is_finite()
        || !tx.is_finite()
        || !ty.is_finite()
        || head_r <= 0.15
        || width == 0
        || band_y0 > band_y1
    {
        return;
    }
    let abx = tx - hx;
    let aby = ty - hy;
    let ab2 = abx * abx + aby * aby;
    if ab2 <= 0.25 {
        return;
    }
    let inv_ab2 = 1.0 / ab2;
    let pad = head_r + 1.0;
    let x0_i = (hx.min(tx) - pad).floor() as isize;
    let x1_i = (hx.max(tx) + pad).ceil() as isize;
    let y0_i = (hy.min(ty) - pad).floor() as isize;
    let y1_i = (hy.max(ty) + pad).ceil() as isize;
    let w_max = width as isize - 1;
    if x1_i < 0 || x0_i > w_max || y1_i < band_y0 as isize || y0_i > band_y1 as isize {
        return;
    }
    let x0 = x0_i.max(0) as usize;
    let x1 = x1_i.min(w_max) as usize;
    let y0 = y0_i.max(band_y0 as isize) as usize;
    let y1 = y1_i.min(band_y1 as isize) as usize;

    // Warm ember glow fading smoothly from head color down toward luminous orange-gold wake
    let tail_col = [
        ((head_col[0] as f32 * 0.65 + 245.0 * 0.35).min(255.0)) as u8,
        ((head_col[1] as f32 * 0.60 + 135.0 * 0.40).min(255.0)) as u8,
        ((head_col[2] as f32 * 0.45 + 35.0 * 0.55).min(255.0)) as u8,
    ];

    for y in y0..=y1 {
        let py = (y as f32 + 0.5) - hy;
        let row = (y - band_y0) * width;
        for x in x0..=x1 {
            let px = (x as f32 + 0.5) - hx;
            let t = ((px * abx + py * aby) * inv_ab2).clamp(0.0, 1.0);
            let dx = px - t * abx;
            let dy = py - t * aby;
            let w_r = (head_r * (1.0 - 0.72 * t)).max(0.45);
            let d2 = dx * dx + dy * dy;
            if d2 < w_r * w_r {
                let radial = (1.0 - d2 / (w_r * w_r)).sqrt();
                let axial = 1.0 - t;
                let glow = (radial * axial * 0.62).clamp(0.0, 0.65);
                let g_u16 = (glow * 256.0) as u16;
                let bg = band_px[row + x].to_array();
                band_px[row + x] = Color32::from_rgb(
                    (bg[0] as u16 + ((tail_col[0] as u16 * g_u16) >> 8)).min(255) as u8,
                    (bg[1] as u16 + ((tail_col[1] as u16 * g_u16) >> 8)).min(255) as u8,
                    (bg[2] as u16 + ((tail_col[2] as u16 * g_u16) >> 8)).min(255) as u8,
                );
            }
        }
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn splat_additive_halo_band(
    band_px: &mut [Color32],
    width: usize,
    band_y0: usize,
    band_y1: usize,
    cx: f32,
    cy: f32,
    r: f32,
    col: [u8; 3],
    intensity: f32,
) {
    if !cx.is_finite()
        || !cy.is_finite()
        || !r.is_finite()
        || width == 0
        || band_y0 > band_y1
    {
        return;
    }
    let x0_i = (cx - r).floor() as isize;
    let x1_i = (cx + r).ceil() as isize;
    let y0_i = (cy - r).floor() as isize;
    let y1_i = (cy + r).ceil() as isize;
    let w_max = width as isize - 1;
    if x1_i < 0 || x0_i > w_max || y1_i < band_y0 as isize || y0_i > band_y1 as isize {
        return;
    }
    let x0 = x0_i.max(0) as usize;
    let x1 = x1_i.min(w_max) as usize;
    let y0 = (y0_i.max(band_y0 as isize)) as usize;
    let y1 = (y1_i.min(band_y1 as isize)) as usize;
    let r2 = r * r;

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5) - cy;
        let dy2 = dy * dy;
        let row = (y - band_y0) * width;
        for x in x0..=x1 {
            let dx = (x as f32 + 0.5) - cx;
            let d2 = dx * dx + dy2;
            if d2 < r2 {
                let falloff = 1.0 - d2 / r2;
                let w = (falloff * falloff * intensity * 255.0) as u16;
                let bg = band_px[row + x].to_array();
                band_px[row + x] = Color32::from_rgb(
                    (bg[0] as u16 + ((col[0] as u16 * w) >> 8)).min(255) as u8,
                    (bg[1] as u16 + ((col[1] as u16 * w) >> 8)).min(255) as u8,
                    (bg[2] as u16 + ((col[2] as u16 * w) >> 8)).min(255) as u8,
                );
            }
        }
    }
}

#[inline(always)]
fn cell_hash(ix: i32, iy: i32, seed: u32) -> u32 {
    let mut h = (ix as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((iy as u32).wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

/// Crisp nearest-neighbor pixelated material texture in body-locked crumb texel coordinates `(tx, ty)`.
#[inline]
#[allow(clippy::too_many_arguments)]
fn crumb_pixel_tex_rgb(
    mat: MaterialId,
    tx: i32,
    ty: i32,
    tex_h: u32,
    seed: u32,
    dmg: f32,
    tex_w: i32,
    rich_tex: bool,
) -> [i16; 3] {
    let n0 = (tex_h & 0xFF) as i32 - 128; // -128..+127 crisp per-texel noise
    if !rich_tex {
        let dl = (((n0 * 11) >> 7) * tex_w) >> 8;
        return [dl as i16, dl as i16, dl as i16];
    }

    let sub = ((tex_h >> 8) & 0xFF) as i32; // 0..255 independent mask
    let fine = ((tex_h >> 16) & 0xFF) as i32 - 128;

    let (mut dr, mut dg, mut db) = match mat {
        MaterialId::RhaSteel | MaterialId::HhaSteel | MaterialId::MildSteel => {
            // Crisp pixelated 2x1 rolled-steel striations + single-texel carbide flecks
            let roll_h = cell_hash(tx >> 1, ty, seed);
            let roll = (roll_h & 0xFF) as i32 - 128;
            let amp = if mat == MaterialId::HhaSteel { 26 } else { 20 };
            let fleck = if (sub & 7) == 0 { fine >> 3 } else { 0 };
            let dl = ((roll * amp) >> 7) + fleck;
            (dl - 1, dl, dl + 2)
        }
        MaterialId::Aluminum | MaterialId::Titanium => {
            // Crisp brushed 2x1 metallic slats + bright oxide pixel glints
            let slat_h = cell_hash(tx.div_euclid(2), ty, seed);
            let slat = (slat_h & 0xFF) as i32 - 128;
            let glint = if (sub & 15) == 0 { 16 } else { 0 };
            let dl = ((slat * 18) >> 7) + glint;
            (dl, dl, dl + 2)
        }
        MaterialId::PlanetaryIron => {
            // Crisp pixelated Widmanstatten octahedral checkerboard lamellae
            let diag = if ((tx + ty) & 1) == 0 { 16 } else { -16 };
            let dl = diag + ((n0 * 10) >> 7);
            (dl, dl, dl + 2)
        }
        MaterialId::Tungsten | MaterialId::DepletedUranium | MaterialId::Lead => {
            // Crisp 2x2 pixelated sintered heavy-metal spheres in dark binder pixels
            let gh = cell_hash(tx >> 1, ty >> 1, seed);
            let g_shade = (((gh & 0xFF) as i32 - 128) * 14) >> 7;
            let checker = if ((tx ^ ty) & 1) == 0 { 16 } else { -20 };
            let dl = checker + g_shade;
            if mat == MaterialId::DepletedUranium {
                (dl + 3, dl + 2, dl - 4)
            } else {
                (dl, dl, dl)
            }
        }
        MaterialId::Copper => {
            // Crisp diagonal pixelated annealing twin bands + warm copper grain
            let twin_idx = (tx + (ty << 1) + ((seed as i32) & 7)) & 3;
            let twin = match twin_idx {
                0 => 20,
                1 => -15,
                2 => 10,
                _ => -21,
            };
            let dl = twin + ((n0 * 9) >> 7);
            (dl + 5, dl, dl - 6)
        }
        MaterialId::BoronCarbide | MaterialId::SiliconCarbide => {
            // Sharp single-texel hot-pressed carbide mosaic + sparkle pixels
            let spark = if (sub & 15) == 0 { 24 } else { 0 };
            let dl = ((n0 * 24) >> 7) + spark;
            if mat == MaterialId::SiliconCarbide {
                (dl - 2, dl + 1, dl + 4)
            } else {
                (dl, dl, dl + 3)
            }
        }
        MaterialId::ArmorGlass => {
            // Crisp diagonal vitreous highlight pixel stripes
            let sheen = if ((tx - ty + ((seed as i32) & 3)) & 3) == 0 {
                24
            } else {
                (n0 * 12) >> 7
            };
            (sheen - 2, sheen + 3, sheen + 6)
        }
        MaterialId::Concrete => {
            // Crisp 2x2 limestone/quartz/basalt aggregate pebbles + 1-texel dark mortar pores
            let gh = cell_hash(tx >> 1, ty >> 1, seed);
            if (gh & 7) >= 5 {
                let g = (n0 * 8) >> 7;
                match (gh >> 3) & 3 {
                    0 | 1 => (28 + g, 24 + g, 14 + g), // warm limestone pebble
                    2 => (34 + g, 34 + g, 32 + g),     // pale quartz pebble
                    _ => (-26 + g, -24 + g, -20 + g),  // dark basalt pebble
                }
            } else if (sub & 15) == 0 {
                // Dark 1-texel air-void cement pore
                (-42, -42, -40)
            } else {
                let sand = (n0 * 20) >> 7;
                (sand, sand, sand + 1)
            }
        }
        MaterialId::BasaltRock => {
            // Volcanic vesicular pores + pale plagioclase mineral flecks
            if (sub & 15) == 0 {
                (-40, -40, -38)
            } else if (sub & 15) >= 14 {
                (26, 27, 29)
            } else {
                let d = (n0 * 22) >> 7;
                (d, d, d - 1)
            }
        }
        MaterialId::WaterIce => {
            // Crisp frost-vein diagonal pixels + glacial sparkle
            let vein = if ((tx ^ (ty << 1)) & 3) == 0 { 22 } else { -6 };
            let dl = vein + ((n0 * 12) >> 7);
            (dl - 4, dl + 2, dl + 8)
        }
        MaterialId::AramidLiner | MaterialId::Textolite => {
            // Authentic pixelated over-under Kevlar / fiberglass basket weave!
            let is_warp = ((tx ^ ty) & 1) == 0;
            let fiber_shade = if is_warp { 22 } else { -18 };
            let yarn = ((n0 * 8) >> 7) + fiber_shade;
            if mat == MaterialId::AramidLiner {
                (yarn + 4, yarn + 2, yarn - 5)
            } else {
                (yarn + 3, yarn + 1, yarn - 2)
            }
        }
        MaterialId::RubberNera => {
            // Matte vulcanized pixel stipple
            let dl = (n0 * 15) >> 7;
            (dl, dl, dl + 1)
        }
        MaterialId::CompB | MaterialId::TntExplosive => {
            // Blocky RDX/TNT crystal platelet pixels
            let dl = (n0 * 24) >> 7;
            (dl + 4, dl + 1, dl - 4)
        }
        MaterialId::C4Explosive => {
            // Molded putty pixel mottle
            let dl = (n0 * 16) >> 7;
            (dl + 1, dl + 1, dl)
        }
        MaterialId::ImpactFuze | MaterialId::DelayFuze => {
            // Precision machined checkerboard knurling
            let knurl = if ((tx ^ ty) & 1) == 0 { 18 } else { -18 };
            let dl = knurl + ((n0 * 8) >> 7);
            (dl + 2, dl + 1, dl - 2)
        }
    };

    // Connected jagged 1-texel micro-crack bolts across damaged crumbs!
    if dmg > 0.16 {
        let wobble_x = ((cell_hash(0, ty, seed ^ 0xC4AC_1111) & 3) as i32) - 1;
        let on_primary = (tx - wobble_x) == 0;
        let on_branch = dmg > 0.45
            && (ty - (((cell_hash(tx, 1, seed ^ 0xC4AC_2222) & 3) as i32) - 1)) == 0;
        if on_primary || on_branch {
            let cut = 34 + (22.0 * dmg) as i32;
            dr -= cut;
            dg -= cut;
            db -= cut;
        }
    }

    [
        ((dr * tex_w) >> 8) as i16,
        ((dg * tex_w) >> 8) as i16,
        ((db * tex_w) >> 8) as i16,
    ]
}

trait BgFill {
    fn assign_bg(&mut self, len: usize);
}

impl BgFill for Vec<Color32> {
    fn assign_bg(&mut self, len: usize) {
        let bg = Color32::from_rgb(11, 14, 20);
        if self.len() != len {
            self.resize(len, bg);
        }
        self.fill(bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splat_offscreen_edges_never_hang_or_panic() {
        let mut canvas = PixelCanvas::new();
        canvas.width = 320;
        canvas.height = 240;
        canvas.pixels.assign_bg(320 * 240);

        let tpls = crumb_templates();
        for (idx, &(cx, cy)) in [
            (-12.0, 120.0),
            (160.0, -12.0),
            (-12.0, -12.0),
            (330.0, 250.0),
            (160.0, 120.0),
            (f32::NAN, 100.0),
            (100.0, f32::INFINITY),
        ]
        .iter()
        .enumerate()
        {
            for &r in &[0.30, 5.5, 28.0] {
                canvas.splat_aa_disk(cx, cy, r, [255, 180, 90]);
                canvas.splat_explosion_plume(
                    cx,
                    cy,
                    r * 1.4,
                    idx as f32 * 1.6,
                    0.45,
                    3800.0 - idx as f32 * 350.0,
                    0xDEAD_BEEFu32.wrapping_add(idx as u32 * 31),
                    [255, 180, 90],
                    true,
                );
                for &mat in &MaterialId::ALL {
                    canvas.splat_crumb_polygon(
                        cx,
                        cy,
                        r,
                        idx as f32 * 0.7,
                        &tpls[(idx * 9 + mat as usize) & 127],
                        mat,
                        0x1234_5678u32.wrapping_add(mat as u32 * 977),
                        0.35,
                        true,
                        [190, 200, 215],
                        13,
                    );
                }
                canvas.splat_additive_halo(cx, cy, 8.0, [255, 180, 90], 0.25);
            }
        }
        canvas.draw_line(Vec2::new(-50.0, -50.0), Vec2::new(-10.0, -10.0), [200, 200, 200], 175);
        canvas.draw_line(Vec2::new(-10.0, 50.0), Vec2::new(50.0, 50.0), [200, 200, 200], 175);
    }

    #[test]
    fn crumb_world_size_is_zoom_invariant() {
        let mut world = ContinuumWorld::new(0.36);
        world.push_particle(Vec2::ZERO, Vec2::ZERO, MaterialId::Concrete, 1, None);

        let mut canvas = PixelCanvas::new();
        let measure_crumb_area = |canvas: &mut PixelCanvas, zoom: f32| -> usize {
            let cam = Camera2D {
                center: Vec2::ZERO,
                zoom,
            };
            let empty = ContinuumWorld::new(0.36);
            canvas.render(&empty, &cam, 512, 512, VisMode::MaterialThermal, false);
            let grid_ref = canvas.pixels.clone();

            canvas.render(&world, &cam, 512, 512, VisMode::MaterialThermal, false);
            canvas
                .pixels
                .iter()
                .zip(grid_ref.iter())
                .filter(|(a, b)| a != b)
                .count()
        };

        let area_z25 = measure_crumb_area(&mut canvas, 25.0) as f32;
        let area_z100 = measure_crumb_area(&mut canvas, 100.0) as f32;
        let ratio = area_z100 / area_z25;
        assert!(
            (13.0..=19.0).contains(&ratio),
            "Crumb world size changed across zoom! area_z25={area_z25}, area_z100={area_z100}, ratio={ratio:.2} (expected ~16.0)"
        );
    }

    #[test]
    fn crumb_textures_are_crisp_pixelated_blocks() {
        let mut canvas = PixelCanvas::new();
        canvas.width = 160;
        canvas.height = 160;
        canvas.pixels.assign_bg(160 * 160);
        let tpls = crumb_templates();
        // Large zoomed-in Kevlar crumb rotated at 0 rad (texel block width = 30 / 5 = 6 px!)
        canvas.splat_crumb_polygon(
            80.0,
            80.0,
            30.0,
            0.0,
            &tpls[0],
            MaterialId::AramidLiner,
            0x0102_0304,
            0.0,
            true,
            [180, 155, 65],
            12,
        );
        // Check a horizontal line across the center: transitions should be crisp blocky jumps between distinct texels,
        // with repeated identical colors inside each 6-px block!
        let row = 80 * 160;
        let center_run: Vec<Color32> = canvas.pixels[row + 70..row + 92].to_vec();
        let identical_pairs = center_run.windows(2).filter(|w| w[0] == w[1]).count();
        let sharp_jumps = center_run.windows(2).filter(|w| w[0] != w[1]).count();
        assert!(
            identical_pairs >= 12 && sharp_jumps >= 2,
            "Expected crisp pixelated blocks inside zoomed crumb, got identical_pairs={identical_pairs}, jumps={sharp_jumps}"
        );
    }

    #[test]
    fn explosion_plume_has_turbulent_fireball_structure_and_fades_cleanly() {
        let mut canvas = PixelCanvas::new();
        canvas.width = 160;
        canvas.height = 160;
        canvas.pixels.assign_bg(160 * 160);

        canvas.splat_explosion_plume(
            80.0,
            80.0,
            24.0,
            0.8,
            0.85,
            4600.0,
            0xA1B2_C3D4,
            [255, 220, 110],
            true,
        );

        let mut distinct = std::collections::HashSet::new();
        for y in 62..98 {
            for x in 62..98 {
                let p = canvas.pixels[y * 160 + x];
                distinct.insert((p.r(), p.g(), p.b()));
            }
        }
        assert!(
            distinct.len() > 25,
            "Expected rich turbulent explosion texture, got only {} colors",
            distinct.len()
        );
    }
}
