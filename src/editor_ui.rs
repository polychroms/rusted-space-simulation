use std::time::Instant;

use eframe::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, FontId, PointerButton, Pos2, Rect, Sense,
    Stroke, TextureHandle, TextureOptions, Vec2 as EguiVec2,
};
use glam::Vec2;

use crate::blueprint::{
    list_saved_blueprints, save_blueprint_to_disk, BuiltinProjectileBp, BuiltinTargetBp,
    PixelBlueprint, SymmetryMode,
};
use crate::materials::{MaterialCategory, MaterialId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BpTool {
    Brush,
    Line,
    Rect,
    Ellipse,
    Wedge,
    FloodFill,
    Eraser,
}

impl BpTool {
    pub const ALL: [BpTool; 7] = [
        BpTool::Brush,
        BpTool::Line,
        BpTool::Rect,
        BpTool::Ellipse,
        BpTool::Wedge,
        BpTool::FloodFill,
        BpTool::Eraser,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BpTool::Brush => "🖌 Brush",
            BpTool::Line => "📏 Line",
            BpTool::Rect => "▭ Rect",
            BpTool::Ellipse => "⬭ Ellipse",
            BpTool::Wedge => "▶ Cone/Wedge",
            BpTool::FloodFill => "🪣 Fill",
            BpTool::Eraser => "⌫ Erase",
        }
    }

    pub fn hint(self, filled: bool) -> &'static str {
        match self {
            BpTool::Brush => "Left-drag to paint cells | Right-drag to erase | Alt+Click eyedropper",
            BpTool::Line => "Drag from point A to point B to draw armor plates, casings, or fuze leads",
            BpTool::Rect => {
                if filled {
                    "Drag corner-to-corner for a solid block/charge"
                } else {
                    "Drag corner-to-corner for a hollow casing/compartment box"
                }
            }
            BpTool::Ellipse => {
                if filled {
                    "Drag bounding box for a solid ellipse/disk/dome"
                } else {
                    "Drag bounding box for a hollow ring/curved shell wall"
                }
            }
            BpTool::Wedge => {
                if filled {
                    "Drag base -> tip for a solid AP cap or nose cone (Tip: use None to carve V-cavities!)"
                } else {
                    "Hollow V-Liner! Drag mouth -> apex to draw a shaped-charge copper Munroe cone!"
                }
            }
            BpTool::FloodFill => "Left-click to flood-fill enclosed cavity with selected material (Right-click clears cavity)",
            BpTool::Eraser => "Drag to carve air cavities, standoff gaps, or hollow compartments",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteFilter {
    All,
    Category(MaterialCategory),
}

pub enum BlueprintEditorAction {
    None,
    SendToSceneAndViewRange(PixelBlueprint),
    StampToSceneKeepEditing(PixelBlueprint),
    UpdateExistingSceneObject(usize, PixelBlueprint),
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CadTexKey {
    bp_epoch: u64,
    preview_pts: Option<((i32, i32), (i32, i32))>,
    tool: BpTool,
    mat: MaterialId,
    symmetry: SymmetryMode,
    brush_radius: i32,
    shape_filled: bool,
    outline_thickness: i32,
}

pub struct BlueprintEditor {
    pub is_projectile: bool,
    pub subfolder: &'static str,
    pub bp: PixelBlueprint,
    bp_epoch: u64,

    pub tool: BpTool,
    pub mat: MaterialId,
    pub filter: PaletteFilter,
    pub symmetry: SymmetryMode,
    pub brush_radius: i32,
    pub shape_filled: bool,
    pub outline_thickness: i32,

    pub spawn_pos: Vec2,
    pub spawn_angle_deg: f32,
    pub launch_speed_kms: f32,
    pub editing_scene_index: Option<usize>,

    pub resize_w: usize,
    pub resize_h: usize,

    pub zoom_px: f32,
    pub pan_cells: Vec2,
    panning_mid: bool,

    undo_stack: Vec<PixelBlueprint>,
    redo_stack: Vec<PixelBlueprint>,
    stroke_snapshot: Option<PixelBlueprint>,
    last_paint_cell: Option<(i32, i32)>,
    drag_shape_start: Option<(i32, i32)>,
    drag_erasing: bool,

    saved_list: Vec<PixelBlueprint>,
    status_msg: Option<(String, Instant)>,
    tex: Option<TextureHandle>,
    last_tex_key: Option<CadTexKey>,
}

impl BlueprintEditor {
    pub fn new_target_editor() -> Self {
        let w = 136;
        let h = 128;
        let mut ed = Self {
            is_projectile: false,
            subfolder: "targets",
            bp: PixelBlueprint::new_empty("Custom Armor Target", w, h),
            bp_epoch: 1,
            tool: BpTool::Rect,
            mat: MaterialId::RhaSteel,
            filter: PaletteFilter::All,
            symmetry: SymmetryMode::None,
            brush_radius: 1,
            shape_filled: true,
            outline_thickness: 2,
            spawn_pos: Vec2::new(14.0, 0.0),
            spawn_angle_deg: 0.0,
            launch_speed_kms: 0.0,
            editing_scene_index: None,
            resize_w: w,
            resize_h: h,
            zoom_px: 5.8,
            pan_cells: Vec2::ZERO,
            panning_mid: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            stroke_snapshot: None,
            last_paint_cell: None,
            drag_shape_start: None,
            drag_erasing: false,
            saved_list: Vec::new(),
            status_msg: None,
            tex: None,
            last_tex_key: None,
        };
        ed.refresh_saved_list();
        ed
    }

    pub fn new_projectile_editor() -> Self {
        let w = 96;
        let h = 48;
        let mut ed = Self {
            is_projectile: true,
            subfolder: "projectiles",
            bp: PixelBlueprint::new_empty("Custom Warhead", w, h),
            bp_epoch: 1,
            tool: BpTool::Brush,
            mat: MaterialId::CompB,
            filter: PaletteFilter::All,
            symmetry: SymmetryMode::HorizontalAxis,
            brush_radius: 1,
            shape_filled: true,
            outline_thickness: 2,
            spawn_pos: Vec2::new(-16.0, 0.0),
            spawn_angle_deg: 0.0,
            launch_speed_kms: 0.85,
            editing_scene_index: None,
            resize_w: w,
            resize_h: h,
            zoom_px: 9.5,
            pan_cells: Vec2::ZERO,
            panning_mid: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            stroke_snapshot: None,
            last_paint_cell: None,
            drag_shape_start: None,
            drag_erasing: false,
            saved_list: Vec::new(),
            status_msg: None,
            tex: None,
            last_tex_key: None,
        };
        ed.refresh_saved_list();
        ed
    }

    pub fn refresh_saved_list(&mut self) {
        self.saved_list = list_saved_blueprints(self.subfolder);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn push_undo(&mut self, prev: PixelBlueprint) {
        if self.undo_stack.last().is_some_and(|b| *b == prev) {
            return;
        }
        if self.undo_stack.len() >= 64 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(prev);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        self.drag_shape_start = None;
        self.stroke_snapshot = None;
        self.last_paint_cell = None;
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        let cur = self.bp.clone();
        if self.redo_stack.len() >= 64 {
            self.redo_stack.remove(0);
        }
        self.redo_stack.push(cur);
        self.resize_w = prev.width;
        self.resize_h = prev.height;
        self.bp = prev;
        self.bp_epoch = self.bp_epoch.wrapping_add(1);
    }

    pub fn redo(&mut self) {
        self.drag_shape_start = None;
        self.stroke_snapshot = None;
        self.last_paint_cell = None;
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        let cur = self.bp.clone();
        if self.undo_stack.len() >= 64 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(cur);
        self.resize_w = next.width;
        self.resize_h = next.height;
        self.bp = next;
        self.bp_epoch = self.bp_epoch.wrapping_add(1);
    }

    pub fn load_blueprint(&mut self, next: PixelBlueprint, editing_idx: Option<usize>) {
        if self.bp != next {
            let prev = self.bp.clone();
            self.push_undo(prev);
        }
        self.resize_w = next.width;
        self.resize_h = next.height;
        self.bp = next;
        self.bp_epoch = self.bp_epoch.wrapping_add(1);
        self.editing_scene_index = editing_idx;
        self.pan_cells = Vec2::ZERO;
    }

    pub fn save_now(&mut self) {
        match save_blueprint_to_disk(self.subfolder, &self.bp) {
            Ok(path) => {
                self.refresh_saved_list();
                self.status_msg = Some((
                    format!(
                        "Saved {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    Instant::now(),
                ));
            }
            Err(e) => {
                self.status_msg = Some((format!("Save error: {}", e), Instant::now()));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_shape_tool(
        bp: &mut PixelBlueprint,
        tool: BpTool,
        p0: (i32, i32),
        p1: (i32, i32),
        brush_radius: i32,
        filled: bool,
        thickness: i32,
        val: Option<MaterialId>,
        sym: SymmetryMode,
    ) {
        match tool {
            BpTool::Line => bp.paint_line(p0, p1, brush_radius, val, sym),
            BpTool::Rect => bp.paint_rect(p0, p1, filled, thickness, val, sym),
            BpTool::Ellipse => bp.paint_ellipse(p0, p1, filled, thickness, val, sym),
            BpTool::Wedge => bp.paint_wedge(p0, p1, filled, thickness, val, sym),
            _ => {}
        }
    }

    pub fn draw_sidebar(&mut self, ui: &mut egui::Ui, sim_dx: f32) -> BlueprintEditorAction {
        let mut action = BlueprintEditorAction::None;

        // 1. Blueprint Name + Preset Cutaways + Disk Save/Load
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Name:").strong());
                ui.text_edit_singleline(&mut self.bp.name);
            });

            ui.horizontal(|ui| {
                if self.is_projectile {
                    egui::ComboBox::from_id_salt("bp_builtin_proj")
                        .width(210.0)
                        .selected_text("📦 Load Warhead Cutaway...")
                        .show_ui(ui, |ui| {
                            for preset in BuiltinProjectileBp::ALL {
                                if ui.selectable_label(false, preset.label()).clicked() {
                                    self.launch_speed_kms = preset.default_speed_kms();
                                    self.load_blueprint(preset.build(), None);
                                }
                            }
                        });
                } else {
                    egui::ComboBox::from_id_salt("bp_builtin_target")
                        .width(210.0)
                        .selected_text("🏰 Load Armor/Hull Cutaway...")
                        .show_ui(ui, |ui| {
                            for preset in BuiltinTargetBp::ALL {
                                if ui.selectable_label(false, preset.label()).clicked() {
                                    self.load_blueprint(preset.build(), None);
                                }
                            }
                        });
                }

                if ui
                    .button("💾 Save")
                    .on_hover_text("Save blueprint to ~/.local/share/rssim/ (Shortcut: Ctrl+S)")
                    .clicked()
                {
                    self.save_now();
                }
            });

            if !self.saved_list.is_empty() {
                let mut pick_idx = None;
                egui::ComboBox::from_id_salt(("bp_saved_combo", self.subfolder))
                    .width(270.0)
                    .selected_text(format!("📂 My Saved Blueprints ({})", self.saved_list.len()))
                    .show_ui(ui, |ui| {
                        for (i, sbp) in self.saved_list.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    false,
                                    format!("{} ({}×{})", sbp.name, sbp.width, sbp.height),
                                )
                                .clicked()
                            {
                                pick_idx = Some(i);
                            }
                        }
                    });
                if let Some(i) = pick_idx {
                    let chosen = self.saved_list[i].clone();
                    self.load_blueprint(chosen, None);
                }
            }

            if let Some((msg, ts)) = &self.status_msg {
                if ts.elapsed().as_secs_f32() < 4.5 {
                    ui.label(
                        egui::RichText::new(msg)
                            .size(11.0)
                            .color(Color32::from_rgb(120, 235, 165)),
                    );
                }
            }

            // Grid dimensions & utility row
            ui.horizontal(|ui| {
                ui.label("Grid:");
                ui.add(egui::DragValue::new(&mut self.resize_w).range(16..=260).prefix("W:"));
                ui.add(egui::DragValue::new(&mut self.resize_h).range(16..=260).prefix("H:"));
                if ui.small_button("Resize").clicked()
                    && (self.resize_w != self.bp.width || self.resize_h != self.bp.height)
                {
                    let prev = self.bp.clone();
                    self.push_undo(prev);
                    self.bp.resize_centered(self.resize_w, self.resize_h);
                    self.bp_epoch = self.bp_epoch.wrapping_add(1);
                }
            });
            ui.horizontal(|ui| {
                if ui.small_button("⇄ Flip H").on_hover_text("Flip horizontally").clicked() {
                    let prev = self.bp.clone();
                    self.push_undo(prev);
                    self.bp.flip_horizontal();
                    self.bp_epoch = self.bp_epoch.wrapping_add(1);
                }
                if ui.small_button("⇅ Flip V").on_hover_text("Flip vertically").clicked() {
                    let prev = self.bp.clone();
                    self.push_undo(prev);
                    self.bp.flip_vertical();
                    self.bp_epoch = self.bp_epoch.wrapping_add(1);
                }
                if ui
                    .small_button("↻ 90°")
                    .on_hover_text("Rotate 90° clockwise")
                    .clicked()
                {
                    let prev = self.bp.clone();
                    self.push_undo(prev);
                    self.bp.rotate_cw_90();
                    self.resize_w = self.bp.width;
                    self.resize_h = self.bp.height;
                    self.bp_epoch = self.bp_epoch.wrapping_add(1);
                }
                if ui
                    .small_button("✂ Crop")
                    .on_hover_text("Trim empty margin around design")
                    .clicked()
                    && self.bp.filled_count() > 0
                {
                    let prev = self.bp.clone();
                    self.bp.auto_crop(4);
                    if self.bp != prev {
                        self.resize_w = self.bp.width;
                        self.resize_h = self.bp.height;
                        self.push_undo(prev);
                        self.bp_epoch = self.bp_epoch.wrapping_add(1);
                    }
                }
                if ui.small_button("⌫ Clear").clicked() && self.bp.filled_count() > 0 {
                    let prev = self.bp.clone();
                    self.push_undo(prev);
                    self.bp.clear();
                    self.bp_epoch = self.bp_epoch.wrapping_add(1);
                }
            });
        });

        ui.add_space(5.0);

        // 2. Drawing Tools, Symmetry & Shape Options
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("CAD Drawing Tool:").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("⌖ Fit View").clicked() {
                        self.pan_cells = Vec2::ZERO;
                        self.zoom_px = if self.is_projectile { 9.5 } else { 5.8 };
                    }
                });
            });

            egui::Grid::new(("bp_tool_grid", self.subfolder))
                .num_columns(4)
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    for (idx, t) in BpTool::ALL.iter().enumerate() {
                        if ui.selectable_label(self.tool == *t, t.label()).clicked() {
                            self.tool = *t;
                        }
                        if idx % 4 == 3 {
                            ui.end_row();
                        }
                    }
                });

            ui.label(
                egui::RichText::new(self.tool.hint(self.shape_filled))
                    .size(10.5)
                    .color(Color32::from_rgb(150, 200, 225)),
            );

            ui.horizontal(|ui| {
                ui.label("Symmetry:");
                for s in SymmetryMode::ALL {
                    if ui
                        .selectable_label(self.symmetry == s, s.label())
                        .clicked()
                    {
                        self.symmetry = s;
                    }
                }
            });

            match self.tool {
                BpTool::Brush | BpTool::Eraser | BpTool::Line => {
                    ui.add(
                        egui::Slider::new(&mut self.brush_radius, 0..=8)
                            .text("Brush Radius (cells)"),
                    );
                }
                BpTool::Rect | BpTool::Ellipse | BpTool::Wedge => {
                    ui.horizontal(|ui| {
                        let fill_lbl = if self.tool == BpTool::Wedge {
                            "Solid Cone"
                        } else {
                            "Filled"
                        };
                        let hollow_lbl = if self.tool == BpTool::Wedge {
                            "Hollow V-Liner (Munroe)"
                        } else {
                            "Hollow Outline"
                        };
                        if ui.selectable_label(self.shape_filled, fill_lbl).clicked() {
                            self.shape_filled = true;
                        }
                        if ui
                            .selectable_label(!self.shape_filled, hollow_lbl)
                            .clicked()
                        {
                            self.shape_filled = false;
                        }
                    });
                    if !self.shape_filled {
                        ui.add(
                            egui::Slider::new(&mut self.outline_thickness, 1..=8)
                                .text("Wall / Liner Thick"),
                        );
                    }
                }
                BpTool::FloodFill => {}
            }
        });

        ui.add_space(5.0);

        // 3. 24-Material Categorized Continuum Material Palette
        ui.group(|ui| {
            ui.label(egui::RichText::new("Material Palette (24 Continuum Materials):").strong());

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = EguiVec2::new(3.0, 3.0);
                if ui
                    .selectable_label(self.filter == PaletteFilter::All, "All")
                    .clicked()
                {
                    self.filter = PaletteFilter::All;
                }
                for cat in MaterialCategory::ALL {
                    if ui
                        .selectable_label(
                            self.filter == PaletteFilter::Category(cat),
                            cat.label(),
                        )
                        .clicked()
                    {
                        self.filter = PaletteFilter::Category(cat);
                    }
                }
            });

            ui.add_space(3.0);
            egui::Grid::new(("bp_mat_palette", self.subfolder))
                .num_columns(3)
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    let mut shown = 0usize;
                    for m in MaterialId::ALL {
                        let p = m.props();
                        if let PaletteFilter::Category(cat) = self.filter {
                            if p.category != cat {
                                continue;
                            }
                        }
                        let is_sel = self.mat == m && self.tool != BpTool::Eraser;
                        let [r, g, b] = p.color;
                        let swatch = Color32::from_rgb(r, g, b);
                        let text_col = if (r as u16 * 2 + g as u16 * 3 + b as u16) > 850 {
                            Color32::from_rgb(15, 18, 24)
                        } else {
                            Color32::from_rgb(242, 246, 252)
                        };

                        let btn = egui::Button::new(
                            egui::RichText::new(p.short_name)
                                .size(11.0)
                                .strong()
                                .color(text_col),
                        )
                        .fill(swatch)
                        .stroke(if is_sel {
                            Stroke::new(2.2_f32, Color32::WHITE)
                        } else {
                            Stroke::new(0.8_f32, Color32::from_rgb(35, 42, 55))
                        })
                        .min_size(EguiVec2::new(92.0, 22.0));

                        if ui.add(btn).on_hover_text(p.name).clicked() {
                            self.mat = m;
                            if self.tool == BpTool::Eraser {
                                self.tool = BpTool::Brush;
                            }
                        }
                        shown += 1;
                        if shown.is_multiple_of(3) {
                            ui.end_row();
                        }
                    }
                });

            // Live material physical & detonation properties card
            let mp = self.mat.props();
            ui.add_space(3.0);
            if mp.is_energetic {
                ui.label(
                    egui::RichText::new(format!(
                        "💥 {} | ρ₀={:.2} | D_CJ={:.2} km/s | P_init={:.2} GPa | Delay={:.1} µs",
                        mp.name,
                        mp.rho0,
                        mp.det_speed_kms(),
                        mp.init_gpa(),
                        mp.delay_us
                    ))
                    .monospace()
                    .size(10.0)
                    .color(Color32::from_rgb(255, 195, 105)),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "{} | ρ₀={:.2} | Z₀={:.1} MRayl | G={:.0} GPa (ν={:.2}) | Y₀={:.2} GPa | Spall={:.2} GPa",
                        mp.short_name,
                        mp.rho0,
                        mp.acoustic_impedance_mrayl(),
                        mp.shear_modulus() * 100.0,
                        mp.poisson_ratio(),
                        mp.yield_gpa(),
                        mp.spall_gpa(),
                    ))
                    .monospace()
                    .size(9.8)
                    .color(Color32::from_rgb(165, 185, 210)),
                );
            }
        });

        ui.add_space(6.0);

        // 4. Spawn Placement / Launch Velocity & Send to Simulation Range!
        ui.group(|ui| {
            let filled = self.bp.filled_count();
            let area_per_cell = 0.866_025_4 * sim_dx * sim_dx;
            let mass_g: f32 = self
                .bp
                .cells
                .iter()
                .flatten()
                .map(|m| m.props().rho0 * area_per_cell)
                .sum();
            let he = self.bp.world_half_extents(sim_dx);
            ui.label(
                egui::RichText::new(format!(
                    "Blueprint: {} pts | {:.1} × {:.1} cm | {:.0} g/cm",
                    filled,
                    2.0 * he.x,
                    2.0 * he.y,
                    mass_g
                ))
                .size(11.0)
                .color(Color32::from_rgb(175, 210, 235)),
            );

            ui.horizontal(|ui| {
                ui.label("Pos X:");
                ui.add(egui::DragValue::new(&mut self.spawn_pos.x).speed(0.4).suffix(" cm"));
                ui.label("Y:");
                ui.add(egui::DragValue::new(&mut self.spawn_pos.y).speed(0.4).suffix(" cm"));
                ui.label("∠:");
                ui.add(
                    egui::DragValue::new(&mut self.spawn_angle_deg)
                        .range(-180.0..=180.0)
                        .speed(1.0)
                        .suffix("°"),
                );
            });

            if self.is_projectile {
                ui.add(
                    egui::Slider::new(&mut self.launch_speed_kms, 0.05..=100.0)
                        .logarithmic(true)
                        .text("Launch Speed (km/s)"),
                );
            }

            if let Some(edit_idx) = self.editing_scene_index {
                let upd_btn = egui::Button::new(
                    egui::RichText::new(format!(
                        "✔ Update Scene {} #{} & View Range",
                        if self.is_projectile { "Projectile" } else { "Target" },
                        edit_idx + 1
                    ))
                    .strong()
                    .color(Color32::from_rgb(16, 24, 20)),
                )
                .fill(Color32::from_rgb(105, 235, 185))
                .min_size(EguiVec2::new(ui.available_width(), 28.0));
                if ui.add_enabled(filled > 0, upd_btn).clicked() {
                    action =
                        BlueprintEditorAction::UpdateExistingSceneObject(edit_idx, self.bp.clone());
                }
            }

            let primary_label = if self.is_projectile {
                "🚀 Place Warhead in Simulation Range ➔"
            } else {
                "🎯 Place Target in Simulation Range ➔"
            };
            let send_btn = egui::Button::new(
                egui::RichText::new(primary_label)
                    .strong()
                    .size(13.0)
                    .color(Color32::from_rgb(18, 24, 18)),
            )
            .fill(if self.is_projectile {
                Color32::from_rgb(255, 190, 82)
            } else {
                Color32::from_rgb(95, 215, 255)
            })
            .min_size(EguiVec2::new(ui.available_width(), 30.0));

            if ui.add_enabled(filled > 0, send_btn).clicked() {
                action = BlueprintEditorAction::SendToSceneAndViewRange(self.bp.clone());
            }

            if ui
                .add_enabled(
                    filled > 0,
                    egui::Button::new("+ Stamp Another Copy into Scene (Stay Here)"),
                )
                .clicked()
            {
                action = BlueprintEditorAction::StampToSceneKeepEditing(self.bp.clone());
            }
        });

        action
    }

    pub fn draw_canvas(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, sim_dx: f32) {
        let avail = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(10, 13, 19));

        // Zoom with scroll wheel
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let factor = (scroll * 0.0024).exp();
                self.zoom_px = (self.zoom_px * factor).clamp(2.5, 38.0);
            }
        }

        // Middle-mouse pan
        let mid_down =
            response.hovered() && ctx.input(|i| i.pointer.button_pressed(PointerButton::Middle));
        let mid_up = ctx.input(|i| i.pointer.button_released(PointerButton::Middle));
        if mid_down {
            self.panning_mid = true;
        } else if mid_up {
            self.panning_mid = false;
        }
        if self.panning_mid {
            let delta = ctx.input(|i| i.pointer.delta());
            self.pan_cells.x += delta.x / self.zoom_px;
            self.pan_cells.y += delta.y / self.zoom_px;
        }

        let grid_w_px = self.bp.width as f32 * self.zoom_px;
        let grid_h_px = self.bp.height as f32 * self.zoom_px;
        let grid_origin = Pos2::new(
            rect.center().x - 0.5 * grid_w_px + self.pan_cells.x * self.zoom_px,
            rect.center().y - 0.5 * grid_h_px + self.pan_cells.y * self.zoom_px,
        );
        let grid_rect = Rect::from_min_size(grid_origin, EguiVec2::new(grid_w_px, grid_h_px));

        let screen_to_cell = |p: Pos2| -> (i32, i32) {
            let cx = ((p.x - grid_origin.x) / self.zoom_px).floor() as i32;
            let cy = ((p.y - grid_origin.y) / self.zoom_px).floor() as i32;
            (cx, cy)
        };

        let pointer_pos = response.interact_pointer_pos().or_else(|| response.hover_pos());
        let hovered_cell = pointer_pos.map(screen_to_cell);

        let alt_held = ctx.input(|i| i.modifiers.alt);
        let left_pressed =
            response.hovered() && ctx.input(|i| i.pointer.button_pressed(PointerButton::Primary));
        let right_pressed =
            response.hovered() && ctx.input(|i| i.pointer.button_pressed(PointerButton::Secondary));
        let left_down = ctx.input(|i| i.pointer.primary_down());
        let right_down = ctx.input(|i| i.pointer.secondary_down());
        let left_released = ctx.input(|i| i.pointer.button_released(PointerButton::Primary));
        let right_released = ctx.input(|i| i.pointer.button_released(PointerButton::Secondary));

        // Handle interactive painting, eyedropper, flood fill & shape drags
        if let Some((cx, cy)) = hovered_cell {
            let in_bounds =
                cx >= 0 && cy >= 0 && (cx as usize) < self.bp.width && (cy as usize) < self.bp.height;

            if left_pressed && alt_held && in_bounds {
                // Eyedropper!
                if let Some(picked) = self.bp.get(cx, cy) {
                    self.mat = picked;
                    if self.tool == BpTool::Eraser {
                        self.tool = BpTool::Brush;
                    }
                }
            } else if (left_pressed || right_pressed) && !alt_held && in_bounds {
                let erasing = right_pressed || self.tool == BpTool::Eraser;
                let paint_val = if erasing { None } else { Some(self.mat) };

                match self.tool {
                    BpTool::FloodFill => {
                        let prev = self.bp.clone();
                        self.bp.flood_fill(cx, cy, paint_val);
                        if self.bp != prev {
                            self.push_undo(prev);
                            self.bp_epoch = self.bp_epoch.wrapping_add(1);
                        }
                    }
                    BpTool::Brush | BpTool::Eraser => {
                        self.stroke_snapshot = Some(self.bp.clone());
                        self.drag_erasing = erasing;
                        self.bp
                            .paint_disk(cx, cy, self.brush_radius, paint_val, self.symmetry);
                        self.bp_epoch = self.bp_epoch.wrapping_add(1);
                        self.last_paint_cell = Some((cx, cy));
                    }
                    BpTool::Line | BpTool::Rect | BpTool::Ellipse | BpTool::Wedge => {
                        if right_pressed {
                            // Right-click always erases like a quick brush!
                            self.stroke_snapshot = Some(self.bp.clone());
                            self.drag_erasing = true;
                            self.bp
                                .paint_disk(cx, cy, self.brush_radius, None, self.symmetry);
                            self.bp_epoch = self.bp_epoch.wrapping_add(1);
                            self.last_paint_cell = Some((cx, cy));
                        } else {
                            self.drag_shape_start = Some((cx, cy));
                            self.drag_erasing = false;
                        }
                    }
                }
            } else if (left_down || right_down) && self.stroke_snapshot.is_some() {
                let val = if self.drag_erasing {
                    None
                } else {
                    Some(self.mat)
                };
                let prev_pt = self.last_paint_cell.unwrap_or((cx, cy));
                self.bp
                    .paint_line(prev_pt, (cx, cy), self.brush_radius, val, self.symmetry);
                self.bp_epoch = self.bp_epoch.wrapping_add(1);
                self.last_paint_cell = Some((cx, cy));
            }

            // Commit shape drag on left release
            if left_released {
                if let Some(start_pt) = self.drag_shape_start.take() {
                    let prev = self.bp.clone();
                    Self::apply_shape_tool(
                        &mut self.bp,
                        self.tool,
                        start_pt,
                        (cx, cy),
                        self.brush_radius,
                        self.shape_filled,
                        self.outline_thickness,
                        Some(self.mat),
                        self.symmetry,
                    );
                    if self.bp != prev {
                        self.push_undo(prev);
                        self.bp_epoch = self.bp_epoch.wrapping_add(1);
                    }
                }
            }
        }

        if left_released || right_released {
            if let Some(snap) = self.stroke_snapshot.take() {
                if self.bp != snap {
                    self.push_undo(snap);
                    self.bp_epoch = self.bp_epoch.wrapping_add(1);
                }
            }
            self.last_paint_cell = None;
            if left_released {
                self.drag_shape_start = None;
            }
        }

        // Build & upload CAD texture ONLY when blueprint cells or active shape-drag preview changed!
        let preview_pts = self.drag_shape_start.zip(hovered_cell);
        let tex_key = CadTexKey {
            bp_epoch: self.bp_epoch,
            preview_pts,
            tool: self.tool,
            mat: self.mat,
            symmetry: self.symmetry,
            brush_radius: self.brush_radius,
            shape_filled: self.shape_filled,
            outline_thickness: self.outline_thickness,
        };
        let w = self.bp.width;
        let h = self.bp.height;

        if self.tex.is_none() || self.last_tex_key != Some(tex_key) {
            let display_bp: std::borrow::Cow<'_, PixelBlueprint> =
                if let Some((start_pt, end_pt)) = preview_pts {
                    let mut temp = self.bp.clone();
                    Self::apply_shape_tool(
                        &mut temp,
                        self.tool,
                        start_pt,
                        end_pt,
                        self.brush_radius,
                        self.shape_filled,
                        self.outline_thickness,
                        Some(self.mat),
                        self.symmetry,
                    );
                    std::borrow::Cow::Owned(temp)
                } else {
                    std::borrow::Cow::Borrowed(&self.bp)
                };

            let mut pixels = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let px = match display_bp.cells[y * w + x] {
                        Some(m) => {
                            let [r, g, b] = m.props().color;
                            Color32::from_rgb(r, g, b)
                        }
                        None => {
                            if (x + y) % 2 == 0 {
                                Color32::from_rgb(17, 22, 31)
                            } else {
                                Color32::from_rgb(20, 26, 36)
                            }
                        }
                    };
                    pixels.push(px);
                }
            }

            let img = ColorImage {
                size: [w, h],
                pixels,
            };
            match &mut self.tex {
                Some(t) => t.set(img, TextureOptions::NEAREST),
                None => {
                    self.tex = Some(ctx.load_texture(
                        format!("bp_canvas_{}", self.subfolder),
                        img,
                        TextureOptions::NEAREST,
                    ));
                }
            }
            self.last_tex_key = Some(tex_key);
        }
        if let Some(t) = &self.tex {
            painter.image(
                t.id(),
                grid_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        // Crisp cell gridlines when zoomed in
        if self.zoom_px >= 6.5 {
            let line_col = Color32::from_rgba_unmultiplied(55, 70, 95, 70);
            let stroke = Stroke::new(1.0_f32, line_col);
            for x in 0..=w {
                let sx = grid_origin.x + x as f32 * self.zoom_px;
                if sx >= rect.min.x && sx <= rect.max.x {
                    painter.line_segment(
                        [
                            Pos2::new(sx, grid_rect.min.y.max(rect.min.y)),
                            Pos2::new(sx, grid_rect.max.y.min(rect.max.y)),
                        ],
                        stroke,
                    );
                }
            }
            for y in 0..=h {
                let sy = grid_origin.y + y as f32 * self.zoom_px;
                if sy >= rect.min.y && sy <= rect.max.y {
                    painter.line_segment(
                        [
                            Pos2::new(grid_rect.min.x.max(rect.min.x), sy),
                            Pos2::new(grid_rect.max.x.min(rect.max.x), sy),
                        ],
                        stroke,
                    );
                }
            }
        }

        // Outer CAD border
        painter.rect_stroke(
            grid_rect,
            CornerRadius::ZERO,
            Stroke::new(1.5_f32, Color32::from_rgb(78, 118, 168)),
            egui::StrokeKind::Outside,
        );

        // Symmetry guide axes
        let sym_stroke = Stroke::new(1.3_f32, Color32::from_rgba_unmultiplied(90, 225, 255, 175));
        if matches!(
            self.symmetry,
            SymmetryMode::HorizontalAxis | SymmetryMode::QuadAxes
        ) {
            let sy = grid_origin.y + 0.5 * grid_h_px;
            painter.line_segment(
                [Pos2::new(grid_rect.min.x, sy), Pos2::new(grid_rect.max.x, sy)],
                sym_stroke,
            );
        }
        if matches!(
            self.symmetry,
            SymmetryMode::VerticalAxis | SymmetryMode::QuadAxes
        ) {
            let sx = grid_origin.x + 0.5 * grid_w_px;
            painter.line_segment(
                [Pos2::new(sx, grid_rect.min.y), Pos2::new(sx, grid_rect.max.y)],
                sym_stroke,
            );
        }

        // Brush / cursor cell highlight
        if let Some((cx, cy)) = hovered_cell {
            if cx >= 0 && cy >= 0 && (cx as usize) < w && (cy as usize) < h {
                let c_center = Pos2::new(
                    grid_origin.x + (cx as f32 + 0.5) * self.zoom_px,
                    grid_origin.y + (cy as f32 + 0.5) * self.zoom_px,
                );
                let r_px = (self.brush_radius as f32 + 0.5) * self.zoom_px;
                if matches!(self.tool, BpTool::Brush | BpTool::Eraser | BpTool::Line) {
                    painter.circle_stroke(
                        c_center,
                        r_px,
                        Stroke::new(1.3_f32, Color32::from_rgb(255, 235, 150)),
                    );
                } else {
                    let cell_r = Rect::from_min_size(
                        Pos2::new(
                            grid_origin.x + cx as f32 * self.zoom_px,
                            grid_origin.y + cy as f32 * self.zoom_px,
                        ),
                        EguiVec2::splat(self.zoom_px),
                    );
                    painter.rect_stroke(
                        cell_r,
                        CornerRadius::ZERO,
                        Stroke::new(1.3_f32, Color32::from_rgb(255, 235, 150)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }

        // Top CAD Orientation Banner
        let header_text = if self.is_projectile {
            "WARHEAD BLUEPRINT CAD  —  Rear (-X)  ◀═══════  NOSE / FLIGHT DIRECTION (+X) ➔"
        } else {
            "TARGET STRUCTURE CAD  —  ➔ Incoming Threat (+X) Strikes Left Strike-Face First"
        };
        let b_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.min.y + 20.0),
            EguiVec2::new(650.0, 24.0),
        );
        painter.rect_filled(
            b_rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(14, 22, 34, 215),
        );
        painter.text(
            b_rect.center(),
            Align2::CENTER_CENTER,
            header_text,
            FontId::monospace(11.5),
            if self.is_projectile {
                Color32::from_rgb(255, 205, 105)
            } else {
                Color32::from_rgb(125, 220, 255)
            },
        );

        // Bottom cell & physical scale status bar
        let cell_info = if let Some((cx, cy)) = hovered_cell {
            if cx >= 0 && cy >= 0 && (cx as usize) < w && (cy as usize) < h {
                let mat_lbl = self
                    .bp
                    .get(cx, cy)
                    .map(|m| m.props().name)
                    .unwrap_or("Empty Air / Standoff Cavity");
                format!(
                    "Cell ({}, {})  |  {}  |  1 cell = {:.2} cm  |  Ctrl+Z Undo / Ctrl+Y Redo",
                    cx, cy, mat_lbl, sim_dx
                )
            } else {
                format!(
                    "Grid {}×{} ({:.1}×{:.1} cm)  |  Scroll: Zoom  |  Middle-Drag: Pan  |  Right-Click: Erase",
                    w,
                    h,
                    w as f32 * sim_dx,
                    h as f32 * sim_dx * 0.866
                )
            }
        } else {
            format!(
                "Grid {}×{} ({:.1}×{:.1} cm)  |  Scroll: Zoom  |  Middle-Drag: Pan  |  Right-Click: Erase",
                w,
                h,
                w as f32 * sim_dx,
                h as f32 * sim_dx * 0.866
            )
        };

        let f_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 14.0, rect.max.y - 34.0),
            EguiVec2::new(620.0, 24.0),
        );
        painter.rect_filled(
            f_rect,
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(12, 16, 26, 210),
        );
        painter.text(
            Pos2::new(f_rect.min.x + 10.0, f_rect.center().y),
            Align2::LEFT_CENTER,
            cell_info,
            FontId::monospace(11.0),
            Color32::from_rgb(195, 212, 235),
        );
    }
}
