use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2 as EguiVec2,
};
use glam::Vec2;

use crate::blueprint::blueprint_storage_dir;
use crate::materials::MaterialId;
use crate::physics::{ContinuumWorld, WorldSnapshot};
use crate::render::{Camera2D, VisMode};

// ============================================================================
// 1. INSTANT TIMELINE SCRUBBER & KEYFRAME RING BUFFER
// ============================================================================

pub struct TimelineRecorder {
    pub keyframes: Vec<WorldSnapshot>,
    pub active_index: Option<usize>,
    last_record_time: f32,
    max_keyframes: usize,
}

impl TimelineRecorder {
    pub fn new() -> Self {
        Self {
            keyframes: Vec::with_capacity(96),
            active_index: None,
            last_record_time: -10.0,
            max_keyframes: 96,
        }
    }

    pub fn reset_with_world(&mut self, world: &ContinuumWorld) {
        self.keyframes.clear();
        self.active_index = None;
        self.last_record_time = -10.0;
        if world.len() > 0 {
            self.keyframes.push(world.capture_snapshot());
            self.last_record_time = world.time_us;
        }
    }

    pub fn maybe_record(&mut self, world: &ContinuumWorld) {
        if world.len() == 0 {
            return;
        }
        // If user scrubbed backward and then resumed, discard invalidated future keyframes
        if let Some(idx) = self.active_index.take() {
            if idx + 1 < self.keyframes.len() {
                self.keyframes.truncate(idx + 1);
            }
        }
        if self.keyframes.is_empty() || world.time_us - self.last_record_time >= 0.85 {
            if self.keyframes.len() >= self.max_keyframes {
                // Thin older keyframes pairwise rather than dropping t=0!
                if self.keyframes.len() > 4 {
                    self.keyframes.remove(1);
                }
            }
            self.keyframes.push(world.capture_snapshot());
            self.last_record_time = world.time_us;
        }
    }

    pub fn scrub_to(&mut self, idx: usize, world: &mut ContinuumWorld) {
        if idx < self.keyframes.len() {
            world.restore_snapshot(&self.keyframes[idx], false);
            self.active_index = Some(idx);
        }
    }

    pub fn commit_branch(&mut self, world: &mut ContinuumWorld) {
        if let Some(idx) = self.active_index.take() {
            if idx < self.keyframes.len() {
                world.restore_snapshot(&self.keyframes[idx], true);
                self.keyframes.truncate(idx + 1);
                self.last_record_time = world.time_us;
            }
        }
    }
}

// ============================================================================
// 2. LAGRANGIAN WITNESS GAUGES (PIEZO / MANGANIN ARMOR SENSORS)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GaugeChannel {
    PressureGpa,
    SpeedKms,
    VonMisesGpa,
    TempKelvin,
}

impl GaugeChannel {
    pub const ALL: [GaugeChannel; 4] = [
        GaugeChannel::PressureGpa,
        GaugeChannel::SpeedKms,
        GaugeChannel::VonMisesGpa,
        GaugeChannel::TempKelvin,
    ];

    pub fn short_label(self) -> &'static str {
        match self {
            GaugeChannel::PressureGpa => "P(t) GPa",
            GaugeChannel::SpeedKms => "v(t) km/s",
            GaugeChannel::VonMisesGpa => "σ_eq GPa",
            GaugeChannel::TempKelvin => "T(t) K",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GaugeSample {
    pub time_us: f32,
    pub pressure_gpa: f32,
    pub speed_kms: f32,
    pub von_mises_gpa: f32,
    pub temp_k: f32,
}

impl GaugeSample {
    pub fn value(self, ch: GaugeChannel) -> f32 {
        match ch {
            GaugeChannel::PressureGpa => self.pressure_gpa,
            GaugeChannel::SpeedKms => self.speed_kms,
            GaugeChannel::VonMisesGpa => self.von_mises_gpa,
            GaugeChannel::TempKelvin => self.temp_k,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WitnessGauge {
    pub id: usize,
    pub particle_idx: usize,
    pub label: String,
    pub color: Color32,
    pub samples: Vec<GaugeSample>,
}

const GAUGE_PALETTE: [Color32; 8] = [
    Color32::from_rgb(80, 230, 255),
    Color32::from_rgb(255, 185, 65),
    Color32::from_rgb(115, 245, 145),
    Color32::from_rgb(255, 95, 185),
    Color32::from_rgb(185, 135, 255),
    Color32::from_rgb(255, 240, 95),
    Color32::from_rgb(255, 110, 95),
    Color32::from_rgb(95, 175, 255),
];

pub struct GaugeBank {
    pub gauges: Vec<WitnessGauge>,
    pub channel: GaugeChannel,
    last_sample_time: f32,
    next_id: usize,
}

impl GaugeBank {
    pub fn new() -> Self {
        Self {
            gauges: Vec::new(),
            channel: GaugeChannel::PressureGpa,
            last_sample_time: -10.0,
            next_id: 1,
        }
    }

    pub fn clear(&mut self) {
        self.gauges.clear();
        self.last_sample_time = -10.0;
        self.next_id = 1;
    }

    pub fn clear_waveforms_keep_pins(&mut self, world: &ContinuumWorld) {
        self.last_sample_time = -10.0;
        self.gauges.retain(|g| g.particle_idx < world.len());
        for g in &mut self.gauges {
            g.samples.clear();
        }
        self.record_now(world);
    }

    pub fn remap_indices(&mut self, old_to_new: &[Option<usize>]) {
        self.gauges.retain_mut(|g| {
            if let Some(&Some(new_idx)) = old_to_new.get(g.particle_idx) {
                g.particle_idx = new_idx;
                true
            } else {
                false
            }
        });
    }

    pub fn toggle_or_pin_at(&mut self, world: &ContinuumWorld, world_p: Vec2) -> Option<String> {
        if world.len() == 0 {
            return None;
        }
        let remove_tol2 = (2.4 * world.dx).max(0.8).powi(2);
        if let Some(existing) = self.gauges.iter().position(|g| {
            g.particle_idx < world.len()
                && (world.pos[g.particle_idx] - world_p).length_squared() <= remove_tol2
        }) {
            let removed = self.gauges.remove(existing);
            return Some(format!("Removed witness gauge {}", removed.label));
        }

        let mut best_i = None;
        let mut best_d2 = (4.5 * world.dx).max(1.8).powi(2);
        for i in 0..world.len() {
            let d2 = (world.pos[i] - world_p).length_squared();
            if d2 < best_d2 {
                best_d2 = d2;
                best_i = Some(i);
            }
        }
        let p_idx = best_i?;
        if self.gauges.len() >= 8 {
            self.gauges.remove(0);
        }
        let id = self.next_id;
        self.next_id += 1;
        let color = GAUGE_PALETTE[(id - 1) % GAUGE_PALETTE.len()];
        let mat_name = world.mat[p_idx].props().short_name;
        let label = format!("G{id} ({mat_name})");
        let mut gauge = WitnessGauge {
            id,
            particle_idx: p_idx,
            label: label.clone(),
            color,
            samples: Vec::with_capacity(512),
        };
        gauge.samples.push(Self::sample_particle(world, p_idx));
        self.gauges.push(gauge);
        Some(format!("Pinned witness gauge {label} at ({:.1}, {:.1}) cm", world.pos[p_idx].x, world.pos[p_idx].y))
    }

    fn sample_particle(world: &ContinuumWorld, i: usize) -> GaugeSample {
        GaugeSample {
            time_us: world.time_us,
            pressure_gpa: world.net_pressure[i] * 100.0,
            speed_kms: world.vel[i].length() * 10.0,
            von_mises_gpa: world.von_mises_gpa(i),
            temp_k: world.temp[i],
        }
    }

    pub fn record_now(&mut self, world: &ContinuumWorld) {
        if self.gauges.is_empty() {
            return;
        }
        if (world.time_us - self.last_sample_time).abs() < 0.08 && self.last_sample_time >= 0.0 {
            return;
        }
        self.last_sample_time = world.time_us;
        for g in &mut self.gauges {
            if g.particle_idx < world.len() {
                if let Some(last) = g.samples.last() {
                    if world.time_us < last.time_us {
                        g.samples.retain(|s| s.time_us <= world.time_us);
                    }
                }
                if g.samples.len() >= 600 {
                    g.samples.remove(0);
                }
                g.samples.push(Self::sample_particle(world, g.particle_idx));
            }
        }
    }

    pub fn draw_markers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        cam: &Camera2D,
        vp_w: f32,
        vp_h: f32,
        world: &ContinuumWorld,
    ) {
        for g in &self.gauges {
            if g.particle_idx >= world.len() {
                continue;
            }
            let s = cam.world_to_screen(world.pos[g.particle_idx], vp_w, vp_h);
            let p = Pos2::new(rect.min.x + s.x, rect.min.y + s.y);
            if !rect.contains(p) {
                continue;
            }
            let r = 5.5;
            let pts = vec![
                Pos2::new(p.x, p.y - r),
                Pos2::new(p.x + r, p.y),
                Pos2::new(p.x, p.y + r),
                Pos2::new(p.x - r, p.y),
                Pos2::new(p.x, p.y - r),
            ];
            painter.add(egui::Shape::line(
                pts,
                Stroke::new(1.8_f32, g.color),
            ));
            painter.circle_filled(p, 1.8, Color32::WHITE);
            painter.text(
                Pos2::new(p.x + 7.0, p.y - 7.0),
                Align2::LEFT_BOTTOM,
                format!("G{}", g.id),
                FontId::monospace(10.5),
                g.color,
            );
        }
    }

    pub fn draw_oscilloscope(&mut self, painter: &egui::Painter, rect: Rect) {
        if self.gauges.is_empty() {
            return;
        }
        let w = 325.0;
        let h = 128.0;
        let panel = Rect::from_min_size(
            Pos2::new(rect.min.x + 14.0, rect.min.y + 56.0),
            EguiVec2::new(w, h),
        );
        painter.rect_filled(
            panel,
            CornerRadius::same(6),
            Color32::from_rgba_unmultiplied(11, 15, 24, 222),
        );
        painter.rect_stroke(
            panel,
            CornerRadius::same(6),
            Stroke::new(1.0_f32, Color32::from_rgb(65, 95, 135)),
            egui::StrokeKind::Outside,
        );

        painter.text(
            Pos2::new(panel.min.x + 8.0, panel.min.y + 6.0),
            Align2::LEFT_TOP,
            format!(
                "WITNESS GAUGE OSCILLOSCOPE — {}",
                self.channel.short_label()
            ),
            FontId::monospace(10.0),
            Color32::from_rgb(160, 215, 250),
        );

        let plot = Rect::from_min_max(
            Pos2::new(panel.min.x + 8.0, panel.min.y + 24.0),
            Pos2::new(panel.max.x - 8.0, panel.max.y - 22.0),
        );
        painter.rect_filled(plot, CornerRadius::same(3), Color32::from_rgb(7, 10, 15));

        let mut t_min = f32::INFINITY;
        let mut t_max = 1.0f32;
        let mut v_min = 0.0f32;
        let mut v_max = 1.0f32;
        for g in &self.gauges {
            for s in &g.samples {
                t_min = t_min.min(s.time_us);
                t_max = t_max.max(s.time_us);
                let val = s.value(self.channel);
                v_min = v_min.min(val);
                v_max = v_max.max(val);
            }
        }
        if !t_min.is_finite() {
            t_min = 0.0;
        }
        let dt_span = (t_max - t_min).max(0.5);
        let dv_span = (v_max - v_min).max(0.2);

        // Zero reference line if negative spall pressures exist
        if v_min < -0.01 && v_max > 0.01 {
            let y_zero = plot.max.y - ((0.0 - v_min) / dv_span) * plot.height();
            painter.line_segment(
                [Pos2::new(plot.min.x, y_zero), Pos2::new(plot.max.x, y_zero)],
                Stroke::new(1.0_f32, Color32::from_rgb(45, 58, 78)),
            );
        }

        for g in &self.gauges {
            if g.samples.len() < 2 {
                continue;
            }
            let pts: Vec<Pos2> = g
                .samples
                .iter()
                .map(|s| {
                    let tx = ((s.time_us - t_min) / dt_span).clamp(0.0, 1.0);
                    let vy = ((s.value(self.channel) - v_min) / dv_span).clamp(0.0, 1.0);
                    Pos2::new(
                        plot.min.x + tx * plot.width(),
                        plot.max.y - vy * plot.height(),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, g.color)));
        }

        // Legend row with live values
        let mut lx = panel.min.x + 8.0;
        for g in self.gauges.iter().take(4) {
            let cur = g
                .samples
                .last()
                .map(|s| s.value(self.channel))
                .unwrap_or(0.0);
            let txt = format!("G{}:{:+.1}", g.id, cur);
            painter.text(
                Pos2::new(lx, panel.max.y - 11.0),
                Align2::LEFT_CENTER,
                &txt,
                FontId::monospace(9.5),
                g.color,
            );
            lx += 76.0;
        }
    }
}

// ============================================================================
// 3. INTERACTIVE BALLISTIC CALIPER & LOS RHAe ARMOR ANALYZER
// ============================================================================

#[derive(Clone, Debug, Default)]
pub struct CaliperReport {
    pub span_cm: f32,
    pub dx_cm: f32,
    pub dy_cm: f32,
    pub angle_deg: f32,
    pub obliquity_deg: f32,
    pub solid_chord_mm: f32,
    pub areal_density_kg_m2: f32,
    pub mass_rhae_mm: f32,
    pub ke_rhae_mm: f32,
}

pub fn analyze_caliper_ray(world: &ContinuumWorld, a: Vec2, b: Vec2) -> CaliperReport {
    let d = b - a;
    let span_cm = d.length();
    if span_cm < 1e-4 {
        return CaliperReport::default();
    }
    let angle_deg = d.y.atan2(d.x).to_degrees();
    // Obliquity relative to horizontal shot line (0° = normal to vertical plate)
    let obliquity_deg = angle_deg.abs().min((180.0 - angle_deg.abs()).abs());

    if world.len() == 0 {
        return CaliperReport {
            span_cm,
            dx_cm: d.x,
            dy_cm: d.y,
            angle_deg,
            obliquity_deg,
            ..Default::default()
        };
    }

    let pad = 1.15 * world.dx;
    let min_x = a.x.min(b.x) - pad;
    let max_x = a.x.max(b.x) + pad;
    let min_y = a.y.min(b.y) - pad;
    let max_y = a.y.max(b.y) + pad;

    // Pre-filter candidate solid particles inside ray AABB for <0.05ms execution
    let mut cands: Vec<usize> = Vec::with_capacity(256);
    for i in 0..world.len() {
        if world.det_state[i] == 2 {
            continue;
        }
        let p = world.pos[i];
        if p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y {
            cands.push(i);
        }
    }

    let step_cm = (0.45 * world.dx).max(0.06);
    let n_steps = ((span_cm / step_cm).ceil() as usize).clamp(1, 2000);
    let ds = span_cm / n_steps as f32;
    let hit_r2 = (0.72 * world.dx) * (0.72 * world.dx);

    let rha_rho = MaterialId::RhaSteel.props().rho0;
    let rha_z = MaterialId::RhaSteel.props().acoustic_impedance_mrayl();
    let rha_y = MaterialId::RhaSteel.props().yield_strength;

    let mut solid_cm = 0.0f32;
    let mut areal_g_cm2 = 0.0f32;
    let mut ke_equiv_cm = 0.0f32;

    for s_idx in 0..n_steps {
        let t = (s_idx as f32 + 0.5) / n_steps as f32;
        let pt = a + d * t;
        let mut best_i = None;
        let mut best_d2 = hit_r2;
        for &ci in &cands {
            let dist2 = (world.pos[ci] - pt).length_squared();
            if dist2 <= best_d2 {
                best_d2 = dist2;
                best_i = Some(ci);
            }
        }
        if let Some(hit) = best_i {
            let props = world.mat[hit].props();
            let rho = world.rho[hit];
            let dmg_factor = 1.0 - 0.55 * world.damage[hit].clamp(0.0, 1.0);
            solid_cm += ds;
            areal_g_cm2 += rho * ds;

            // Ballistic KE protection efficiency factor combining density, shock impedance & strength
            let z_ratio = (props.acoustic_impedance_mrayl() / rha_z).clamp(0.25, 2.2);
            let str_bonus =
                1.0 + 0.22 * ((props.yield_strength / rha_y).clamp(0.1, 6.0) - 1.0);
            let ke_factor =
                (0.55 * (rho / rha_rho) + 0.45 * z_ratio) * str_bonus * dmg_factor;
            ke_equiv_cm += ds * ke_factor.max(0.15);
        }
    }

    CaliperReport {
        span_cm,
        dx_cm: d.x,
        dy_cm: d.y,
        angle_deg,
        obliquity_deg,
        solid_chord_mm: solid_cm * 10.0,
        areal_density_kg_m2: areal_g_cm2 * 10.0,
        mass_rhae_mm: (areal_g_cm2 / rha_rho) * 10.0,
        ke_rhae_mm: ke_equiv_cm * 10.0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_caliper_overlay(
    painter: &egui::Painter,
    rect: Rect,
    cam: &Camera2D,
    vp_w: f32,
    vp_h: f32,
    a_w: Vec2,
    b_w: Vec2,
    rep: &CaliperReport,
) {
    let sa = cam.world_to_screen(a_w, vp_w, vp_h);
    let sb = cam.world_to_screen(b_w, vp_w, vp_h);
    let pa = Pos2::new(rect.min.x + sa.x, rect.min.y + sa.y);
    let pb = Pos2::new(rect.min.x + sb.x, rect.min.y + sb.y);

    let col = Color32::from_rgb(115, 245, 190);
    painter.line_segment([pa, pb], Stroke::new(2.0_f32, col));

    let seg = EguiVec2::new(pb.x - pa.x, pb.y - pa.y);
    let len = seg.length();
    if len > 2.0 {
        let perp = EguiVec2::new(-seg.y / len, seg.x / len) * 6.5;
        painter.line_segment([pa - perp, pa + perp], Stroke::new(2.0_f32, col));
        painter.line_segment([pb - perp, pb + perp], Stroke::new(2.0_f32, col));
    }
    painter.circle_filled(pa, 3.2, Color32::WHITE);
    painter.circle_filled(pb, 3.2, Color32::WHITE);

    let text = format!(
        "CALIPER: {:.2} cm ({:+.1}, {:+.1}) | ∠{:.1}° (Obliq {:.1}°)\nSolid LOS: {:5.1} mm  |  Areal: {:5.0} kg/m²\nMass RHAe: {:5.1} mm  |  KE RHAe: {:5.1} mm",
        rep.span_cm,
        rep.dx_cm,
        rep.dy_cm,
        rep.angle_deg,
        rep.obliquity_deg,
        rep.solid_chord_mm,
        rep.areal_density_kg_m2,
        rep.mass_rhae_mm,
        rep.ke_rhae_mm,
    );

    let mid = Pos2::new(0.5 * (pa.x + pb.x), 0.5 * (pa.y + pb.y) - 34.0);
    let card = Rect::from_center_size(
        Pos2::new(
            mid.x.clamp(rect.min.x + 170.0, rect.max.x - 170.0),
            mid.y.clamp(rect.min.y + 42.0, rect.max.y - 42.0),
        ),
        EguiVec2::new(336.0, 52.0),
    );
    painter.rect_filled(
        card,
        CornerRadius::same(5),
        Color32::from_rgba_unmultiplied(10, 18, 24, 228),
    );
    painter.rect_stroke(
        card,
        CornerRadius::same(5),
        Stroke::new(1.2_f32, col),
        egui::StrokeKind::Outside,
    );
    painter.text(
        Pos2::new(card.min.x + 8.0, card.min.y + 6.0),
        Align2::LEFT_TOP,
        text,
        FontId::monospace(10.2),
        Color32::from_rgb(215, 245, 232),
    );
}

// ============================================================================
// 4. QUANTITATIVE COLORBAR SCALE & FLOW VELOCITY VECTOR OVERLAY
// ============================================================================

pub fn draw_quantitative_colorbar(painter: &egui::Painter, rect: Rect, mode: VisMode) {
    let (title, min_lbl, mid_lbl, max_lbl, stops): (
        &str,
        &str,
        &str,
        &str,
        &[[u8; 3]; 5],
    ) = match mode {
        VisMode::MaterialThermal | VisMode::DamageAndPhase | VisMode::PenetrationXray => return,
        VisMode::ShockPressure => (
            "SHOCK / SPALL P",
            "-3.2 GPa",
            "0 GPa",
            "+38+ GPa",
            &[
                [255, 36, 148],
                [33, 41, 59],
                [20, 148, 235],
                [107, 250, 245],
                [255, 250, 194],
            ],
        ),
        VisMode::PlasticStrain => (
            "PLASTIC STRAIN εp",
            "0.00",
            "0.16",
            "0.65+",
            &[
                [20, 15, 56],
                [122, 28, 110],
                [230, 92, 41],
                [252, 204, 56],
                [255, 252, 224],
            ],
        ),
        VisMode::VelocityField => (
            "SPEED |v|",
            "0.0 km/s",
            "1.4 km/s",
            "5.5+ km/s",
            &[
                [36, 46, 122],
                [26, 173, 224],
                [82, 235, 107],
                [252, 184, 38],
                [255, 56, 46],
            ],
        ),
        VisMode::TemperatureFlir => (
            "FLIR TEMP (K)",
            "290 K",
            "1100 K",
            "3600+ K",
            &[
                [20, 15, 56],
                [122, 28, 110],
                [230, 92, 41],
                [252, 204, 56],
                [255, 252, 224],
            ],
        ),
        VisMode::BondNetworkXray => (
            "VON MISES σ_eq",
            "0.0 GPa",
            "0.65 GPa",
            "2.6+ GPa",
            &[
                [36, 46, 122],
                [26, 173, 224],
                [82, 235, 107],
                [252, 184, 38],
                [255, 56, 46],
            ],
        ),
    };

    let w = 212.0;
    let h = 46.0;
    let box_r = Rect::from_min_size(
        Pos2::new(rect.max.x - w - 14.0, rect.min.y + 12.0),
        EguiVec2::new(w, h),
    );
    painter.rect_filled(
        box_r,
        CornerRadius::same(5),
        Color32::from_rgba_unmultiplied(12, 16, 26, 215),
    );
    painter.rect_stroke(
        box_r,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, Color32::from_rgb(58, 82, 118)),
        egui::StrokeKind::Outside,
    );
    painter.text(
        Pos2::new(box_r.center().x, box_r.min.y + 8.0),
        Align2::CENTER_CENTER,
        title,
        FontId::monospace(9.8),
        Color32::from_rgb(185, 215, 245),
    );

    let bar = Rect::from_min_max(
        Pos2::new(box_r.min.x + 10.0, box_r.min.y + 17.0),
        Pos2::new(box_r.max.x - 10.0, box_r.min.y + 27.0),
    );
    let segs = 32usize;
    let sw = bar.width() / segs as f32;
    for k in 0..segs {
        let u = (k as f32 + 0.5) / segs as f32 * 4.0;
        let idx = (u.floor() as usize).min(3);
        let f = u - idx as f32;
        let c0 = stops[idx];
        let c1 = stops[idx + 1];
        let col = Color32::from_rgb(
            (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * f) as u8,
            (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * f) as u8,
            (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * f) as u8,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(bar.min.x + k as f32 * sw, bar.min.y),
                Pos2::new(bar.min.x + (k + 1) as f32 * sw + 0.5, bar.max.y),
            ),
            CornerRadius::ZERO,
            col,
        );
    }

    let tc = Color32::from_rgb(175, 195, 220);
    painter.text(
        Pos2::new(bar.min.x, bar.max.y + 8.0),
        Align2::LEFT_CENTER,
        min_lbl,
        FontId::monospace(9.2),
        tc,
    );
    painter.text(
        Pos2::new(bar.center().x, bar.max.y + 8.0),
        Align2::CENTER_CENTER,
        mid_lbl,
        FontId::monospace(9.2),
        tc,
    );
    painter.text(
        Pos2::new(bar.max.x, bar.max.y + 8.0),
        Align2::RIGHT_CENTER,
        max_lbl,
        FontId::monospace(9.2),
        tc,
    );
}

pub fn draw_velocity_vectors(
    painter: &egui::Painter,
    rect: Rect,
    cam: &Camera2D,
    vp_w: f32,
    vp_h: f32,
    world: &ContinuumWorld,
) {
    let n = world.len();
    if n == 0 {
        return;
    }
    let stride = (n / 420).max(1);
    let mut i = 0usize;
    while i < n {
        let v = world.vel[i];
        let speed_kms = v.length() * 10.0;
        if speed_kms >= 0.18 {
            let s0 = cam.world_to_screen(world.pos[i], vp_w, vp_h);
            let p0 = Pos2::new(rect.min.x + s0.x, rect.min.y + s0.y);
            if rect.contains(p0) {
                let dir = v.normalize_or_zero();
                let len_px = (5.0 + speed_kms * 4.2).clamp(6.0, 28.0);
                let p1 = Pos2::new(p0.x + dir.x * len_px, p0.y - dir.y * len_px);
                let col = if speed_kms > 3.5 {
                    Color32::from_rgba_unmultiplied(255, 230, 110, 210)
                } else {
                    Color32::from_rgba_unmultiplied(110, 225, 255, 185)
                };
                painter.line_segment([p0, p1], Stroke::new(1.3_f32, col));
                let wing = EguiVec2::new(-dir.y, -dir.x) * 2.6;
                let back = Pos2::new(p1.x - dir.x * 3.8, p1.y + dir.y * 3.8);
                painter.line_segment([p1, back + wing], Stroke::new(1.2_f32, col));
                painter.line_segment([p1, back - wing], Stroke::new(1.2_f32, col));
            }
        }
        i += stride;
    }
}

// ============================================================================
// 5. ZERO-DEPENDENCY LOSSLESS PNG FRAME EXPORTER & CSV TELEMETRY EXPORTER
// ============================================================================

fn crc32_ieee(chunks: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for slice in chunks {
        for &b in *slice {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
    }
    !crc
}

fn push_png_chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(payload);
    let crc = crc32_ieee(&[tag, payload]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode an `egui::Color32` pixel buffer into a valid RFC-2083 24-bit RGB `.png` byte stream!
pub fn encode_rgb_png(width: usize, height: usize, pixels: &[Color32]) -> Vec<u8> {
    let row_len = 1 + width * 3;
    let raw_len = row_len * height;
    let mut raw = Vec::with_capacity(raw_len);
    for y in 0..height {
        raw.push(0u8); // PNG filter type 0 (None)
        let row = &pixels[y * width..(y + 1) * width];
        for px in row {
            let [r, g, b, _] = px.to_array();
            raw.push(r);
            raw.push(g);
            raw.push(b);
        }
    }

    // Zlib stream wrapping stored DEFLATE blocks (RFC 1950 / 1951) + Adler-32 checksum
    let mut zlib = Vec::with_capacity(raw.len() + (raw.len() / 65535 + 2) * 8);
    zlib.push(0x78); // CMF: deflate, 32K window
    zlib.push(0x01); // FLG: no preset dict, FCHECK valid (0x7801 % 31 == 0)

    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in &raw {
        s1 = (s1 + b as u32) % 65_521;
        s2 = (s2 + s1) % 65_521;
    }
    let adler = (s2 << 16) | s1;

    let chunks: Vec<&[u8]> = raw.chunks(65_535).collect();
    if chunks.is_empty() {
        zlib.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        for (idx, block) in chunks.iter().enumerate() {
            let is_last = idx + 1 == chunks.len();
            zlib.push(u8::from(is_last));
            let len = block.len() as u16;
            let nlen = !len;
            zlib.extend_from_slice(&len.to_le_bytes());
            zlib.extend_from_slice(&nlen.to_le_bytes());
            zlib.extend_from_slice(block);
        }
    }
    zlib.extend_from_slice(&adler.to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 64);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.push(8); // 8 bits/channel
    ihdr.push(2); // Color type 2 = Truecolor RGB
    ihdr.push(0); // Compression
    ihdr.push(0); // Filter
    ihdr.push(0); // Interlace = None
    push_png_chunk(&mut png, b"IHDR", &ihdr);
    push_png_chunk(&mut png, b"IDAT", &zlib);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn unix_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn export_viewport_png(
    prefix: &str,
    width: usize,
    height: usize,
    pixels: &[Color32],
) -> Result<PathBuf, String> {
    if width == 0 || height == 0 || pixels.len() < width * height {
        return Err("Viewport buffer is empty".to_string());
    }
    let dir = blueprint_storage_dir("captures");
    let safe: String = prefix
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(24)
        .collect();
    let path = dir.join(format!("{}_{}.png", safe.trim_matches('_'), unix_stamp()));
    let bytes = encode_rgb_png(width, height, pixels);
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn export_telemetry_csv(
    prefix: &str,
    world: &ContinuumWorld,
    gauges: &GaugeBank,
) -> Result<PathBuf, String> {
    let dir = blueprint_storage_dir("exports");
    let safe: String = prefix
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(24)
        .collect();
    let path = dir.join(format!("{}_{}.csv", safe.trim_matches('_'), unix_stamp()));

    let mut csv = String::with_capacity(8192);
    csv.push_str("# Rusted Space Simulation (rssim) — Continuum Hydrocode Telemetry Export\n");
    csv.push_str("time_us,kinetic_energy,internal_energy,elastic_energy,total_energy,peak_pressure_gpa,max_speed_kms,spall_fraction,momentum_x,momentum_y\n");
    for s in &world.history {
        let tot_e = s.kinetic_energy + s.internal_energy + s.elastic_energy;
        csv.push_str(&format!(
            "{:.4},{:.6},{:.6},{:.6},{:.6},{:.3},{:.3},{:.5},{:.6},{:.6}\n",
            s.time_us,
            s.kinetic_energy,
            s.internal_energy,
            s.elastic_energy,
            tot_e,
            s.peak_pressure_gpa,
            s.max_speed_kms,
            s.fractured_frac,
            s.momentum.x,
            s.momentum.y
        ));
    }

    if !gauges.gauges.is_empty() {
        csv.push_str("\n# Lagrangian Witness Gauge Waveforms\n");
        csv.push_str("gauge_id,gauge_label,time_us,net_pressure_gpa,von_mises_gpa,speed_kms,temp_k\n");
        for g in &gauges.gauges {
            for s in &g.samples {
                csv.push_str(&format!(
                    "G{},\"{}\",{:.4},{:.4},{:.4},{:.4},{:.1}\n",
                    g.id, g.label, s.time_us, s.pressure_gpa, s.von_mises_gpa, s.speed_kms, s.temp_k
                ));
            }
        }
    }

    fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossless_png_encoder_produces_valid_png_stream() {
        let pixels = vec![Color32::from_rgb(180, 95, 42); 16 * 12];
        let png = encode_rgb_png(16, 12, &pixels);
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        assert!(png.len() > 16 * 12 * 3);
    }

    #[test]
    fn caliper_rhae_analyzer_measures_rha_plate_accurately() {
        let mut world = ContinuumWorld::new(0.35);
        crate::scenarios::spawn_oriented_box(
            &mut world,
            Vec2::new(10.0, 0.0),
            Vec2::new(2.5, 10.0), // 5.0 cm = 50.0 mm thick RHA plate!
            0.0,
            Vec2::ZERO,
            MaterialId::RhaSteel,
            None,
            None,
        );
        let rep = analyze_caliper_ray(&world, Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0));
        assert!(
            (rep.mass_rhae_mm - 50.0).abs() < 6.0,
            "Expected ~50 mm RHAe across 5 cm RHA plate, got {:.1} mm",
            rep.mass_rhae_mm
        );
        assert!(
            (rep.ke_rhae_mm - 50.0).abs() < 6.0,
            "Expected ~50 mm KE RHAe across 5 cm RHA plate, got {:.1} mm",
            rep.ke_rhae_mm
        );
    }
}
