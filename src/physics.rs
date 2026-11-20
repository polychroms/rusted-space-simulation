use std::sync::atomic::{AtomicU64, Ordering};

use glam::Vec2;
use rayon::prelude::*;

use crate::materials::{MaterialId, MATERIALS};

static NEXT_WORLD_ID: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuParticle {
    pub pos_vel: [f32; 4],  // x, y, vx, vy
    pub acc_xsph: [f32; 4], // ax, ay, xsph_x, xsph_y
    pub thermo: [f32; 4],   // rho, rho0, energy, temp
    pub stress: [f32; 4],   // s_xx, s_yy, s_xy, sph_pressure
    pub mech: [f32; 4],     // plastic_strain, damage, net_pressure, mass
    pub det_aux: [f32; 4],  // det_timer, det_age, shock_rate, pad
    pub grav_pad: [f32; 4], // grav_x, grav_y, 0, 0
    pub meta: [u32; 4],     // mat_id, body_id, det_state, flags
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuMaterial {
    pub eos: [f32; 4],       // rho0, c0, hugoniot_s, gamma0
    pub strength: [f32; 4],  // shear_mod, yield_strength, hardening, pressure_hardening
    pub fracture: [f32; 4],  // spall_strength, fracture_strain, melt_temp, cv
    pub energetic: [f32; 4], // detonation_vel, detonation_energy, initiation_pressure, delay_us
    pub flags: [f32; 4],     // friction, is_energetic, is_ductile, youngs_mod
    pub jwl_ab: [f32; 4],    // jwl_a, jwl_b, jwl_r1, jwl_r2
    pub jc_jwl: [f32; 4],    // jwl_omega, jc_rate_c, thermal_m, poisson_nu
    pub gravity: [f32; 4],   // grav_charge_mult, pad, pad, pad
}

impl GpuMaterial {
    pub fn table() -> [GpuMaterial; 34] {
        let mut out = [GpuMaterial::default(); 34];
        for (idx, m) in MATERIALS.iter().enumerate() {
            let mat_id = MaterialId::from_index(idx);
            let jwl = mat_id.jwl_params();
            out[idx] = GpuMaterial {
                eos: [m.rho0, m.c0, m.hugoniot_s, m.gamma0],
                strength: [
                    m.shear_modulus(),
                    m.yield_strength,
                    m.hardening,
                    m.pressure_hardening,
                ],
                fracture: [m.spall_strength, m.fracture_strain, m.melt_temp, m.cv],
                energetic: [
                    m.detonation_vel,
                    m.detonation_energy,
                    m.initiation_pressure,
                    m.delay_us,
                ],
                flags: [
                    m.friction,
                    if m.is_energetic { 1.0 } else { 0.0 },
                    if m.is_ductile() { 1.0 } else { 0.0 },
                    m.youngs_mod,
                ],
                jwl_ab: [jwl[0], jwl[1], jwl[2], jwl[3]],
                jc_jwl: [
                    jwl[4],
                    mat_id.jc_rate_c(),
                    mat_id.thermal_m(),
                    m.poisson_ratio(),
                ],
                gravity: [mat_id.grav_charge_mult(), 0.0, 0.0, 0.0],
            };
        }
        out
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TelemetrySample {
    pub time_us: f32,
    pub kinetic_energy: f32,
    pub internal_energy: f32,
    pub elastic_energy: f32,
    pub peak_pressure_gpa: f32,
    pub max_speed_kms: f32,
    pub fractured_frac: f32,
    pub momentum: Vec2,
}

#[derive(Clone)]
pub struct WorldSnapshot {
    pub time_us: f32,
    pub step_count: u64,
    pub last_dt: f32,
    pub pos: Vec<Vec2>,
    pub vel: Vec<Vec2>,
    pub acc: Vec<Vec2>,
    pub xsph_dv: Vec<Vec2>,
    pub mass: Vec<f32>,
    pub rho0: Vec<f32>,
    pub rho: Vec<f32>,
    pub s_xx: Vec<f32>,
    pub s_yy: Vec<f32>,
    pub s_xy: Vec<f32>,
    pub sph_pressure: Vec<f32>,
    pub net_pressure: Vec<f32>,
    pub energy: Vec<f32>,
    pub temp: Vec<f32>,
    pub plastic_strain: Vec<f32>,
    pub damage: Vec<f32>,
    pub shock_rate: Vec<f32>,
    pub mat: Vec<MaterialId>,
    pub body_id: Vec<u16>,
    pub det_state: Vec<u8>,
    pub det_timer: Vec<f32>,
    pub det_age: Vec<f32>,
    pub crumb_angle: Vec<f32>,
    pub crumb_spin: Vec<f32>,
    pub crumb_seed: Vec<u32>,
    pub fractured_count: usize,
    pub melted_count: usize,
    pub detonated_count: usize,
    pub next_body_id: u16,
    pub history_len: usize,
}

#[derive(Clone, Copy, Default)]
struct GravCell {
    mass: f32,
    com: Vec2,
}

#[derive(Clone, Copy, Default)]
struct ContinuumStepOut {
    acc: Vec2,
    xsph: Vec2,
    drho: f32,
    de: f32,
    shock_comp: f32,
    symp_burn_dt: f32,
    grad_v: [f32; 4], // dvx_dx, dvx_dy, dvy_dx, dvy_dy
    fric_alpha: f32,
    neighbors: u16,
}

pub struct ContinuumWorld {
    /// Particle spacing dx (cm)
    pub dx: f32,
    /// SPH smoothing length h (cm) and cutoff radius H = 2*h (cm)
    pub h: f32,
    pub cutoff: f32,
    pub self_gravity: bool,
    pub grav_g: f32,
    pub down_gravity: f32,

    // --- Continuum Elasto-Plastic Particle SoA arrays ---
    pub pos: Vec<Vec2>,
    pub vel: Vec<Vec2>,
    pub acc: Vec<Vec2>,
    pub grav_acc: Vec<Vec2>,
    pub xsph_dv: Vec<Vec2>,
    pub mass: Vec<f32>,
    pub rho0: Vec<f32>,
    pub rho: Vec<f32>,
    /// Deviatoric Cauchy stress tensor components (Mbar)
    pub s_xx: Vec<f32>,
    pub s_yy: Vec<f32>,
    pub s_xy: Vec<f32>,
    /// Isotropic EOS pressure (Mbar; negative allowed in intact tensile solids)
    pub sph_pressure: Vec<f32>,
    /// Net mean pressure -1/2*(sigma_xx + sigma_yy) (Mbar; positive = compression, negative = tension)
    pub net_pressure: Vec<f32>,
    /// Specific internal energy e ((cm/µs)²)
    pub energy: Vec<f32>,
    /// Temperature (Kelvin)
    pub temp: Vec<f32>,
    /// Accumulated equivalent plastic strain
    pub plastic_strain: Vec<f32>,
    /// Continuum fracture / spall damage D in [0, 1]
    pub damage: Vec<f32>,
    /// Local shock compression rate for shockwave rendering
    pub shock_rate: Vec<f32>,
    pub mat: Vec<MaterialId>,
    pub body_id: Vec<u16>,
    /// Detonation state: 0 = unreacted solid, 1 = triggered/delay burning, 2 = reacted detonation gas
    pub det_state: Vec<u8>,
    pub det_timer: Vec<f32>,
    pub det_age: Vec<f32>,
    /// Procedural angular grain orientation (radians), angular velocity (rad/µs), and deterministic seed
    pub crumb_angle: Vec<f32>,
    pub crumb_spin: Vec<f32>,
    pub crumb_seed: Vec<u32>,

    // --- Scratch & Spatial Hash ---
    step_scratch: Vec<ContinuumStepOut>,
    pub grid_cell_size: f32,
    grid_mask: usize,
    grid_head: Vec<i32>,
    grid_next: Vec<i32>,

    // --- GPU synchronization & render cache invalidation ---
    pub gpu_dirty: bool,
    pub world_id: u64,
    pub state_epoch: u64,

    // --- Simulation clock & telemetry ---
    pub time_us: f32,
    pub step_count: u64,
    pub last_dt: f32,
    pub fractured_count: usize,
    pub melted_count: usize,
    pub detonated_count: usize,
    pub next_body_id: u16,
    pub history: Vec<TelemetrySample>,
    last_history_time: f32,
}

impl ContinuumWorld {
    pub fn new(dx: f32) -> Self {
        let h = 1.35 * dx;
        let cutoff = 2.0 * h; // 2.70 * dx
        let buckets = 131_072usize;

        Self {
            dx,
            h,
            cutoff,
            self_gravity: false,
            grav_g: 0.0,
            down_gravity: 0.0,
            pos: Vec::new(),
            vel: Vec::new(),
            acc: Vec::new(),
            grav_acc: Vec::new(),
            xsph_dv: Vec::new(),
            mass: Vec::new(),
            rho0: Vec::new(),
            rho: Vec::new(),
            s_xx: Vec::new(),
            s_yy: Vec::new(),
            s_xy: Vec::new(),
            sph_pressure: Vec::new(),
            net_pressure: Vec::new(),
            energy: Vec::new(),
            temp: Vec::new(),
            plastic_strain: Vec::new(),
            damage: Vec::new(),
            shock_rate: Vec::new(),
            mat: Vec::new(),
            body_id: Vec::new(),
            det_state: Vec::new(),
            det_timer: Vec::new(),
            det_age: Vec::new(),
            crumb_angle: Vec::new(),
            crumb_spin: Vec::new(),
            crumb_seed: Vec::new(),
            step_scratch: Vec::new(),
            grid_cell_size: cutoff,
            grid_mask: buckets - 1,
            grid_head: vec![-1; buckets],
            grid_next: Vec::new(),
            gpu_dirty: true,
            world_id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            state_epoch: 0,
            time_us: 0.0,
            step_count: 0,
            last_dt: 0.04,
            fractured_count: 0,
            melted_count: 0,
            detonated_count: 0,
            next_body_id: 1,
            history: Vec::with_capacity(400),
            last_history_time: -10.0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    #[inline]
    pub fn von_mises_gpa(&self, i: usize) -> f32 {
        let sx = self.s_xx[i];
        let sy = self.s_yy[i];
        let sz = -(sx + sy);
        let sxy = self.s_xy[i];
        let j2 = 0.5 * (sx * sx + sy * sy + sz * sz) + sxy * sxy;
        (3.0 * j2).max(0.0).sqrt() * 100.0
    }

    #[cfg(test)]
    pub fn debug_max_cell_count(&mut self) -> usize {
        self.rebuild_spatial_grid();
        let mut max_c = 0usize;
        for &h in &self.grid_head {
            let mut cur = h;
            let mut cnt = 0usize;
            while cur >= 0 {
                cnt += 1;
                cur = self.grid_next[cur as usize];
            }
            if cnt > max_c {
                max_c = cnt;
            }
        }
        max_c
    }

    pub fn alloc_body_id(&mut self) -> u16 {
        let id = self.next_body_id;
        self.next_body_id = self.next_body_id.wrapping_add(1).max(1);
        id
    }

    /// Area per particle in hexagonal close packing: (sqrt(3)/2) * dx²
    #[inline]
    pub fn particle_area(&self) -> f32 {
        0.866_025_4 * self.dx * self.dx
    }

    pub fn push_particle(
        &mut self,
        pos: Vec2,
        vel: Vec2,
        mat: MaterialId,
        body_id: u16,
        temp_override: Option<f32>,
    ) -> usize {
        if mat.is_gravity_material() {
            self.self_gravity = true;
            if self.grav_g <= 0.0 {
                self.grav_g = 7.5e-5;
            }
        }
        let props = mat.props();
        let m = props.rho0 * self.particle_area();
        let t0 = temp_override.unwrap_or(if mat == MaterialId::WaterIce {
            150.0
        } else {
            293.0
        });

        let idx = self.pos.len();
        self.pos.push(pos);
        self.vel.push(vel);
        self.acc.push(Vec2::ZERO);
        self.grav_acc.push(Vec2::ZERO);
        self.xsph_dv.push(Vec2::ZERO);
        self.mass.push(m);
        self.rho0.push(props.rho0);
        self.rho.push(props.rho0);
        self.s_xx.push(0.0);
        self.s_yy.push(0.0);
        self.s_xy.push(0.0);
        self.sph_pressure.push(0.0);
        self.net_pressure.push(0.0);
        self.energy.push(0.0);
        self.temp.push(t0);
        self.plastic_strain.push(0.0);
        self.damage.push(0.0);
        self.shock_rate.push(0.0);
        self.mat.push(mat);
        self.body_id.push(body_id);
        self.det_state.push(0);
        self.det_timer.push(0.0);
        self.det_age.push(0.0);
        let mut seed = (idx as u32)
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add((body_id as u32).wrapping_mul(0x85EB_CA6B))
            ^ ((mat as u32).wrapping_mul(0xC2B2_AE35));
        seed ^= seed >> 16;
        seed = seed.wrapping_mul(0x7FEB_352D);
        seed ^= seed >> 15;
        let init_angle = ((seed & 0xFFFF) as f32 / 65535.0) * std::f32::consts::TAU;
        self.crumb_angle.push(init_angle);
        self.crumb_spin.push(0.0);
        self.crumb_seed.push(seed);
        self.gpu_dirty = true;
        self.state_epoch = self.state_epoch.wrapping_add(1);
        idx
    }

    /// Unify continuum body cohesion across welded multi-material structures.
    pub fn bond_range(&mut self, start_idx: usize, end_idx: usize, weld_across_materials: bool) {
        if weld_across_materials && end_idx > start_idx + 1 {
            let unified = self.body_id[start_idx];
            for b in &mut self.body_id[start_idx..end_idx] {
                *b = unified;
            }
        }
        self.gpu_dirty = true;
        self.state_epoch = self.state_epoch.wrapping_add(1);
    }

    #[inline]
    fn hash_cell(cx: i32, cy: i32, mask: usize) -> usize {
        let ux = (cx as u32).wrapping_mul(0x9E37_79B1);
        let uy = (cy as u32).wrapping_mul(0x85EB_CA77);
        let h = ux ^ uy.wrapping_add(0xC2B2_AE3D).wrapping_add((ux << 6) | (ux >> 2));
        (h as usize) & mask
    }

    fn rebuild_spatial_grid(&mut self) {
        let n = self.pos.len();
        self.grid_head.fill(-1);
        self.grid_next.resize(n, -1);
        let inv_cell = 1.0 / self.grid_cell_size;
        let mask = self.grid_mask;
        for i in 0..n {
            let p = self.pos[i];
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            let cx = (p.x * inv_cell).floor() as i32;
            let cy = (p.y * inv_cell).floor() as i32;
            let bucket = Self::hash_cell(cx, cy, mask);
            self.grid_next[i] = self.grid_head[bucket];
            self.grid_head[bucket] = i as i32;
        }
    }

    /// Adaptive Courant-Friedrichs-Lewy (CFL) timestep including nonlinear shock stiffening & shear wave speed!
    pub fn compute_cfl_dt(&self) -> f32 {
        let n = self.pos.len();
        if n == 0 {
            return 0.05;
        }
        let calc_sig = |i: usize| -> f32 {
            let v = self.vel[i].length();
            let mp = self.mat[i].props();
            let det = self.det_state[i];
            let c_eff = if det == 2 {
                mp.detonation_vel.max(0.80)
            } else {
                let rho = self.rho[i].max(0.1);
                let p_pos = self.sph_pressure[i].max(0.0);
                let mu = (rho / mp.rho0.max(1e-4) - 1.0).max(0.0);
                let c_shock_sq = mp.c0 * mp.c0
                    + (1.0 + 2.0 * mp.hugoniot_s * mu) * p_pos / rho
                    + 1.3333 * mp.shear_modulus() / mp.rho0.max(0.1);
                let c_long = c_shock_sq.max(0.01).sqrt();
                if det == 1 {
                    c_long.max(mp.detonation_vel)
                } else {
                    c_long
                }
            };
            c_eff + 1.45 * v
        };

        let max_signal = if n >= 2048 {
            (0..n).into_par_iter().map(calc_sig).reduce(|| 0.45f32, f32::max)
        } else {
            (0..n).map(calc_sig).fold(0.45f32, f32::max)
        };
        (0.22 * self.h / max_signal).clamp(0.0010, 0.085)
    }

    /// Compute `[min_x, min_y, box_span, grav_g]` for GPU-resident 32x32 self-gravity grid!
    pub fn gravity_domain_params(&self) -> [f32; 4] {
        let n = self.pos.len();
        if !self.self_gravity || self.grav_g <= 0.0 || n == 0 {
            return [0.0, 0.0, 100.0, 0.0];
        }
        let stride = (n / 2048).max(1);
        let mut total_m = 0.0f32;
        let mut com = Vec2::ZERO;
        let mut i = 0usize;
        while i < n {
            let p = self.pos[i];
            if p.is_finite() {
                let m = self.mass[i];
                total_m += m;
                com += p * m;
            }
            i += stride;
        }
        if total_m <= 0.0 {
            return [-50.0, -50.0, 100.0, self.grav_g];
        }
        com /= total_m;
        let mut r2_sum = 0.0f32;
        let mut j = 0usize;
        while j < n {
            let p = self.pos[j];
            if p.is_finite() {
                r2_sum += self.mass[j] * (p - com).length_squared();
            }
            j += stride;
        }
        let r_rms = (r2_sum / total_m).sqrt();
        let half_span = (3.4 * r_rms).clamp(28.0, 600.0);
        [
            com.x - half_span,
            com.y - half_span,
            2.0 * half_span,
            self.grav_g,
        ]
    }

    pub fn capture_snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            time_us: self.time_us,
            step_count: self.step_count,
            last_dt: self.last_dt,
            pos: self.pos.clone(),
            vel: self.vel.clone(),
            acc: self.acc.clone(),
            xsph_dv: self.xsph_dv.clone(),
            mass: self.mass.clone(),
            rho0: self.rho0.clone(),
            rho: self.rho.clone(),
            s_xx: self.s_xx.clone(),
            s_yy: self.s_yy.clone(),
            s_xy: self.s_xy.clone(),
            sph_pressure: self.sph_pressure.clone(),
            net_pressure: self.net_pressure.clone(),
            energy: self.energy.clone(),
            temp: self.temp.clone(),
            plastic_strain: self.plastic_strain.clone(),
            damage: self.damage.clone(),
            shock_rate: self.shock_rate.clone(),
            mat: self.mat.clone(),
            body_id: self.body_id.clone(),
            det_state: self.det_state.clone(),
            det_timer: self.det_timer.clone(),
            det_age: self.det_age.clone(),
            crumb_angle: self.crumb_angle.clone(),
            crumb_spin: self.crumb_spin.clone(),
            crumb_seed: self.crumb_seed.clone(),
            fractured_count: self.fractured_count,
            melted_count: self.melted_count,
            detonated_count: self.detonated_count,
            next_body_id: self.next_body_id,
            history_len: self.history.len(),
        }
    }

    pub fn restore_snapshot(&mut self, snap: &WorldSnapshot, truncate_history: bool) {
        let n = snap.pos.len();
        self.time_us = snap.time_us;
        self.step_count = snap.step_count;
        self.last_dt = snap.last_dt;
        self.pos.clone_from(&snap.pos);
        self.vel.clone_from(&snap.vel);
        self.acc.clone_from(&snap.acc);
        self.xsph_dv.clone_from(&snap.xsph_dv);
        self.grav_acc.resize(n, Vec2::ZERO);
        self.step_scratch.resize(n, ContinuumStepOut::default());
        self.grid_next.resize(n, -1);
        self.mass.clone_from(&snap.mass);
        self.rho0.clone_from(&snap.rho0);
        self.rho.clone_from(&snap.rho);
        self.s_xx.clone_from(&snap.s_xx);
        self.s_yy.clone_from(&snap.s_yy);
        self.s_xy.clone_from(&snap.s_xy);
        self.sph_pressure.clone_from(&snap.sph_pressure);
        self.net_pressure.clone_from(&snap.net_pressure);
        self.energy.clone_from(&snap.energy);
        self.temp.clone_from(&snap.temp);
        self.plastic_strain.clone_from(&snap.plastic_strain);
        self.damage.clone_from(&snap.damage);
        self.shock_rate.clone_from(&snap.shock_rate);
        self.mat.clone_from(&snap.mat);
        self.body_id.clone_from(&snap.body_id);
        self.det_state.clone_from(&snap.det_state);
        self.det_timer.clone_from(&snap.det_timer);
        self.det_age.clone_from(&snap.det_age);
        self.crumb_angle.clone_from(&snap.crumb_angle);
        self.crumb_spin.clone_from(&snap.crumb_spin);
        self.crumb_seed.clone_from(&snap.crumb_seed);
        self.fractured_count = snap.fractured_count;
        self.melted_count = snap.melted_count;
        self.detonated_count = snap.detonated_count;
        self.next_body_id = snap.next_body_id;
        if truncate_history && self.history.len() > snap.history_len {
            self.history.truncate(snap.history_len);
            self.last_history_time = self.history.last().map_or(-10.0, |h| h.time_us);
        }
        self.gpu_dirty = true;
        self.state_epoch = self.state_epoch.wrapping_add(1);
    }

    /// Export particle state to GPU buffer slice (updating coarse self-gravity if enabled).
    pub fn export_gpu_particles(&mut self, dst: &mut [GpuParticle]) {
        let n = self.len().min(dst.len());
        if self.self_gravity && self.grav_g > 0.0 {
            self.update_self_gravity();
        } else {
            self.grav_acc.fill(Vec2::ZERO);
        }
        for (i, slot) in dst.iter_mut().enumerate().take(n) {
            *slot = GpuParticle {
                pos_vel: [self.pos[i].x, self.pos[i].y, self.vel[i].x, self.vel[i].y],
                acc_xsph: [
                    self.acc[i].x,
                    self.acc[i].y,
                    self.xsph_dv[i].x,
                    self.xsph_dv[i].y,
                ],
                thermo: [self.rho[i], self.rho0[i], self.energy[i], self.temp[i]],
                stress: [
                    self.s_xx[i],
                    self.s_yy[i],
                    self.s_xy[i],
                    self.sph_pressure[i],
                ],
                mech: [
                    self.plastic_strain[i],
                    self.damage[i],
                    self.net_pressure[i],
                    self.mass[i],
                ],
                det_aux: [
                    self.det_timer[i],
                    self.det_age[i],
                    self.shock_rate[i],
                    self.crumb_angle[i],
                ],
                grav_pad: [
                    self.grav_acc[i].x,
                    self.grav_acc[i].y,
                    self.crumb_spin[i],
                    0.0,
                ],
                meta: [
                    self.mat[i] as u32,
                    self.body_id[i] as u32,
                    self.det_state[i] as u32,
                    self.crumb_seed[i],
                ],
            };
        }
        self.gpu_dirty = false;
    }

    /// Import particle state back from HOST_CACHED staging RAM after a Vulkan compute batch.
    pub fn import_gpu_particles(&mut self, src: &[GpuParticle], substeps: usize, dt: f32) {
        let n = self.len().min(src.len());
        let (melted, detonated, fractured) = if n >= 2048 {
            const CHUNK: usize = 512;
            let src_s = &src[..n];
            let mat_ref = &self.mat[..n];
            let (_, ((m1, d1, f1), (m2, d2, f2))) = rayon::join(
                || {
                    rayon::join(
                        || {
                            self.pos[..n]
                                .par_chunks_mut(CHUNK)
                                .zip(self.vel[..n].par_chunks_mut(CHUNK))
                                .zip(self.acc[..n].par_chunks_mut(CHUNK))
                                .zip(self.xsph_dv[..n].par_chunks_mut(CHUNK))
                                .zip(self.rho[..n].par_chunks_mut(CHUNK))
                                .zip(self.energy[..n].par_chunks_mut(CHUNK))
                                .enumerate()
                                .for_each(|(c, (((((pos, vel), acc), xsph), rho), energy))| {
                                    let base = c * CHUNK;
                                    for k in 0..pos.len() {
                                        let p = &src_s[base + k];
                                        pos[k] = Vec2::new(p.pos_vel[0], p.pos_vel[1]);
                                        vel[k] = Vec2::new(p.pos_vel[2], p.pos_vel[3]);
                                        acc[k] = Vec2::new(p.acc_xsph[0], p.acc_xsph[1]);
                                        xsph[k] = Vec2::new(p.acc_xsph[2], p.acc_xsph[3]);
                                        rho[k] = p.thermo[0];
                                        energy[k] = p.thermo[2];
                                    }
                                });
                        },
                        || {
                            self.s_xx[..n]
                                .par_chunks_mut(CHUNK)
                                .zip(self.s_yy[..n].par_chunks_mut(CHUNK))
                                .zip(self.s_xy[..n].par_chunks_mut(CHUNK))
                                .zip(self.sph_pressure[..n].par_chunks_mut(CHUNK))
                                .zip(self.plastic_strain[..n].par_chunks_mut(CHUNK))
                                .zip(self.net_pressure[..n].par_chunks_mut(CHUNK))
                                .enumerate()
                                .for_each(|(c, (((((sxx, syy), sxy), sphp), ep), np))| {
                                    let base = c * CHUNK;
                                    for k in 0..sxx.len() {
                                        let p = &src_s[base + k];
                                        sxx[k] = p.stress[0];
                                        syy[k] = p.stress[1];
                                        sxy[k] = p.stress[2];
                                        sphp[k] = p.stress[3];
                                        ep[k] = p.mech[0];
                                        np[k] = p.mech[2];
                                    }
                                });
                        },
                    )
                },
                || {
                    rayon::join(
                        || {
                            self.temp[..n]
                                .par_chunks_mut(CHUNK)
                                .zip(self.damage[..n].par_chunks_mut(CHUNK))
                                .zip(self.det_state[..n].par_chunks_mut(CHUNK))
                                .enumerate()
                                .map(|(c, ((temp, dmg), det_s))| {
                                    let base = c * CHUNK;
                                    let mut m = 0usize;
                                    let mut d = 0usize;
                                    let mut f = 0usize;
                                    for k in 0..temp.len() {
                                        let idx = base + k;
                                        let p = &src_s[idx];
                                        let t = p.thermo[3];
                                        let dv = p.mech[1];
                                        let ds = p.meta[2] as u8;
                                        temp[k] = t;
                                        dmg[k] = dv;
                                        det_s[k] = ds;
                                        let mp = mat_ref[idx].props();
                                        if t >= mp.melt_temp {
                                            m += 1;
                                        }
                                        if ds == 2 {
                                            d += 1;
                                        } else if dv >= 0.70 {
                                            f += 1;
                                        }
                                    }
                                    (m, d, f)
                                })
                                .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2))
                        },
                        || {
                            self.det_timer[..n]
                                .par_chunks_mut(CHUNK)
                                .zip(self.det_age[..n].par_chunks_mut(CHUNK))
                                .zip(self.shock_rate[..n].par_chunks_mut(CHUNK))
                                .zip(self.crumb_angle[..n].par_chunks_mut(CHUNK))
                                .zip(self.crumb_spin[..n].par_chunks_mut(CHUNK))
                                .enumerate()
                                .for_each(|(c, ((((dt_buf, da), sr), ca), cs))| {
                                    let base = c * CHUNK;
                                    for k in 0..dt_buf.len() {
                                        let p = &src_s[base + k];
                                        dt_buf[k] = p.det_aux[0];
                                        da[k] = p.det_aux[1];
                                        sr[k] = p.det_aux[2];
                                        ca[k] = p.det_aux[3];
                                        cs[k] = p.grav_pad[2];
                                    }
                                });
                            (0usize, 0usize, 0usize)
                        },
                    )
                },
            );
            (m1 + m2, d1 + d2, f1 + f2)
        } else {
            let mut melted = 0usize;
            let mut detonated = 0usize;
            let mut fractured = 0usize;
            for (i, p) in src.iter().enumerate().take(n) {
                self.pos[i] = Vec2::new(p.pos_vel[0], p.pos_vel[1]);
                self.vel[i] = Vec2::new(p.pos_vel[2], p.pos_vel[3]);
                self.acc[i] = Vec2::new(p.acc_xsph[0], p.acc_xsph[1]);
                self.xsph_dv[i] = Vec2::new(p.acc_xsph[2], p.acc_xsph[3]);
                self.rho[i] = p.thermo[0];
                self.energy[i] = p.thermo[2];
                self.temp[i] = p.thermo[3];
                self.s_xx[i] = p.stress[0];
                self.s_yy[i] = p.stress[1];
                self.s_xy[i] = p.stress[2];
                self.sph_pressure[i] = p.stress[3];
                self.plastic_strain[i] = p.mech[0];
                self.damage[i] = p.mech[1];
                self.net_pressure[i] = p.mech[2];
                self.det_timer[i] = p.det_aux[0];
                self.det_age[i] = p.det_aux[1];
                self.shock_rate[i] = p.det_aux[2];
                self.crumb_angle[i] = p.det_aux[3];
                self.crumb_spin[i] = p.grav_pad[2];
                self.det_state[i] = p.meta[2] as u8;

                let mp = self.mat[i].props();
                if self.temp[i] >= mp.melt_temp {
                    melted += 1;
                }
                if self.det_state[i] == 2 {
                    detonated += 1;
                } else if self.damage[i] >= 0.70 {
                    fractured += 1;
                }
            }
            (melted, detonated, fractured)
        };

        self.melted_count = melted;
        self.detonated_count = detonated;
        self.fractured_count = fractured;
        self.last_dt = dt;
        self.time_us += dt * substeps as f32;
        self.step_count += substeps as u64;
        self.state_epoch = self.state_epoch.wrapping_add(1);

        if self.time_us - self.last_history_time >= 0.45 {
            self.record_telemetry();
            self.last_history_time = self.time_us;
        }
    }

    /// Parallel CPU Continuum Elasto-Plastic Cauchy-Stress solver (exact match to Vulkan shaders).
    pub fn step(&mut self, dt: f32) {
        let n = self.pos.len();
        if n == 0 {
            return;
        }
        self.last_dt = dt;
        if self.step_scratch.len() != n {
            self.step_scratch.resize(n, ContinuumStepOut::default());
        }

        // 1. Velocity-Verlet first half-kick + XSPH drift
        if self.step_count > 0 {
            let half_dt = 0.5 * dt;
            self.pos
                .par_iter_mut()
                .zip(self.vel.par_iter_mut())
                .zip(self.acc.par_iter())
                .zip(self.xsph_dv.par_iter())
                .for_each(|(((pos, vel), acc), xsph)| {
                    *vel += *acc * half_dt;
                    *pos += (*vel + *xsph) * dt;
                });
        }

        // 2. Spatial Hash Grid + Polynomial Mie-Grüneisen / JWL Gas EOS
        self.rebuild_spatial_grid();

        let rho_ref = &self.rho;
        let rho0_ref = &self.rho0;
        let e_ref = &self.energy;
        let temp_ref = &self.temp;
        let dmg_ref = &self.damage;
        let mat_ref = &self.mat;
        let det_ref = &self.det_state;

        self.sph_pressure
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, p_out)| {
                let mat_id = mat_ref[i];
                let m = mat_id.props();
                let rho = rho_ref[i];
                let rho0 = rho0_ref[i].max(1e-4);
                let e = e_ref[i];
                if det_ref[i] == 2 {
                    let v_rel = (rho0 / rho.max(1e-4)).clamp(0.52, 16.0);
                    let [a, b, r1, r2, omega] = mat_id.jwl_params();
                    let p_jwl = a * (1.0 - omega / (r1 * v_rel)) * (-r1 * v_rel).exp()
                        + b * (1.0 - omega / (r2 * v_rel)) * (-r2 * v_rel).exp()
                        + omega * rho * e;
                    *p_out = p_jwl.clamp(0.0, 0.95);
                } else {
                    let mu = (rho / rho0 - 1.0).clamp(-0.16, 0.72);
                    let k1 = rho0 * m.c0 * m.c0;
                    let p_cold = if mu >= 0.0 {
                        let s = m.hugoniot_s;
                        let k2 = k1 * (2.0 * s - 1.0);
                        let k3 = k1 * s * (3.0 * s - 1.0);
                        k1 * mu + k2 * mu * mu + k3 * mu * mu * mu
                    } else {
                        k1 * mu
                    };
                    let e_vap = (12.0 * m.cv * (m.melt_temp - 300.0).max(200.0)).max(0.05);
                    let gamma_eff = 0.45 + (m.gamma0 - 0.45) / (1.0 + e.max(0.0) / e_vap);
                    let vapor_frac = ((e - 0.45 * e_vap) / (0.55 * e_vap)).clamp(0.0, 1.0);
                    let cond_expand_gate = if rho >= rho0 {
                        1.0
                    } else {
                        ((rho / rho0 - 0.92) / 0.08).clamp(0.0, 1.0)
                    };
                    let th_gate = cond_expand_gate + (1.0 - cond_expand_gate) * vapor_frac;
                    let p_th = gamma_eff * rho.min(rho0) * e * th_gate;
                    let dmg = dmg_ref[i].clamp(0.0, 1.0);
                    let melt_soft =
                        ((temp_ref[i] - 300.0) / (m.melt_temp - 300.0).max(50.0)).clamp(0.0, 1.0);
                    let min_tens = -m.spall_strength * (1.0 - dmg) * (1.0 - melt_soft);
                    *p_out = (p_cold + p_th).clamp(min_tens, 8.00);
                }
            });

        // 3. Optional Planetary Self-Gravity
        if self.self_gravity && self.grav_g > 0.0 {
            if self.step_count.is_multiple_of(8) {
                self.update_self_gravity();
            }
        } else if self.step_count == 0 {
            self.grav_acc.fill(Vec2::ZERO);
        }

        // 4. Parallel Continuum Cauchy-Stress & Velocity-Gradient Pass
        let rho_ref = &self.rho;
        let rho0_ref = &self.rho0;
        let temp_ref = &self.temp;
        let dmg_ref = &self.damage;
        let mat_ref = &self.mat;
        let det_ref = &self.det_state;
        let dx_val = self.dx;
        let h = self.h;
        let h2 = h * h;
        let cutoff = self.cutoff;
        let cutoff_sq = cutoff * cutoff;
        let pi_c = std::f32::consts::PI;
        let w_norm = 7.0 / (pi_c * cutoff_sq);
        let dw_norm = -140.0 / (pi_c * cutoff_sq * cutoff);
        let q_ref = (dx_val / cutoff).clamp(0.05, 0.95);
        let omq_ref = 1.0 - q_ref;
        let w_ref =
            (w_norm * omq_ref * omq_ref * omq_ref * omq_ref * (1.0 + 4.0 * q_ref)).max(1e-6);
        let d_contact = 0.88 * dx_val;
        // Compact solid contact hitbox: d_hit = 1.04*dx -> radius r_hit = 0.52*dx (< visual crumb ~0.76*dx)
        // Infalling same-body planetary rubble under active gravity uses d_rubble = 1.10*dx for mantle support.
        let d_hit = 1.04 * dx_val;
        let d_rubble = 1.10 * dx_val;
        let d_core = 0.94 * dx_val;

        let pos_ref = &self.pos;
        let vel_ref = &self.vel;
        let grav_ref = &self.grav_acc;
        let mass_ref = &self.mass;
        let s_xx_ref = &self.s_xx;
        let s_yy_ref = &self.s_yy;
        let s_xy_ref = &self.s_xy;
        let p_ref = &self.sph_pressure;
        let body_ref = &self.body_id;
        let det_age_ref = &self.det_age;
        let grid_head = &self.grid_head;
        let grid_next = &self.grid_next;
        let grid_mask = self.grid_mask;
        let grid_cell_size = self.grid_cell_size;
        let down_grav = self.down_gravity;
        let self_grav_on = self.self_gravity && self.grav_g > 0.0;

        self.step_scratch
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, out)| {
                let pi = pos_ref[i];
                let vi = vel_ref[i];
                let rho_i = rho_ref[i];
                let rho0_i = rho0_ref[i];
                let temp_i = temp_ref[i];
                let press_i = p_ref[i];
                let dmg_i = dmg_ref[i].clamp(0.0, 1.0);
                let mass_i = mass_ref[i];
                let mat_i = mat_ref[i];
                let m_i = mat_i.props();
                let body_i = body_ref[i];
                let det_i = det_ref[i];
                let z_i = if det_i == 2 {
                    rho_i * 0.22
                } else {
                    rho_i * m_i.c0
                };

                let melt_i = if temp_i >= m_i.melt_temp {
                    1.0
                } else {
                    ((temp_i - 300.0) / (m_i.melt_temp - 300.0).max(50.0)).clamp(0.0, 1.0)
                };
                let intact_i = if det_i == 2 {
                    0.0
                } else {
                    (1.0 - dmg_i) * (1.0 - melt_i * melt_i.sqrt())
                };
                let s_ix = s_xx_ref[i] * intact_i;
                let s_iy = s_yy_ref[i] * intact_i;
                let s_ixy = s_xy_ref[i] * intact_i;
                let rho_i2 = (rho_i * rho_i).max(1e-6);

                let cx = (pi.x / grid_cell_size).floor() as i32;
                let cy = (pi.y / grid_cell_size).floor() as i32;

                let mut acc_i = Vec2::new(0.0, -down_grav);
                if self_grav_on {
                    acc_i += grav_ref[i];
                }
                let grav_active = (self_grav_on && grav_ref[i].length_squared() > 1e-12)
                    || down_grav.abs() > 1e-6;
                let mut xsph = Vec2::ZERO;
                let mut drho = 0.0f32;
                let mut de = 0.0f32;
                let mut shock_comp = 0.0f32;
                let mut symp_burn_dt = 0.0f32;
                let mut grad_v = [0.0f32; 4];
                let mut c_drho = 0.0f32;
                let mut c_grad_v = [0.0f32; 4];
                let mut c_w_sum = 0.0f32;
                let mut m_xx = 0.0f32;
                let mut m_xy = 0.0f32;
                let mut m_yy = 0.0f32;
                let mut fric_alpha = 0.0f32;
                let mut neighbors = 0u16;
                let can_symp = m_i.is_energetic && det_i == 0;
                let i_ductile = m_i.is_ductile();

                for dy in -1..=1 {
                    for dx_c in -1..=1 {
                        let bucket = Self::hash_cell(cx + dx_c, cy + dy, grid_mask);
                        let mut j_i32 = grid_head[bucket];
                        let mut guard = 0;
                        while j_i32 >= 0 && guard < 512 {
                            guard += 1;
                            let j = j_i32 as usize;
                            j_i32 = grid_next[j];
                            if i == j {
                                continue;
                            }

                            let xij = pi - pos_ref[j];
                            let r2 = xij.length_squared();
                            if r2 >= cutoff_sq || r2 < 1e-10 {
                                continue;
                            }

                            let r = r2.sqrt();
                            let nij = xij / r;
                            let vj = vel_ref[j];
                            let vij = vi - vj;
                            let rho_j = rho_ref[j];
                            let rho0_j = rho0_ref[j];
                            let temp_j = temp_ref[j];
                            let press_j = p_ref[j];
                            let dmg_j = dmg_ref[j].clamp(0.0, 1.0);
                            let mj = mass_ref[j];
                            let m_j = mat_ref[j].props();
                            let body_j = body_ref[j];
                            let det_j = det_ref[j];

                            if can_symp {
                                if det_j == 2 && det_age_ref[j] < 4.0 && r < 2.25 * dx_val {
                                    let arrival =
                                        (r / m_i.detonation_vel.max(0.25)).max(0.015);
                                    if symp_burn_dt == 0.0 || arrival < symp_burn_dt {
                                        symp_burn_dt = arrival;
                                    }
                                } else if mat_i == MaterialId::ImpactFuze
                                    && body_i != body_j
                                    && det_j != 2
                                    && r < 0.98 * dx_val
                                    && vij.length() > 0.015
                                {
                                    symp_burn_dt = 0.015;
                                }
                            }

                            let melt_j = if temp_j >= m_j.melt_temp {
                                1.0
                            } else {
                                ((temp_j - 300.0) / (m_j.melt_temp - 300.0).max(50.0))
                                    .clamp(0.0, 1.0)
                            };
                            let intact_j = if det_j == 2 {
                                0.0
                            } else {
                                (1.0 - dmg_j) * (1.0 - melt_j * melt_j.sqrt())
                            };
                            let c_coh = if body_i == body_j && det_i != 2 && det_j != 2 {
                                intact_i * intact_j
                            } else {
                                0.0
                            };

                            // REGIME C: Non-bonded solid, shattered debris, or molten-solid contact (c_coh <= 0.03, neither is HE gas)
                            // Cross-body & zero-gravity debris use strict hitbox r < d_hit = 1.00*dx (r_hit = 0.50*dx < visual crumb ~0.76*dx)!
                            if c_coh <= 0.03 && det_i != 2 && det_j != 2 {
                                let vn_pre = vij.dot(nij);
                                let mut d_pair = d_hit;
                                if body_i == body_j {
                                    if melt_i > 0.65 && melt_j > 0.65 {
                                        d_pair = 1.22 * dx_val;
                                    } else if grav_active && vn_pre < -1e-4 {
                                        d_pair = d_rubble;
                                    }
                                }
                                if r >= d_pair {
                                    continue;
                                }
                                neighbors = neighbors.saturating_add(1);
                                let z_j = rho_j * m_j.c0;
                                let w_visc_i = z_j / (z_i + z_j).max(1e-5);
                                let cbar = 0.5 * (m_i.c0 + m_j.c0);
                                let ramp = ((d_pair - r) / (0.09 * dx_val)).clamp(0.0, 1.0);
                                let vn = vn_pre;
                                let v_close = (-vn).max(0.0);
                                let area_c = 0.94 * dx_val;
                                let z_sum = (z_i + z_j).max(1e-5);
                                let z_eff = (z_i * z_j) / z_sum;
                                let m_red = (mass_i * mj) / (mass_i + mj).max(1e-6);

                                let p_i_pos = press_i.max(0.0);
                                let p_j_pos = press_j.max(0.0);
                                let p_eos_avg = (z_j * p_i_pos + z_i * p_j_pos) / z_sum;
                                let p_riemann =
                                    ((z_j * p_i_pos + z_i * p_j_pos + z_i * z_j * (-vn)) / z_sum)
                                        .max(0.0);
                                let p_quad = if v_close > 0.0 {
                                    let rho_red = (rho_i * rho_j) / (rho_i + rho_j).max(1e-5);
                                    let s_bar = 0.5 * (m_i.hugoniot_s + m_j.hugoniot_s);
                                    0.5 * rho_red * s_bar * v_close * v_close
                                } else {
                                    0.0
                                };

                                let f_eos_part = p_eos_avg * area_c * ramp;
                                let f_visc_raw = if v_close > 0.0 {
                                    (z_eff * v_close + p_quad) * area_c * ramp
                                } else {
                                    0.0
                                };
                                let f_visc_cap = 0.45 * m_red * v_close / dt.max(0.005);
                                let f_visc = f_visc_raw.min(f_visc_cap);
                                let f_eos_used = if vn >= 0.0 {
                                    p_riemann * area_c * ramp
                                } else {
                                    f_eos_part
                                };

                                let solid_f = (1.0 - melt_i).min(1.0 - melt_j);
                                let f_core = if r < d_core {
                                    let ov_c = (d_core - r) / d_core;
                                    let k_i = rho0_i * m_i.c0 * m_i.c0;
                                    let k_j = rho0_j * m_j.c0 * m_j.c0;
                                    let k_eff = (2.0 * k_i * k_j) / (k_i + k_j).max(1e-5);
                                    k_eff * (0.28 + 0.50 * solid_f) * ov_c * ov_c * area_c
                                } else {
                                    0.0
                                };

                                let f_n = f_eos_used + f_visc + f_core;
                                let a_n = f_n / mass_i.max(1e-6);
                                acc_i += nij * a_n;

                                if !grav_active || v_close > 0.022 || vn > 0.022 {
                                    de += w_visc_i * (-f_eos_used * vn + 0.5 * f_visc * v_close)
                                        / mass_i.max(1e-6);
                                }
                                if ((!grav_active && temp_i.max(temp_j) > 450.0)
                                    || (grav_active && temp_i.max(temp_j) > 1600.0))
                                    && (temp_j - temp_i).abs() > 80.0
                                {
                                    de += 2.2e-7
                                        * ramp
                                        * (temp_j - temp_i)
                                        * (mj / (mass_i + mj).max(1e-6));
                                }
                                if body_i == body_j
                                    && melt_i > 0.75
                                    && melt_j > 0.75
                                    && r > 0.98 * dx_val
                                    && r < 1.22 * dx_val
                                {
                                    let phase_liq =
                                        (pi_c * (r - 0.98 * dx_val) / (0.24 * dx_val)).sin();
                                    let m_pair_liq = (2.0 * mj) / (mass_i + mj).max(1e-6);
                                    acc_i -= nij
                                        * (0.0014 * cbar * cbar / dx_val.max(0.05))
                                        * phase_liq
                                        * m_pair_liq;
                                }

                                c_w_sum += ramp;
                                if grav_active {
                                    c_drho += rho_i * w_visc_i * (-vn / dx_val) * ramp;
                                }
                                if v_close > 0.0 {
                                    if !grav_active {
                                        c_drho += rho_i * w_visc_i * (v_close / dx_val) * ramp;
                                    }
                                    shock_comp += (v_close / cbar.max(0.05)) * ramp;
                                    let g_c = -(w_visc_i * v_close / dx_val) * ramp;
                                    c_grad_v[0] += nij.x * nij.x * g_c;
                                    c_grad_v[1] += nij.x * nij.y * g_c;
                                    c_grad_v[2] += nij.y * nij.x * g_c;
                                    c_grad_v[3] += nij.y * nij.y * g_c;
                                }

                                let vt = vij - nij * vn;
                                let vt_len = vt.length();
                                if vt_len > 1e-5 && f_n > 0.0 && solid_f > 0.05 {
                                    let mu_f = 0.5 * (m_i.friction + m_j.friction) * solid_f;
                                    let a_fric = (mu_f * a_n).min(
                                        0.30 * (2.0 * mj / (mass_i + mj).max(1e-6)) * vt_len
                                            / dt.max(0.005),
                                    );
                                    let f_tan = -(vt / vt_len) * a_fric;
                                    acc_i += f_tan;
                                    de += 0.25 * a_fric * vt_len;
                                    let cross_z = (-nij.x) * f_tan.y - (-nij.y) * f_tan.x;
                                    fric_alpha += cross_z / dx_val.max(0.05);
                                }

                                xsph += 0.03 * ramp * (mj / (mass_i + mj).max(1e-6)) * (vj - vi);
                                continue;
                            }

                            // REGIME A (Coherent intact solid) & REGIME B (Detonation gas fluid)
                            neighbors = neighbors.saturating_add(1);
                            let q = r / cutoff;
                            let omq = 1.0 - q;
                            let omq3 = omq * omq * omq;
                            let w = w_norm * omq3 * omq * (1.0 + 4.0 * q);
                            let dw_dr = dw_norm * q * omq3;
                            let grad_w = nij * dw_dr;

                            let z_j = if det_j == 2 {
                                rho_j * 0.22
                            } else {
                                rho_j * m_j.c0
                            };
                            let w_visc_i = z_j / (z_i + z_j).max(1e-5);
                            let coh_gate = if c_coh > 0.03 && r > d_hit {
                                ((c_coh - 0.03) / 0.32).clamp(0.0, 1.0)
                            } else {
                                1.0
                            };

                            let rho_vol_j = if det_j == 2 {
                                rho_j.max(0.52 * rho0_j)
                            } else {
                                rho_j
                            };
                            let vol_j = (mj / rho_vol_j.max(1e-5)) * coh_gate;
                            let vij_dot_gradw = vij.dot(grad_w);
                            let gas_solid_comp = if det_i != 2 && det_j == 2 {
                                (2.0 * w_visc_i).min(1.0)
                            } else {
                                1.0
                            };
                            drho += gas_solid_comp * rho_i * vol_j * vij_dot_gradw;

                            let cbar = 0.5 * (m_i.c0 + m_j.c0);
                            if det_i == det_j {
                                let comp_diff =
                                    (rho_j / rho0_j.max(1e-4)) - (rho_i / rho0_i.max(1e-4));
                                let pos_kf = (r * (-dw_dr)) / (r2 + 0.01 * h2);
                                drho += 0.12 * h * cbar * rho0_i * vol_j * 2.0 * comp_diff * pos_kf;
                            }

                            let vij_dot_xij = vij.dot(xij);
                            let pi_visc = if vij_dot_xij < 0.0 {
                                let mu_v = (h * vij_dot_xij) / (r2 + 0.01 * h2);
                                let rho_bar = 0.5 * (rho_i + rho_j);
                                shock_comp += (-mu_v) * vol_j * w * cutoff;
                                ((-1.05 * cbar * mu_v + 2.00 * mu_v * mu_v) / rho_bar.max(1e-4))
                                    * coh_gate
                            } else {
                                0.0
                            };

                            let rho_j2 = (rho_j * rho_j).max(1e-6);

                            if c_coh > 0.03 {
                                let vji = vj - vi;
                                let wg = vol_j * c_coh;
                                grad_v[0] += vji.x * grad_w.x * wg;
                                grad_v[1] += vji.x * grad_w.y * wg;
                                grad_v[2] += vji.y * grad_w.x * wg;
                                grad_v[3] += vji.y * grad_w.y * wg;
                                m_xx += (-xij.x) * grad_w.x * wg;
                                m_xy += -0.5 * (xij.x * grad_w.y + xij.y * grad_w.x) * wg;
                                m_yy += (-xij.y) * grad_w.y * wg;

                                let s_jx = s_xx_ref[j] * intact_j;
                                let s_jy = s_yy_ref[j] * intact_j;
                                let s_jxy = s_xy_ref[j] * intact_j;

                                let sig_xx_i = -press_i + s_ix;
                                let sig_yy_i = -press_i + s_iy;
                                let sig_xx_j = -press_j + s_jx;
                                let sig_yy_j = -press_j + s_jy;

                                let tens_i = sig_xx_i.max(sig_yy_i).max(0.0) / rho_i2;
                                let tens_j = sig_xx_j.max(sig_yy_j).max(0.0) / rho_j2;
                                let fij = (w / w_ref).clamp(0.0, 1.25);
                                let fij2 = fij * fij;
                                let r_art = 0.30 * (tens_i + tens_j) * fij2 * fij2;

                                let t_xx = (sig_xx_i / rho_i2 + sig_xx_j / rho_j2 - r_art)
                                    * coh_gate
                                    - pi_visc;
                                let t_yy = (sig_yy_i / rho_i2 + sig_yy_j / rho_j2 - r_art)
                                    * coh_gate
                                    - pi_visc;
                                let t_xy = (s_ixy / rho_i2 + s_jxy / rho_j2) * coh_gate;

                                acc_i += mj
                                    * Vec2::new(
                                        t_xx * grad_w.x + t_xy * grad_w.y,
                                        t_xy * grad_w.x + t_yy * grad_w.y,
                                    );
                                de += mj
                                    * (press_i.max(0.0) / rho_i2 * coh_gate + w_visc_i * pi_visc)
                                    * vij_dot_gradw;

                                let m_pair = (2.0 * mj) / (mass_i + mj).max(1e-6);
                                if r < d_contact {
                                    let ov = 1.0 - r / d_contact;
                                    let k_rep = 1.15 * cbar * cbar / d_contact;
                                    acc_i += nij * (k_rep * ov * ov * m_pair);
                                } else if i_ductile
                                    && m_j.is_ductile()
                                    && r > 1.14 * dx_val
                                    && r < 1.58 * dx_val
                                {
                                    let y_coh = m_i.yield_strength.min(m_j.yield_strength)
                                        + 0.38 * m_i.spall_strength.min(m_j.spall_strength);
                                    let phase_s =
                                        (pi_c * (r - 1.14 * dx_val) / (0.44 * dx_val)).sin();
                                    let a_coh = (1.30 * c_coh * y_coh
                                        / (0.5 * (rho0_i + rho0_j) * dx_val))
                                        * phase_s;
                                    acc_i -= nij * (a_coh * m_pair * coh_gate);
                                }

                                xsph += (0.075 * c_coh + 0.015) * vol_j * (vj - vi) * w;
                            } else {
                                // DETONATION GAS FLUID INTERACTION (det_i == 2 || det_j == 2)
                                let p_pos_i = press_i.max(0.0);
                                let p_pos_j = press_j.max(0.0);
                                let rho_eff_i = if det_i == 2 {
                                    rho_i.max(0.45 * rho0_i)
                                } else {
                                    rho_i
                                };
                                let rho_eff_j = if det_j == 2 {
                                    rho_j.max(0.45 * rho0_j)
                                } else {
                                    rho_j
                                };
                                let p_term = p_pos_i / (rho_eff_i * rho_eff_i).max(1e-6)
                                    + p_pos_j / (rho_eff_j * rho_eff_j).max(1e-6)
                                    + pi_visc;
                                acc_i -= mj * p_term * grad_w;
                                de += mj
                                    * (p_pos_i / rho_i2 + w_visc_i * pi_visc)
                                    * vij_dot_gradw;
                                xsph += 0.025 * vol_j * (vj - vi) * w;
                            }
                        }
                    }
                }

                let det_m = m_xx * m_yy - m_xy * m_xy;
                let tr_m = m_xx + m_yy;
                if det_m > 0.22 && tr_m > 0.75 {
                    let inv_det = 1.0 / det_m;
                    let b_xx = (m_yy * inv_det).clamp(0.5, 1.85);
                    let b_xy = (-m_xy * inv_det).clamp(-0.65, 0.65);
                    let b_yy = (m_xx * inv_det).clamp(0.5, 1.85);
                    let g0 = grad_v[0];
                    let g1 = grad_v[1];
                    let g2 = grad_v[2];
                    let g3 = grad_v[3];
                    grad_v = [
                        g0 * b_xx + g1 * b_xy,
                        g0 * b_xy + g1 * b_yy,
                        g2 * b_xx + g3 * b_xy,
                        g2 * b_xy + g3 * b_yy,
                    ];
                } else if tr_m > 0.25 {
                    let s = (2.0 / tr_m).clamp(1.0, 2.8);
                    for g in &mut grad_v {
                        *g *= s;
                    }
                }
                if c_w_sum > 0.0 {
                    let c_norm = 0.95 / c_w_sum.max(1.0);
                    drho += c_drho * c_norm;
                    for k in 0..4 {
                        grad_v[k] += c_grad_v[k] * c_norm;
                    }
                }

                *out = ContinuumStepOut {
                    acc: acc_i,
                    xsph,
                    drho,
                    de,
                    shock_comp,
                    symp_burn_dt,
                    grad_v,
                    fric_alpha,
                    neighbors,
                };
            });

        // 5. Parallel Plasticity, Fracture, Detonation & Velocity Kick Integration
        let first_step = self.step_count == 0;
        let step_idx = self.step_count;
        let half_dt = 0.5 * dt;
        let scratch_ref = &self.step_scratch;
        let rho0_ref = &self.rho0;
        let sph_p_ref = &self.sph_pressure;
        let mat_ref = &self.mat;

        let (melted_cnt, det_cnt, frac_cnt) = self
            .pos
            .par_iter_mut()
            .zip(self.vel.par_iter_mut())
            .zip(self.acc.par_iter_mut())
            .zip(self.xsph_dv.par_iter_mut())
            .zip(self.rho.par_iter_mut())
            .zip(self.s_xx.par_iter_mut())
            .zip(self.s_yy.par_iter_mut())
            .zip(self.s_xy.par_iter_mut())
            .zip(self.plastic_strain.par_iter_mut())
            .zip(self.damage.par_iter_mut())
            .zip(self.net_pressure.par_iter_mut())
            .zip(self.energy.par_iter_mut())
            .zip(self.temp.par_iter_mut())
            .zip(self.shock_rate.par_iter_mut())
            .zip(self.det_state.par_iter_mut())
            .zip(self.det_timer.par_iter_mut())
            .zip(self.det_age.par_iter_mut())
            .enumerate()
            .map(
                |(
                    i,
                    (
                        (
                            (
                                (
                                    (
                                        (
                                            (
                                                (
                                                    (
                                                        (
                                                            (
                                                                (
                                                                    (
                                                                        (((pos, vel), acc), xsph),
                                                                        rho,
                                                                    ),
                                                                    s_xx,
                                                                ),
                                                                s_yy,
                                                            ),
                                                            s_xy,
                                                        ),
                                                        eps_p,
                                                    ),
                                                    dmg,
                                                ),
                                                net_p,
                                            ),
                                            energy,
                                        ),
                                        temp,
                                    ),
                                    shock_r,
                                ),
                                det_state,
                            ),
                            det_timer,
                        ),
                        det_age,
                    ),
                )| {
                    let fo = scratch_ref[i];
                    let mat_id = mat_ref[i];
                    let m = mat_id.props();
                    let rho0 = rho0_ref[i];
                    let sph_p = sph_p_ref[i];

                    *acc = fo.acc;
                    *xsph = fo.xsph;
                    *shock_r = 0.65 * *shock_r + 0.35 * fo.shock_comp;

                    // Density update (rate-limited per substep to prevent single-cell detonation Euler overshoot)
                    let delta_rho = (fo.drho * dt).clamp(-0.10 * rho0, 0.12 * rho0);
                    let mut new_rho = *rho + delta_rho;
                    if *det_state == 2 {
                        if fo.neighbors < 10 {
                            let target_gas = 0.08 * rho0;
                            if new_rho > target_gas {
                                new_rho -= (new_rho - target_gas) * (0.35 * dt).min(0.25);
                            }
                        }
                        *rho = new_rho.clamp(0.05 * rho0, 2.2 * rho0);
                    } else {
                        if *dmg > 0.45 || *temp >= m.melt_temp || fo.neighbors <= 1 {
                            if new_rho < rho0 {
                                let relax = (dt / 0.30).min(0.25);
                                new_rho += (rho0 - new_rho) * relax;
                            } else if new_rho > rho0
                                && (fo.neighbors <= 3 || (*dmg > 0.70 && fo.shock_comp < 0.03))
                            {
                                let relax = (dt / 0.40).min(0.22);
                                new_rho += (rho0 - new_rho) * relax;
                            }
                        }
                        *rho = new_rho.clamp(0.85 * rho0, 1.72 * rho0);
                    }

                    // Deviatoric Stress + Johnson-Cook Viscoplastic Radial Return
                    let mut plastic_heat = 0.0f32;
                    let mut sigma_eq_eff = 0.0f32;
                    let theta =
                        ((*temp - 293.0) / (m.melt_temp - 293.0).max(50.0)).clamp(0.0, 0.999);
                    let thermal_soft = if theta > 1e-4 {
                        1.0 - theta.powf(mat_id.thermal_m())
                    } else {
                        1.0
                    };

                    if *det_state == 2 || *temp >= m.melt_temp {
                        *s_xx = 0.0;
                        *s_yy = 0.0;
                        *s_xy = 0.0;
                    } else {
                        let lv0 = fo.grad_v[0].clamp(-4.5, 4.5);
                        let lv1 = fo.grad_v[1].clamp(-4.5, 4.5);
                        let lv2 = fo.grad_v[2].clamp(-4.5, 4.5);
                        let lv3 = fo.grad_v[3].clamp(-4.5, 4.5);

                        let eps_xx = lv0;
                        let eps_yy = lv3;
                        let eps_xy = 0.5 * (lv1 + lv2);
                        let omega_xy = 0.5 * (lv1 - lv2);
                        let div_v = eps_xx + eps_yy;

                        let d_xx = eps_xx - 0.3333333 * div_v;
                        let d_yy = eps_yy - 0.3333333 * div_v;
                        let d_zz = -0.3333333 * div_v;

                        let dmg_soft = 1.0 - 0.85 * *dmg;
                        let g_eff = m.shear_modulus() * dmg_soft * thermal_soft;
                        let old_sx = *s_xx;
                        let old_sy = *s_yy;
                        let old_sxy = *s_xy;
                        *s_xx += dt * (2.0 * g_eff * d_xx + 2.0 * old_sxy * omega_xy);
                        *s_yy += dt * (2.0 * g_eff * d_yy - 2.0 * old_sxy * omega_xy);
                        *s_xy += dt * (2.0 * g_eff * eps_xy - (old_sx - old_sy) * omega_xy);

                        let s_zz = -(*s_xx + *s_yy);
                        let j2 =
                            0.5 * (*s_xx * *s_xx + *s_yy * *s_yy + s_zz * s_zz) + *s_xy * *s_xy;
                        let mut sigma_eq = (3.0 * j2).max(0.0).sqrt();

                        let edot_eq = (0.6666667
                            * (d_xx * d_xx + d_yy * d_yy + d_zz * d_zz + 2.0 * eps_xy * eps_xy))
                            .max(0.0)
                            .sqrt();
                        let rate_mult = 1.0 + mat_id.jc_rate_c() * (edot_eq * 1.0e6).max(1.0).ln();

                        let p_conf = sph_p.max(0.0);
                        let work_h = if *eps_p > 1e-6 {
                            eps_p.powf(0.48)
                        } else {
                            0.0
                        };
                        let y_coh = m.yield_strength * (1.0 + m.hardening * work_h) * dmg_soft;
                        let y_rub = m.pressure_hardening * p_conf * (1.0 - 0.35 * *dmg);
                        let y_flow = (y_coh + y_rub) * rate_mult * thermal_soft;

                        if sigma_eq > y_flow && sigma_eq > 1e-7 {
                            let sigma_eq_old = sigma_eq;
                            let scale = y_flow / sigma_eq;
                            *s_xx *= scale;
                            *s_yy *= scale;
                            *s_xy *= scale;
                            sigma_eq = y_flow;
                            let d_eps_p =
                                (sigma_eq_old - y_flow) / (3.0 * g_eff.max(0.005)).max(1e-5);
                            *eps_p += d_eps_p;
                            plastic_heat = (0.90 * y_flow * d_eps_p) / rho.max(0.1);
                        }
                        sigma_eq_eff = sigma_eq;
                    }

                    // Continuum Spall & Johnson-Cook Ductile Failure
                    let sigma_mean = -sph_p + 0.5 * (*s_xx + *s_yy);
                    let tau_max =
                        (0.25 * (*s_xx - *s_yy) * (*s_xx - *s_yy) + *s_xy * *s_xy).sqrt();
                    let sigma_1_tens = sigma_mean + tau_max;
                    let spall_th = (m.spall_strength * thermal_soft).max(1e-4);

                    if *det_state == 2 {
                        *dmg = 1.0;
                    } else if m.is_ductile() {
                        if sigma_mean > 0.25 * spall_th && sigma_1_tens > spall_th {
                            let over = (sigma_1_tens - spall_th) / spall_th;
                            *dmg = (*dmg + dt * 1.65 * over).clamp(0.0, 1.0);
                        }
                        let k1 = (rho0 * m.c0 * m.c0).max(0.05);
                        let triax = sigma_mean / sigma_eq_eff.max(m.yield_strength.max(0.005));
                        let eps_f_jc = if sigma_mean > 0.0 {
                            m.fracture_strain * (0.38 + 0.62 * (-1.3 * triax).exp()).max(0.35)
                        } else {
                            m.fracture_strain * (1.5 + 4.5 * sph_p.max(0.0) / k1).min(3.0)
                        };
                        if *eps_p > eps_f_jc {
                            let over_jc = (*eps_p - eps_f_jc) / eps_f_jc.max(0.05);
                            *dmg = (*dmg + dt * 2.80 * over_jc).clamp(0.0, 1.0);
                        }
                    } else {
                        if sigma_mean > 0.15 * spall_th && sigma_1_tens > spall_th {
                            let over = (sigma_1_tens - spall_th) / spall_th;
                            *dmg = (*dmg + dt * 1.8 * over).clamp(0.0, 1.0);
                        }
                        let k1 = (rho0 * m.c0 * m.c0).max(0.05);
                        let eps_f_conf = m.fracture_strain * (1.0 + 3.2 * sph_p.max(0.0) / k1);
                        if *eps_p > eps_f_conf {
                            let over_p = (*eps_p - eps_f_conf) / eps_f_conf.max(0.005);
                            *dmg = (*dmg + dt * 1.2 * over_p).clamp(0.0, 1.0);
                        }
                    }

                    // Energetic Detonation State Machine
                    if m.is_energetic {
                        let total_comp_p = sph_p.max(0.0) + 0.5 * tau_max;
                        let is_dfuze = mat_ref[i] == MaterialId::DelayFuze;
                        let inertia_trip = is_dfuze
                            && step_idx > 6
                            && acc.length_squared() >= (0.0018 * 0.0018)
                            && total_comp_p >= 0.00025;
                        if *det_state == 0 {
                            if fo.symp_burn_dt > 0.0 {
                                *det_state = 1;
                                *det_timer = if is_dfuze {
                                    m.delay_us.max(fo.symp_burn_dt)
                                } else {
                                    fo.symp_burn_dt
                                };
                            } else if total_comp_p >= m.initiation_pressure
                                || inertia_trip
                                || *temp >= m.melt_temp
                            {
                                *det_state = 1;
                                *det_timer = m.delay_us;
                            }
                        }
                        if *det_state == 1 {
                            if !is_dfuze && fo.symp_burn_dt > 0.0 && fo.symp_burn_dt < *det_timer {
                                *det_timer = fo.symp_burn_dt;
                            }
                            *det_timer -= dt;
                            if *det_timer <= 0.0 {
                                *det_state = 2;
                                *det_age = 0.0;
                                *dmg = 1.0;
                                *s_xx = 0.0;
                                *s_yy = 0.0;
                                *s_xy = 0.0;
                                *energy += m.detonation_energy;
                                *temp = (*temp + m.detonation_energy / m.cv.max(1e-7))
                                    .clamp(2600.0, 5200.0);
                            }
                        } else if *det_state == 2 {
                            *det_age += dt;
                        }
                    } else if mat_ref[i] == MaterialId::ThermitePyro {
                        if *det_state == 0 && (*temp >= 1150.0 || fo.shock_comp >= 0.045) {
                            *det_state = 1;
                            *det_age = 0.0;
                        }
                        if *det_state == 1 {
                            let burn_dt = dt.min((8.0 - *det_age).max(0.0));
                            *det_age += dt;
                            plastic_heat += (m.detonation_energy / 8.0) * burn_dt;
                            if *det_age >= 8.0 {
                                *det_state = 3;
                            }
                        }
                    }

                    // Velocity kick + thermal update
                    if first_step {
                        *vel += *acc * dt;
                        *pos += (*vel + *xsph) * dt;
                    } else {
                        *vel += *acc * half_dt;
                    }

                    let mut rad_cool_e = 0.0f32;
                    if *det_state != 2 && fo.neighbors < 7 && *temp > 700.0 {
                        let t_norm = temp.min(8000.0) * 0.001;
                        let exposure = (7.5 - fo.neighbors as f32).clamp(0.0, 6.0);
                        rad_cool_e = 1.6e-5 * exposure * (t_norm * t_norm * t_norm * t_norm) * dt;
                    }
                    if *det_state != 2
                        && *dmg > 0.35
                        && *temp > (0.55 * m.melt_temp).max(450.0)
                        && *temp < 0.90 * m.melt_temp
                        && fo.neighbors >= 4
                        && fo.shock_comp < 0.02
                    {
                        *dmg = (*dmg - 0.035 * dt).max(0.20);
                    }

                    let mut mech_e = (*energy + fo.de * dt + plastic_heat).max(0.0);
                    if *det_state == 2 && fo.neighbors < 8 {
                        mech_e *= (1.0 - 0.14 * dt).max(0.85);
                    }
                    let mech_de = mech_e - *energy;
                    *energy = (mech_e - rad_cool_e).max(0.0);
                    let temp_floor = if *temp >= 293.0 { 293.0 } else { 50.0 };
                    *temp = (*temp + (mech_de - rad_cool_e) / m.cv.max(1e-7))
                        .clamp(temp_floor, 24_000.0);

                    *net_p = if *det_state == 2 { sph_p } else { -sigma_mean };

                    let is_melt = *temp >= m.melt_temp;
                    let is_det = *det_state == 2;
                    let is_frac = !is_det && *dmg >= 0.70;
                    (usize::from(is_melt), usize::from(is_det), usize::from(is_frac))
                },
            )
            .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));

        self.crumb_angle
            .par_iter_mut()
            .zip(self.crumb_spin.par_iter_mut())
            .zip(self.det_state.par_iter())
            .enumerate()
            .for_each(|(i, ((c_ang, c_spin), &det_s))| {
                let fo = scratch_ref[i];
                if det_s != 2 {
                    if fo.neighbors >= 2 {
                        let vort = 0.5 * (fo.grad_v[2] - fo.grad_v[1]).clamp(-3.5, 3.5);
                        let blend = (dt * 7.5).min(0.45);
                        *c_spin = (*c_spin * (1.0 - blend) + vort * blend + fo.fric_alpha * dt)
                            .clamp(-3.5, 3.5);
                    } else {
                        *c_spin = (*c_spin + fo.fric_alpha * dt).clamp(-3.5, 3.5);
                    }
                    *c_ang += *c_spin * dt;
                }
            });

        self.melted_count = melted_cnt;
        self.detonated_count = det_cnt;
        self.fractured_count = frac_cnt;
        self.time_us += dt;
        self.step_count += 1;
        self.state_epoch = self.state_epoch.wrapping_add(1);

        if self.time_us - self.last_history_time >= 0.45 {
            self.record_telemetry();
            self.last_history_time = self.time_us;
        }
    }

    fn update_self_gravity(&mut self) {
        const G_DIM: usize = 32;
        if self.pos.is_empty() {
            return;
        }
        let mut total_m = 0.0f32;
        let mut com_all = Vec2::ZERO;
        for i in 0..self.pos.len() {
            let m = self.mass[i];
            total_m += m;
            com_all += self.pos[i] * m;
        }
        if total_m <= 0.0 {
            return;
        }
        com_all /= total_m;

        let mut r2_sum = 0.0f32;
        for i in 0..self.pos.len() {
            r2_sum += self.mass[i] * (self.pos[i] - com_all).length_squared();
        }
        let r_rms = (r2_sum / total_m).sqrt();
        let half_span = (3.4 * r_rms).clamp(28.0, 600.0);
        let grid_min = com_all - Vec2::splat(half_span);
        let cell_sz = Vec2::splat((2.0 * half_span) / G_DIM as f32);
        let mut cells_pos = [GravCell::default(); G_DIM * G_DIM];
        let mut cells_neg = [GravCell::default(); G_DIM * G_DIM];

        for i in 0..self.pos.len() {
            let rel = (self.pos[i] - grid_min) / cell_sz;
            let gx = (rel.x.floor() as isize).clamp(0, G_DIM as isize - 1) as usize;
            let gy = (rel.y.floor() as isize).clamp(0, G_DIM as isize - 1) as usize;
            let idx = gy * G_DIM + gx;
            let qm = self.mat[i].grav_charge_mult() * self.mass[i];
            if qm >= 0.0 {
                cells_pos[idx].com += self.pos[i] * qm;
                cells_pos[idx].mass += qm;
            } else {
                let mag = -qm;
                cells_neg[idx].com += self.pos[i] * mag;
                cells_neg[idx].mass += mag;
            }
        }
        let mut active_cells: Vec<(Vec2, f32)> = Vec::with_capacity(G_DIM * G_DIM);
        for c in cells_pos {
            if c.mass > 0.0 {
                active_cells.push((c.com / c.mass, c.mass));
            }
        }
        for c in cells_neg {
            if c.mass > 0.0 {
                active_cells.push((c.com / c.mass, -c.mass));
            }
        }

        let g_const = self.grav_g;
        let soft_sq = (2.4 * self.dx) * (2.4 * self.dx) + 0.20 * cell_sz.x * cell_sz.y;
        let pos_ref = &self.pos;
        let mat_ref = &self.mat;

        self.grav_acc
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, g_out)| {
                let pi = pos_ref[i];
                let qi = mat_ref[i].grav_charge_mult();
                let mut g = Vec2::ZERO;
                for &(c_com, c_qm) in &active_cells {
                    let d = c_com - pi;
                    let r2 = d.length_squared() + soft_sq;
                    let inv_r = 1.0 / r2.sqrt();
                    let inv_r3 = inv_r / r2;
                    g += d * (g_const * c_qm * inv_r3);
                }
                *g_out = g * qi;
            });
    }

    /// Pre-compress self-gravitating bodies (`dP/dr = -rho * g_r`) so planets launch in hydrostatic equilibrium
    /// with zero initial free-fall collapse shockwave or artificial spallation.
    pub fn equilibrate_self_gravity(&mut self) {
        if !self.self_gravity || self.grav_g <= 0.0 || self.pos.is_empty() {
            return;
        }
        self.update_self_gravity();
        let n = self.len();

        let mut body_ids: Vec<u16> = self.body_id.clone();
        body_ids.sort_unstable();
        body_ids.dedup();

        const BINS: usize = 48;
        for bid in body_ids {
            let mut m_tot = 0.0f32;
            let mut com = Vec2::ZERO;
            let mut count = 0usize;
            for i in 0..n {
                if self.body_id[i] == bid {
                    m_tot += self.mass[i];
                    com += self.pos[i] * self.mass[i];
                    count += 1;
                }
            }
            if count < 24 || m_tot <= 0.0 {
                continue;
            }
            com /= m_tot;

            let mut r_max = 0.0f32;
            for i in 0..n {
                if self.body_id[i] == bid {
                    r_max = r_max.max((self.pos[i] - com).length());
                }
            }
            if r_max < self.dx * 2.5 {
                continue;
            }

            let dr = (r_max + 0.25 * self.dx) / BINS as f32;
            let mut bin_sum_rg = [0.0f32; BINS];
            let mut bin_sum_rho = [0.0f32; BINS];
            let mut bin_cnt = [0usize; BINS];

            for i in 0..n {
                if self.body_id[i] == bid {
                    let d = self.pos[i] - com;
                    let r = d.length();
                    if r > 1e-4 {
                        let b = ((r / dr).floor() as usize).min(BINS - 1);
                        let e_r = d / r;
                        let g_in = -self.grav_acc[i].dot(e_r);
                        if g_in > 0.0 {
                            bin_sum_rg[b] += self.rho0[i] * g_in;
                        }
                        bin_sum_rho[b] += self.rho0[i];
                        bin_cnt[b] += 1;
                    }
                }
            }

            let mut p_shell = [0.0f32; BINS];
            let mut p_acc = 0.0f32;
            for b in (0..BINS).rev() {
                let d_dp = if bin_cnt[b] > 0 {
                    (bin_sum_rg[b] / bin_cnt[b] as f32) * dr
                } else {
                    0.0
                };
                p_shell[b] = p_acc + 0.5 * d_dp;
                p_acc += d_dp;
            }

            for i in 0..n {
                if self.body_id[i] == bid {
                    let r = (self.pos[i] - com).length();
                    let b = ((r / dr).floor() as usize).min(BINS - 1);
                    let p_h = p_shell[b];
                    if p_h > 1e-7 {
                        let m = self.mat[i].props();
                        let k1 = (m.rho0 * m.c0 * m.c0).max(0.05);
                        let s = m.hugoniot_s;
                        let k2 = k1 * (2.0 * s - 1.0);
                        let k3 = k1 * s * (3.0 * s - 1.0);
                        let mut mu = (p_h / k1).min(0.28);
                        for _ in 0..4 {
                            let f = ((k3 * mu + k2) * mu + k1) * mu - p_h;
                            let df = ((3.0 * k3 * mu + 2.0 * k2) * mu + k1).max(1e-4);
                            mu = (mu - f / df).clamp(0.0, 0.28);
                        }
                        self.rho[i] = m.rho0 * (1.0 + mu);
                        self.sph_pressure[i] = p_h;
                        self.net_pressure[i] = p_h;
                    }
                }
            }
        }
        self.gpu_dirty = true;
    }

    pub fn record_telemetry(&mut self) {
        let mut ke = 0.0f32;
        let mut ie = 0.0f32;
        let mut ee = 0.0f32;
        let mut peak_p = 0.0f32;
        let mut max_v2 = 0.0f32;
        let mut mom = Vec2::ZERO;
        for i in 0..self.pos.len() {
            let m = self.mass[i];
            let v = self.vel[i];
            let v2 = v.length_squared();
            if v2 > max_v2 {
                max_v2 = v2;
            }
            mom += v * m;
            ke += 0.5 * m * v2;
            ie += m * self.energy[i];
            let g_mod = self.mat[i].props().shear_modulus().max(0.01);
            let sx = self.s_xx[i];
            let sy = self.s_yy[i];
            let sz = -(sx + sy);
            let j2 = 0.5 * (sx * sx + sy * sy + sz * sz) + self.s_xy[i] * self.s_xy[i];
            ee += m * (j2 / (2.0 * g_mod * self.rho[i].max(0.1)));
            let p_gpa = self.sph_pressure[i] * 100.0;
            if p_gpa > peak_p {
                peak_p = p_gpa;
            }
        }
        let frac_ratio = if !self.pos.is_empty() {
            self.fractured_count as f32 / self.pos.len() as f32
        } else {
            0.0
        };
        if self.history.len() >= 480 {
            self.history.remove(0);
        }
        self.history.push(TelemetrySample {
            time_us: self.time_us,
            kinetic_energy: ke,
            internal_energy: ie,
            elastic_energy: ee,
            peak_pressure_gpa: peak_p,
            max_speed_kms: max_v2.sqrt() * 10.0,
            fractured_frac: frac_ratio,
            momentum: mom,
        });
    }

    /// Recompute summary counters after direct live-canvas mutations.
    pub fn recount_status(&mut self) {
        let mut melted = 0usize;
        let mut detonated = 0usize;
        let mut fractured = 0usize;
        for i in 0..self.pos.len() {
            let m = self.mat[i].props();
            if self.temp[i] >= m.melt_temp {
                melted += 1;
            }
            if self.det_state[i] == 2 {
                detonated += 1;
            } else if self.damage[i] >= 0.70 {
                fractured += 1;
            }
        }
        self.melted_count = melted;
        self.detonated_count = detonated;
        self.fractured_count = fractured;
    }

    /// Compact all 28 per-particle SoA buffers using `keep(old_idx)` and return old->new index map.
    pub fn retain_particles<F>(&mut self, mut keep: F) -> (usize, Vec<Option<usize>>)
    where
        F: FnMut(usize) -> bool,
    {
        let n = self.pos.len();
        self.grav_acc.resize(n, Vec2::ZERO);
        self.step_scratch.resize(n, ContinuumStepOut::default());
        self.grid_next.resize(n, -1);
        let mut map = vec![None; n];
        let mut w = 0usize;
        for (i, slot) in map.iter_mut().enumerate().take(n) {
            if keep(i) {
                *slot = Some(w);
                if w != i {
                    self.pos[w] = self.pos[i];
                    self.vel[w] = self.vel[i];
                    self.acc[w] = self.acc[i];
                    self.grav_acc[w] = self.grav_acc[i];
                    self.xsph_dv[w] = self.xsph_dv[i];
                    self.mass[w] = self.mass[i];
                    self.rho0[w] = self.rho0[i];
                    self.rho[w] = self.rho[i];
                    self.s_xx[w] = self.s_xx[i];
                    self.s_yy[w] = self.s_yy[i];
                    self.s_xy[w] = self.s_xy[i];
                    self.sph_pressure[w] = self.sph_pressure[i];
                    self.net_pressure[w] = self.net_pressure[i];
                    self.energy[w] = self.energy[i];
                    self.temp[w] = self.temp[i];
                    self.plastic_strain[w] = self.plastic_strain[i];
                    self.damage[w] = self.damage[i];
                    self.shock_rate[w] = self.shock_rate[i];
                    self.mat[w] = self.mat[i];
                    self.body_id[w] = self.body_id[i];
                    self.det_state[w] = self.det_state[i];
                    self.det_timer[w] = self.det_timer[i];
                    self.det_age[w] = self.det_age[i];
                    self.crumb_angle[w] = self.crumb_angle[i];
                    self.crumb_spin[w] = self.crumb_spin[i];
                    self.crumb_seed[w] = self.crumb_seed[i];
                    self.step_scratch[w] = self.step_scratch[i];
                    self.grid_next[w] = self.grid_next[i];
                }
                w += 1;
            }
        }
        let removed = n - w;
        if removed > 0 {
            self.pos.truncate(w);
            self.vel.truncate(w);
            self.acc.truncate(w);
            self.grav_acc.truncate(w);
            self.xsph_dv.truncate(w);
            self.mass.truncate(w);
            self.rho0.truncate(w);
            self.rho.truncate(w);
            self.s_xx.truncate(w);
            self.s_yy.truncate(w);
            self.s_xy.truncate(w);
            self.sph_pressure.truncate(w);
            self.net_pressure.truncate(w);
            self.energy.truncate(w);
            self.temp.truncate(w);
            self.plastic_strain.truncate(w);
            self.damage.truncate(w);
            self.shock_rate.truncate(w);
            self.mat.truncate(w);
            self.body_id.truncate(w);
            self.det_state.truncate(w);
            self.det_timer.truncate(w);
            self.det_age.truncate(w);
            self.crumb_angle.truncate(w);
            self.crumb_spin.truncate(w);
            self.crumb_seed.truncate(w);
            self.step_scratch.truncate(w);
            self.grid_next.truncate(w);
            self.recount_status();
            self.gpu_dirty = true;
            self.state_epoch = self.state_epoch.wrapping_add(1);
        }
        (removed, map)
    }

    /// Paint non-overlapping honeycomb continuum particles inside a disc.
    pub fn paint_matter_disc(
        &mut self,
        center: Vec2,
        radius: f32,
        mat: MaterialId,
        vel: Vec2,
        temp_override: Option<f32>,
        body_id: u16,
    ) -> usize {
        const MAX_PAINT_TOTAL: usize = 190_000;
        let r = radius.max(0.5 * self.dx);
        let dx = self.dx.max(0.05);
        let row_dy = dx * 0.866_025_4;
        let min_sep2 = (0.76 * dx) * (0.76 * dx);
        let r2 = r * r;

        self.rebuild_spatial_grid();
        let inv_cell = 1.0 / self.grid_cell_size.max(1e-4);
        let mask = self.grid_mask;

        let nx = (r / dx).ceil() as i32;
        let ny = (r / row_dy).ceil() as i32;
        let base_gx = (center.x / dx).round();
        let base_gy = (center.y / row_dy).round();
        let mut added = 0usize;

        'outer: for iy in -ny..=ny {
            let gy_idx = base_gy as i32 + iy;
            let py = gy_idx as f32 * row_dy;
            let row_shift = if gy_idx & 1 != 0 { 0.5 * dx } else { 0.0 };
            for ix in -nx..=nx {
                if self.pos.len() >= MAX_PAINT_TOTAL {
                    break 'outer;
                }
                let px = (base_gx as i32 + ix) as f32 * dx + row_shift;
                let candidate = Vec2::new(px, py);
                if (candidate - center).length_squared() > r2 {
                    continue;
                }

                // Check existing particles + newly painted particles via spatial hash
                let cx = (candidate.x * inv_cell).floor() as i32;
                let cy = (candidate.y * inv_cell).floor() as i32;
                let mut overlap = false;
                'cell_check: for oy in -1..=1 {
                    for ox in -1..=1 {
                        let bucket = Self::hash_cell(cx + ox, cy + oy, mask);
                        let mut cur = self.grid_head[bucket];
                        while cur >= 0 {
                            let j = cur as usize;
                            if (self.pos[j] - candidate).length_squared() < min_sep2 {
                                overlap = true;
                                break 'cell_check;
                            }
                            cur = self.grid_next[j];
                        }
                    }
                }
                if overlap {
                    continue;
                }

                let idx = self.push_particle(candidate, vel, mat, body_id, temp_override);
                let bucket = Self::hash_cell(cx, cy, mask);
                if idx >= self.grid_next.len() {
                    self.grid_next.resize(idx + 1, -1);
                }
                self.grid_next[idx] = self.grid_head[bucket];
                self.grid_head[bucket] = idx as i32;
                added += 1;
            }
        }
        if added > 0 {
            self.recount_status();
            self.gpu_dirty = true;
            self.state_epoch = self.state_epoch.wrapping_add(1);
        }
        added
    }

    /// Erase particles inside a circular void brush and return `(removed, old_to_new)`.
    pub fn erase_matter_disc(&mut self, center: Vec2, radius: f32) -> (usize, Vec<Option<usize>>) {
        let r2 = (radius.max(0.25 * self.dx)).powi(2);
        let keep: Vec<bool> = self
            .pos
            .iter()
            .map(|&p| (p - center).length_squared() > r2)
            .collect();
        self.retain_particles(|i| keep[i])
    }

    /// Raycast an ablation thermal laser from `origin` along `dir`, halting at the front solid surface.
    pub fn apply_thermal_laser(
        &mut self,
        origin: Vec2,
        dir: Vec2,
        beam_radius: f32,
        power_mult: f32,
        dt_eff: f32,
    ) -> Option<Vec2> {
        let n = self.pos.len();
        if n == 0 {
            return None;
        }
        let d_hat = dir.normalize_or(Vec2::X);
        let perp_hat = Vec2::new(-d_hat.y, d_hat.x);
        let rad = beam_radius.max(0.75 * self.dx);
        let hit_slab = rad + 0.55 * self.dx;

        let mut min_t = f32::INFINITY;
        for i in 0..n {
            if self.det_state[i] == 2 {
                continue;
            }
            let rel = self.pos[i] - origin;
            let t = rel.dot(d_hat);
            if t < 0.0 {
                continue;
            }
            let perp = rel.dot(perp_hat).abs();
            if perp <= hit_slab && t < min_t {
                min_t = t;
            }
        }
        if !min_t.is_finite() {
            return None;
        }

        let depth_cap = min_t + 1.65 * rad;
        let step_scale = power_mult.max(0.1) * dt_eff.max(0.02);
        let mut hit_sum = Vec2::ZERO;
        let mut hit_w = 0.0f32;

        for i in 0..n {
            if self.det_state[i] == 2 {
                continue;
            }
            let rel = self.pos[i] - origin;
            let t = rel.dot(d_hat);
            if t < min_t - 0.65 * self.dx || t > depth_cap {
                continue;
            }
            let perp = rel.dot(perp_hat).abs();
            if perp > hit_slab {
                continue;
            }
            let radial_falloff = (1.0 - (perp / hit_slab).powi(2)).clamp(0.12, 1.0);
            let depth_falloff =
                (1.0 - ((t - min_t).max(0.0) / (depth_cap - min_t).max(0.1))).clamp(0.15, 1.0);
            let w = radial_falloff * depth_falloff;

            let m = self.mat[i].props();
            let e_add = 0.055 * step_scale * w;
            self.energy[i] += e_add;
            self.temp[i] = (self.temp[i] + e_add / m.cv.max(1e-7)).clamp(50.0, 24_000.0);
            self.damage[i] = (self.damage[i] + 0.14 * step_scale * w).min(1.0);
            self.shock_rate[i] += 0.25 * w;
            self.vel[i] += d_hat * (0.006 * step_scale * w / m.rho0.max(0.4));

            if m.category == crate::materials::MaterialCategory::ExplosivesFuzes
                && m.detonation_vel > 0.0
                && self.det_state[i] == 0
                && self.temp[i] >= m.melt_temp.max(460.0)
            {
                self.det_state[i] = 1;
                self.det_timer[i] = 0.015;
            } else if self.mat[i] == MaterialId::ThermitePyro
                && self.det_state[i] == 0
                && self.temp[i] >= 1150.0
            {
                self.det_state[i] = 1;
                self.det_age[i] = 0.0;
            }
            hit_sum += self.pos[i] * w;
            hit_w += w;
        }

        if hit_w > 0.0 {
            self.recount_status();
            self.gpu_dirty = true;
            self.state_epoch = self.state_epoch.wrapping_add(1);
            Some(hit_sum / hit_w)
        } else {
            None
        }
    }

    /// Apply a softened 2D gravity-well or repulsor vortex centered at `center`.
    pub fn apply_gravity_well(&mut self, center: Vec2, strength: f32, swirl: f32, dt_eff: f32) {
        let n = self.pos.len();
        if n == 0 {
            return;
        }
        let soft = 2.8 * self.dx.max(0.1);
        let scale = dt_eff.max(0.015);
        let pos_ref = &self.pos;
        self.vel.par_iter_mut().enumerate().for_each(|(i, vel)| {
            let d = center - pos_ref[i];
            let r = (d.length_squared() + soft * soft).sqrt();
            let n_hat = d / r;
            let t_hat = Vec2::new(-n_hat.y, n_hat.x);
            let a = n_hat * (strength / (r + soft)) + t_hat * (swirl / (r + 1.25 * soft));
            let dv = (a * scale).clamp_length_max(1.25);
            *vel += dv;
        });
        self.gpu_dirty = true;
        self.state_epoch = self.state_epoch.wrapping_add(1);
    }

    /// Detonate an instantaneous radial shockwave + thermal flash over `radius`.
    pub fn trigger_shock_blast(
        &mut self,
        center: Vec2,
        radius: f32,
        peak_vel: f32,
        peak_temp_k: f32,
    ) -> usize {
        let r_max = radius.max(self.dx);
        let r2_max = r_max * r_max;
        let mut hit = 0usize;
        for i in 0..self.pos.len() {
            let d = self.pos[i] - center;
            let r2 = d.length_squared();
            if r2 > r2_max {
                continue;
            }
            let r = r2.sqrt();
            let f = (1.0 - r / r_max).clamp(0.0, 1.0).powf(1.25);
            let dir = if r > 1e-4 { d / r } else { Vec2::X };
            let m = self.mat[i].props();

            self.vel[i] += dir * (peak_vel * f / m.rho0.sqrt().max(0.55));
            let d_temp = (peak_temp_k - self.temp[i]).max(0.0) * f;
            self.energy[i] += d_temp * m.cv;
            self.temp[i] = (self.temp[i] + d_temp).clamp(50.0, 24_000.0);
            self.shock_rate[i] += 1.4 * f;
            self.damage[i] = (self.damage[i] + 0.90 * f * f).min(1.0);
            if m.category == crate::materials::MaterialCategory::ExplosivesFuzes
                && m.detonation_vel > 0.0
                && self.det_state[i] == 0
            {
                self.det_state[i] = 1;
                self.det_timer[i] = 0.01;
            } else if self.mat[i] == MaterialId::ThermitePyro
                && self.det_state[i] == 0
                && self.temp[i] >= 1150.0
            {
                self.det_state[i] = 1;
                self.det_age[i] = 0.0;
            }
            hit += 1;
        }
        if hit > 0 {
            self.recount_status();
            self.gpu_dirty = true;
            self.state_epoch = self.state_epoch.wrapping_add(1);
        }
        hit
    }
}
