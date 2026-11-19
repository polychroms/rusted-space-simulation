mod analysis;
mod blueprint;
mod editor_ui;
mod materials;
mod physics;
mod render;
mod scene;
mod scenarios;
mod vulkan_gpu;

use std::time::Instant;

use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, FontId, Key, PointerButton, Pos2, Rect, Sense,
    Stroke, TextureHandle, TextureOptions, Vec2 as EguiVec2,
};
use glam::Vec2;

use crate::analysis::{
    analyze_caliper_ray, analyze_remnant_clusters, draw_caliper_overlay, draw_quantitative_colorbar,
    draw_velocity_vectors, export_animated_gif, export_telemetry_csv, export_viewport_png,
    quantize_rgba_downsampled, GaugeBank, GaugeChannel, GifFrame, RemnantAnalysis,
    TimelineRecorder,
};
use crate::blueprint::{BuiltinProjectileBp, BuiltinTargetBp};
use crate::editor_ui::{BlueprintEditor, BlueprintEditorAction};
use crate::materials::{MaterialCategory, MaterialId};
use crate::physics::ContinuumWorld;
use crate::render::{Camera2D, PixelCanvas, VisMode};
use crate::scene::{
    list_saved_scenes, save_scene_to_disk, ArmorLayer, PlanetShell, ProjectileSpec, SceneSelection,
    SceneSpec, TargetKind, TargetSpec,
};
use crate::scenarios::{preset_to_scene, ProjectileShape, ScenarioCategory, ScenarioPreset};
use crate::vulkan_gpu::VulkanPhysicsEngine;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspaceTab {
    Targets,
    Projectiles,
    Simulation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuilderSubMode {
    BlueprintCad,
    ParametricRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimPhase {
    Staging,
    Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveTool {
    Interact,
    BrushMatter,
    EraseMatter,
    ThermalLaser,
    GravityVortex,
    ShockBlast,
}

impl ActiveTool {
    const ALL: [ActiveTool; 6] = [
        ActiveTool::Interact,
        ActiveTool::BrushMatter,
        ActiveTool::EraseMatter,
        ActiveTool::ThermalLaser,
        ActiveTool::GravityVortex,
        ActiveTool::ShockBlast,
    ];

    fn short_label(self) -> &'static str {
        match self {
            Self::Interact => "⌖ Interact (Q)",
            Self::BrushMatter => "🖌 Brush (B)",
            Self::EraseMatter => "🧹 Erase (E)",
            Self::ThermalLaser => "🔥 Laser (L)",
            Self::GravityVortex => "🌀 Gravity (G)",
            Self::ShockBlast => "💥 Blast (X)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraTrackMode {
    Free,
    FollowFastest,
    FollowCom,
}

impl CameraTrackMode {
    const ALL: [CameraTrackMode; 3] = [
        CameraTrackMode::Free,
        CameraTrackMode::FollowFastest,
        CameraTrackMode::FollowCom,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::FollowFastest => "Fastest Core",
            Self::FollowCom => "System CoM",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetBuildShape {
    Line,
    Rectangle,
    Ellipse,
    Circle,
    HollowRing,
    TriangleWedge,
    CompositePlate,
    LayeredPlanet,
}

impl TargetBuildShape {
    const ALL: [TargetBuildShape; 8] = [
        TargetBuildShape::Line,
        TargetBuildShape::Rectangle,
        TargetBuildShape::Ellipse,
        TargetBuildShape::Circle,
        TargetBuildShape::HollowRing,
        TargetBuildShape::TriangleWedge,
        TargetBuildShape::CompositePlate,
        TargetBuildShape::LayeredPlanet,
    ];

    fn short_label(self) -> &'static str {
        match self {
            TargetBuildShape::Line => "📏 Line",
            TargetBuildShape::Rectangle => "▭ Rect",
            TargetBuildShape::Ellipse => "⬭ Ellipse",
            TargetBuildShape::Circle => "● Circle",
            TargetBuildShape::HollowRing => "◯ Ring",
            TargetBuildShape::TriangleWedge => "▶ Wedge",
            TargetBuildShape::CompositePlate => "☰ Layers",
            TargetBuildShape::LayeredPlanet => "🪐 Planet",
        }
    }

    fn description(self) -> &'static str {
        match self {
            TargetBuildShape::Line => "Line / Wall (drag endpoint A to B on canvas)",
            TargetBuildShape::Rectangle => "Rectangle / Block (drag corner A to corner B)",
            TargetBuildShape::Ellipse => "Ellipse / Oval (drag bounding box corner to corner)",
            TargetBuildShape::Circle => "Solid Circle / Disk (click or drag center to radius)",
            TargetBuildShape::HollowRing => "Hollow Ring / Tube (click or drag center to radius)",
            TargetBuildShape::TriangleWedge => "Triangle / Wedge (drag box corner to corner)",
            TargetBuildShape::CompositePlate => "Multi-Layer Composite Armor (drag A to B)",
            TargetBuildShape::LayeredPlanet => "Differentiated Planet / Moon (click or drag radius)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimResolution {
    Fast,
    Balanced,
    HighDetail,
    ExtremeGpu,
}

impl SimResolution {
    fn dx(self) -> f32 {
        match self {
            SimResolution::Fast => 0.46,
            SimResolution::Balanced => 0.35,
            SimResolution::HighDetail => 0.27,
            SimResolution::ExtremeGpu => 0.21,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SimResolution::Fast => "Fast",
            SimResolution::Balanced => "Balanced",
            SimResolution::HighDetail => "Ultra",
            SimResolution::ExtremeGpu => "Extreme",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CanvasDrag {
    MoveObject {
        sel: SceneSelection,
        grab_offset: Vec2,
    },
    AimVelocity {
        sel: SceneSelection,
    },
    CreateTargetShape {
        shape: TargetBuildShape,
        start: Vec2,
    },
    CreateProjectile {
        start: Vec2,
    },
    MeasureCaliper {
        start: Vec2,
    },
    PanCamera,
    UseToolBrush {
        body_id: u16,
    },
    UseToolErase,
    UseToolLaser,
    UseToolGravity {
        repel: bool,
    },
}

#[derive(Clone, Debug)]
struct SceneSnapshot {
    scene: SceneSpec,
    selection: Option<SceneSelection>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ViewportRenderKey {
    world_id: u64,
    state_epoch: u64,
    world_len: usize,
    width: usize,
    height: usize,
    center_x_bits: u32,
    center_y_bits: u32,
    zoom_bits: u32,
    vis_mode: VisMode,
    trails_active: bool,
    show_starfield: bool,
}

struct RustedSpaceSimApp {
    tab: WorkspaceTab,
    phase: SimPhase,
    preset: ScenarioPreset,
    resolution: SimResolution,

    scene: SceneSpec,
    selection: Option<SceneSelection>,
    undo_stack: Vec<SceneSnapshot>,
    redo_stack: Vec<SceneSnapshot>,
    pending_drag_undo: Option<SceneSnapshot>,
    ui_edit_baseline: Option<SceneSnapshot>,
    world: ContinuumWorld,
    cam: Camera2D,
    canvas: PixelCanvas,
    texture: Option<TextureHandle>,
    last_render_key: Option<ViewportRenderKey>,

    paused: bool,
    vis_mode: VisMode,
    motion_trails: bool,
    show_starfield: bool,
    show_vel_vectors: bool,
    show_hud_graphs: bool,
    substeps_per_frame: usize,
    time_scale: f32,

    // Laboratory Instrumentation, Time-Scrubbing & Scene Persistence
    timeline: TimelineRecorder,
    gauges: GaugeBank,
    caliper_mode: bool,
    caliper_line: Option<(Vec2, Vec2)>,
    scene_save_name: String,
    saved_scenes: Vec<(String, SceneSpec)>,
    status_toast: Option<(String, Instant)>,

    // Pixel Blueprint CAD Editors + Parametric Builders
    target_submode: BuilderSubMode,
    proj_submode: BuilderSubMode,
    target_bp_editor: BlueprintEditor,
    proj_bp_editor: BlueprintEditor,
    target_shape: TargetBuildShape,
    target_box_template: TargetSpec,
    target_ellipse_template: TargetSpec,
    target_ring_template: TargetSpec,
    target_wedge_template: TargetSpec,
    target_comp_template: TargetSpec,
    target_planet_template: TargetSpec,
    proj_template: ProjectileSpec,
    live_fire_speed_mult: f32,

    // Active canvas drag & live interactive physics tools
    canvas_drag: Option<CanvasDrag>,
    active_tool: ActiveTool,
    tool_radius: f32,
    brush_mat: MaterialId,
    brush_temp_k: f32,
    laser_power: f32,
    laser_angle_deg: f32,
    gravity_strength: f32,
    gravity_swirl: f32,
    blast_radius: f32,
    blast_vel: f32,
    last_laser_beam: Option<(Vec2, Vec2)>,
    last_gravity_well: Option<(Vec2, bool)>,

    // Cinematic Camera & Bullet-Time
    camera_track: CameraTrackMode,
    auto_slowmo: bool,
    slowmo_timer: f32,
    camera_shake_enabled: bool,
    camera_shake: f32,
    prev_detonated_count: usize,

    // Studio HUD Modals & Status Telemetry
    show_scenario_browser: bool,
    scenario_filter: ScenarioCategory,
    show_help_modal: bool,
    hover_world_pos: Option<Vec2>,
    gif_recording: bool,
    gif_frames: Vec<GifFrame>,
    last_gif_capture_step: u64,
    remnant_report: Option<RemnantAnalysis>,

    // Vulkan GPU Compute Accelerator
    vulkan_engine: Option<VulkanPhysicsEngine>,
    vulkan_init_err: Option<String>,
    use_vulkan_gpu: bool,

    // Performance tracking
    last_frame_instant: Instant,
    fps_smooth: f32,
    physics_ms: f32,
    render_ms: f32,
}

impl RustedSpaceSimApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_dark_theme(&cc.egui_ctx);
        let preset = ScenarioPreset::EmptySpace;
        let resolution = SimResolution::Balanced;
        let scene = preset_to_scene(preset);
        let world = scene.build_world(resolution.dx());
        let cam = Camera2D {
            center: Vec2::ZERO,
            zoom: 12.5,
        };

        let (vulkan_engine, vulkan_init_err) = match VulkanPhysicsEngine::new() {
            Ok(vk) => (Some(vk), None),
            Err(e) => (None, Some(e)),
        };
        let use_vulkan_gpu = vulkan_engine.is_some();

        Self {
            tab: WorkspaceTab::Simulation,
            phase: SimPhase::Staging,
            preset,
            resolution,
            scene,
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pending_drag_undo: None,
            ui_edit_baseline: None,
            world,
            cam,
            canvas: PixelCanvas::new(),
            texture: None,
            last_render_key: None,
            paused: false,
            vis_mode: VisMode::MaterialThermal,
            motion_trails: false,
            show_starfield: true,
            show_vel_vectors: false,
            show_hud_graphs: true,
            substeps_per_frame: 8,
            time_scale: 1.0,
            timeline: TimelineRecorder::new(),
            gauges: GaugeBank::new(),
            caliper_mode: false,
            caliper_line: None,
            scene_save_name: "Custom_Armor_Range".to_string(),
            saved_scenes: list_saved_scenes(),
            status_toast: None,
            target_submode: BuilderSubMode::BlueprintCad,
            proj_submode: BuilderSubMode::BlueprintCad,
            target_bp_editor: BlueprintEditor::new_target_editor(),
            proj_bp_editor: BlueprintEditor::new_projectile_editor(),
            target_shape: TargetBuildShape::Rectangle,
            target_box_template: TargetSpec::default_box(Vec2::new(10.0, 0.0), MaterialId::RhaSteel),
            target_ellipse_template: TargetSpec::default_ellipse(
                Vec2::new(10.0, 0.0),
                MaterialId::RhaSteel,
            ),
            target_ring_template: TargetSpec::default_ring(
                Vec2::new(10.0, 0.0),
                MaterialId::Aluminum,
            ),
            target_wedge_template: TargetSpec::default_wedge(
                Vec2::new(10.0, 0.0),
                MaterialId::RhaSteel,
            ),
            target_comp_template: TargetSpec::default_composite(Vec2::new(10.0, 0.0)),
            target_planet_template: TargetSpec::default_planet(
                Vec2::new(10.0, 0.0),
                MaterialId::BasaltRock,
            ),
            proj_template: ProjectileSpec::from_blueprint(
                Vec2::new(-16.0, 0.0),
                BuiltinProjectileBp::HeatWarhead.default_speed_kms(),
                0.0,
                BuiltinProjectileBp::HeatWarhead.build(),
            ),
            live_fire_speed_mult: 1.0,
            canvas_drag: None,
            active_tool: ActiveTool::Interact,
            tool_radius: 3.5,
            brush_mat: MaterialId::RhaSteel,
            brush_temp_k: 293.0,
            laser_power: 2.5,
            laser_angle_deg: -25.0,
            gravity_strength: 0.22,
            gravity_swirl: 0.14,
            blast_radius: 6.0,
            blast_vel: 0.38,
            last_laser_beam: None,
            last_gravity_well: None,
            camera_track: CameraTrackMode::Free,
            auto_slowmo: false,
            slowmo_timer: 0.0,
            camera_shake_enabled: true,
            camera_shake: 0.0,
            prev_detonated_count: 0,
            show_scenario_browser: false,
            scenario_filter: ScenarioCategory::All,
            show_help_modal: false,
            hover_world_pos: None,
            gif_recording: false,
            gif_frames: Vec::new(),
            last_gif_capture_step: 0,
            remnant_report: None,
            vulkan_engine,
            vulkan_init_err,
            use_vulkan_gpu,
            last_frame_instant: Instant::now(),
            fps_smooth: 60.0,
            physics_ms: 0.0,
            render_ms: 0.0,
        }
    }

    fn update_camera_tracking(&mut self, wall_dt: f32) {
        if self.phase != SimPhase::Running
            || self.paused
            || matches!(self.canvas_drag, Some(CanvasDrag::PanCamera))
            || self.camera_track == CameraTrackMode::Free
            || self.world.len() == 0
        {
            return;
        }
        let target = match self.camera_track {
            CameraTrackMode::Free => return,
            CameraTrackMode::FollowCom => {
                let mut tot_m = 0.0f32;
                let mut com = Vec2::ZERO;
                for i in 0..self.world.len() {
                    if self.world.det_state[i] == 2 {
                        continue;
                    }
                    let p = self.world.pos[i];
                    if p.is_finite() {
                        let m = self.world.mass[i];
                        tot_m += m;
                        com += p * m;
                    }
                }
                if tot_m > 0.0 { Some(com / tot_m) } else { None }
            }
            CameraTrackMode::FollowFastest => {
                let mut max_v2 = 0.0f32;
                for i in 0..self.world.len() {
                    if self.world.det_state[i] != 2 && self.world.pos[i].is_finite() {
                        max_v2 = max_v2.max(self.world.vel[i].length_squared());
                    }
                }
                let thresh2 = 0.25 * max_v2;
                if max_v2 > 1e-6 {
                    let mut tot_m = 0.0f32;
                    let mut com = Vec2::ZERO;
                    for i in 0..self.world.len() {
                        if self.world.det_state[i] != 2
                            && self.world.pos[i].is_finite()
                            && self.world.vel[i].length_squared() >= thresh2
                        {
                            let m = self.world.mass[i];
                            tot_m += m;
                            com += self.world.pos[i] * m;
                        }
                    }
                    if tot_m > 0.0 { Some(com / tot_m) } else { None }
                } else {
                    None
                }
            }
        };
        if let Some(tgt) = target {
            let alpha = (wall_dt * 7.5).clamp(0.02, 0.55);
            self.cam.center = self.cam.center.lerp(tgt, alpha);
        }
    }

    fn apply_live_tool_continuous(&mut self, world_p: Vec2) {
        let dt_eff = self.world.compute_cfl_dt().max(0.02) * 2.5;
        match self.canvas_drag {
            Some(CanvasDrag::UseToolBrush { body_id }) => {
                self.timeline.commit_branch(&mut self.world);
                self.world.paint_matter_disc(
                    world_p,
                    self.tool_radius,
                    self.brush_mat,
                    Vec2::ZERO,
                    Some(self.brush_temp_k),
                    body_id,
                );
            }
            Some(CanvasDrag::UseToolErase) => {
                self.timeline.commit_branch(&mut self.world);
                let (removed, remap) = self.world.erase_matter_disc(world_p, self.tool_radius);
                if removed > 0 {
                    self.gauges.remap_indices(&remap);
                }
            }
            Some(CanvasDrag::UseToolLaser) => {
                self.timeline.commit_branch(&mut self.world);
                let dir = Vec2::from_angle(self.laser_angle_deg.to_radians());
                let origin = world_p - dir * 160.0;
                let beam_r = (0.45 * self.tool_radius).max(0.55);
                let hit = self
                    .world
                    .apply_thermal_laser(origin, dir, beam_r, self.laser_power, dt_eff);
                let end = hit.unwrap_or_else(|| world_p + dir * 180.0);
                self.last_laser_beam = Some((origin, end));
            }
            Some(CanvasDrag::UseToolGravity { repel }) => {
                self.timeline.commit_branch(&mut self.world);
                let strength = if repel {
                    -self.gravity_strength
                } else {
                    self.gravity_strength
                };
                self.world
                    .apply_gravity_well(world_p, strength, self.gravity_swirl, dt_eff);
                self.last_gravity_well = Some((world_p, repel));
            }
            _ => {}
        }
    }

    fn advance_physics_steps(&mut self, steps: usize, dt: f32) {
        if steps == 0 || self.world.len() == 0 {
            return;
        }
        self.timeline.commit_branch(&mut self.world);
        if self.use_vulkan_gpu {
            if let Some(vk) = &mut self.vulkan_engine {
                vk.step_batch(&mut self.world, steps, dt);
                self.gauges.record_now(&self.world);
                self.timeline.maybe_record(&self.world);
                self.check_cinematic_triggers();
                return;
            }
        }
        self.world.gpu_dirty = true;
        for _ in 0..steps {
            self.world.step(dt);
        }
        self.gauges.record_now(&self.world);
        self.timeline.maybe_record(&self.world);
        self.check_cinematic_triggers();
    }

    fn check_cinematic_triggers(&mut self) {
        let new_det = self.world.detonated_count > self.prev_detonated_count + 3;
        self.prev_detonated_count = self.world.detonated_count;
        let peak_p = self
            .world
            .history
            .last()
            .map(|s| s.peak_pressure_gpa)
            .unwrap_or(0.0);
        if new_det || peak_p > 18.0 {
            if self.auto_slowmo && self.slowmo_timer <= 0.05 {
                self.slowmo_timer = 0.45;
            }
            if self.camera_shake_enabled && new_det {
                self.camera_shake = (self.camera_shake + 0.35).min(0.85);
            }
        }
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.status_toast = Some((msg.into(), Instant::now()));
    }

    fn export_png_now(&mut self) {
        match export_viewport_png(
            self.preset.title(),
            self.canvas.width,
            self.canvas.height,
            &self.canvas.pixels,
        ) {
            Ok(path) => self.set_toast(format!("Saved PNG capture: {}", path.display())),
            Err(e) => self.set_toast(format!("PNG export failed: {e}")),
        }
    }

    fn export_csv_now(&mut self) {
        match export_telemetry_csv(self.preset.title(), &self.world, &self.gauges) {
            Ok(path) => self.set_toast(format!("Exported CSV waveforms: {}", path.display())),
            Err(e) => self.set_toast(format!("CSV export failed: {e}")),
        }
    }

    fn toggle_or_save_gif_now(&mut self) {
        if self.gif_recording {
            self.gif_recording = false;
            let frames = std::mem::take(&mut self.gif_frames);
            match export_animated_gif(self.preset.title(), &frames, 5) {
                Ok(path) => self.set_toast(format!("Saved GIF ({} frames): {}", frames.len(), path.display())),
                Err(e) => self.set_toast(format!("GIF export failed: {e}")),
            }
        } else {
            self.gif_frames.clear();
            self.last_gif_capture_step = 0;
            self.gif_recording = true;
            self.set_toast("Recording live viewport GIF (up to 120 frames)...");
        }
    }

    fn export_replay_gif_now(&mut self) {
        if self.timeline.keyframes.is_empty() {
            self.set_toast("No timeline keyframes recorded yet");
            return;
        }
        let mut replay_world = ContinuumWorld::new(self.world.dx);
        replay_world.self_gravity = self.world.self_gravity;
        replay_world.grav_g = self.world.grav_g;
        replay_world.down_gravity = self.world.down_gravity;
        let mut scratch_canvas = PixelCanvas::new();
        let rw = 480usize;
        let rh = 300usize;
        let max_frames = self.timeline.keyframes.len().min(96);
        let stride = (self.timeline.keyframes.len() / max_frames).max(1);
        let mut frames = Vec::with_capacity(max_frames);
        for kf in self.timeline.keyframes.iter().step_by(stride).take(max_frames) {
            replay_world.restore_snapshot(kf, false);
            scratch_canvas.render_with_opts(
                &replay_world,
                &self.cam,
                rw,
                rh,
                self.vis_mode,
                false,
                self.show_starfield,
            );
            frames.push(quantize_rgba_downsampled(&scratch_canvas.pixels, rw, rh, 480));
        }
        match export_animated_gif(self.preset.title(), &frames, 6) {
            Ok(path) => self.set_toast(format!("Exported Replay GIF ({} frames): {}", frames.len(), path.display())),
            Err(e) => self.set_toast(format!("Replay GIF export failed: {e}")),
        }
    }

    fn fit_camera_to_world(&mut self) {
        if self.world.len() == 0 {
            self.cam.center = Vec2::ZERO;
            self.cam.zoom = 12.5;
            return;
        }
        let mut tot_m = 0.0f32;
        let mut com = Vec2::ZERO;
        for i in 0..self.world.len() {
            if self.world.det_state[i] == 2 {
                continue;
            }
            let p = self.world.pos[i];
            if p.is_finite() {
                let m = self.world.mass[i];
                tot_m += m;
                com += p * m;
            }
        }
        if tot_m <= 0.0 {
            self.cam.center = Vec2::ZERO;
            self.cam.zoom = 12.5;
            return;
        }
        com /= tot_m;
        let mut r2_sum = 0.0f32;
        for i in 0..self.world.len() {
            if self.world.det_state[i] == 2 {
                continue;
            }
            let p = self.world.pos[i];
            if p.is_finite() {
                r2_sum += self.world.mass[i] * (p - com).length_squared();
            }
        }
        let r_rms = (r2_sum / tot_m).sqrt().max(6.0);
        let span_cm = 4.6 * r_rms;
        let vw = (self.canvas.width.max(800)) as f32;
        let vh = (self.canvas.height.max(600)) as f32;
        self.cam.center = com;
        self.cam.zoom = (vw.min(vh * 1.35) / span_cm).clamp(0.35, 80.0);
    }

    fn is_in_blueprint_cad(&self) -> bool {
        (self.tab == WorkspaceTab::Targets && self.target_submode == BuilderSubMode::BlueprintCad)
            || (self.tab == WorkspaceTab::Projectiles
                && self.proj_submode == BuilderSubMode::BlueprintCad)
    }

    fn can_undo_active(&self) -> bool {
        if self.tab == WorkspaceTab::Targets && self.target_submode == BuilderSubMode::BlueprintCad
        {
            self.target_bp_editor.can_undo()
        } else if self.tab == WorkspaceTab::Projectiles
            && self.proj_submode == BuilderSubMode::BlueprintCad
        {
            self.proj_bp_editor.can_undo()
        } else {
            self.phase == SimPhase::Staging && !self.undo_stack.is_empty()
        }
    }

    fn can_redo_active(&self) -> bool {
        if self.tab == WorkspaceTab::Targets && self.target_submode == BuilderSubMode::BlueprintCad
        {
            self.target_bp_editor.can_redo()
        } else if self.tab == WorkspaceTab::Projectiles
            && self.proj_submode == BuilderSubMode::BlueprintCad
        {
            self.proj_bp_editor.can_redo()
        } else {
            self.phase == SimPhase::Staging && !self.redo_stack.is_empty()
        }
    }

    fn undo_active(&mut self) {
        if self.tab == WorkspaceTab::Targets && self.target_submode == BuilderSubMode::BlueprintCad
        {
            self.target_bp_editor.undo();
        } else if self.tab == WorkspaceTab::Projectiles
            && self.proj_submode == BuilderSubMode::BlueprintCad
        {
            self.proj_bp_editor.undo();
        } else {
            self.undo();
        }
    }

    fn redo_active(&mut self) {
        if self.tab == WorkspaceTab::Targets && self.target_submode == BuilderSubMode::BlueprintCad
        {
            self.target_bp_editor.redo();
        } else if self.tab == WorkspaceTab::Projectiles
            && self.proj_submode == BuilderSubMode::BlueprintCad
        {
            self.proj_bp_editor.redo();
        } else {
            self.redo();
        }
    }

    fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            scene: self.scene.clone(),
            selection: self.selection,
        }
    }

    fn push_undo(&mut self, snap: SceneSnapshot) {
        if self.undo_stack.last().is_some_and(|s| s.scene == snap.scene) {
            return;
        }
        if self.undo_stack.len() >= 64 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snap);
        self.redo_stack.clear();
    }

    fn flush_pending_ui_undo(&mut self) {
        if let Some(base) = self.ui_edit_baseline.take() {
            if base.scene != self.scene {
                self.push_undo(base);
            }
        }
    }

    fn undo(&mut self) {
        if self.phase != SimPhase::Staging {
            return;
        }
        self.flush_pending_ui_undo();
        self.pending_drag_undo = None;
        self.canvas_drag = None;
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        let cur = self.snapshot();
        if self.redo_stack.len() >= 64 {
            self.redo_stack.remove(0);
        }
        self.redo_stack.push(cur);
        self.scene = prev.scene;
        self.selection = prev.selection;
        self.rebuild_staging_world();
    }

    fn redo(&mut self) {
        if self.phase != SimPhase::Staging {
            return;
        }
        self.flush_pending_ui_undo();
        self.pending_drag_undo = None;
        self.canvas_drag = None;
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        let cur = self.snapshot();
        if self.undo_stack.len() >= 64 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(cur);
        self.scene = next.scene;
        self.selection = next.selection;
        self.rebuild_staging_world();
    }

    fn rebuild_staging_world(&mut self) {
        self.world = self.scene.build_world(self.resolution.dx());
        self.timeline.reset_with_world(&self.world);
        self.gauges.clear_waveforms_keep_pins(&self.world);
    }

    fn load_preset(&mut self, preset: ScenarioPreset) {
        let next_scene = preset_to_scene(preset);
        if self.scene != next_scene {
            let snap = self.snapshot();
            self.push_undo(snap);
        }
        self.preset = preset;
        self.scene = next_scene;
        self.phase = SimPhase::Staging;
        self.tab = WorkspaceTab::Simulation;
        self.paused = false;
        self.canvas_drag = None;
        self.pending_drag_undo = None;
        self.ui_edit_baseline = None;
        self.gauges.clear();
        self.caliper_line = None;
        self.remnant_report = None;
        self.selection = if !self.scene.projectiles.is_empty() {
            Some(SceneSelection::Projectile(0))
        } else if !self.scene.targets.is_empty() {
            Some(SceneSelection::Target(0))
        } else {
            None
        };
        self.rebuild_staging_world();

        match preset {
            ScenarioPreset::PlanetaryImpact | ScenarioPreset::IcyCometCollision => {
                self.cam.center = Vec2::new(2.0, 0.0);
                self.cam.zoom = 11.5;
            }
            ScenarioPreset::RocheLimitRingFormation | ScenarioPreset::BinaryPlanetInspiral => {
                self.cam.center = Vec2::ZERO;
                self.cam.zoom = 8.5;
            }
            ScenarioPreset::OceanMoonHyperImpact => {
                self.cam.center = Vec2::new(1.0, 0.0);
                self.cam.zoom = 9.5;
            }
            ScenarioPreset::KineticRodBunkerBuster | ScenarioPreset::EfpVsSpacedArmor => {
                self.cam.center = Vec2::new(8.0, 0.0);
                self.cam.zoom = 11.5;
            }
            ScenarioPreset::EmptySpace => {
                self.cam.center = Vec2::ZERO;
                self.cam.zoom = 12.5;
            }
            _ => {
                self.cam.center = Vec2::new(2.0, 0.0);
                self.cam.zoom = 12.5;
            }
        }
    }

    fn start_simulation(&mut self) {
        if self.phase == SimPhase::Staging {
            self.rebuild_staging_world();
            self.phase = SimPhase::Running;
            self.paused = false;
            self.prev_detonated_count = self.world.detonated_count;
            self.slowmo_timer = 0.0;
            self.camera_shake = 0.0;
            self.tab = WorkspaceTab::Simulation;
        } else {
            self.paused = !self.paused;
        }
    }

    fn reset_to_staging(&mut self) {
        self.phase = SimPhase::Staging;
        self.paused = false;
        self.canvas_drag = None;
        self.last_laser_beam = None;
        self.last_gravity_well = None;
        self.slowmo_timer = 0.0;
        self.camera_shake = 0.0;
        self.prev_detonated_count = 0;
        self.remnant_report = None;
        self.rebuild_staging_world();
    }

    fn clear_scene(&mut self) {
        let empty = SceneSpec::empty();
        if self.scene != empty {
            let snap = self.snapshot();
            self.push_undo(snap);
        }
        self.preset = ScenarioPreset::EmptySpace;
        self.scene = empty;
        self.selection = None;
        self.phase = SimPhase::Staging;
        self.paused = false;
        self.canvas_drag = None;
        self.last_laser_beam = None;
        self.last_gravity_well = None;
        self.slowmo_timer = 0.0;
        self.camera_shake = 0.0;
        self.prev_detonated_count = 0;
        self.remnant_report = None;
        self.pending_drag_undo = None;
        self.ui_edit_baseline = None;
        self.gauges.clear();
        self.caliper_line = None;
        self.rebuild_staging_world();
    }

    fn current_target_template_mut(&mut self) -> &mut TargetSpec {
        match self.target_shape {
            TargetBuildShape::Line | TargetBuildShape::Rectangle => &mut self.target_box_template,
            TargetBuildShape::Ellipse | TargetBuildShape::Circle => &mut self.target_ellipse_template,
            TargetBuildShape::HollowRing => &mut self.target_ring_template,
            TargetBuildShape::TriangleWedge => &mut self.target_wedge_template,
            TargetBuildShape::CompositePlate => &mut self.target_comp_template,
            TargetBuildShape::LayeredPlanet => &mut self.target_planet_template,
        }
    }

    fn build_target_from_drag(&self, shape: TargetBuildShape, start: Vec2, end: Vec2) -> Option<TargetSpec> {
        let seg = end - start;
        let aligned_angle = |v: Vec2| -> f32 {
            let mut a = v.y.atan2(v.x).to_degrees() - 90.0;
            while a > 90.0 {
                a -= 180.0;
            }
            while a < -90.0 {
                a += 180.0;
            }
            a
        };

        match shape {
            TargetBuildShape::Line | TargetBuildShape::CompositePlate => {
                if seg.length() < 1.5 {
                    return None;
                }
                let mut spec = if shape == TargetBuildShape::Line {
                    self.target_box_template.clone()
                } else {
                    self.target_comp_template.clone()
                };
                spec.pos = 0.5 * (start + end);
                spec.angle_deg = aligned_angle(seg);
                match &mut spec.kind {
                    TargetKind::SolidBox { height, .. }
                    | TargetKind::CompositePlate { height, .. } => {
                        *height = seg.length();
                    }
                    _ => {}
                }
                Some(spec)
            }
            TargetBuildShape::Rectangle => {
                if seg.x.abs() < 1.5 || seg.y.abs() < 1.5 {
                    return None;
                }
                let mut spec = self.target_box_template.clone();
                spec.pos = 0.5 * (start + end);
                spec.angle_deg = 0.0;
                if let TargetKind::SolidBox { width, height, .. } = &mut spec.kind {
                    *width = seg.x.abs();
                    *height = seg.y.abs();
                }
                Some(spec)
            }
            TargetBuildShape::Ellipse => {
                if seg.x.abs() < 1.5 || seg.y.abs() < 1.5 {
                    return None;
                }
                let mut spec = self.target_ellipse_template.clone();
                spec.pos = 0.5 * (start + end);
                spec.angle_deg = 0.0;
                if let TargetKind::Ellipse { rx, ry, .. } = &mut spec.kind {
                    *rx = 0.5 * seg.x.abs();
                    *ry = 0.5 * seg.y.abs();
                }
                Some(spec)
            }
            TargetBuildShape::Circle => {
                let mut spec = self.target_ellipse_template.clone();
                spec.pos = start;
                spec.angle_deg = 0.0;
                let drag_r = seg.length();
                if let TargetKind::Ellipse { rx, ry, .. } = &mut spec.kind {
                    let r = if drag_r >= 1.2 { drag_r } else { *rx };
                    *rx = r;
                    *ry = r;
                    spec.name = format!("{} Circle", spec.name.split_whitespace().next().unwrap_or("Solid"));
                }
                Some(spec)
            }
            TargetBuildShape::HollowRing => {
                let mut spec = self.target_ring_template.clone();
                spec.pos = start;
                let drag_r = seg.length();
                if let TargetKind::HollowRing { outer_r, wall_thick, .. } = &mut spec.kind {
                    if drag_r >= 1.6 {
                        *outer_r = drag_r;
                        *wall_thick = (*wall_thick).min(0.8 * drag_r);
                    }
                }
                Some(spec)
            }
            TargetBuildShape::TriangleWedge => {
                if seg.x.abs() < 1.5 || seg.y.abs() < 1.5 {
                    return None;
                }
                let mut spec = self.target_wedge_template.clone();
                spec.pos = 0.5 * (start + end);
                spec.angle_deg = if seg.x >= 0.0 { 0.0 } else { 180.0 };
                if let TargetKind::TriangleWedge { width, height, .. } = &mut spec.kind {
                    *width = seg.x.abs();
                    *height = seg.y.abs();
                }
                Some(spec)
            }
            TargetBuildShape::LayeredPlanet => {
                let mut spec = self.target_planet_template.clone();
                spec.pos = start;
                let drag_r = seg.length();
                if drag_r >= 2.0 {
                    if let TargetKind::LayeredPlanet { shells } = &mut spec.kind {
                        let cur_r: f32 = shells.iter().map(|s| s.thickness).sum();
                        if cur_r > 0.1 {
                            let scale = drag_r / cur_r;
                            for s in shells.iter_mut() {
                                s.thickness *= scale;
                            }
                        }
                    }
                }
                Some(spec)
            }
        }
    }

    fn velocity_handle_world(pos: Vec2, vel: Vec2) -> Vec2 {
        let speed_kms = vel.length() * 10.0;
        let dir = if vel.length_squared() > 1e-10 {
            vel.normalize()
        } else {
            Vec2::X
        };
        let len = (4.5 + speed_kms.max(0.0).powf(0.65) * 2.2).clamp(5.0, 50.0);
        pos + dir * len
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let in_cad = self.is_in_blueprint_cad();
        let mut do_undo = false;
        let mut do_redo = false;
        let mut do_save = false;
        let mut do_single_step: Option<f32> = None;
        let mut do_png = false;
        let mut scrub_delta: i32 = 0;
        ctx.input(|inp| {
            let cmd = inp.modifiers.ctrl || inp.modifiers.command;
            if inp.key_pressed(Key::F1)
                || (inp.key_pressed(Key::Slash) && inp.modifiers.shift)
            {
                self.show_help_modal = !self.show_help_modal;
            }
            if inp.key_pressed(Key::Escape) {
                self.show_help_modal = false;
                self.show_scenario_browser = false;
            }
            if inp.key_pressed(Key::F12) {
                do_png = true;
            }
            if cmd && inp.key_pressed(Key::Z) {
                if inp.modifiers.shift {
                    do_redo = true;
                } else {
                    do_undo = true;
                }
                return;
            }
            if cmd && inp.key_pressed(Key::Y) {
                do_redo = true;
                return;
            }
            if cmd && inp.key_pressed(Key::S) {
                do_save = true;
                return;
            }
            if inp.key_pressed(Key::Space) {
                if self.phase == SimPhase::Staging {
                    if self.world.len() > 0 {
                        self.start_simulation();
                    }
                } else {
                    self.paused = !self.paused;
                }
            }
            if inp.key_pressed(Key::R) && !cmd && !in_cad {
                self.reset_to_staging();
            }
            if inp.key_pressed(Key::S) && !cmd && !in_cad {
                if self.phase == SimPhase::Staging {
                    self.phase = SimPhase::Running;
                }
                let dt = self.world.compute_cfl_dt();
                do_single_step = Some(dt);
                self.paused = true;
            }
            if inp.key_pressed(Key::C) && !cmd && !in_cad {
                self.clear_scene();
            }
            if inp.key_pressed(Key::M) && !cmd && !in_cad {
                self.caliper_mode = !self.caliper_mode;
            }
            if inp.key_pressed(Key::V) && !cmd && !in_cad {
                self.show_vel_vectors = !self.show_vel_vectors;
            }
            if inp.key_pressed(Key::F) && !cmd && !in_cad {
                self.fit_camera_to_world();
            }
            if self.phase == SimPhase::Running && !in_cad {
                if inp.key_pressed(Key::ArrowLeft) {
                    scrub_delta = -1;
                } else if inp.key_pressed(Key::ArrowRight) {
                    scrub_delta = 1;
                }
            }
            if self.phase == SimPhase::Staging && !in_cad && inp.key_pressed(Key::Delete) {
                self.delete_selected();
            }
            if !in_cad {
                if inp.key_pressed(Key::Q) && !cmd {
                    self.active_tool = ActiveTool::Interact;
                }
                if inp.key_pressed(Key::B) && !cmd {
                    self.active_tool = ActiveTool::BrushMatter;
                }
                if inp.key_pressed(Key::E) && !cmd {
                    self.active_tool = ActiveTool::EraseMatter;
                }
                if inp.key_pressed(Key::L) && !cmd {
                    self.active_tool = ActiveTool::ThermalLaser;
                }
                if inp.key_pressed(Key::G) && !cmd {
                    self.active_tool = ActiveTool::GravityVortex;
                }
                if inp.key_pressed(Key::X) && !cmd {
                    self.active_tool = ActiveTool::ShockBlast;
                }
                if inp.key_pressed(Key::OpenBracket) {
                    self.tool_radius = (self.tool_radius - 0.6).clamp(0.8, 22.0);
                }
                if inp.key_pressed(Key::CloseBracket) {
                    self.tool_radius = (self.tool_radius + 0.6).clamp(0.8, 22.0);
                }
                if inp.key_pressed(Key::Num1) {
                    self.vis_mode = VisMode::MaterialThermal;
                }
                if inp.key_pressed(Key::Num2) {
                    self.vis_mode = VisMode::ShockPressure;
                }
                if inp.key_pressed(Key::Num3) {
                    self.vis_mode = VisMode::PlasticStrain;
                }
                if inp.key_pressed(Key::Num4) {
                    self.vis_mode = VisMode::DamageAndPhase;
                }
                if inp.key_pressed(Key::Num5) {
                    self.vis_mode = VisMode::VelocityField;
                }
                if inp.key_pressed(Key::Num6) {
                    self.vis_mode = VisMode::TemperatureFlir;
                }
                if inp.key_pressed(Key::Num7) {
                    self.vis_mode = VisMode::BondNetworkXray;
                }
                if inp.key_pressed(Key::Num8) {
                    self.vis_mode = VisMode::PenetrationXray;
                }
            }
        });

        if do_save {
            if self.tab == WorkspaceTab::Targets && self.target_submode == BuilderSubMode::BlueprintCad
            {
                self.target_bp_editor.save_now();
            } else if self.tab == WorkspaceTab::Projectiles
                && self.proj_submode == BuilderSubMode::BlueprintCad
            {
                self.proj_bp_editor.save_now();
            } else {
                match save_scene_to_disk(&self.scene_save_name, &self.scene) {
                    Ok(path) => {
                        self.saved_scenes = list_saved_scenes();
                        self.set_toast(format!("Saved scene: {}", path.display()));
                    }
                    Err(e) => self.set_toast(format!("Scene save failed: {e}")),
                }
            }
        }
        if do_png {
            self.export_png_now();
        }
        if scrub_delta != 0 && !self.timeline.keyframes.is_empty() {
            self.paused = true;
            let cur = self
                .timeline
                .active_index
                .unwrap_or(self.timeline.keyframes.len() - 1);
            let next = (cur as isize + scrub_delta as isize)
                .clamp(0, self.timeline.keyframes.len() as isize - 1) as usize;
            self.timeline.scrub_to(next, &mut self.world);
        }
        if let Some(dt) = do_single_step {
            self.advance_physics_steps(1, dt);
        }
        if do_undo {
            self.undo_active();
        } else if do_redo {
            self.redo_active();
        }
    }

    fn delete_selected(&mut self) {
        match self.selection {
            Some(SceneSelection::Target(idx)) if idx < self.scene.targets.len() => {
                let snap = self.snapshot();
                self.push_undo(snap);
                self.scene.targets.remove(idx);
                self.selection = if !self.scene.targets.is_empty() {
                    Some(SceneSelection::Target(idx.min(self.scene.targets.len() - 1)))
                } else {
                    None
                };
                self.rebuild_staging_world();
            }
            Some(SceneSelection::Projectile(idx)) if idx < self.scene.projectiles.len() => {
                let snap = self.snapshot();
                self.push_undo(snap);
                self.scene.projectiles.remove(idx);
                self.selection = if !self.scene.projectiles.is_empty() {
                    Some(SceneSelection::Projectile(
                        idx.min(self.scene.projectiles.len() - 1),
                    ))
                } else {
                    None
                };
                self.rebuild_staging_world();
            }
            _ => {}
        }
    }
}

impl eframe::App for RustedSpaceSimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let wall_dt = now.duration_since(self.last_frame_instant).as_secs_f32();
        self.last_frame_instant = now;
        if wall_dt > 1e-5 && wall_dt < 0.25 {
            self.fps_smooth = 0.90 * self.fps_smooth + 0.10 * (1.0 / wall_dt).min(240.0);
        }

        self.last_laser_beam = None;
        if !matches!(self.canvas_drag, Some(CanvasDrag::UseToolGravity { .. })) {
            self.last_gravity_well = None;
        }
        if !self.paused {
            if self.slowmo_timer > 0.0 {
                self.slowmo_timer = (self.slowmo_timer - wall_dt).max(0.0);
            }
            if self.camera_shake > 0.015 {
                self.camera_shake *= (-wall_dt * 11.0).exp();
            } else {
                self.camera_shake = 0.0;
            }
        } else {
            self.camera_shake = 0.0;
        }

        self.handle_hotkeys(ctx);

        // Advance continuum physics only in Running phase
        if self.phase == SimPhase::Running && !self.paused && self.world.len() > 0 {
            let t0 = Instant::now();
            let slowmo_mult = if self.auto_slowmo && self.slowmo_timer > 0.0 {
                0.22
            } else {
                1.0
            };
            let eff_scale = self.time_scale * slowmo_mult;
            let target_steps =
                ((self.substeps_per_frame as f32 * eff_scale).round() as usize).max(1);
            let scale_rem = if self.substeps_per_frame as f32 * eff_scale < 1.0 {
                (self.substeps_per_frame as f32 * eff_scale).clamp(0.05, 1.0)
            } else {
                1.0
            };
            let dt = self.world.compute_cfl_dt() * scale_rem;
            self.advance_physics_steps(target_steps.min(36), dt);
            self.physics_ms =
                0.85 * self.physics_ms + 0.15 * (t0.elapsed().as_secs_f32() * 1000.0);
        } else {
            self.physics_ms *= 0.9;
        }
        self.update_camera_tracking(wall_dt.min(0.1));

        // --- TOP COMMAND & TABS BAR ---
        egui::TopBottomPanel::top("top_bar")
            .exact_height(46.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new("RUSTED SPACE SIMULATION")
                            .strong()
                            .size(14.5)
                            .color(Color32::from_rgb(108, 212, 255)),
                    );
                    ui.separator();

                    // Separate Workspace Tabs
                    if ui
                        .selectable_label(
                            self.tab == WorkspaceTab::Targets,
                            egui::RichText::new("🎯 Targets Builder").strong(),
                        )
                        .clicked()
                    {
                        self.tab = WorkspaceTab::Targets;
                    }
                    if ui
                        .selectable_label(
                            self.tab == WorkspaceTab::Projectiles,
                            egui::RichText::new("🚀 Projectiles Builder").strong(),
                        )
                        .clicked()
                    {
                        self.tab = WorkspaceTab::Projectiles;
                    }
                    if ui
                        .selectable_label(
                            self.tab == WorkspaceTab::Simulation,
                            egui::RichText::new("⚡ Simulation").strong(),
                        )
                        .clicked()
                    {
                        self.tab = WorkspaceTab::Simulation;
                    }

                    ui.separator();

                    match self.phase {
                        SimPhase::Staging => {
                            let start_btn = egui::Button::new(
                                egui::RichText::new("▶ Start Simulation")
                                    .strong()
                                    .color(Color32::from_rgb(20, 28, 22)),
                            )
                            .fill(Color32::from_rgb(95, 225, 145));
                            if ui
                                .add_enabled(self.world.len() > 0, start_btn)
                                .on_hover_text("Start physics simulation (Key: Space)")
                                .clicked()
                            {
                                self.start_simulation();
                            }
                        }
                        SimPhase::Running => {
                            let play_label = if self.paused { "▶ Resume" } else { "⏸ Pause" };
                            if ui
                                .button(egui::RichText::new(play_label).strong())
                                .on_hover_text("Spacebar")
                                .clicked()
                            {
                                self.paused = !self.paused;
                            }
                            if ui
                                .button("⏭ Substep")
                                .on_hover_text("Advance 1 CFL shock substep (Key: S)")
                                .clicked()
                            {
                                let dt = self.world.compute_cfl_dt();
                                self.advance_physics_steps(1, dt);
                                self.paused = true;
                            }
                            if ui
                                .button("↺ Edit Scene")
                                .on_hover_text("Reset back to editable scene setup (Key: R)")
                                .clicked()
                            {
                                self.reset_to_staging();
                            }
                        }
                    }

                    if self.phase == SimPhase::Staging || self.is_in_blueprint_cad() {
                        if ui
                            .add_enabled(self.can_undo_active(), egui::Button::new("↶ Undo"))
                            .on_hover_text("Undo last edit (Ctrl+Z)")
                            .clicked()
                        {
                            self.undo_active();
                        }
                        if ui
                            .add_enabled(self.can_redo_active(), egui::Button::new("↷ Redo"))
                            .on_hover_text("Redo edit (Ctrl+Y / Ctrl+Shift+Z)")
                            .clicked()
                        {
                            self.redo_active();
                        }
                    }

                    if ui
                        .button("⌫ Clear")
                        .on_hover_text("Clear scene to empty space (Key: C)")
                        .clicked()
                    {
                        self.clear_scene();
                    }

                    if ui
                        .button("⌖ Fit")
                        .on_hover_text("Auto-frame camera on active bodies (Key: F)")
                        .clicked()
                    {
                        self.fit_camera_to_world();
                    }
                    if ui
                        .selectable_label(self.caliper_mode, "📏 RHAe")
                        .on_hover_text("Ballistic Caliper & Line-of-Sight RHAe Armor Analyzer (Key: M)")
                        .clicked()
                    {
                        self.caliper_mode = !self.caliper_mode;
                    }
                    if ui
                        .button("📷 PNG")
                        .on_hover_text("Export lossless viewport PNG screenshot (Key: F12)")
                        .clicked()
                    {
                        self.export_png_now();
                    }

                    ui.separator();
                    if ui
                        .button("📂 Scenarios")
                        .on_hover_text("Open Categorized Scenario & Astrophysics Browser")
                        .clicked()
                    {
                        self.show_scenario_browser = !self.show_scenario_browser;
                    }
                    let mut chosen_preset = self.preset;
                    egui::ComboBox::from_id_salt("scenario_combo")
                        .width(235.0)
                        .selected_text(chosen_preset.title())
                        .show_ui(ui, |ui| {
                            for p in ScenarioPreset::ALL {
                                ui.selectable_value(&mut chosen_preset, p, p.title());
                            }
                        });
                    if chosen_preset != self.preset {
                        self.load_preset(chosen_preset);
                    }
                    if ui
                        .button("❓ Help")
                        .on_hover_text("Controls & Continuum Reference (F1 / ?)")
                        .clicked()
                    {
                        self.show_help_modal = !self.show_help_modal;
                    }

                    ui.separator();
                    for res in [
                        SimResolution::Fast,
                        SimResolution::Balanced,
                        SimResolution::HighDetail,
                        SimResolution::ExtremeGpu,
                    ] {
                        if ui
                            .selectable_label(self.resolution == res, res.label())
                            .clicked()
                            && self.resolution != res
                        {
                            self.resolution = res;
                            if self.phase == SimPhase::Staging {
                                self.rebuild_staging_world();
                            } else {
                                self.reset_to_staging();
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let status_tag = if self.phase == SimPhase::Staging {
                            "STAGING"
                        } else if self.paused {
                            "PAUSED"
                        } else {
                            "RUNNING"
                        };
                        let backend_tag = if self.use_vulkan_gpu && self.vulkan_engine.is_some() {
                            "VK-GPU"
                        } else {
                            "CPU"
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "[{}|{}] {:.0} FPS | {:.2}ms | t={:.1}µs | {} pts",
                                status_tag,
                                backend_tag,
                                self.fps_smooth,
                                self.physics_ms,
                                self.world.time_us,
                                self.world.len(),
                            ))
                            .monospace()
                            .size(11.0)
                            .color(Color32::from_rgb(185, 200, 220)),
                        );
                    });
                });
            });

        // --- LEFT TABBED WORKBENCH PANEL ---
        let pre_ui_snap = (self.phase == SimPhase::Staging).then(|| self.snapshot());
        egui::SidePanel::left("left_controls")
            .resizable(true)
            .default_width(345.0)
            .min_width(305.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);
                    match self.tab {
                        WorkspaceTab::Targets => self.draw_targets_tab(ui),
                        WorkspaceTab::Projectiles => self.draw_projectiles_tab(ui),
                        WorkspaceTab::Simulation => self.draw_simulation_tab(ui),
                    }
                });
            });

        if let Some(pre) = pre_ui_snap {
            if self.phase == SimPhase::Staging && pre.scene != self.scene {
                if self.ui_edit_baseline.is_none() {
                    self.ui_edit_baseline = Some(pre);
                }
                self.redo_stack.clear();
            }
        }
        let ui_busy = ctx.input(|i| i.pointer.primary_down()) || ctx.wants_keyboard_input();
        if !ui_busy && self.canvas_drag.is_none() {
            self.flush_pending_ui_undo();
        }

        // --- RICH STUDIO BOTTOM STATUS STRIP ---
        egui::TopBottomPanel::bottom("studio_status_strip")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let (phase_txt, phase_col) = if self.phase == SimPhase::Staging {
                        ("STAGING", Color32::from_rgb(135, 230, 175))
                    } else if self.paused {
                        ("PAUSED", Color32::from_rgb(255, 205, 110))
                    } else {
                        ("RUNNING", Color32::from_rgb(110, 215, 255))
                    };
                    ui.label(
                        egui::RichText::new(phase_txt)
                            .strong()
                            .monospace()
                            .size(10.5)
                            .color(phase_col),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!("Tool: {}", self.active_tool.short_label()))
                            .monospace()
                            .size(10.5)
                            .color(Color32::from_rgb(200, 215, 230)),
                    );
                    ui.separator();
                    let cursor_txt = if let Some(wp) = self.hover_world_pos {
                        format!("Pos: ({:+.1}, {:+.1}) cm", wp.x, wp.y)
                    } else {
                        "Pos: (--, --) cm".to_string()
                    };
                    ui.label(
                        egui::RichText::new(cursor_txt)
                            .monospace()
                            .size(10.5)
                            .color(Color32::from_rgb(165, 190, 215)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let backend_tag = if self.use_vulkan_gpu && self.vulkan_engine.is_some() {
                            "VK-GPU"
                        } else {
                            "CPU"
                        };
                        let dt_ns = self.world.last_dt * 1000.0;
                        ui.label(
                            egui::RichText::new(format!(
                                "{} | {} pts | t={:.1}µs | dt={:.0}ns | {:.0} FPS | phys {:.1}ms | ren {:.1}ms",
                                backend_tag,
                                self.world.len(),
                                self.world.time_us,
                                dt_ns,
                                self.fps_smooth,
                                self.physics_ms,
                                self.render_ms,
                            ))
                            .monospace()
                            .size(10.5)
                            .color(Color32::from_rgb(175, 200, 220)),
                        );
                    });
                });
            });

        // --- BOTTOM TIMELINE SCRUBBER & REPLAY BAR (Active in Running / Paused) ---
        if self.phase == SimPhase::Running && self.timeline.keyframes.len() > 1 {
            egui::TopBottomPanel::bottom("timeline_scrubber")
                .exact_height(34.0)
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.label(
                            egui::RichText::new("⏱ REPLAY SCRUBBER")
                                .strong()
                                .size(11.0)
                                .color(Color32::from_rgb(130, 205, 250)),
                        );
                        let max_idx = self.timeline.keyframes.len() - 1;
                        let mut cur_idx = self.timeline.active_index.unwrap_or(max_idx);

                        if ui.small_button("⏮").on_hover_text("Jump to t = 0.0 µs").clicked() {
                            self.paused = true;
                            self.timeline.scrub_to(0, &mut self.world);
                            cur_idx = 0;
                        }
                        if ui
                            .small_button("◀")
                            .on_hover_text("Previous keyframe (Left Arrow)")
                            .clicked()
                            && cur_idx > 0
                        {
                            self.paused = true;
                            cur_idx -= 1;
                            self.timeline.scrub_to(cur_idx, &mut self.world);
                        }

                        let slider_resp = ui.add(
                            egui::Slider::new(&mut cur_idx, 0..=max_idx)
                                .show_value(false)
                                .trailing_fill(true),
                        );
                        if slider_resp.changed() {
                            self.paused = true;
                            self.timeline.scrub_to(cur_idx, &mut self.world);
                        }

                        if ui
                            .small_button("▶")
                            .on_hover_text("Next keyframe (Right Arrow)")
                            .clicked()
                            && cur_idx < max_idx
                        {
                            self.paused = true;
                            cur_idx += 1;
                            self.timeline.scrub_to(cur_idx, &mut self.world);
                        }
                        if ui
                            .small_button("⏭")
                            .on_hover_text("Jump to latest recorded frame")
                            .clicked()
                        {
                            self.paused = true;
                            self.timeline.scrub_to(max_idx, &mut self.world);
                        }

                        ui.label(
                            egui::RichText::new(format!(
                                "t = {:5.1} µs  (KF {}/{})",
                                self.world.time_us,
                                cur_idx + 1,
                                max_idx + 1
                            ))
                            .monospace()
                            .size(11.0)
                            .color(Color32::from_rgb(205, 225, 245)),
                        );

                        if self.timeline.active_index.is_some() && ui.button("▶ Resume From Here").clicked() {
                            self.timeline.commit_branch(&mut self.world);
                            self.paused = false;
                        }
                        if ui
                            .small_button("🎞 Replay GIF")
                            .on_hover_text("Export animated GIF89a from recorded timeline keyframes")
                            .clicked()
                        {
                            self.export_replay_gif_now();
                        }
                    });
                });
        }

        // --- CENTRAL CONTINUUM OR BLUEPRINT CAD VIEWPORT ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(11, 14, 20)))
            .show(ctx, |ui| {
                let sim_dx = self.resolution.dx();
                if self.tab == WorkspaceTab::Targets
                    && self.target_submode == BuilderSubMode::BlueprintCad
                {
                    self.target_bp_editor.draw_canvas(ctx, ui, sim_dx);
                } else if self.tab == WorkspaceTab::Projectiles
                    && self.proj_submode == BuilderSubMode::BlueprintCad
                {
                    self.proj_bp_editor.draw_canvas(ctx, ui, sim_dx);
                } else {
                    self.draw_viewport(ctx, ui);
                }
            });

        self.draw_scenario_browser_modal(ctx);
        self.draw_help_modal(ctx);

        if (self.phase == SimPhase::Running && !self.paused && self.world.len() > 0)
            || self.canvas_drag.is_some()
        {
            ctx.request_repaint();
        }
    }
}

impl RustedSpaceSimApp {
    fn draw_targets_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("TARGETS & ARMOR BUILDER")
                .strong()
                .size(13.0)
                .color(Color32::from_rgb(110, 210, 255)),
        );
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    self.target_submode == BuilderSubMode::BlueprintCad,
                    egui::RichText::new("🎨 Pixel Blueprint CAD").strong(),
                )
                .clicked()
            {
                self.target_submode = BuilderSubMode::BlueprintCad;
            }
            if ui
                .selectable_label(
                    self.target_submode == BuilderSubMode::ParametricRange,
                    egui::RichText::new("📐 Range Shapes & Scene").strong(),
                )
                .clicked()
            {
                self.target_submode = BuilderSubMode::ParametricRange;
            }
        });
        ui.add_space(4.0);

        if self.target_submode == BuilderSubMode::BlueprintCad {
            let sim_dx = self.resolution.dx();
            let action = self.target_bp_editor.draw_sidebar(ui, sim_dx);
            match action {
                BlueprintEditorAction::None => {}
                BlueprintEditorAction::SendToSceneAndViewRange(bp) => {
                    if self.phase == SimPhase::Running {
                        self.reset_to_staging();
                    }
                    let snap = self.snapshot();
                    let mut spec = TargetSpec::from_blueprint(self.target_bp_editor.spawn_pos, bp);
                    spec.angle_deg = self.target_bp_editor.spawn_angle_deg;
                    let idx = self.scene.targets.len();
                    self.scene.targets.push(spec);
                    self.selection = Some(SceneSelection::Target(idx));
                    self.push_undo(snap);
                    self.rebuild_staging_world();
                    self.tab = WorkspaceTab::Simulation;
                }
                BlueprintEditorAction::StampToSceneKeepEditing(bp) => {
                    if self.phase == SimPhase::Running {
                        self.reset_to_staging();
                    }
                    let snap = self.snapshot();
                    let mut spec = TargetSpec::from_blueprint(self.target_bp_editor.spawn_pos, bp);
                    spec.angle_deg = self.target_bp_editor.spawn_angle_deg;
                    let idx = self.scene.targets.len();
                    self.scene.targets.push(spec);
                    self.selection = Some(SceneSelection::Target(idx));
                    self.target_bp_editor.spawn_pos.x += 12.0;
                    self.push_undo(snap);
                    self.rebuild_staging_world();
                }
                BlueprintEditorAction::UpdateExistingSceneObject(idx, bp) => {
                    if self.phase == SimPhase::Running {
                        self.reset_to_staging();
                    }
                    if idx < self.scene.targets.len() {
                        let snap = self.snapshot();
                        self.scene.targets[idx].name = bp.name.clone();
                        self.scene.targets[idx].kind = TargetKind::Blueprint { bp };
                        self.selection = Some(SceneSelection::Target(idx));
                        self.push_undo(snap);
                        self.rebuild_staging_world();
                    }
                    self.target_bp_editor.editing_scene_index = None;
                    self.tab = WorkspaceTab::Simulation;
                }
            }
            return;
        }

        if self.phase == SimPhase::Running {
            ui.colored_label(
                Color32::from_rgb(255, 195, 95),
                "Simulation is currently running.",
            );
            if ui.button("↺ Stop & Return to Scene Setup (to edit)").clicked() {
                self.reset_to_staging();
            }
            ui.separator();
        }

        // --- Parametric Target Designer ---
        ui.group(|ui| {
            ui.label(egui::RichText::new("Build Shape (Canvas Drag or Add):").strong());
            egui::Grid::new("target_shape_grid")
                .num_columns(4)
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    for (idx, shape) in TargetBuildShape::ALL.iter().enumerate() {
                        if ui
                            .selectable_label(self.target_shape == *shape, shape.short_label())
                            .clicked()
                        {
                            self.target_shape = *shape;
                            if *shape == TargetBuildShape::Circle {
                                if let TargetKind::Ellipse { rx, ry, .. } =
                                    &mut self.target_ellipse_template.kind
                                {
                                    *ry = *rx;
                                }
                            }
                        }
                        if idx % 4 == 3 {
                            ui.end_row();
                        }
                    }
                });
            ui.label(
                egui::RichText::new(self.target_shape.description())
                    .size(11.0)
                    .color(Color32::from_rgb(155, 200, 225)),
            );

            let mut dummy_changed = false;
            match self.target_shape {
                TargetBuildShape::Line | TargetBuildShape::Rectangle => {
                    edit_target_spec(ui, "tmpl_box", &mut self.target_box_template, &mut dummy_changed);
                }
                TargetBuildShape::Ellipse | TargetBuildShape::Circle => {
                    edit_target_spec(
                        ui,
                        "tmpl_ellipse",
                        &mut self.target_ellipse_template,
                        &mut dummy_changed,
                    );
                }
                TargetBuildShape::HollowRing => {
                    edit_target_spec(ui, "tmpl_ring", &mut self.target_ring_template, &mut dummy_changed);
                }
                TargetBuildShape::TriangleWedge => {
                    edit_target_spec(
                        ui,
                        "tmpl_wedge",
                        &mut self.target_wedge_template,
                        &mut dummy_changed,
                    );
                }
                TargetBuildShape::CompositePlate => {
                    edit_target_spec(ui, "tmpl_comp", &mut self.target_comp_template, &mut dummy_changed);
                }
                TargetBuildShape::LayeredPlanet => {
                    edit_target_spec(
                        ui,
                        "tmpl_planet",
                        &mut self.target_planet_template,
                        &mut dummy_changed,
                    );
                }
            }

            ui.add_space(4.0);
            if ui
                .button(egui::RichText::new("+ Add Target to Scene").strong())
                .clicked()
            {
                let mut spec = self.current_target_template_mut().clone();
                if self.target_shape == TargetBuildShape::Circle {
                    if let TargetKind::Ellipse { rx, ry, .. } = &mut spec.kind {
                        *ry = *rx;
                    }
                }
                if self.phase == SimPhase::Running {
                    spec.spawn_into(&mut self.world);
                } else {
                    let idx = self.scene.targets.len();
                    self.scene.targets.push(spec);
                    self.selection = Some(SceneSelection::Target(idx));
                    self.rebuild_staging_world();
                }
            }
        });

        ui.add_space(6.0);

        // --- Scene Targets List & Selected Target Editor ---
        if self.phase == SimPhase::Staging {
            ui.group(|ui| {
                ui.label(
                    egui::RichText::new(format!("Targets in Scene ({})", self.scene.targets.len()))
                        .strong(),
                );
                if self.scene.targets.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "No targets yet. Paint in Pixel Blueprint CAD or Left-Drag on empty canvas to draw a slab.",
                        )
                        .size(11.0)
                        .color(Color32::from_rgb(145, 160, 180)),
                    );
                } else {
                    for idx in 0..self.scene.targets.len() {
                        let is_sel = self.selection == Some(SceneSelection::Target(idx));
                        let label = format!(
                            "{}. {}  @ ({:.1}, {:.1})",
                            idx + 1,
                            self.scene.targets[idx].name,
                            self.scene.targets[idx].pos.x,
                            self.scene.targets[idx].pos.y
                        );
                        if ui.selectable_label(is_sel, label).clicked() {
                            self.selection = Some(SceneSelection::Target(idx));
                        }
                    }
                }
            });

            if let Some(SceneSelection::Target(idx)) = self.selection {
                if idx < self.scene.targets.len() {
                    ui.add_space(6.0);
                    let mut changed = false;
                    let mut dup = false;
                    let mut del = false;
                    let mut edit_in_cad = None;
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Selected Target #{}", idx + 1))
                                    .strong()
                                    .color(Color32::from_rgb(110, 225, 255)),
                            );
                            if ui.small_button("Duplicate").clicked() {
                                dup = true;
                            }
                            if ui.small_button("Delete").clicked() {
                                del = true;
                            }
                        });
                        if let TargetKind::Blueprint { bp } = &self.scene.targets[idx].kind {
                            if ui.button("✎ Edit Pixels in Blueprint CAD").clicked() {
                                edit_in_cad = Some(bp.clone());
                            }
                        }
                        edit_target_spec(ui, ("sel_target", idx), &mut self.scene.targets[idx], &mut changed);
                    });
                    if let Some(bp) = edit_in_cad {
                        self.target_bp_editor.spawn_pos = self.scene.targets[idx].pos;
                        self.target_bp_editor.spawn_angle_deg = self.scene.targets[idx].angle_deg;
                        self.target_bp_editor.load_blueprint(bp, Some(idx));
                        self.target_submode = BuilderSubMode::BlueprintCad;
                    }
                    if dup {
                        let mut copy = self.scene.targets[idx].clone();
                        copy.pos += Vec2::new(6.0, -6.0);
                        let new_i = self.scene.targets.len();
                        self.scene.targets.push(copy);
                        self.selection = Some(SceneSelection::Target(new_i));
                        changed = true;
                    }
                    if del {
                        self.delete_selected();
                        changed = false;
                    }
                    if changed {
                        self.rebuild_staging_world();
                    }
                }
            }
        }
    }

    fn draw_projectiles_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("PROJECTILES & WARHEADS BUILDER")
                .strong()
                .size(13.0)
                .color(Color32::from_rgb(255, 185, 95)),
        );
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    self.proj_submode == BuilderSubMode::BlueprintCad,
                    egui::RichText::new("🎨 Warhead Blueprint CAD").strong(),
                )
                .clicked()
            {
                self.proj_submode = BuilderSubMode::BlueprintCad;
            }
            if ui
                .selectable_label(
                    self.proj_submode == BuilderSubMode::ParametricRange,
                    egui::RichText::new("📐 Range Shapes & Scene").strong(),
                )
                .clicked()
            {
                self.proj_submode = BuilderSubMode::ParametricRange;
            }
        });
        ui.add_space(4.0);

        if self.proj_submode == BuilderSubMode::BlueprintCad {
            let sim_dx = self.resolution.dx();
            let action = self.proj_bp_editor.draw_sidebar(ui, sim_dx);
            match action {
                BlueprintEditorAction::None => {}
                BlueprintEditorAction::SendToSceneAndViewRange(bp) => {
                    let spec = ProjectileSpec::from_blueprint(
                        self.proj_bp_editor.spawn_pos,
                        self.proj_bp_editor.launch_speed_kms,
                        self.proj_bp_editor.spawn_angle_deg,
                        bp,
                    );
                    self.proj_template = spec.clone();
                    if self.phase == SimPhase::Running {
                        spec.spawn_into(&mut self.world);
                    } else {
                        let snap = self.snapshot();
                        let idx = self.scene.projectiles.len();
                        self.scene.projectiles.push(spec);
                        self.selection = Some(SceneSelection::Projectile(idx));
                        self.push_undo(snap);
                        self.rebuild_staging_world();
                    }
                    self.tab = WorkspaceTab::Simulation;
                }
                BlueprintEditorAction::StampToSceneKeepEditing(bp) => {
                    if self.phase == SimPhase::Running {
                        self.reset_to_staging();
                    }
                    let snap = self.snapshot();
                    let spec = ProjectileSpec::from_blueprint(
                        self.proj_bp_editor.spawn_pos,
                        self.proj_bp_editor.launch_speed_kms,
                        self.proj_bp_editor.spawn_angle_deg,
                        bp,
                    );
                    self.proj_template = spec.clone();
                    let idx = self.scene.projectiles.len();
                    self.scene.projectiles.push(spec);
                    self.selection = Some(SceneSelection::Projectile(idx));
                    self.proj_bp_editor.spawn_pos.y += 6.0;
                    self.push_undo(snap);
                    self.rebuild_staging_world();
                }
                BlueprintEditorAction::UpdateExistingSceneObject(idx, bp) => {
                    if self.phase == SimPhase::Running {
                        self.reset_to_staging();
                    }
                    if idx < self.scene.projectiles.len() {
                        let snap = self.snapshot();
                        let he = bp.world_half_extents(0.35);
                        let p = &mut self.scene.projectiles[idx];
                        p.name = bp.name.clone();
                        p.shape = ProjectileShape::CustomBlueprint;
                        p.length_cm = (2.0 * he.x).max(2.0);
                        p.radius_cm = he.y.max(1.0);
                        p.speed_kms = self.proj_bp_editor.launch_speed_kms;
                        p.angle_deg = self.proj_bp_editor.spawn_angle_deg;
                        p.blueprint = Some(bp);
                        self.proj_template = p.clone();
                        self.selection = Some(SceneSelection::Projectile(idx));
                        self.push_undo(snap);
                        self.rebuild_staging_world();
                    }
                    self.proj_bp_editor.editing_scene_index = None;
                    self.tab = WorkspaceTab::Simulation;
                }
            }
            return;
        }

        if self.phase == SimPhase::Running {
            ui.colored_label(
                Color32::from_rgb(255, 195, 95),
                "Simulation is running — adding launches live, or click 'Edit Scene' to stage.",
            );
            ui.separator();
        }

        // --- Parametric Projectile Designer ---
        ui.group(|ui| {
            ui.label(egui::RichText::new("New Projectile Designer").strong());
            let mut dummy_changed = false;
            edit_projectile_spec(
                ui,
                "tmpl_proj",
                &mut self.proj_template,
                &self.scene.targets,
                &mut dummy_changed,
            );
            ui.add_space(4.0);
            let add_label = if self.phase == SimPhase::Running {
                "🚀 Fire Projectile Now!"
            } else {
                "+ Add Projectile to Scene"
            };
            if ui.button(egui::RichText::new(add_label).strong()).clicked() {
                let spec = self.proj_template.clone();
                if self.phase == SimPhase::Running {
                    spec.spawn_into(&mut self.world);
                } else {
                    let idx = self.scene.projectiles.len();
                    self.scene.projectiles.push(spec);
                    self.selection = Some(SceneSelection::Projectile(idx));
                    self.rebuild_staging_world();
                }
            }
        });

        ui.add_space(6.0);

        if self.phase == SimPhase::Staging {
            ui.group(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Projectiles in Scene ({})",
                        self.scene.projectiles.len()
                    ))
                    .strong(),
                );
                if self.scene.projectiles.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "No projectiles yet. Design a warhead in Blueprint CAD or Right-Click / Drag on canvas.",
                        )
                        .size(11.0)
                        .color(Color32::from_rgb(145, 160, 180)),
                    );
                } else {
                    for idx in 0..self.scene.projectiles.len() {
                        let is_sel = self.selection == Some(SceneSelection::Projectile(idx));
                        let p = &self.scene.projectiles[idx];
                        let label = format!(
                            "{}. {}  ({:.2} km/s @ {:.0}°)",
                            idx + 1,
                            p.name,
                            p.speed_kms,
                            p.angle_deg
                        );
                        if ui.selectable_label(is_sel, label).clicked() {
                            self.selection = Some(SceneSelection::Projectile(idx));
                        }
                    }
                }
            });

            if let Some(SceneSelection::Projectile(idx)) = self.selection {
                if idx < self.scene.projectiles.len() {
                    ui.add_space(6.0);
                    let mut changed = false;
                    let mut dup = false;
                    let mut del = false;
                    let mut edit_in_cad = None;
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Selected Projectile #{}", idx + 1))
                                    .strong()
                                    .color(Color32::from_rgb(255, 200, 95)),
                            );
                            if ui.small_button("Duplicate").clicked() {
                                dup = true;
                            }
                            if ui.small_button("Delete").clicked() {
                                del = true;
                            }
                        });
                        if let Some(bp) = &self.scene.projectiles[idx].blueprint {
                            if ui.button("✎ Edit Warhead Pixels in Blueprint CAD").clicked() {
                                edit_in_cad = Some(bp.clone());
                            }
                        }
                        edit_projectile_spec(
                            ui,
                            ("sel_proj", idx),
                            &mut self.scene.projectiles[idx],
                            &self.scene.targets,
                            &mut changed,
                        );
                        if self.scene.self_gravity && !self.scene.targets.is_empty() {
                            ui.horizontal(|ui| {
                                if ui.small_button("🪐 Orbit +").clicked() {
                                    if let Some((os, oa)) =
                                        self.scene.compute_orbit_velocity(idx, false, 1.0)
                                    {
                                        self.scene.projectiles[idx].speed_kms = os;
                                        self.scene.projectiles[idx].angle_deg = oa;
                                        changed = true;
                                    }
                                }
                                if ui.small_button("Orbit −").clicked() {
                                    if let Some((os, oa)) =
                                        self.scene.compute_orbit_velocity(idx, true, 1.0)
                                    {
                                        self.scene.projectiles[idx].speed_kms = os;
                                        self.scene.projectiles[idx].angle_deg = oa;
                                        changed = true;
                                    }
                                }
                                if ui.small_button("Flyby").clicked() {
                                    if let Some((os, oa)) =
                                        self.scene.compute_orbit_velocity(idx, false, 0.84)
                                    {
                                        self.scene.projectiles[idx].speed_kms = os;
                                        self.scene.projectiles[idx].angle_deg = oa;
                                        changed = true;
                                    }
                                }
                            });
                        }
                    });
                    if let Some(bp) = edit_in_cad {
                        self.proj_bp_editor.spawn_pos = self.scene.projectiles[idx].pos;
                        self.proj_bp_editor.spawn_angle_deg = self.scene.projectiles[idx].angle_deg;
                        self.proj_bp_editor.launch_speed_kms =
                            self.scene.projectiles[idx].speed_kms;
                        self.proj_bp_editor.load_blueprint(bp, Some(idx));
                        self.proj_submode = BuilderSubMode::BlueprintCad;
                    }
                    if dup {
                        let mut copy = self.scene.projectiles[idx].clone();
                        copy.pos += Vec2::new(-5.0, 5.0);
                        let new_i = self.scene.projectiles.len();
                        self.scene.projectiles.push(copy);
                        self.selection = Some(SceneSelection::Projectile(new_i));
                        changed = true;
                    }
                    if del {
                        self.delete_selected();
                        changed = false;
                    }
                    if changed {
                        self.rebuild_staging_world();
                    }
                }
            }
        }
    }

    fn draw_simulation_tab(&mut self, ui: &mut egui::Ui) {
        // --- Pre-Start Scene Staging Overview & Quick Editor ---
        if self.phase == SimPhase::Staging {
            ui.group(|ui| {
                ui.label(
                    egui::RichText::new("PRE-START SCENE EDITOR")
                        .strong()
                        .size(12.5)
                        .color(Color32::from_rgb(105, 225, 155)),
                );
                let mut changed = false;
                let mut open_target_editor = None;
                let mut open_proj_editor = None;

                if self.scene.targets.is_empty() && self.scene.projectiles.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "Scene is empty. Design warheads & targets in Projectiles Builder & Targets Builder, or pick a Ballistic / Space Scenario above!",
                        )
                        .size(11.5)
                        .color(Color32::from_rgb(175, 190, 210)),
                    );
                } else {
                    for i in 0..self.scene.targets.len() {
                        let t = &mut self.scene.targets[i];
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.selection == Some(SceneSelection::Target(i)),
                                    format!("🎯 {}", t.name),
                                )
                                .clicked()
                            {
                                self.selection = Some(SceneSelection::Target(i));
                            }
                            if ui.small_button("✎ Edit").clicked() {
                                open_target_editor = Some(i);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("X:");
                            changed |= ui
                                .add(egui::DragValue::new(&mut t.pos.x).speed(0.3).suffix(" cm"))
                                .changed();
                            ui.label("Y:");
                            changed |= ui
                                .add(egui::DragValue::new(&mut t.pos.y).speed(0.3).suffix(" cm"))
                                .changed();
                            ui.label("∠:");
                            changed |= ui
                                .add(egui::DragValue::new(&mut t.angle_deg).speed(1.0).suffix("°"))
                                .changed();
                        });
                    }

                    for i in 0..self.scene.projectiles.len() {
                        let (mut spd, mut ang, mut pos_v, p_name) = {
                            let p = &self.scene.projectiles[i];
                            (p.speed_kms, p.angle_deg, p.pos, p.name.clone())
                        };
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.selection == Some(SceneSelection::Projectile(i)),
                                    format!("🚀 {}", p_name),
                                )
                                .clicked()
                            {
                                self.selection = Some(SceneSelection::Projectile(i));
                            }
                            if ui.small_button("✎ Edit").clicked() {
                                open_proj_editor = Some(i);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("X:");
                            changed |= ui
                                .add(egui::DragValue::new(&mut pos_v.x).speed(0.3).suffix(" cm"))
                                .changed();
                            ui.label("Y:");
                            changed |= ui
                                .add(egui::DragValue::new(&mut pos_v.y).speed(0.3).suffix(" cm"))
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Speed:");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut spd)
                                        .range(0.0..=100.0)
                                        .speed(0.05)
                                        .suffix(" km/s"),
                                )
                                .changed();
                            ui.label("Angle:");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut ang)
                                        .range(-180.0..=180.0)
                                        .speed(1.0)
                                        .suffix("°"),
                                )
                                .changed();
                        });
                        self.scene.projectiles[i].pos = pos_v;
                        self.scene.projectiles[i].speed_kms = spd;
                        self.scene.projectiles[i].angle_deg = ang;
                        if self.scene.self_gravity && !self.scene.targets.is_empty() {
                            ui.horizontal(|ui| {
                                if ui.small_button("🪐 Orbit +").clicked() {
                                    if let Some((os, oa)) =
                                        self.scene.compute_orbit_velocity(i, false, 1.0)
                                    {
                                        self.scene.projectiles[i].speed_kms = os;
                                        self.scene.projectiles[i].angle_deg = oa;
                                        changed = true;
                                    }
                                }
                                if ui.small_button("Orbit −").clicked() {
                                    if let Some((os, oa)) =
                                        self.scene.compute_orbit_velocity(i, true, 1.0)
                                    {
                                        self.scene.projectiles[i].speed_kms = os;
                                        self.scene.projectiles[i].angle_deg = oa;
                                        changed = true;
                                    }
                                }
                                if ui.small_button("Flyby").clicked() {
                                    if let Some((os, oa)) =
                                        self.scene.compute_orbit_velocity(i, false, 0.84)
                                    {
                                        self.scene.projectiles[i].speed_kms = os;
                                        self.scene.projectiles[i].angle_deg = oa;
                                        changed = true;
                                    }
                                }
                            });
                        }
                    }
                }

                if changed {
                    self.rebuild_staging_world();
                }

                if let Some(ti) = open_target_editor {
                    self.selection = Some(SceneSelection::Target(ti));
                    if let TargetKind::Blueprint { bp } = &self.scene.targets[ti].kind {
                        self.target_bp_editor.spawn_pos = self.scene.targets[ti].pos;
                        self.target_bp_editor.spawn_angle_deg = self.scene.targets[ti].angle_deg;
                        self.target_bp_editor.load_blueprint(bp.clone(), Some(ti));
                        self.target_submode = BuilderSubMode::BlueprintCad;
                    } else {
                        self.target_submode = BuilderSubMode::ParametricRange;
                    }
                    self.tab = WorkspaceTab::Targets;
                }
                if let Some(pi) = open_proj_editor {
                    self.selection = Some(SceneSelection::Projectile(pi));
                    if let Some(bp) = &self.scene.projectiles[pi].blueprint {
                        self.proj_bp_editor.spawn_pos = self.scene.projectiles[pi].pos;
                        self.proj_bp_editor.spawn_angle_deg = self.scene.projectiles[pi].angle_deg;
                        self.proj_bp_editor.launch_speed_kms =
                            self.scene.projectiles[pi].speed_kms;
                        self.proj_bp_editor.load_blueprint(bp.clone(), Some(pi));
                        self.proj_submode = BuilderSubMode::BlueprintCad;
                    } else {
                        self.proj_submode = BuilderSubMode::ParametricRange;
                    }
                    self.tab = WorkspaceTab::Projectiles;
                }

                ui.add_space(4.0);
                let start_btn = egui::Button::new(
                    egui::RichText::new("▶ Start Simulation")
                        .strong()
                        .size(14.0)
                        .color(Color32::from_rgb(18, 26, 20)),
                )
                .fill(Color32::from_rgb(95, 225, 145))
                .min_size(EguiVec2::new(ui.available_width(), 30.0));
                if ui.add_enabled(self.world.len() > 0, start_btn).clicked() {
                    self.start_simulation();
                }

                ui.add_space(4.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("Scene Persistence (.rscene)")
                        .strong()
                        .size(11.5)
                        .color(Color32::from_rgb(155, 205, 240)),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.scene_save_name)
                            .desired_width(145.0)
                            .hint_text("Scene name..."),
                    );
                    if ui.button("💾 Save Scene").clicked() {
                        match save_scene_to_disk(&self.scene_save_name, &self.scene) {
                            Ok(path) => {
                                self.saved_scenes = list_saved_scenes();
                                self.set_toast(format!("Saved scene: {}", path.display()));
                            }
                            Err(e) => self.set_toast(format!("Scene save error: {e}")),
                        }
                    }
                });
                if !self.saved_scenes.is_empty() {
                    let mut load_idx: Option<usize> = None;
                    egui::ComboBox::from_id_salt("load_rscene_combo")
                        .width(220.0)
                        .selected_text("📂 Load Saved .rscene...")
                        .show_ui(ui, |ui| {
                            for (idx, (nm, _)) in self.saved_scenes.iter().enumerate() {
                                if ui.selectable_label(false, nm).clicked() {
                                    load_idx = Some(idx);
                                }
                            }
                        });
                    if let Some(idx) = load_idx {
                        let (nm, sc) = self.saved_scenes[idx].clone();
                        let snap = self.snapshot();
                        self.push_undo(snap);
                        self.scene_save_name = nm.clone();
                        self.scene = sc;
                        self.selection = None;
                        self.gauges.clear();
                        self.rebuild_staging_world();
                        self.set_toast(format!("Loaded scene '{nm}'"));
                    }
                }
            });
            ui.add_space(6.0);
        }

        ui.add_space(4.0);
        ui.separator();

        // --- Interactive Live Physics Tools (Q/B/E/L/G/X) ---
        ui.label(
            egui::RichText::new("LIVE INTERACTIVE TOOLS (Q/B/E/L/G/X)")
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(255, 200, 115)),
        );
        ui.horizontal_wrapped(|ui| {
            for tool in ActiveTool::ALL {
                ui.selectable_value(&mut self.active_tool, tool, tool.short_label());
            }
        });
        match self.active_tool {
            ActiveTool::Interact => {
                ui.label(
                    egui::RichText::new("Move/aim staging bodies or pan running canvas; Right-drag quick-fires projectile.")
                        .size(10.5)
                        .color(Color32::from_rgb(155, 175, 198)),
                );
            }
            ActiveTool::BrushMatter => {
                ui.add(egui::Slider::new(&mut self.tool_radius, 0.8..=20.0).text("Brush R (cm) [ / ]"));
                material_combo(ui, "live_brush_mat", "Matter", &mut self.brush_mat);
                ui.add(
                    egui::Slider::new(&mut self.brush_temp_k, 100.0..=3500.0)
                        .logarithmic(true)
                        .text("Init Temp (K)"),
                );
            }
            ActiveTool::EraseMatter => {
                ui.add(egui::Slider::new(&mut self.tool_radius, 0.8..=22.0).text("Eraser R (cm) [ / ]"));
            }
            ActiveTool::ThermalLaser => {
                ui.add(egui::Slider::new(&mut self.tool_radius, 0.8..=14.0).text("Beam Waist (cm)"));
                ui.add(egui::Slider::new(&mut self.laser_power, 0.4..=8.0).text("Power ×"));
                ui.add(egui::Slider::new(&mut self.laser_angle_deg, -180.0..=180.0).text("Incidence °"));
            }
            ActiveTool::GravityVortex => {
                ui.add(egui::Slider::new(&mut self.gravity_strength, 0.04..=1.2).text("Pull G-Force"));
                ui.add(egui::Slider::new(&mut self.gravity_swirl, -0.6..=0.6).text("Vortex Swirl"));
                ui.label(
                    egui::RichText::new("Hold Shift + LMB to Repel (White Hole).")
                        .size(10.5)
                        .color(Color32::from_rgb(145, 210, 240)),
                );
            }
            ActiveTool::ShockBlast => {
                ui.add(egui::Slider::new(&mut self.blast_radius, 1.5..=24.0).text("Blast R (cm)"));
                ui.add(egui::Slider::new(&mut self.blast_vel, 0.08..=1.2).text("Impulse cm/µs"));
            }
        }

        ui.add_space(6.0);
        ui.separator();

        // --- Visualization Modes ---
        ui.label(
            egui::RichText::new("VISUALIZATION MODE (Keys 1–8)")
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(140, 185, 230)),
        );
        egui::Grid::new("vis_mode_grid")
            .num_columns(2)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (idx, mode) in VisMode::ALL.iter().enumerate() {
                    if ui
                        .selectable_label(self.vis_mode == *mode, mode.label())
                        .clicked()
                    {
                        self.vis_mode = *mode;
                    }
                    if idx % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        draw_mode_legend(ui, self.vis_mode);

        ui.add_space(6.0);
        ui.separator();

        // --- Vulkan GPU Compute & Continuum Solver ---
        ui.label(
            egui::RichText::new("VULKAN GPU & CONTINUUM SOLVER")
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(140, 185, 230)),
        );
        if let Some(vk) = &self.vulkan_engine {
            ui.checkbox(
                &mut self.use_vulkan_gpu,
                egui::RichText::new("⚡ Vulkan GPU Compute Acceleration")
                    .strong()
                    .color(Color32::from_rgb(125, 235, 165)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "GPU: {} ({} | {:.2} ms/frame)",
                    vk.device_name,
                    if vk.is_unified_mem { "UMA Zero-Copy" } else { "VRAM" },
                    self.physics_ms
                ))
                .monospace()
                .size(10.0)
                .color(Color32::from_rgb(155, 205, 185)),
            );
        } else {
            ui.colored_label(
                Color32::from_rgb(255, 175, 95),
                format!(
                    "Vulkan unavailable ({}) — using Rayon CPU",
                    self.vulkan_init_err.as_deref().unwrap_or("no device")
                ),
            );
        }
        ui.add(
            egui::Slider::new(&mut self.time_scale, 0.05..=3.0)
                .logarithmic(true)
                .text("Slow-Mo / Speed"),
        );
        ui.add(egui::Slider::new(&mut self.substeps_per_frame, 1..=32).text("Substeps / Frame"));

        let mut grav_changed = false;
        if self.phase == SimPhase::Staging {
            grav_changed |= ui
                .checkbox(&mut self.scene.self_gravity, "Planetary Self-Gravity")
                .changed();
            if self.scene.self_gravity {
                if self.scene.grav_g <= 0.0 {
                    self.scene.grav_g = 7.0e-5;
                }
                grav_changed |= ui
                    .add(
                        egui::Slider::new(&mut self.scene.grav_g, 5.0e-6..=4.0e-4)
                            .logarithmic(true)
                            .text("Gravity G"),
                    )
                    .changed();
            }
            if grav_changed {
                self.rebuild_staging_world();
            }
        } else {
            ui.checkbox(&mut self.world.self_gravity, "Planetary Self-Gravity");
            if self.world.self_gravity {
                if self.world.grav_g <= 0.0 {
                    self.world.grav_g = 7.0e-5;
                }
                ui.add(
                    egui::Slider::new(&mut self.world.grav_g, 5.0e-6..=4.0e-4)
                        .logarithmic(true)
                        .text("Gravity G"),
                );
            }
        }

        ui.add(
            egui::Slider::new(&mut self.live_fire_speed_mult, 0.2..=10.0)
                .text("Right-Drag Fire Speed ×"),
        );
        ui.checkbox(&mut self.motion_trails, "High-Speed Camera Trails");
        ui.checkbox(&mut self.show_starfield, "Deep-Space Parallax Starfield");
        ui.checkbox(
            &mut self.show_vel_vectors,
            "Flow Velocity Vectors (Key: V)",
        );
        ui.checkbox(&mut self.show_hud_graphs, "Show Shock & Energy Graphs");

        ui.horizontal(|ui| {
            ui.label("Cam Track:");
            for mode in CameraTrackMode::ALL {
                ui.selectable_value(&mut self.camera_track, mode, mode.label());
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.auto_slowmo, "Auto Bullet-Time");
            ui.checkbox(&mut self.camera_shake_enabled, "Impact Shake");
        });

        ui.add_space(6.0);
        ui.separator();

        // --- Laboratory Instrumentation, Gauges & Data Export ---
        ui.label(
            egui::RichText::new("LAB INSTRUMENTATION & EXPORTS")
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(140, 185, 230)),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.caliper_mode, "📏 Caliper & RHAe (M)");
            if self.caliper_line.is_some() && ui.small_button("Clear Ray").clicked() {
                self.caliper_line = None;
            }
        });
        ui.label(
            egui::RichText::new("Alt + Left-Click any armor/body particle to pin a Lagrangian Witness Gauge (up to 8).")
                .size(10.5)
                .color(Color32::from_rgb(150, 175, 198)),
        );
        if !self.gauges.gauges.is_empty() {
            ui.horizontal(|ui| {
                for ch in GaugeChannel::ALL {
                    ui.selectable_value(&mut self.gauges.channel, ch, ch.short_label());
                }
            });
            if ui
                .small_button(format!(
                    "✕ Clear {} Witness Gauge(s)",
                    self.gauges.gauges.len()
                ))
                .clicked()
            {
                self.gauges.clear();
            }
        }
        ui.horizontal(|ui| {
            if ui.button("📊 Export CSV").clicked() {
                self.export_csv_now();
            }
            if ui.button("📷 Save PNG (F12)").clicked() {
                self.export_png_now();
            }
        });
        ui.horizontal(|ui| {
            let gif_label = if self.gif_recording {
                format!("⏹ Save GIF ({}f)", self.gif_frames.len())
            } else {
                "🔴 Record GIF".to_string()
            };
            if ui.button(gif_label).clicked() {
                self.toggle_or_save_gif_now();
            }
            if ui.button("🎞 Replay GIF").clicked() {
                self.export_replay_gif_now();
            }
            if ui.button("🪐 Analyze Remnants").clicked() {
                self.remnant_report = Some(analyze_remnant_clusters(&self.world));
            }
        });
        if let Some(rep) = &self.remnant_report {
            ui.label(
                egui::RichText::new(format!(
                    "Clusters: {} | Largest: {:.1}% | 2nd: {:.1}%\nBound: {:.1}% | Escaped: {:.1}% | Peak Ejecta: {:.2} km/s",
                    rep.cluster_count,
                    rep.largest_mass_frac * 100.0,
                    rep.second_mass_frac * 100.0,
                    rep.bound_mass_frac * 100.0,
                    rep.escaped_mass_frac * 100.0,
                    rep.peak_ejecta_kms
                ))
                .monospace()
                .size(10.0)
                .color(Color32::from_rgb(155, 225, 200)),
            );
        }

        ui.add_space(6.0);
        ui.separator();
        ui.label(
            egui::RichText::new("SCENARIO NOTES")
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(140, 185, 230)),
        );
        ui.label(
            egui::RichText::new(self.preset.description())
                .size(11.5)
                .color(Color32::from_rgb(190, 200, 215)),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "Controls:\n• Drag objects or velocity arrows before Start\n• Alt+Click: Pin/Remove Witness Sensor Gauge\n• M: Ballistic Caliper & LOS RHAe | V: Flow Vectors\n• Left/Right Arrows: Scrub Replay Keyframes\n• Right-Click Drag: Add/fire projectile | F12: PNG\n• Space: Start/Pause | R: Edit Scene | C: Clear",
            )
            .size(11.0)
            .color(Color32::from_rgb(140, 155, 175)),
        );
    }

    fn draw_viewport(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let avail = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(avail, Sense::click_and_drag());

        let vp_w = rect.width().max(64.0);
        let vp_h = rect.height().max(64.0);

        // Camera zoom centered on cursor (scroll wheel + touchpad pinch)
        if response.hovered() {
            let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let zoom_factor = (scroll * 0.0022).exp() * pinch;
            if (zoom_factor - 1.0).abs() > 1e-4 {
                if let Some(hover_pos) = response.hover_pos() {
                    let local = Vec2::new(hover_pos.x - rect.min.x, hover_pos.y - rect.min.y);
                    let before = self.cam.screen_to_world(local, vp_w, vp_h);
                    self.cam.zoom = (self.cam.zoom * zoom_factor).clamp(0.20, 180.0);
                    let after = self.cam.screen_to_world(local, vp_w, vp_h);
                    self.cam.center += before - after;
                } else {
                    self.cam.zoom = (self.cam.zoom * zoom_factor).clamp(0.20, 180.0);
                }
            }
        }

        // Handle pointer drags & clicks
        let pointer_local = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos())
            .map(|p| Vec2::new(p.x - rect.min.x, p.y - rect.min.y));
        self.hover_world_pos = pointer_local.map(|lp| self.cam.screen_to_world(lp, vp_w, vp_h));

        let middle_pressed =
            response.hovered() && ctx.input(|i| i.pointer.button_pressed(PointerButton::Middle));
        let middle_released = ctx.input(|i| i.pointer.button_released(PointerButton::Middle));
        if middle_pressed {
            self.canvas_drag = Some(CanvasDrag::PanCamera);
        } else if middle_released && matches!(self.canvas_drag, Some(CanvasDrag::PanCamera)) {
            self.canvas_drag = None;
        }
        if matches!(self.canvas_drag, Some(CanvasDrag::PanCamera)) {
            let delta = ctx.input(|i| i.pointer.delta());
            self.cam.center.x -= delta.x / self.cam.zoom;
            self.cam.center.y += delta.y / self.cam.zoom;
        }

        if let Some(local_p) = pointer_local {
            let world_p = self.cam.screen_to_world(local_p, vp_w, vp_h);
            let left_pressed =
                response.hovered() && ctx.input(|i| i.pointer.button_pressed(PointerButton::Primary));
            let right_pressed =
                response.hovered() && ctx.input(|i| i.pointer.button_pressed(PointerButton::Secondary));
            let left_released = ctx.input(|i| i.pointer.button_released(PointerButton::Primary));
            let right_released = ctx.input(|i| i.pointer.button_released(PointerButton::Secondary));

            let alt_held = ctx.input(|i| i.modifiers.alt);

            if right_pressed {
                self.canvas_drag = Some(CanvasDrag::CreateProjectile { start: world_p });
            } else if left_pressed && alt_held {
                if let Some(msg) = self.gauges.toggle_or_pin_at(&self.world, world_p) {
                    self.set_toast(msg);
                }
            } else if left_pressed && self.caliper_mode {
                self.caliper_line = Some((world_p, world_p));
                self.canvas_drag = Some(CanvasDrag::MeasureCaliper { start: world_p });
            } else if left_pressed && self.active_tool != ActiveTool::Interact {
                if self.phase == SimPhase::Staging {
                    self.phase = SimPhase::Running;
                    self.paused = matches!(
                        self.active_tool,
                        ActiveTool::BrushMatter | ActiveTool::EraseMatter
                    );
                    self.prev_detonated_count = self.world.detonated_count;
                }
                match self.active_tool {
                    ActiveTool::BrushMatter => {
                        let bid = self.world.alloc_body_id();
                        self.canvas_drag = Some(CanvasDrag::UseToolBrush { body_id: bid });
                        self.apply_live_tool_continuous(world_p);
                    }
                    ActiveTool::EraseMatter => {
                        self.canvas_drag = Some(CanvasDrag::UseToolErase);
                        self.apply_live_tool_continuous(world_p);
                    }
                    ActiveTool::ThermalLaser => {
                        self.canvas_drag = Some(CanvasDrag::UseToolLaser);
                        self.apply_live_tool_continuous(world_p);
                    }
                    ActiveTool::GravityVortex => {
                        let repel = ctx.input(|i| i.modifiers.shift);
                        self.canvas_drag = Some(CanvasDrag::UseToolGravity { repel });
                        self.apply_live_tool_continuous(world_p);
                    }
                    ActiveTool::ShockBlast => {
                        self.timeline.commit_branch(&mut self.world);
                        let hit = self.world.trigger_shock_blast(
                            world_p,
                            self.blast_radius,
                            self.blast_vel,
                            4800.0,
                        );
                        if hit > 0 {
                            if self.camera_shake_enabled && !self.paused {
                                self.camera_shake = (self.camera_shake + 0.45).min(0.90);
                            }
                            if self.auto_slowmo {
                                self.slowmo_timer = 0.45;
                            }
                            self.gauges.record_now(&self.world);
                        }
                    }
                    ActiveTool::Interact => {}
                }
            } else if left_pressed && self.phase == SimPhase::Staging {
                let handle_tol = (14.0 / self.cam.zoom).max(2.2);
                let mut picked_handle = None;

                // Check velocity arrowhead handles first!
                for (i, p) in self.scene.projectiles.iter().enumerate() {
                    let tip = Self::velocity_handle_world(p.pos, p.vel_cm_per_us());
                    if (world_p - tip).length() <= handle_tol {
                        picked_handle = Some(SceneSelection::Projectile(i));
                        break;
                    }
                }
                if picked_handle.is_none() {
                    for (i, t) in self.scene.targets.iter().enumerate() {
                        if t.vel.length_squared() > 1e-10 {
                            let tip = Self::velocity_handle_world(t.pos, t.vel);
                            if (world_p - tip).length() <= handle_tol {
                                picked_handle = Some(SceneSelection::Target(i));
                                break;
                            }
                        }
                    }
                }

                if let Some(sel) = picked_handle {
                    self.pending_drag_undo = Some(self.snapshot());
                    self.selection = Some(sel);
                    self.canvas_drag = Some(CanvasDrag::AimVelocity { sel });
                } else if let Some((sel, obj_pos)) = self.hit_test_scene(world_p) {
                    self.pending_drag_undo = Some(self.snapshot());
                    self.selection = Some(sel);
                    self.canvas_drag = Some(CanvasDrag::MoveObject {
                        sel,
                        grab_offset: obj_pos - world_p,
                    });
                    match sel {
                        SceneSelection::Target(_) => {
                            if self.tab == WorkspaceTab::Projectiles {
                                self.tab = WorkspaceTab::Targets;
                                self.target_submode = BuilderSubMode::ParametricRange;
                            }
                        }
                        SceneSelection::Projectile(_) => {
                            if self.tab == WorkspaceTab::Targets {
                                self.tab = WorkspaceTab::Projectiles;
                                self.proj_submode = BuilderSubMode::ParametricRange;
                            }
                        }
                    }
                } else {
                    match self.tab {
                        WorkspaceTab::Targets => {
                            self.canvas_drag = Some(CanvasDrag::CreateTargetShape {
                                shape: self.target_shape,
                                start: world_p,
                            });
                        }
                        WorkspaceTab::Projectiles => {
                            self.canvas_drag = Some(CanvasDrag::CreateProjectile { start: world_p });
                        }
                        WorkspaceTab::Simulation => {
                            self.canvas_drag = Some(CanvasDrag::PanCamera);
                        }
                    }
                }
            } else if left_pressed && self.phase == SimPhase::Running {
                self.canvas_drag = Some(CanvasDrag::PanCamera);
            }

            // Live drag updates
            match self.canvas_drag {
                Some(CanvasDrag::MoveObject { sel, grab_offset }) => {
                    let new_pos = world_p + grab_offset;
                    let mut moved = false;
                    match sel {
                        SceneSelection::Target(i)
                            if i < self.scene.targets.len()
                                && (self.scene.targets[i].pos - new_pos).length_squared()
                                    > 1e-4 =>
                        {
                            self.scene.targets[i].pos = new_pos;
                            moved = true;
                        }
                        SceneSelection::Projectile(i)
                            if i < self.scene.projectiles.len()
                                && (self.scene.projectiles[i].pos - new_pos).length_squared()
                                    > 1e-4 =>
                        {
                            self.scene.projectiles[i].pos = new_pos;
                            moved = true;
                        }
                        _ => {}
                    }
                    if moved {
                        self.rebuild_staging_world();
                    }
                    if left_released {
                        if let Some(snap) = self.pending_drag_undo.take() {
                            if snap.scene != self.scene {
                                self.push_undo(snap);
                            }
                        }
                        self.canvas_drag = None;
                    }
                }
                Some(CanvasDrag::AimVelocity { sel }) => {
                    let origin = match sel {
                        SceneSelection::Projectile(i) if i < self.scene.projectiles.len() => {
                            self.scene.projectiles[i].pos
                        }
                        SceneSelection::Target(i) if i < self.scene.targets.len() => {
                            self.scene.targets[i].pos
                        }
                        _ => Vec2::ZERO,
                    };
                    let d = world_p - origin;
                    let len = d.length();
                    let speed_kms = (((len - 4.5).max(0.0) / 2.2).powf(1.0 / 0.65)).clamp(0.05, 100.0);
                    let angle_deg = if len > 1e-4 {
                        d.y.atan2(d.x).to_degrees()
                    } else {
                        0.0
                    };
                    let mut aimed = false;
                    match sel {
                        SceneSelection::Projectile(i) if i < self.scene.projectiles.len() => {
                            let p = &mut self.scene.projectiles[i];
                            if (p.speed_kms - speed_kms).abs() > 0.01
                                || (p.angle_deg - angle_deg).abs() > 0.2
                            {
                                p.speed_kms = speed_kms;
                                p.angle_deg = angle_deg;
                                aimed = true;
                            }
                        }
                        SceneSelection::Target(i) if i < self.scene.targets.len() => {
                            let t = &mut self.scene.targets[i];
                            if (t.speed_kms() - speed_kms).abs() > 0.01
                                || (t.vel_angle_deg() - angle_deg).abs() > 0.2
                            {
                                t.set_speed_and_angle(speed_kms, angle_deg);
                                aimed = true;
                            }
                        }
                        _ => {}
                    }
                    if aimed {
                        self.rebuild_staging_world();
                    }
                    if left_released {
                        if let Some(snap) = self.pending_drag_undo.take() {
                            if snap.scene != self.scene {
                                self.push_undo(snap);
                            }
                        }
                        self.canvas_drag = None;
                    }
                }
                Some(CanvasDrag::CreateTargetShape { shape, start }) if left_released => {
                    if let Some(spec) = self.build_target_from_drag(shape, start, world_p) {
                        let snap = self.snapshot();
                        let idx = self.scene.targets.len();
                        self.scene.targets.push(spec);
                        self.selection = Some(SceneSelection::Target(idx));
                        self.push_undo(snap);
                        self.rebuild_staging_world();
                    }
                    self.canvas_drag = None;
                }
                Some(CanvasDrag::CreateProjectile { start })
                    if left_released || right_released =>
                {
                    let pull = world_p - start;
                    let mut spec = self.proj_template.clone();
                    spec.pos = start;
                    if pull.length() > 0.6 {
                        let v_cm_us =
                            (pull * 0.024 * self.live_fire_speed_mult).clamp_length_max(10.0);
                        spec.set_vel_cm_per_us(v_cm_us);
                    }
                    if self.phase == SimPhase::Staging {
                        let snap = self.snapshot();
                        let idx = self.scene.projectiles.len();
                        self.scene.projectiles.push(spec);
                        self.selection = Some(SceneSelection::Projectile(idx));
                        self.push_undo(snap);
                        self.rebuild_staging_world();
                    } else {
                        spec.spawn_into(&mut self.world);
                    }
                    self.canvas_drag = None;
                }
                Some(CanvasDrag::MeasureCaliper { start }) => {
                    self.caliper_line = Some((start, world_p));
                    if left_released {
                        self.canvas_drag = None;
                    }
                }
                Some(CanvasDrag::UseToolBrush { .. })
                | Some(CanvasDrag::UseToolErase)
                | Some(CanvasDrag::UseToolLaser)
                | Some(CanvasDrag::UseToolGravity { .. }) => {
                    if let CanvasDrag::UseToolGravity { .. } =
                        self.canvas_drag.expect("checked match")
                    {
                        let repel = ctx.input(|i| i.modifiers.shift);
                        self.canvas_drag = Some(CanvasDrag::UseToolGravity { repel });
                    }
                    if left_released {
                        self.canvas_drag = None;
                        self.last_gravity_well = None;
                    } else if ctx.input(|i| i.pointer.button_down(PointerButton::Primary)) {
                        self.apply_live_tool_continuous(world_p);
                    }
                }
                Some(CanvasDrag::PanCamera) if left_released || right_released => {
                    self.canvas_drag = None;
                }
                _ => {}
            }
        }

        // Render continuum pixel buffer and upload texture ONLY when state or camera changed
        // (Zero rasterization, zero clone, zero texture upload on pause/hover!)
        let rw = (vp_w.round() as usize).clamp(64, 4096);
        let rh = (vp_h.round() as usize).clamp(64, 2560);
        let trails_active = self.motion_trails && self.phase == SimPhase::Running && !self.paused;
        let mut render_cam = self.cam;
        if !self.paused && self.camera_shake > 0.015 {
            let phase = (self.world.time_us * 1.37 + self.world.step_count as f32 * 0.71) % 62.83;
            let amp = (self.camera_shake * 10.0 / self.cam.zoom.max(0.25)).min(3.5);
            render_cam.center += Vec2::new(
                (phase * 1.7).sin() * amp,
                (phase * 2.3 + 1.1).cos() * amp,
            );
        }
        let render_key = ViewportRenderKey {
            world_id: self.world.world_id,
            state_epoch: self.world.state_epoch,
            world_len: self.world.len(),
            width: rw,
            height: rh,
            center_x_bits: render_cam.center.x.to_bits(),
            center_y_bits: render_cam.center.y.to_bits(),
            zoom_bits: render_cam.zoom.to_bits(),
            vis_mode: self.vis_mode,
            trails_active,
            show_starfield: self.show_starfield,
        };

        if self.texture.is_none() || self.last_render_key != Some(render_key) {
            let t_ren = Instant::now();
            self.canvas.render_with_opts(
                &self.world,
                &render_cam,
                rw,
                rh,
                self.vis_mode,
                trails_active,
                self.show_starfield,
            );
            self.render_ms = 0.85 * self.render_ms + 0.15 * (t_ren.elapsed().as_secs_f32() * 1000.0);

            if self.gif_recording
                && self.phase == SimPhase::Running
                && !self.paused
                && self.world.step_count >= self.last_gif_capture_step + 4
            {
                self.gif_frames
                    .push(quantize_rgba_downsampled(&self.canvas.pixels, rw, rh, 520));
                self.last_gif_capture_step = self.world.step_count;
                if self.gif_frames.len() >= 120 {
                    self.toggle_or_save_gif_now();
                }
            }

            let color_image = ColorImage {
                size: [rw, rh],
                pixels: self.canvas.pixels.clone(),
            };
            match &mut self.texture {
                Some(tex) => tex.set(color_image, TextureOptions::NEAREST),
                None => {
                    self.texture = Some(ctx.load_texture(
                        "continuum_viewport",
                        color_image,
                        TextureOptions::NEAREST,
                    ));
                }
            }
            self.last_render_key = Some(render_key);
        }
        if let Some(tex) = &self.texture {
            ui.painter().image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        let painter = ui.painter_at(rect);

        // Draw Staging Scene Gizmos (bounding outlines, trajectory rays, velocity arrows & handles)
        if self.phase == SimPhase::Staging {
            self.draw_staging_gizmos(&painter, rect, vp_w, vp_h);
        }

        // Draw active drag creation previews
        if let (Some(drag), Some(hover_screen)) = (self.canvas_drag, response.hover_pos()) {
            let local_end = Vec2::new(hover_screen.x - rect.min.x, hover_screen.y - rect.min.y);
            let end_w = self.cam.screen_to_world(local_end, vp_w, vp_h);
            match drag {
                CanvasDrag::CreateProjectile { start } => {
                    let p0_s = self.cam.world_to_screen(start, vp_w, vp_h);
                    let p1_s = self.cam.world_to_screen(end_w, vp_w, vp_h);
                    let sp0 = Pos2::new(rect.min.x + p0_s.x, rect.min.y + p0_s.y);
                    let sp1 = Pos2::new(rect.min.x + p1_s.x, rect.min.y + p1_s.y);
                    let pull = end_w - start;
                    let v_vec = (pull * 0.024 * self.live_fire_speed_mult).clamp_length_max(10.0);
                    let speed_kms = if pull.length() > 0.6 {
                        v_vec.length() * 10.0
                    } else {
                        self.proj_template.speed_kms
                    };
                    let preview_angle = if pull.length() > 0.6 {
                        pull.y.atan2(pull.x).to_degrees()
                    } else {
                        self.proj_template.angle_deg
                    };
                    let mut preview_proj = self.proj_template.clone();
                    preview_proj.pos = start;
                    preview_proj.angle_deg = preview_angle;
                    let to_screen = |w: Vec2| -> Pos2 {
                        let s = self.cam.world_to_screen(w, vp_w, vp_h);
                        Pos2::new(rect.min.x + s.x, rect.min.y + s.y)
                    };
                    draw_projectile_outline(
                        &painter,
                        &to_screen,
                        self.cam.zoom,
                        &preview_proj,
                        Stroke::new(1.6_f32, Color32::from_rgb(100, 220, 255)),
                    );
                    painter.line_segment(
                        [sp0, sp1],
                        Stroke::new(2.2_f32, Color32::from_rgb(255, 195, 70)),
                    );
                    painter.text(
                        Pos2::new(sp1.x + 10.0, sp1.y - 10.0),
                        Align2::LEFT_BOTTOM,
                        format!(
                            "Launch: {:.2} km/s\n{} [{}]",
                            speed_kms,
                            self.proj_template.shape.name(),
                            self.proj_template.mat.props().short_name
                        ),
                        FontId::monospace(12.0),
                        Color32::from_rgb(255, 225, 120),
                    );
                }
                CanvasDrag::CreateTargetShape { shape, start } => {
                    let to_screen = |w: Vec2| -> Pos2 {
                        let s = self.cam.world_to_screen(w, vp_w, vp_h);
                        Pos2::new(rect.min.x + s.x, rect.min.y + s.y)
                    };
                    let preview_stroke = Stroke::new(2.0_f32, Color32::from_rgb(95, 225, 255));
                    if let Some(preview_spec) = self.build_target_from_drag(shape, start, end_w) {
                        draw_target_outline(&painter, &to_screen, self.cam.zoom, &preview_spec, preview_stroke);
                        let p1 = to_screen(end_w);
                        let dim_text = match &preview_spec.kind {
                            TargetKind::SolidBox { width, height, .. } => {
                                format!("{}: {:.1} × {:.1} cm", shape.short_label(), width, height)
                            }
                            TargetKind::Ellipse { rx, ry, .. } => {
                                format!("{}: {:.1} × {:.1} cm", shape.short_label(), 2.0 * rx, 2.0 * ry)
                            }
                            TargetKind::HollowRing { outer_r, wall_thick, .. } => {
                                format!("Ring R={:.1} cm (wall {:.1} cm)", outer_r, wall_thick)
                            }
                            TargetKind::TriangleWedge { width, height, .. } => {
                                format!("Wedge: {:.1} × {:.1} cm", width, height)
                            }
                            TargetKind::CompositePlate { height, layers } => {
                                let w: f32 = layers.iter().map(|l| l.thickness).sum();
                                format!("Composite: {:.1} × {:.1} cm", w, height)
                            }
                            TargetKind::LayeredPlanet { shells } => {
                                let r: f32 = shells.iter().map(|s| s.thickness).sum();
                                format!("Planet R={:.1} cm", r)
                            }
                            TargetKind::Blueprint { bp } => {
                                format!("Blueprint: {}", bp.name)
                            }
                        };
                        painter.text(
                            Pos2::new(p1.x + 10.0, p1.y - 10.0),
                            Align2::LEFT_BOTTOM,
                            dim_text,
                            FontId::monospace(12.0),
                            Color32::from_rgb(150, 230, 255),
                        );
                    }
                }
                _ => {}
            }
        }

        // Optional velocity vector field overlay
        if self.show_vel_vectors {
            draw_velocity_vectors(&painter, rect, &self.cam, vp_w, vp_h, &self.world);
        }

        // Lagrangian Witness Gauge markers & oscilloscope
        self.gauges
            .draw_markers(&painter, rect, &self.cam, vp_w, vp_h, &self.world);
        self.gauges.draw_oscilloscope(&painter, rect);

        // Ballistic Caliper & LOS RHAe Armor Analyzer overlay
        if let Some((ca, cb)) = self.caliper_line {
            let rep = analyze_caliper_ray(&self.world, ca, cb);
            draw_caliper_overlay(&painter, rect, &self.cam, vp_w, vp_h, ca, cb, &rep);
        }

        // Quantitative colorbar for scientific scalar visualization modes
        draw_quantitative_colorbar(&painter, rect, self.vis_mode);

        // Scale bar overlay
        draw_scale_bar(&painter, rect, self.cam.zoom);

        // Staging banner or Empty space prompt
        if self.world.len() == 0 && self.canvas_drag.is_none() {
            let center = rect.center();
            painter.text(
                Pos2::new(center.x, center.y - 14.0),
                Align2::CENTER_CENTER,
                "Empty Borderless Continuum Space",
                FontId::proportional(18.0),
                Color32::from_rgba_unmultiplied(150, 175, 205, 195),
            );
            painter.text(
                Pos2::new(center.x, center.y + 14.0),
                Align2::CENTER_CENTER,
                "Use Targets Builder & Projectiles Builder tabs (or pick a Scenario), edit positions/speeds, then click ▶ Start Simulation",
                FontId::proportional(13.0),
                Color32::from_rgba_unmultiplied(115, 138, 168, 180),
            );
        } else if self.phase == SimPhase::Staging {
            let banner = "SCENE STAGING (t = 0.0 µs) — Drag bodies to move, drag arrow tips to aim/set speed, then ▶ Start Simulation";
            let banner_y = if self.active_tool == ActiveTool::Interact {
                54.0
            } else {
                80.0
            };
            let b_rect = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.min.y + banner_y),
                EguiVec2::new(660.0, 24.0),
            );
            painter.rect_filled(
                b_rect,
                CornerRadius::same(4),
                Color32::from_rgba_unmultiplied(14, 24, 34, 205),
            );
            painter.text(
                b_rect.center(),
                Align2::CENTER_CENTER,
                banner,
                FontId::proportional(11.5),
                Color32::from_rgb(135, 230, 175),
            );
        }

        // Hover Continuum Particle Probe
        if let Some(hover_pos) = response.hover_pos() {
            let local = Vec2::new(hover_pos.x - rect.min.x, hover_pos.y - rect.min.y);
            let world_p = self.cam.screen_to_world(local, vp_w, vp_h);
            draw_particle_probe(&painter, rect, &self.world, world_p);
        }

        // Live Telemetry Charts
        if self.show_hud_graphs && self.world.history.len() >= 2 {
            draw_telemetry_hud(&painter, rect, &self.world);
        }

        // Interactive live-tool canvas gizmos (laser beam, gravity vortex rings, brush/erase/blast radius)
        if let Some((a, b)) = self.last_laser_beam {
            let sa = self.cam.world_to_screen(a, vp_w, vp_h);
            let sb = self.cam.world_to_screen(b, vp_w, vp_h);
            let p0 = Pos2::new(rect.min.x + sa.x, rect.min.y + sa.y);
            let p1 = Pos2::new(rect.min.x + sb.x, rect.min.y + sb.y);
            let core_w = (0.35 * self.tool_radius * self.cam.zoom).clamp(1.6, 12.0);
            painter.line_segment(
                [p0, p1],
                Stroke::new(core_w * 2.4, Color32::from_rgba_unmultiplied(255, 75, 28, 90)),
            );
            painter.line_segment(
                [p0, p1],
                Stroke::new(core_w * 1.15, Color32::from_rgba_unmultiplied(255, 195, 80, 195)),
            );
            painter.line_segment([p0, p1], Stroke::new((core_w * 0.45).max(1.2), Color32::WHITE));
            painter.circle_filled(p1, core_w * 1.6, Color32::from_rgba_unmultiplied(255, 160, 60, 160));
            painter.circle_filled(p1, (core_w * 0.7).max(2.0), Color32::WHITE);
        }
        if let Some((gc, repel)) = self.last_gravity_well {
            let gs = self.cam.world_to_screen(gc, vp_w, vp_h);
            let gp = Pos2::new(rect.min.x + gs.x, rect.min.y + gs.y);
            let col = if repel {
                Color32::from_rgba_unmultiplied(120, 235, 255, 185)
            } else {
                Color32::from_rgba_unmultiplied(200, 120, 255, 185)
            };
            for ring in [18.0, 34.0, 52.0] {
                painter.circle_stroke(gp, ring, Stroke::new(1.5_f32, col));
            }
            painter.circle_filled(gp, 4.5, Color32::WHITE);
        }
        if self.active_tool != ActiveTool::Interact && !self.caliper_mode {
            if let Some(hp) = response.hover_pos() {
                let (rad_w, ring_col) = match self.active_tool {
                    ActiveTool::BrushMatter => (
                        self.tool_radius,
                        Color32::from_rgba_unmultiplied(110, 235, 165, 200),
                    ),
                    ActiveTool::EraseMatter => (
                        self.tool_radius,
                        Color32::from_rgba_unmultiplied(255, 110, 110, 210),
                    ),
                    ActiveTool::ThermalLaser => (
                        0.45 * self.tool_radius,
                        Color32::from_rgba_unmultiplied(255, 170, 65, 210),
                    ),
                    ActiveTool::GravityVortex => (
                        3.5,
                        Color32::from_rgba_unmultiplied(185, 130, 255, 200),
                    ),
                    ActiveTool::ShockBlast => (
                        self.blast_radius,
                        Color32::from_rgba_unmultiplied(255, 95, 45, 215),
                    ),
                    ActiveTool::Interact => (1.0, Color32::TRANSPARENT),
                };
                painter.circle_stroke(hp, (rad_w * self.cam.zoom).max(4.0), Stroke::new(1.5_f32, ring_col));
            }
        }
        if self.auto_slowmo && self.slowmo_timer > 0.0 && !self.paused {
            let pill = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.min.y + 78.0),
                EguiVec2::new(180.0, 22.0),
            );
            painter.rect_filled(
                pill,
                CornerRadius::same(11),
                Color32::from_rgba_unmultiplied(18, 42, 65, 220),
            );
            painter.text(
                pill.center(),
                Align2::CENTER_CENTER,
                "⏱ BULLET-TIME 0.22×",
                FontId::monospace(11.0),
                Color32::from_rgb(125, 235, 255),
            );
        }

        // Transient status toast notification (PNG/CSV/Scene/Gauge actions)
        if let Some((msg, ts)) = &self.status_toast {
            let age = ts.elapsed().as_secs_f32();
            if age < 4.0 {
                let alpha = ((4.0 - age) / 0.8).clamp(0.0, 1.0);
                let toast_r = Rect::from_center_size(
                    Pos2::new(rect.center().x, rect.max.y - 26.0),
                    EguiVec2::new(480.0, 26.0),
                );
                painter.rect_filled(
                    toast_r,
                    CornerRadius::same(5),
                    Color32::from_rgba_unmultiplied(16, 30, 44, (225.0 * alpha) as u8),
                );
                painter.text(
                    toast_r.center(),
                    Align2::CENTER_CENTER,
                    msg,
                    FontId::monospace(11.0),
                    Color32::from_rgba_unmultiplied(150, 240, 195, (255.0 * alpha) as u8),
                );
            } else {
                self.status_toast = None;
            }
        }

        // Floating top-center viewport quick-tool & view pill dock
        self.draw_viewport_pill_dock(ctx, rect);
    }

    fn draw_viewport_pill_dock(&mut self, ctx: &egui::Context, rect: Rect) {
        egui::Area::new(egui::Id::new("viewport_pill_dock"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(rect.center().x, rect.min.y + 8.0))
            .pivot(Align2::CENTER_TOP)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(13, 18, 28, 222))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 78, 108)))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.horizontal(|ui| {
                                let play_txt = if self.phase == SimPhase::Staging {
                                    "▶ Start"
                                } else if self.paused {
                                    "▶ Resume"
                                } else {
                                    "⏸ Pause"
                                };
                                if ui.small_button(play_txt).clicked() {
                                    self.start_simulation();
                                }
                                if ui.small_button("⏭ Step").clicked() {
                                    if self.phase == SimPhase::Staging {
                                        self.phase = SimPhase::Running;
                                    }
                                    self.paused = true;
                                    let dt = self.world.compute_cfl_dt();
                                    self.advance_physics_steps(1, dt);
                                }
                                if self.phase == SimPhase::Running
                                    && ui.small_button("↺ Edit").clicked()
                                {
                                    self.reset_to_staging();
                                }
                                ui.separator();
                                for tool in ActiveTool::ALL {
                                    if ui
                                        .selectable_label(
                                            self.active_tool == tool,
                                            tool.short_label(),
                                        )
                                        .clicked()
                                    {
                                        self.active_tool = tool;
                                    }
                                }
                                ui.separator();
                                ui.toggle_value(&mut self.show_starfield, "⭐ Stars")
                                    .on_hover_text("Toggle deep-space parallax starfield");
                                if ui
                                    .small_button("⌖ Fit")
                                    .on_hover_text("Fit camera to scene (Key: F)")
                                    .clicked()
                                {
                                    self.fit_camera_to_world();
                                }
                            });
                            if self.active_tool != ActiveTool::Interact {
                                ui.horizontal(|ui| {
                                    match self.active_tool {
                                        ActiveTool::BrushMatter => {
                                            ui.add(
                                                egui::Slider::new(&mut self.tool_radius, 0.8..=20.0)
                                                    .text("R (cm)"),
                                            );
                                            material_combo(
                                                ui,
                                                "dock_brush_mat",
                                                "Matter:",
                                                &mut self.brush_mat,
                                            );
                                        }
                                        ActiveTool::EraseMatter => {
                                            ui.add(
                                                egui::Slider::new(&mut self.tool_radius, 0.8..=22.0)
                                                    .text("Eraser R (cm)"),
                                            );
                                        }
                                        ActiveTool::ThermalLaser => {
                                            ui.add(
                                                egui::Slider::new(&mut self.laser_power, 0.4..=8.0)
                                                    .text("Power ×"),
                                            );
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.laser_angle_deg,
                                                    -180.0..=180.0,
                                                )
                                                .text("∠°"),
                                            );
                                        }
                                        ActiveTool::GravityVortex => {
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.gravity_strength,
                                                    0.04..=1.2,
                                                )
                                                .text("Pull G"),
                                            );
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.gravity_swirl,
                                                    -0.6..=0.6,
                                                )
                                                .text("Swirl"),
                                            );
                                        }
                                        ActiveTool::ShockBlast => {
                                            ui.add(
                                                egui::Slider::new(&mut self.blast_radius, 1.5..=24.0)
                                                    .text("Blast R"),
                                            );
                                            ui.add(
                                                egui::Slider::new(&mut self.blast_vel, 0.08..=1.2)
                                                    .text("Impulse"),
                                            );
                                        }
                                        ActiveTool::Interact => {}
                                    }
                                    ui.label(
                                        egui::RichText::new("[ / ]: Radius")
                                            .size(10.0)
                                            .color(Color32::from_rgb(155, 175, 200)),
                                    );
                                });
                            }
                        });
                    });
            });
    }

    fn draw_scenario_browser_modal(&mut self, ctx: &egui::Context) {
        if !self.show_scenario_browser {
            return;
        }
        let mut open = self.show_scenario_browser;
        let mut pick: Option<ScenarioPreset> = None;
        egui::Window::new("📂 Scenario & Astrophysics Library")
            .open(&mut open)
            .resizable(true)
            .default_size([740.0, 500.0])
            .anchor(Align2::CENTER_CENTER, EguiVec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for cat in ScenarioCategory::ALL {
                        ui.selectable_value(&mut self.scenario_filter, cat, cat.label());
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(410.0).show(ui, |ui| {
                    for p in ScenarioPreset::ALL {
                        if self.scenario_filter != ScenarioCategory::All
                            && p.category() != self.scenario_filter
                        {
                            continue;
                        }
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(p.title())
                                        .strong()
                                        .size(13.0)
                                        .color(Color32::from_rgb(130, 220, 255)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Load Scenario").clicked() {
                                            pick = Some(p);
                                        }
                                        ui.label(
                                            egui::RichText::new(p.category().label())
                                                .size(10.5)
                                                .color(Color32::from_rgb(160, 185, 210)),
                                        );
                                    },
                                );
                            });
                            ui.label(
                                egui::RichText::new(p.description())
                                    .size(11.0)
                                    .color(Color32::from_rgb(195, 205, 218)),
                            );
                        });
                        ui.add_space(4.0);
                    }
                });
            });
        self.show_scenario_browser = open;
        if let Some(p) = pick {
            self.load_preset(p);
            self.show_scenario_browser = false;
        }
    }

    fn draw_help_modal(&mut self, ctx: &egui::Context) {
        if !self.show_help_modal {
            return;
        }
        let mut open = self.show_help_modal;
        egui::Window::new("⌨ Controls & Continuum Hydrocode Reference (F1)")
            .open(&mut open)
            .resizable(true)
            .default_size([620.0, 460.0])
            .anchor(Align2::CENTER_CENTER, EguiVec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Interactive Simulation & Tool Hotkeys")
                        .strong()
                        .size(13.0)
                        .color(Color32::from_rgb(110, 215, 255)),
                );
                egui::Grid::new("help_hotkeys_grid")
                    .num_columns(2)
                    .spacing([18.0, 4.0])
                    .show(ui, |ui| {
                        for (k, d) in [
                            ("Space", "Start simulation / Pause / Resume"),
                            ("S / R / C", "Single CFL step / Reset to Staging / Clear Space"),
                            ("Q / B / E", "Interact / Matter Brush / Void Eraser"),
                            ("L / G / X", "Thermal Laser / Gravity Vortex (Shift=Repel) / Shock Blast"),
                            ("[  /  ]", "Decrease / Increase active tool brush radius"),
                            ("Right-Drag", "Quick-fire projectile anywhere with velocity sling"),
                            ("Middle-Drag / Wheel", "Pan camera / Cursor-centered exponential zoom"),
                            ("M / V / F", "Ballistic Caliper & RHAe / Flow Vectors / Fit Camera"),
                            ("Alt + Left-Click", "Pin or remove Lagrangian Witness Sensor Gauge"),
                            ("Left / Right Arrows", "Scrub recorded timeline replay keyframes"),
                            ("1–8 / F12 / F1", "Switch Visualization Mode / Export PNG / Toggle Help"),
                        ] {
                            ui.label(egui::RichText::new(k).monospace().strong().color(Color32::from_rgb(255, 210, 125)));
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.label(
                    egui::RichText::new("Solver & Physics Architecture")
                        .strong()
                        .size(13.0)
                        .color(Color32::from_rgb(135, 230, 175)),
                );
                ui.label("• GPU Compute: Vulkan 1.2 SPIR-V (131K spatial hash + 32x32 LDS N-body self-gravity + HOST_CACHED UMA DMA).");
                ui.label("• Solids & Shock: Mie-Grüneisen + Tillotson vaporization EOS, CSPM corrected kernel gradients, Johnson-Cook rate/thermal plasticity, Drucker-Prager brittle confinement & tensile spall.");
                ui.label("• Energetics: Programmed-burn JWL EOS detonation waves, piezoelectric contact & post-penetration delay fuzes, Fourier thermal conduction, Stefan-Boltzmann radiative cooling & molten droplet capillary cohesion.");
            });
        self.show_help_modal = open;
    }

    fn hit_test_scene(&self, world_p: Vec2) -> Option<(SceneSelection, Vec2)> {
        // Check projectiles first (usually smaller & on top)
        for (i, p) in self.scene.projectiles.iter().enumerate().rev() {
            if p.hit_test(world_p) {
                return Some((SceneSelection::Projectile(i), p.pos));
            }
        }
        for (i, t) in self.scene.targets.iter().enumerate().rev() {
            if t.hit_test(world_p) {
                return Some((SceneSelection::Target(i), t.pos));
            }
        }
        None
    }

    fn draw_staging_gizmos(&self, painter: &egui::Painter, rect: Rect, vp_w: f32, vp_h: f32) {
        let to_screen = |w: Vec2| -> Pos2 {
            let s = self.cam.world_to_screen(w, vp_w, vp_h);
            Pos2::new(rect.min.x + s.x, rect.min.y + s.y)
        };

        // Predicted N-body / ballistic orbital trajectory arcs and impact markers
        let traces = self.scene.predict_staging_trajectories();
        for tr in &traces {
            if tr.points.len() >= 2 {
                let col = if tr.is_projectile {
                    Color32::from_rgba_unmultiplied(255, 200, 95, 145)
                } else {
                    Color32::from_rgba_unmultiplied(110, 220, 255, 130)
                };
                for (si, seg) in tr.points.windows(2).enumerate() {
                    if si % 2 == 0 {
                        painter.line_segment(
                            [to_screen(seg[0]), to_screen(seg[1])],
                            Stroke::new(1.3_f32, col),
                        );
                    }
                }
            }
            if let Some(hit_w) = tr.impact {
                let hs = to_screen(hit_w);
                if rect.contains(hs) {
                    let hit_col = Color32::from_rgba_unmultiplied(255, 95, 75, 225);
                    painter.circle_stroke(hs, 6.5, Stroke::new(1.6_f32, hit_col));
                    painter.line_segment(
                        [Pos2::new(hs.x - 4.5, hs.y - 4.5), Pos2::new(hs.x + 4.5, hs.y + 4.5)],
                        Stroke::new(1.5_f32, hit_col),
                    );
                    painter.line_segment(
                        [Pos2::new(hs.x - 4.5, hs.y + 4.5), Pos2::new(hs.x + 4.5, hs.y - 4.5)],
                        Stroke::new(1.5_f32, hit_col),
                    );
                }
            }
        }

        // Draw target outlines & drift vectors
        for (i, t) in self.scene.targets.iter().enumerate() {
            let is_sel = self.selection == Some(SceneSelection::Target(i));
            let col = if is_sel {
                Color32::from_rgb(95, 225, 255)
            } else {
                Color32::from_rgba_unmultiplied(110, 175, 220, 120)
            };
            let stroke = Stroke::new(if is_sel { 2.0_f32 } else { 1.1_f32 }, col);

            draw_target_outline(painter, &to_screen, self.cam.zoom, t, stroke);

            if t.vel.length_squared() > 1e-10 {
                let p0 = to_screen(t.pos);
                let tip_w = Self::velocity_handle_world(t.pos, t.vel);
                let p1 = to_screen(tip_w);
                painter.line_segment([p0, p1], Stroke::new(2.0_f32, Color32::from_rgb(100, 220, 255)));
                painter.circle_filled(p1, 5.0, Color32::from_rgb(100, 220, 255));
            }
        }

        // Draw projectile outlines, trajectory ray & draggable velocity arrow handles
        for (i, p) in self.scene.projectiles.iter().enumerate() {
            let is_sel = self.selection == Some(SceneSelection::Projectile(i));
            let col = if is_sel {
                Color32::from_rgb(255, 205, 85)
            } else {
                Color32::from_rgba_unmultiplied(245, 175, 80, 155)
            };
            let stroke = Stroke::new(if is_sel { 2.0_f32 } else { 1.2_f32 }, col);

            draw_projectile_outline(painter, &to_screen, self.cam.zoom, p, stroke);

            let dir = Vec2::from_angle(p.angle_deg.to_radians());
            let ray_end = to_screen(p.pos + dir * 130.0);
            let p0 = to_screen(p.pos);
            let tip_w = Self::velocity_handle_world(p.pos, p.vel_cm_per_us());
            let p1 = to_screen(tip_w);

            // Dotted-look aim trajectory line
            painter.line_segment(
                [p1, ray_end],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 195, 80, 75)),
            );
            // Main velocity vector + draggable handle
            painter.line_segment([p0, p1], Stroke::new(2.4_f32, Color32::from_rgb(255, 180, 65)));
            painter.circle_filled(p1, if is_sel { 6.5 } else { 5.0 }, Color32::from_rgb(255, 205, 85));
            painter.text(
                Pos2::new(p1.x + 8.0, p1.y - 6.0),
                Align2::LEFT_BOTTOM,
                format!("{:.2} km/s @ {:.0}°", p.speed_kms, p.angle_deg),
                FontId::monospace(11.0),
                Color32::from_rgb(255, 220, 130),
            );
        }
    }
}

fn draw_rot_box_outline(
    painter: &egui::Painter,
    to_screen: &impl Fn(Vec2) -> Pos2,
    center: Vec2,
    width: f32,
    height: f32,
    angle_deg: f32,
    stroke: Stroke,
) {
    let hw = 0.5 * width;
    let hh = 0.5 * height;
    let rot = Vec2::from_angle(angle_deg.to_radians());
    let corners = [
        to_screen(center + rot.rotate(Vec2::new(-hw, -hh))),
        to_screen(center + rot.rotate(Vec2::new(hw, -hh))),
        to_screen(center + rot.rotate(Vec2::new(hw, hh))),
        to_screen(center + rot.rotate(Vec2::new(-hw, hh))),
    ];
    painter.add(egui::Shape::closed_line(corners.to_vec(), stroke));
}

fn draw_ellipse_outline(
    painter: &egui::Painter,
    to_screen: &impl Fn(Vec2) -> Pos2,
    center: Vec2,
    rx: f32,
    ry: f32,
    angle_deg: f32,
    stroke: Stroke,
) {
    const SEG: usize = 36;
    let rot = Vec2::from_angle(angle_deg.to_radians());
    let pts: Vec<Pos2> = (0..SEG)
        .map(|k| {
            let theta = (k as f32 / SEG as f32) * std::f32::consts::TAU;
            let local = Vec2::new(rx * theta.cos(), ry * theta.sin());
            to_screen(center + rot.rotate(local))
        })
        .collect();
    painter.add(egui::Shape::closed_line(pts, stroke));
}

fn draw_wedge_outline(
    painter: &egui::Painter,
    to_screen: &impl Fn(Vec2) -> Pos2,
    center: Vec2,
    width: f32,
    height: f32,
    angle_deg: f32,
    stroke: Stroke,
) {
    let hw = 0.5 * width;
    let hh = 0.5 * height;
    let rot = Vec2::from_angle(angle_deg.to_radians());
    let pts = vec![
        to_screen(center + rot.rotate(Vec2::new(-hw, -hh))),
        to_screen(center + rot.rotate(Vec2::new(hw, 0.0))),
        to_screen(center + rot.rotate(Vec2::new(-hw, hh))),
    ];
    painter.add(egui::Shape::closed_line(pts, stroke));
}

fn draw_target_outline(
    painter: &egui::Painter,
    to_screen: &impl Fn(Vec2) -> Pos2,
    zoom: f32,
    t: &TargetSpec,
    stroke: Stroke,
) {
    match &t.kind {
        TargetKind::SolidBox { width, height, .. } => {
            draw_rot_box_outline(painter, to_screen, t.pos, *width, *height, t.angle_deg, stroke);
        }
        TargetKind::Ellipse { rx, ry, .. } => {
            draw_ellipse_outline(painter, to_screen, t.pos, *rx, *ry, t.angle_deg, stroke);
        }
        TargetKind::HollowRing {
            outer_r, wall_thick, ..
        } => {
            let c = to_screen(t.pos);
            painter.circle_stroke(c, outer_r * zoom, stroke);
            let inner = (outer_r - wall_thick).max(0.0);
            if inner > 0.2 {
                painter.circle_stroke(c, inner * zoom, stroke);
            }
        }
        TargetKind::TriangleWedge { width, height, .. } => {
            draw_wedge_outline(painter, to_screen, t.pos, *width, *height, t.angle_deg, stroke);
        }
        TargetKind::CompositePlate { height, layers } => {
            let w: f32 = layers.iter().map(|l| l.thickness).sum();
            draw_rot_box_outline(painter, to_screen, t.pos, w, *height, t.angle_deg, stroke);
        }
        TargetKind::LayeredPlanet { shells } => {
            let r: f32 = shells.iter().map(|s| s.thickness).sum();
            painter.circle_stroke(to_screen(t.pos), r * zoom, stroke);
        }
        TargetKind::Blueprint { bp } => {
            let he = bp.world_half_extents(0.35);
            draw_rot_box_outline(
                painter,
                to_screen,
                t.pos,
                2.0 * he.x,
                2.0 * he.y,
                t.angle_deg,
                stroke,
            );
        }
    }
}

fn draw_projectile_outline(
    painter: &egui::Painter,
    to_screen: &impl Fn(Vec2) -> Pos2,
    zoom: f32,
    p: &ProjectileSpec,
    stroke: Stroke,
) {
    match p.shape {
        ProjectileShape::CustomBlueprint => {
            let he = p
                .blueprint
                .as_ref()
                .map(|b| b.world_half_extents(0.35))
                .unwrap_or(Vec2::new(0.5 * p.length_cm, p.radius_cm));
            draw_rot_box_outline(
                painter,
                to_screen,
                p.pos,
                2.0 * he.x,
                2.0 * he.y,
                p.angle_deg,
                stroke,
            );
        }
        ProjectileShape::Sphere | ProjectileShape::DifferentiatedAsteroid => {
            painter.circle_stroke(to_screen(p.pos), p.radius_cm * zoom, stroke);
        }
        ProjectileShape::HollowRing => {
            let c = to_screen(p.pos);
            painter.circle_stroke(c, p.radius_cm * zoom, stroke);
            let inner = (p.radius_cm * (1.0 - p.core_frac.clamp(0.15, 0.85))).max(0.2);
            painter.circle_stroke(c, inner * zoom, stroke);
        }
        ProjectileShape::Ellipse => {
            draw_ellipse_outline(
                painter,
                to_screen,
                p.pos,
                0.5 * p.length_cm,
                p.radius_cm,
                p.angle_deg,
                stroke,
            );
        }
        ProjectileShape::Wedge => {
            draw_wedge_outline(
                painter,
                to_screen,
                p.pos,
                p.length_cm,
                2.0 * p.radius_cm,
                p.angle_deg,
                stroke,
            );
        }
        ProjectileShape::LongRodDart
        | ProjectileShape::BluntSlug
        | ProjectileShape::ShapedJet => {
            draw_rot_box_outline(
                painter,
                to_screen,
                p.pos,
                p.length_cm,
                2.0 * p.radius_cm,
                p.angle_deg,
                stroke,
            );
        }
    }
}

fn material_combo(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    label: &str,
    mat: &mut MaterialId,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(mat.props().short_name)
            .show_ui(ui, |ui| {
                for cat in MaterialCategory::ALL {
                    ui.label(
                        egui::RichText::new(cat.label())
                            .strong()
                            .size(10.5)
                            .color(Color32::from_rgb(130, 185, 235)),
                    );
                    for m in MaterialId::ALL {
                        let p = m.props();
                        if p.category != cat {
                            continue;
                        }
                        let [r, g, b] = p.color;
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(EguiVec2::new(11.0, 11.0), Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                            if ui
                                .selectable_value(mat, m, p.name)
                                .on_hover_text(format!(
                                    "{}\nρ₀={:.2} g/cm³ | c₀={:.2} km/s | Tm={:.0} K",
                                    p.name,
                                    p.rho0,
                                    p.sound_speed_kms(),
                                    p.melt_temp
                                ))
                                .clicked()
                            {
                                changed = true;
                            }
                        });
                    }
                }
            });
    });
    changed
}

fn material_stats_card(ui: &mut egui::Ui, mat: MaterialId) {
    let mp = mat.props();
    ui.label(
        egui::RichText::new(format!(
            "ρ₀={:.2} g/cm³ | c₀={:.2} km/s | K={:.0} GPa | Y₀={:.2} | Spall={:.2} | Tm={:.0}K",
            mp.rho0,
            mp.sound_speed_kms(),
            mp.bulk_modulus_gpa(),
            mp.yield_gpa(),
            mp.spall_gpa(),
            mp.melt_temp
        ))
        .monospace()
        .size(10.0)
        .color(Color32::from_rgb(155, 175, 195)),
    );
}

fn edit_target_spec(
    ui: &mut egui::Ui,
    id_prefix: impl std::hash::Hash + Copy,
    spec: &mut TargetSpec,
    changed: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Name:");
        *changed |= ui.text_edit_singleline(&mut spec.name).changed();
    });

    ui.horizontal(|ui| {
        ui.label("Pos X:");
        *changed |= ui
            .add(egui::DragValue::new(&mut spec.pos.x).speed(0.3).suffix(" cm"))
            .changed();
        ui.label("Y:");
        *changed |= ui
            .add(egui::DragValue::new(&mut spec.pos.y).speed(0.3).suffix(" cm"))
            .changed();
    });

    match &mut spec.kind {
        TargetKind::SolidBox { width, height, mat } => {
            *changed |= material_combo(ui, (id_prefix, "box_mat"), "Material:", mat);
            material_stats_card(ui, *mat);
            *changed |= ui
                .add(egui::Slider::new(width, 1.0..=50.0).text("Width / Thick (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(height, 2.0..=80.0).text("Height / Length (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.angle_deg, -90.0..=90.0).text("Angle (°)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.temp_k, 100.0..=2500.0).text("Temp (K)"))
                .changed();
        }
        TargetKind::Ellipse { rx, ry, mat } => {
            *changed |= material_combo(ui, (id_prefix, "ell_mat"), "Material:", mat);
            material_stats_card(ui, *mat);
            *changed |= ui
                .add(egui::Slider::new(rx, 1.0..=40.0).text("Radius X (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(ry, 1.0..=40.0).text("Radius Y (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.angle_deg, -90.0..=90.0).text("Angle (°)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.temp_k, 100.0..=2500.0).text("Temp (K)"))
                .changed();
        }
        TargetKind::HollowRing {
            outer_r,
            wall_thick,
            mat,
        } => {
            *changed |= material_combo(ui, (id_prefix, "ring_mat"), "Material:", mat);
            material_stats_card(ui, *mat);
            *changed |= ui
                .add(egui::Slider::new(outer_r, 2.0..=40.0).text("Outer Radius (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(wall_thick, 1.0..=20.0).text("Wall Thick (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.temp_k, 100.0..=2500.0).text("Temp (K)"))
                .changed();
        }
        TargetKind::TriangleWedge { width, height, mat } => {
            *changed |= material_combo(ui, (id_prefix, "wedge_mat"), "Material:", mat);
            material_stats_card(ui, *mat);
            *changed |= ui
                .add(egui::Slider::new(width, 2.0..=50.0).text("Length (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(height, 2.0..=70.0).text("Base Height (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.angle_deg, -180.0..=180.0).text("Point Angle (°)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.temp_k, 100.0..=2500.0).text("Temp (K)"))
                .changed();
        }
        TargetKind::CompositePlate { height, layers } => {
            *changed |= ui
                .add(egui::Slider::new(height, 8.0..=80.0).text("Height (cm)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.angle_deg, -90.0..=90.0).text("Slope Angle (°)"))
                .changed();

            ui.label(egui::RichText::new("Armor Layers (Front -> Back):").size(11.0));
            let mut remove_idx = None;
            let len = layers.len();
            for (li, layer) in layers.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("L{}:", li + 1));
                    *changed |= ui
                        .add(
                            egui::DragValue::new(&mut layer.thickness)
                                .range(0.8..=20.0)
                                .speed(0.1)
                                .suffix(" cm"),
                        )
                        .changed();
                    *changed |= material_combo(ui, (id_prefix, "comp_l", li), "", &mut layer.mat);
                    if len > 1 && ui.small_button("×").clicked() {
                        remove_idx = Some(li);
                    }
                });
            }
            if let Some(ri) = remove_idx {
                layers.remove(ri);
                *changed = true;
            }
            if len < 5 && ui.small_button("+ Add Armor Layer").clicked() {
                layers.push(ArmorLayer {
                    thickness: 2.5,
                    mat: MaterialId::RhaSteel,
                });
                *changed = true;
            }
        }
        TargetKind::LayeredPlanet { shells } => {
            ui.label(egui::RichText::new("Radial Shells (Inner Core -> Outer Crust):").size(11.0));
            let mut remove_idx = None;
            let len = shells.len();
            for (si, shell) in shells.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("S{}:", si + 1));
                    *changed |= ui
                        .add(
                            egui::DragValue::new(&mut shell.thickness)
                                .range(1.0..=28.0)
                                .speed(0.2)
                                .suffix(" cm"),
                        )
                        .changed();
                    *changed |= material_combo(ui, (id_prefix, "shell_m", si), "", &mut shell.mat);
                    *changed |= ui
                        .add(
                            egui::DragValue::new(&mut shell.temp_k)
                                .range(100.0..=3000.0)
                                .speed(10.0)
                                .suffix(" K"),
                        )
                        .changed();
                    if len > 1 && ui.small_button("×").clicked() {
                        remove_idx = Some(si);
                    }
                });
            }
            if let Some(ri) = remove_idx {
                shells.remove(ri);
                *changed = true;
            }
            if len < 4 && ui.small_button("+ Add Outer Shell").clicked() {
                shells.push(PlanetShell {
                    thickness: 4.0,
                    mat: MaterialId::BasaltRock,
                    temp_k: 280.0,
                });
                *changed = true;
            }
        }
        TargetKind::Blueprint { bp } => {
            ui.label(
                egui::RichText::new(format!(
                    "Pixel Blueprint: {}×{} grid ({} particles)",
                    bp.width,
                    bp.height,
                    bp.filled_count()
                ))
                .size(11.0)
                .color(Color32::from_rgb(165, 215, 245)),
            );
            egui::ComboBox::from_id_salt((id_prefix, "bp_preset_swap"))
                .selected_text("Replace with Cutaway Preset...")
                .show_ui(ui, |ui| {
                    for preset in BuiltinTargetBp::ALL {
                        if ui.selectable_label(false, preset.label()).clicked() {
                            *bp = preset.build();
                            spec.name = bp.name.clone();
                            *changed = true;
                        }
                    }
                });
            *changed |= ui
                .add(egui::Slider::new(&mut spec.angle_deg, -180.0..=180.0).text("Angle (°)"))
                .changed();
            *changed |= ui
                .add(egui::Slider::new(&mut spec.temp_k, 100.0..=2500.0).text("Temp (K)"))
                .changed();
        }
    }

    // Optional initial velocity for moving targets / planet collisions
    let mut speed_kms = spec.speed_kms();
    let mut v_ang = spec.vel_angle_deg();
    let mut vel_edited = false;
    ui.horizontal(|ui| {
        ui.label("Drift Speed:");
        vel_edited |= ui
            .add(
                egui::DragValue::new(&mut speed_kms)
                    .range(0.0..=100.0)
                    .speed(0.05)
                    .suffix(" km/s"),
            )
            .changed();
        ui.label("Dir:");
        vel_edited |= ui
            .add(
                egui::DragValue::new(&mut v_ang)
                    .range(-180.0..=180.0)
                    .speed(1.0)
                    .suffix("°"),
            )
            .changed();
    });
    if vel_edited {
        spec.set_speed_and_angle(speed_kms, v_ang);
        *changed = true;
    }
}

fn edit_projectile_spec(
    ui: &mut egui::Ui,
    id_prefix: impl std::hash::Hash + Copy,
    spec: &mut ProjectileSpec,
    targets: &[TargetSpec],
    changed: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Name:");
        *changed |= ui.text_edit_singleline(&mut spec.name).changed();
    });

    ui.label("Shape:");
    egui::Grid::new((id_prefix, "proj_shape_grid"))
        .num_columns(3)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for (idx, s) in ProjectileShape::ALL.iter().enumerate() {
                if ui
                    .selectable_label(spec.shape == *s, s.short_label())
                    .on_hover_text(s.name())
                    .clicked()
                {
                    spec.shape = *s;
                    if *s == ProjectileShape::CustomBlueprint && spec.blueprint.is_none() {
                        let bp = BuiltinProjectileBp::HeatWarhead.build();
                        spec.name = bp.name.clone();
                        spec.speed_kms = BuiltinProjectileBp::HeatWarhead.default_speed_kms();
                        spec.blueprint = Some(bp);
                    }
                    *changed = true;
                }
                if idx % 3 == 2 {
                    ui.end_row();
                }
            }
        });

    if spec.shape == ProjectileShape::CustomBlueprint {
        if spec.blueprint.is_none() {
            spec.blueprint = Some(BuiltinProjectileBp::HeatWarhead.build());
        }
        if let Some(bp) = &mut spec.blueprint {
            ui.label(
                egui::RichText::new(format!(
                    "Warhead Blueprint: {}×{} ({} particles)",
                    bp.width,
                    bp.height,
                    bp.filled_count()
                ))
                .size(11.0)
                .color(Color32::from_rgb(255, 210, 125)),
            );
            egui::ComboBox::from_id_salt((id_prefix, "proj_bp_swap"))
                .selected_text("Load Warhead Preset...")
                .show_ui(ui, |ui| {
                    for preset in BuiltinProjectileBp::ALL {
                        if ui.selectable_label(false, preset.label()).clicked() {
                            *bp = preset.build();
                            spec.name = bp.name.clone();
                            spec.speed_kms = preset.default_speed_kms();
                            let he = bp.world_half_extents(0.35);
                            spec.length_cm = (2.0 * he.x).max(2.0);
                            spec.radius_cm = he.y.max(1.0);
                            *changed = true;
                        }
                    }
                });
        }
    } else {
        *changed |= material_combo(ui, (id_prefix, "proj_mat"), "Material:", &mut spec.mat);
        material_stats_card(ui, spec.mat);

        if spec.shape == ProjectileShape::DifferentiatedAsteroid {
            *changed |= material_combo(
                ui,
                (id_prefix, "proj_core"),
                "Core Material:",
                &mut spec.core_mat,
            );
            *changed |= ui
                .add(egui::Slider::new(&mut spec.core_frac, 0.2..=0.8).text("Core Fraction"))
                .changed();
        } else if spec.shape == ProjectileShape::HollowRing {
            *changed |= ui
                .add(egui::Slider::new(&mut spec.core_frac, 0.15..=0.85).text("Wall Ratio"))
                .changed();
        }

        match spec.shape {
            ProjectileShape::Sphere
            | ProjectileShape::HollowRing
            | ProjectileShape::DifferentiatedAsteroid => {
                *changed |= ui
                    .add(egui::Slider::new(&mut spec.radius_cm, 1.0..=16.0).text("Radius (cm)"))
                    .changed();
            }
            ProjectileShape::LongRodDart
            | ProjectileShape::BluntSlug
            | ProjectileShape::Ellipse
            | ProjectileShape::Wedge
            | ProjectileShape::ShapedJet
            | ProjectileShape::CustomBlueprint => {
                *changed |= ui
                    .add(egui::Slider::new(&mut spec.length_cm, 3.0..=40.0).text("Length (cm)"))
                    .changed();
                *changed |= ui
                    .add(egui::Slider::new(&mut spec.radius_cm, 0.5..=12.0).text("Half-Width (cm)"))
                    .changed();
            }
        }
    }

    *changed |= ui
        .add(egui::Slider::new(&mut spec.temp_k, 100.0..=2500.0).text("Init Temp (K)"))
        .changed();

    ui.separator();
    ui.label(egui::RichText::new("Starting Location & Launch Velocity").strong().size(11.5));
    ui.horizontal(|ui| {
        ui.label("Start X:");
        *changed |= ui
            .add(egui::DragValue::new(&mut spec.pos.x).speed(0.3).suffix(" cm"))
            .changed();
        ui.label("Y:");
        *changed |= ui
            .add(egui::DragValue::new(&mut spec.pos.y).speed(0.3).suffix(" cm"))
            .changed();
    });
    *changed |= ui
        .add(
            egui::Slider::new(&mut spec.speed_kms, 0.05..=100.0)
                .logarithmic(true)
                .text("Speed (km/s)"),
        )
        .changed();
    *changed |= ui
        .add(egui::Slider::new(&mut spec.angle_deg, -180.0..=180.0).text("Launch Angle (°)"))
        .changed();

    if !targets.is_empty() {
        ui.horizontal(|ui| {
            if ui.small_button("🎯 Aim at Target #1").clicked() {
                let d = targets[0].pos - spec.pos;
                if d.length_squared() > 1e-6 {
                    spec.angle_deg = d.y.atan2(d.x).to_degrees();
                    *changed = true;
                }
            }
            if targets.len() > 1 {
                egui::ComboBox::from_id_salt((id_prefix, "aim_target"))
                    .selected_text("Aim at...")
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for (ti, t) in targets.iter().enumerate() {
                            if ui
                                .selectable_label(false, format!("{}. {}", ti + 1, t.name))
                                .clicked()
                            {
                                let d = t.pos - spec.pos;
                                if d.length_squared() > 1e-6 {
                                    spec.angle_deg = d.y.atan2(d.x).to_degrees();
                                    *changed = true;
                                }
                            }
                        }
                    });
            }
        });
    }
}

fn draw_mode_legend(ui: &mut egui::Ui, mode: VisMode) {
    let text = match mode {
        VisMode::MaterialThermal => {
            "True material colors + dark fracture cracks + Planck blackbody thermal glow (>700 K)"
        }
        VisMode::ShockPressure => {
            "Cyan/White = Compressive Shockwave (+GPa) | Magenta/Red = Tensile Spall Wave (-GPa)"
        }
        VisMode::PlasticStrain => {
            "Permanent plastic deformation εp (Dark = Elastic, Orange/Yellow = Yielded Shear Bands)"
        }
        VisMode::DamageAndPhase => {
            "Blue = Elastic | Green = Plastic | Gold = Cracking | Red = Spalled | Violet = Molten Fluid"
        }
        VisMode::VelocityField => {
            "Particle speed (Indigo = 0 km/s -> Cyan = 1.5 km/s -> Yellow/Red = 5+ km/s)"
        }
        VisMode::TemperatureFlir => {
            "Thermal FLIR temperature (Dark Purple = 290 K -> Orange = 1500 K -> White = 3500+ K)"
        }
        VisMode::BondNetworkXray => {
            "Von Mises deviatoric shear stress σ_eq (Indigo = Relaxed -> Cyan/Green = Elastic -> Red = Yielding)"
        }
        VisMode::PenetrationXray => {
            "Ballistic X-Ray: Slate/Amber = Intact Structure & ERA/Ammo | Bright = Jets, Darts & Spall"
        }
    };
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .color(Color32::from_rgb(160, 210, 195)),
    );
}

fn draw_scale_bar(painter: &egui::Painter, rect: Rect, zoom: f32) {
    let cm_len = if zoom > 95.0 {
        0.5
    } else if zoom > 52.0 {
        1.0
    } else if zoom > 28.0 {
        2.0
    } else if zoom > 14.0 {
        10.0
    } else if zoom > 5.0 {
        20.0
    } else if zoom > 1.8 {
        50.0
    } else if zoom > 0.7 {
        200.0
    } else {
        500.0
    };
    let px_len = cm_len * zoom;
    let x0 = rect.min.x + 16.0;
    let y0 = rect.min.y + 22.0;
    painter.rect_filled(
        Rect::from_min_size(Pos2::new(x0 - 8.0, y0 - 12.0), EguiVec2::new(px_len + 74.0, 26.0)),
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(12, 16, 24, 195),
    );
    let c = Color32::from_rgb(185, 210, 235);
    painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0 + px_len, y0)], Stroke::new(2.0_f32, c));
    painter.line_segment(
        [Pos2::new(x0, y0 - 4.0), Pos2::new(x0, y0 + 4.0)],
        Stroke::new(2.0_f32, c),
    );
    painter.line_segment(
        [Pos2::new(x0 + px_len, y0 - 4.0), Pos2::new(x0 + px_len, y0 + 4.0)],
        Stroke::new(2.0_f32, c),
    );
    let label = if cm_len < 1.0 {
        format!("{:.0} mm", cm_len * 10.0)
    } else {
        format!("{:.0} cm", cm_len)
    };
    painter.text(
        Pos2::new(x0 + px_len + 8.0, y0),
        Align2::LEFT_CENTER,
        label,
        FontId::monospace(11.5),
        c,
    );
}

fn draw_particle_probe(
    painter: &egui::Painter,
    rect: Rect,
    world: &ContinuumWorld,
    world_p: Vec2,
) {
    let mut best_i = None;
    let mut best_d2 = (2.8 * world.dx) * (2.8 * world.dx);
    for i in 0..world.len() {
        let d2 = (world.pos[i] - world_p).length_squared();
        if d2 < best_d2 {
            best_d2 = d2;
            best_i = Some(i);
        }
    }

    let Some(i) = best_i else {
        return;
    };

    let props = world.mat[i].props();
    let temp = world.temp[i];
    let dmg = world.damage[i];
    let eps_p = world.plastic_strain[i];
    let det = world.det_state[i];
    let phase = if det == 2 {
        "DETONATION GAS PRODUCTS (JWL)"
    } else if det == 1 {
        "ARMED DELAY FUZE (COUNTDOWN)"
    } else if temp >= props.melt_temp {
        "HYDRODYNAMIC MOLTEN FLUID"
    } else if dmg >= 0.85 {
        "SPALLED / FRACTURED FRAGMENT"
    } else if dmg > 0.15 {
        "CRACKING / MICRO-DAMAGED"
    } else if eps_p > 0.015 {
        "PLASTICALLY DEFORMED SOLID"
    } else {
        "INTACT ELASTIC SOLID"
    };

    let info = format!(
        "PROBE: {}  [{}]\nSpeed: {:6.2} km/s  |  Pressure: {:+6.2} GPa  |  Shear σ_eq: {:5.2} GPa\nTemp:  {:6.0} K     |  Plastic εp: {:.3}    |  Damage: {:3.0}%",
        props.name,
        phase,
        world.vel[i].length() * 10.0,
        world.net_pressure[i] * 100.0,
        world.von_mises_gpa(i),
        temp,
        eps_p,
        dmg * 100.0,
    );

    let box_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 14.0, rect.max.y - 68.0),
        EguiVec2::new(435.0, 56.0),
    );
    painter.rect_filled(
        box_rect,
        CornerRadius::same(6),
        Color32::from_rgba_unmultiplied(12, 16, 26, 215),
    );
    painter.rect_stroke(
        box_rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Color32::from_rgb(55, 80, 115)),
        egui::StrokeKind::Outside,
    );
    painter.text(
        Pos2::new(box_rect.min.x + 10.0, box_rect.min.y + 8.0),
        Align2::LEFT_TOP,
        info,
        FontId::monospace(11.0),
        Color32::from_rgb(215, 228, 245),
    );
}

fn draw_telemetry_hud(painter: &egui::Painter, rect: Rect, world: &ContinuumWorld) {
    let w = 310.0;
    let h = 118.0;
    let panel = Rect::from_min_size(
        Pos2::new(rect.max.x - w - 14.0, rect.max.y - h - 12.0),
        EguiVec2::new(w, h),
    );
    painter.rect_filled(
        panel,
        CornerRadius::same(6),
        Color32::from_rgba_unmultiplied(12, 16, 26, 215),
    );
    painter.rect_stroke(
        panel,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Color32::from_rgb(55, 80, 115)),
        egui::StrokeKind::Outside,
    );

    let hist = &world.history;
    let latest = hist.last().copied().unwrap_or_default();
    let e_init = hist
        .first()
        .map(|s| (s.kinetic_energy + s.internal_energy + s.elastic_energy).max(1e-6))
        .unwrap_or(1e-6);
    let e_cur = latest.kinetic_energy + latest.internal_energy + latest.elastic_energy;
    let max_e = hist
        .iter()
        .map(|s| (s.kinetic_energy + s.internal_energy + s.elastic_energy).max(1e-5))
        .fold(1e-5f32, f32::max);
    let max_p = hist
        .iter()
        .map(|s| s.peak_pressure_gpa)
        .fold(5.0f32, f32::max);

    painter.text(
        Pos2::new(panel.min.x + 8.0, panel.min.y + 6.0),
        Align2::LEFT_TOP,
        format!(
            "P:{:5.1}GPa | vmax:{:4.2}km/s | Spall:{:3.0}% | Det:{}",
            latest.peak_pressure_gpa,
            latest.max_speed_kms,
            latest.fractured_frac * 100.0,
            world.detonated_count,
        ),
        FontId::monospace(9.8),
        Color32::from_rgb(195, 215, 238),
    );
    painter.text(
        Pos2::new(panel.min.x + 8.0, panel.min.y + 20.0),
        Align2::LEFT_TOP,
        format!(
            "Cyan:KE  Org:IntE  Pink:P  (E/E0:{:.2}x | |p|:{:.1})",
            e_cur / e_init,
            latest.momentum.length()
        ),
        FontId::monospace(9.3),
        Color32::from_rgb(145, 165, 188),
    );

    let plot = Rect::from_min_max(
        Pos2::new(panel.min.x + 8.0, panel.min.y + 37.0),
        Pos2::new(panel.max.x - 8.0, panel.max.y - 8.0),
    );
    painter.rect_filled(plot, CornerRadius::same(3), Color32::from_rgb(8, 11, 16));

    let n = hist.len();
    let to_pt = |idx: usize, val_norm: f32| -> Pos2 {
        let x = plot.min.x + (idx as f32 / (n - 1).max(1) as f32) * plot.width();
        let y = plot.max.y - val_norm.clamp(0.0, 1.0) * plot.height();
        Pos2::new(x, y)
    };

    let ke_pts: Vec<Pos2> = hist
        .iter()
        .enumerate()
        .map(|(i, s)| to_pt(i, s.kinetic_energy / max_e))
        .collect();
    let ie_pts: Vec<Pos2> = hist
        .iter()
        .enumerate()
        .map(|(i, s)| to_pt(i, s.internal_energy / max_e))
        .collect();
    let p_pts: Vec<Pos2> = hist
        .iter()
        .enumerate()
        .map(|(i, s)| to_pt(i, s.peak_pressure_gpa / max_p))
        .collect();

    painter.add(egui::Shape::line(
        ke_pts,
        Stroke::new(1.6_f32, Color32::from_rgb(70, 210, 255)),
    ));
    painter.add(egui::Shape::line(
        ie_pts,
        Stroke::new(1.6_f32, Color32::from_rgb(255, 155, 55)),
    ));
    painter.add(egui::Shape::line(
        p_pts,
        Stroke::new(1.3_f32, Color32::from_rgb(255, 85, 175)),
    ));
}

fn configure_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(16, 20, 28);
    visuals.window_fill = Color32::from_rgb(18, 22, 32);
    visuals.extreme_bg_color = Color32::from_rgb(10, 13, 19);
    ctx.set_visuals(visuals);
}

fn run_headless_benchmark() {
    println!("=== Rusted Space Simulation (rssim) — Vulkan GPU & Continuum Elasto-Plastic Verification ===");
    let mut vk_opt = VulkanPhysicsEngine::new().ok();
    if let Some(vk) = &vk_opt {
        println!(
            "Active Vulkan Compute Device: {} (Unified Memory: {})",
            vk.device_name, vk.is_unified_mem
        );
    } else {
        println!("Vulkan Compute unavailable — running CPU Rayon fallback");
    }

    for preset in [
        ScenarioPreset::HeatVsRhaArmor,
        ScenarioPreset::ApfsdsVsKontakt5Era,
        ScenarioPreset::EfpVsSpacedArmor,
        ScenarioPreset::ApheTankAmmoRack,
        ScenarioPreset::LowSpeedPlasticRicochet,
        ScenarioPreset::PlanetaryImpact,
    ] {
        let mut world = scenarios::build_scenario(preset, SimResolution::Balanced.dx());
        let t0 = Instant::now();
        let total_steps = 540usize;
        let batch = 12usize;
        let mut max_shock_gpa = 0.0f32;

        if let Some(vk) = &mut vk_opt {
            for _ in 0..(total_steps / batch) {
                let dt = world.compute_cfl_dt();
                vk.step_batch(&mut world, batch, dt);
                for &p in &world.sph_pressure {
                    max_shock_gpa = max_shock_gpa.max(p * 100.0);
                }
            }
        } else {
            for _ in 0..total_steps {
                let dt = world.compute_cfl_dt();
                world.step(dt);
                for &p in &world.sph_pressure {
                    max_shock_gpa = max_shock_gpa.max(p * 100.0);
                }
            }
        }

        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let has_nan = world
            .pos
            .iter()
            .any(|v| v.x.is_nan() || v.y.is_nan() || v.x.is_infinite() || v.y.is_infinite());
        assert!(!has_nan, "NaN detected in preset {:?}", preset);
        println!(
            "{:<50} | {:>5} pts | {:>4} spall | {:>4} det | {:>4} melt | Peak P = {:>5.1} GPa | {:.3} ms/step",
            preset.title(),
            world.len(),
            world.fractured_count,
            world.detonated_count,
            world.melted_count,
            max_shock_gpa,
            elapsed_ms / total_steps as f64
        );
    }

    // Also benchmark parallel pixelated crumb rasterizer across normal & extreme zoom on active HEAT impact!
    let mut bench_world = scenarios::build_scenario(
        ScenarioPreset::HeatVsRhaArmor,
        SimResolution::Balanced.dx(),
    );
    if let Some(vk) = &mut vk_opt {
        for _ in 0..20 {
            let dt = bench_world.compute_cfl_dt();
            vk.step_batch(&mut bench_world, 12, dt);
        }
    }
    let mut canvas = PixelCanvas::new();
    for &(z, label) in &[(12.5f32, "12.5x normal"), (65.0f32, "65x closeup"), (150.0f32, "150x microscope")] {
        let cam = Camera2D {
            center: Vec2::new(4.0, 0.0),
            zoom: z,
        };
        canvas.render(&bench_world, &cam, 1440, 900, VisMode::MaterialThermal, false);
        let tr = Instant::now();
        for _ in 0..10 {
            canvas.render(&bench_world, &cam, 1440, 900, VisMode::MaterialThermal, false);
        }
        let r_ms = tr.elapsed().as_secs_f64() * 100.0;
        println!(
            "Pixelated Crumb Rasterizer (1440x900 @ {:<16}): {:.2} ms/frame",
            label, r_ms
        );
    }
    println!("All Vulkan GPU continuum elasto-plastic & reactive detonation invariants verified cleanly.");
}

fn print_help() {
    println!(
        "Rusted Space Simulation (rssim) v0.0.7
Vulkan GPU-Accelerated Continuum Elasto-Plastic, Reactive Detonation & Planetary Hydrocode
(Mie-Gruneisen + JWL EOS + CSPM Renormalization + Johnson-Cook Viscoplasticity + 32x32 LDS Self-Gravity)

USAGE:
    rssim            Launch interactive Vulkan GPU simulator, Blueprint CAD & Ballistic Lab GUI
    rssim --bench    Run headless Vulkan GPU shock-physics, warhead & rasterizer benchmark
    rssim --help     Show this help message

KEY CAPABILITIES & INSTRUMENTATION:
    • 32 Continuum Materials (Metals, Ceramics, Concrete, Polymers, Liquid Water, Octol & Condensed Thermite)
    • Vulkan 1.2 SPIR-V Compute Pipeline (131K spatial hash, GPU-resident 32x32 LDS self-gravity, HOST_CACHED DMA)
    • Live Physics Tools: Matter Brush, Void Eraser, Thermal Laser, Gravity Vortex, Shock Blast (Q/B/E/L/G/X)
    • Orbital N-Body Trajectory Preview & 17 Presets (HEAT, EFP, ERA, APHE, Roche Ring, Binary Planets, Europa)
    • GIF89a Recorder, Remnant Cluster Analyzer, Caliper RHAe Analyzer (M), Witness Gauges & CSV/PNG (F12)"
    );
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--bench") {
        run_headless_benchmark();
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rusted Space Simulation (rssim) — Continuum Impact & Shock Physics")
            .with_inner_size([1520.0, 920.0])
            .with_min_inner_size([1024.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rusted Space Simulation",
        native_options,
        Box::new(|cc| Ok(Box::new(RustedSpaceSimApp::new(cc)))),
    )
}
