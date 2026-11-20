use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use glam::Vec2;

use crate::materials::MaterialId;
use crate::physics::ContinuumWorld;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymmetryMode {
    None,
    HorizontalAxis, // Mirrors Y across horizontal centerline (ideal for projectiles!)
    VerticalAxis,   // Mirrors X across vertical centerline
    QuadAxes,       // Mirrors both X and Y
}

impl SymmetryMode {
    pub const ALL: [SymmetryMode; 4] = [
        SymmetryMode::None,
        SymmetryMode::HorizontalAxis,
        SymmetryMode::VerticalAxis,
        SymmetryMode::QuadAxes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SymmetryMode::None => "Off",
            SymmetryMode::HorizontalAxis => "Horiz ↔",
            SymmetryMode::VerticalAxis => "Vert ↕",
            SymmetryMode::QuadAxes => "4-Way +",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PixelBlueprint {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Option<MaterialId>>,
}

impl PixelBlueprint {
    pub fn new_empty(name: impl Into<String>, width: usize, height: usize) -> Self {
        let width = width.clamp(8, 320);
        let height = height.clamp(8, 320);
        Self {
            name: name.into(),
            width,
            height,
            cells: vec![None; width * height],
        }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<MaterialId> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.cells[y as usize * self.width + x as usize]
    }

    #[inline]
    pub fn set_raw(&mut self, x: i32, y: i32, val: Option<MaterialId>) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.cells[y as usize * self.width + x as usize] = val;
        }
    }

    #[inline]
    pub fn set_sym(&mut self, x: i32, y: i32, val: Option<MaterialId>, sym: SymmetryMode) {
        self.set_raw(x, y, val);
        let mx = (self.width as i32 - 1) - x;
        let my = (self.height as i32 - 1) - y;
        match sym {
            SymmetryMode::None => {}
            SymmetryMode::HorizontalAxis => {
                self.set_raw(x, my, val);
            }
            SymmetryMode::VerticalAxis => {
                self.set_raw(mx, y, val);
            }
            SymmetryMode::QuadAxes => {
                self.set_raw(x, my, val);
                self.set_raw(mx, y, val);
                self.set_raw(mx, my, val);
            }
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(None);
    }

    pub fn resize_centered(&mut self, new_w: usize, new_h: usize) {
        let new_w = new_w.clamp(12, 300);
        let new_h = new_h.clamp(12, 300);
        if new_w == self.width && new_h == self.height {
            return;
        }
        let mut next = vec![None; new_w * new_h];
        let ox = (new_w as isize - self.width as isize) / 2;
        let oy = (new_h as isize - self.height as isize) / 2;
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as isize + ox;
                let ny = y as isize + oy;
                if nx >= 0 && ny >= 0 && (nx as usize) < new_w && (ny as usize) < new_h {
                    next[ny as usize * new_w + nx as usize] = self.cells[y * self.width + x];
                }
            }
        }
        self.width = new_w;
        self.height = new_h;
        self.cells = next;
    }

    pub fn flip_horizontal(&mut self) {
        for y in 0..self.height {
            let row = y * self.width;
            self.cells[row..row + self.width].reverse();
        }
    }

    pub fn flip_vertical(&mut self) {
        for y in 0..(self.height / 2) {
            let top = y * self.width;
            let bot = (self.height - 1 - y) * self.width;
            for x in 0..self.width {
                self.cells.swap(top + x, bot + x);
            }
        }
    }

    pub fn rotate_cw_90(&mut self) {
        let new_w = self.height;
        let new_h = self.width;
        let mut next = vec![None; new_w * new_h];
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = self.height - 1 - y;
                let ny = x;
                next[ny * new_w + nx] = self.cells[y * self.width + x];
            }
        }
        self.width = new_w;
        self.height = new_h;
        self.cells = next;
    }

    pub fn auto_crop(&mut self, margin: usize) {
        let Some((min_x, min_y, max_x, max_y)) = self.occupied_bounds() else {
            return;
        };
        let content_w = max_x - min_x + 1;
        let content_h = max_y - min_y + 1;
        let new_w = (content_w + 2 * margin).clamp(16, 300);
        let new_h = (content_h + 2 * margin).clamp(16, 300);
        let pad_x = (new_w.saturating_sub(content_w)) / 2;
        let pad_y = (new_h.saturating_sub(content_h)) / 2;
        let mut next = vec![None; new_w * new_h];
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let nx = pad_x + (x - min_x);
                let ny = pad_y + (y - min_y);
                if nx < new_w && ny < new_h {
                    next[ny * new_w + nx] = self.cells[y * self.width + x];
                }
            }
        }
        self.width = new_w;
        self.height = new_h;
        self.cells = next;
    }

    pub fn filled_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// Tight bounding box `(min_x, min_y, max_x, max_y)` of occupied cells.
    pub fn occupied_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let mut min_x = usize::MAX;
        let mut min_y = usize::MAX;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut any = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.cells[y * self.width + x].is_some() {
                    any = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        any.then_some((min_x, min_y, max_x, max_y))
    }

    pub fn paint_disk(
        &mut self,
        cx: i32,
        cy: i32,
        radius: i32,
        val: Option<MaterialId>,
        sym: SymmetryMode,
    ) {
        let r = radius.max(0);
        if r == 0 {
            self.set_sym(cx, cy, val, sym);
            return;
        }
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r2 {
                    self.set_sym(cx + dx, cy + dy, val, sym);
                }
            }
        }
    }

    pub fn paint_line(
        &mut self,
        p0: (i32, i32),
        p1: (i32, i32),
        radius: i32,
        val: Option<MaterialId>,
        sym: SymmetryMode,
    ) {
        let dx = p1.0 - p0.0;
        let dy = p1.1 - p0.1;
        let steps = dx.abs().max(dy.abs()).max(1);
        for s in 0..=steps {
            let x = p0.0 + ((dx as f32 * (s as f32 / steps as f32)).round() as i32);
            let y = p0.1 + ((dy as f32 * (s as f32 / steps as f32)).round() as i32);
            self.paint_disk(x, y, radius, val, sym);
        }
    }

    pub fn paint_rect(
        &mut self,
        p0: (i32, i32),
        p1: (i32, i32),
        filled: bool,
        thickness: i32,
        val: Option<MaterialId>,
        sym: SymmetryMode,
    ) {
        let x0 = p0.0.min(p1.0);
        let x1 = p0.0.max(p1.0);
        let y0 = p0.1.min(p1.1);
        let y1 = p0.1.max(p1.1);
        let t = thickness.max(1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let on_border = x - x0 < t || x1 - x < t || y - y0 < t || y1 - y < t;
                if filled || on_border {
                    self.set_sym(x, y, val, sym);
                }
            }
        }
    }

    pub fn paint_ellipse(
        &mut self,
        p0: (i32, i32),
        p1: (i32, i32),
        filled: bool,
        thickness: i32,
        val: Option<MaterialId>,
        sym: SymmetryMode,
    ) {
        let x0 = p0.0.min(p1.0);
        let x1 = p0.0.max(p1.0);
        let y0 = p0.1.min(p1.1);
        let y1 = p0.1.max(p1.1);
        let cx = 0.5 * (x0 + x1) as f32;
        let cy = 0.5 * (y0 + y1) as f32;
        let rx = (0.5 * (x1 - x0) as f32).max(0.5);
        let ry = (0.5 * (y1 - y0) as f32).max(0.5);
        let rx_in = (rx - thickness.max(1) as f32).max(0.0);
        let ry_in = (ry - thickness.max(1) as f32).max(0.0);

        for y in y0..=y1 {
            for x in x0..=x1 {
                let nx = (x as f32 - cx) / rx;
                let ny = (y as f32 - cy) / ry;
                if nx * nx + ny * ny <= 1.05 {
                    if filled || rx_in <= 0.2 || ry_in <= 0.2 {
                        self.set_sym(x, y, val, sym);
                    } else {
                        let ix = (x as f32 - cx) / rx_in;
                        let iy = (y as f32 - cy) / ry_in;
                        if ix * ix + iy * iy >= 0.95 {
                            self.set_sym(x, y, val, sym);
                        }
                    }
                }
            }
        }
    }

    /// Paint either a filled wedge/cone OR a hollow V-shaped liner (perfect for HEAT copper cones!).
    pub fn paint_wedge(
        &mut self,
        p0: (i32, i32),
        p1: (i32, i32),
        filled: bool,
        thickness: i32,
        val: Option<MaterialId>,
        sym: SymmetryMode,
    ) {
        let x_base = p0.0;
        let x_tip = p1.0;
        let y0 = p0.1.min(p1.1);
        let y1 = p0.1.max(p1.1);
        let cy = (y0 + y1) / 2;
        let half_h = ((y1 - y0) / 2).max(1);

        if !filled {
            // V-shaped liner legs from (x_base, cy - half_h) -> (x_tip, cy) -> (x_base, cy + half_h)
            let r = (thickness.max(1) - 1) / 2;
            self.paint_line((x_base, cy - half_h), (x_tip, cy), r, val, sym);
            self.paint_line((x_base, cy + half_h), (x_tip, cy), r, val, sym);
            return;
        }

        let xa = x_base.min(x_tip);
        let xb = x_base.max(x_tip);
        let span = (xb - xa).max(1) as f32;
        for x in xa..=xb {
            let s = if x_tip >= x_base {
                (x - x_base) as f32 / span
            } else {
                (x_base - x) as f32 / span
            };
            let allowed = ((1.0 - s) * half_h as f32).round() as i32;
            for y in (cy - allowed)..=(cy + allowed) {
                self.set_sym(x, y, val, sym);
            }
        }
    }

    pub fn flood_fill(&mut self, sx: i32, sy: i32, replacement: Option<MaterialId>) {
        if sx < 0 || sy < 0 || sx as usize >= self.width || sy as usize >= self.height {
            return;
        }
        let target = self.get(sx, sy);
        if target == replacement {
            return;
        }
        let mut q = VecDeque::new();
        q.push_back((sx, sy));
        self.set_raw(sx, sy, replacement);

        while let Some((x, y)) = q.pop_front() {
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if nx >= 0
                    && ny >= 0
                    && (nx as usize) < self.width
                    && (ny as usize) < self.height
                    && self.get(nx, ny) == target
                {
                    self.set_raw(nx, ny, replacement);
                    q.push_back((nx, ny));
                }
            }
        }
    }

    /// Spawn all occupied cells of this blueprint into `world` centered on its occupied bounding box!
    pub fn spawn_into(
        &self,
        world: &mut ContinuumWorld,
        center: Vec2,
        angle_rad: f32,
        vel: Vec2,
        temp_override: Option<f32>,
    ) {
        let Some((min_x, min_y, max_x, max_y)) = self.occupied_bounds() else {
            return;
        };
        let start_idx = world.len();
        let body_id = world.alloc_body_id();
        let dx = world.dx;
        let dy = dx * 0.866_025_4;
        let rot = Vec2::from_angle(angle_rad);

        let mid_gx = 0.5 * (min_x + max_x) as f32;
        let mid_gy = 0.5 * (min_y + max_y) as f32;

        // Note: grid y=0 is top in editor screen space, so world local_y flips `(mid_gy - y)`!
        for y in min_y..=max_y {
            let row_off = if y % 2 == 1 { 0.5 * dx } else { 0.0 };
            let ly = (mid_gy - y as f32) * dy;
            for x in min_x..=max_x {
                if let Some(mat) = self.cells[y * self.width + x] {
                    // Check if surrounded on all 4 cardinal sides -> apply organic Voronoi grain jitter!
                    let xi = x as i32;
                    let yi = y as i32;
                    let is_interior = self.get(xi - 1, yi) == Some(mat)
                        && self.get(xi + 1, yi) == Some(mat)
                        && self.get(xi, yi - 1) == Some(mat)
                        && self.get(xi, yi + 1) == Some(mat);
                    let mut h = (x as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                        ^ ((body_id as u64) << 32);
                    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                    h ^= h >> 31;
                    let jx = ((h & 0xFFFF) as f32 / 65535.0) - 0.5;
                    let jy = (((h >> 16) & 0xFFFF) as f32 / 65535.0) - 0.5;
                    let amp = if is_interior && !mat.props().is_energetic {
                        0.025 * dx
                    } else {
                        0.0
                    };

                    let lx = (x as f32 - mid_gx) * dx + row_off + jx * amp;
                    let world_p = center + rot.rotate(Vec2::new(lx, ly + jy * amp));
                    world.push_particle(world_p, vel, mat, body_id, temp_override);
                }
            }
        }

        let end_idx = world.len();
        world.bond_range(start_idx, end_idx, true);
    }

    pub fn world_half_extents(&self, dx: f32) -> Vec2 {
        let Some((min_x, min_y, max_x, max_y)) = self.occupied_bounds() else {
            return Vec2::splat(2.0);
        };
        let dy = dx * 0.866_025_4;
        Vec2::new(
            0.5 * ((max_x - min_x + 1) as f32) * dx,
            0.5 * ((max_y - min_y + 1) as f32) * dy,
        )
    }

    // --- Simple ASCII Serialization for Custom User Blueprints ---
    pub fn serialize_text(&self) -> String {
        let mut out = format!("RSSIM_BP_V1\nname={}\nsize={}x{}\n", self.name, self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                if x > 0 {
                    out.push(' ');
                }
                match self.cells[y * self.width + x] {
                    Some(m) => out.push_str(m.props().code),
                    None => out.push('.'),
                }
            }
            out.push('\n');
        }
        out
    }

    pub fn deserialize_text(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()?.trim() != "RSSIM_BP_V1" {
            return None;
        }
        let name = lines.next()?.strip_prefix("name=")?.to_string();
        let sz_str = lines.next()?.strip_prefix("size=")?;
        let (w_s, h_s) = sz_str.split_once('x')?;
        let width: usize = w_s.parse().ok()?;
        let height: usize = h_s.parse().ok()?;
        let mut bp = Self::new_empty(name, width, height);
        for y in 0..height {
            let Some(row_line) = lines.next() else {
                break;
            };
            for (x, tok) in row_line.split_whitespace().take(width).enumerate() {
                if tok != "." {
                    bp.cells[y * width + x] = MaterialId::from_code(tok);
                }
            }
        }
        Some(bp)
    }
}

pub fn blueprint_storage_dir(subfolder: &str) -> PathBuf {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        })
        .join("rssim");
    #[cfg(not(windows))]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/rssim");
    let base = root.join(subfolder);
    let _ = fs::create_dir_all(&base);
    base
}

pub fn save_blueprint_to_disk(subfolder: &str, bp: &PixelBlueprint) -> Result<PathBuf, String> {
    let dir = blueprint_storage_dir(subfolder);
    let safe_name: String = bp
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let file_name = if safe_name.trim_matches('_').is_empty() {
        "custom_blueprint.rsim".to_string()
    } else {
        format!("{}.rsim", safe_name)
    };
    let path = dir.join(file_name);
    fs::write(&path, bp.serialize_text()).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn list_saved_blueprints(subfolder: &str) -> Vec<PixelBlueprint> {
    let dir = blueprint_storage_dir(subfolder);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "rsim") {
                if let Ok(txt) = fs::read_to_string(entry.path()) {
                    if let Some(bp) = PixelBlueprint::deserialize_text(&txt) {
                        out.push(bp);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ============================================================================
// BUILT-IN PROJECTILE & TARGET CUTAWAY BLUEPRINTS
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinProjectileBp {
    HeatWarhead,
    TandemHeat,
    EfpDishWarhead,
    ApfsdsTungsten,
    ApfsdsDepletedU,
    ApheDelayShell,
    HeshSquashHead,
}

impl BuiltinProjectileBp {
    pub const ALL: [BuiltinProjectileBp; 7] = [
        BuiltinProjectileBp::HeatWarhead,
        BuiltinProjectileBp::TandemHeat,
        BuiltinProjectileBp::EfpDishWarhead,
        BuiltinProjectileBp::ApfsdsTungsten,
        BuiltinProjectileBp::ApfsdsDepletedU,
        BuiltinProjectileBp::ApheDelayShell,
        BuiltinProjectileBp::HeshSquashHead,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BuiltinProjectileBp::HeatWarhead => "HEAT Shaped-Charge (PG-7)",
            BuiltinProjectileBp::TandemHeat => "Tandem HEAT Anti-ERA",
            BuiltinProjectileBp::EfpDishWarhead => "EFP Self-Forging Dish Liner",
            BuiltinProjectileBp::ApfsdsTungsten => "APFSDS Tungsten Long-Rod",
            BuiltinProjectileBp::ApfsdsDepletedU => "APFSDS Depleted Uranium",
            BuiltinProjectileBp::ApheDelayShell => "APHE Armor-Piercing HE (88mm)",
            BuiltinProjectileBp::HeshSquashHead => "HESH Squash-Head (120mm)",
        }
    }

    pub fn default_speed_kms(self) -> f32 {
        match self {
            BuiltinProjectileBp::HeatWarhead => 0.32,
            BuiltinProjectileBp::TandemHeat => 0.30,
            BuiltinProjectileBp::EfpDishWarhead => 0.25, // Detonation forges a ~2.2 km/s copper slug!
            BuiltinProjectileBp::ApfsdsTungsten => 1.75,
            BuiltinProjectileBp::ApfsdsDepletedU => 1.70,
            BuiltinProjectileBp::ApheDelayShell => 1.15,
            BuiltinProjectileBp::HeshSquashHead => 0.73,
        }
    }

    pub fn build(self) -> PixelBlueprint {
        match self {
            BuiltinProjectileBp::HeatWarhead => {
                // 92x57 grid (odd height -> exact hex-parity symmetry!): Confined Comp-B + 48° V-shaped Copper cone + open standoff cowl
                let mut bp = PixelBlueprint::new_empty("PG-7 HEAT Warhead", 92, 57);
                let cy = 28;
                let sym = SymmetryMode::HorizontalAxis;

                // 1. Main Comp-B Explosive charge (y = cy-14..cy+14, ample charge thickness even at cone mouth!)
                bp.paint_rect((12, cy - 14), (48, cy + 14), true, 1, Some(MaterialId::CompB), sym);
                // Carve open V-cavity on front of Comp-B so copper cone sits cleanly inside
                bp.paint_wedge((49, cy - 10), (19, cy), true, 1, None, sym);

                // 2. Heavy Mild-Steel warhead confinement jacket & rear tamper (directs detonation energy inward)
                bp.paint_line((9, cy - 16), (49, cy - 16), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_line((9, cy - 15), (49, cy - 15), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_line((9, cy - 16), (9, cy - 2), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_line((10, cy - 15), (10, cy - 2), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_rect((1, cy - 3), (6, cy + 3), true, 1, Some(MaterialId::Aluminum), sym);

                // 3. V-shaped OFHC Copper Shaped-Charge Liner (apex x=19, mouth x=48, 2 cells thick for hypersonic jet stream)
                bp.paint_wedge((48, cy - 9), (19, cy), false, 2, Some(MaterialId::Copper), sym);

                // 4. Open-bore Aluminum Standoff Cowl + flush Piezo Impact Nose Ring + insulated rear firing wire
                bp.paint_line((49, cy - 15), (81, cy - 7), 0, Some(MaterialId::Aluminum), sym);
                bp.paint_line((50, cy - 17), (81, cy - 8), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_line((7, cy - 17), (50, cy - 17), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_line((7, cy - 17), (7, cy), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((7, cy - 1), (12, cy + 1), true, 1, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((81, cy - 8), (87, cy - 5), true, 1, Some(MaterialId::ImpactFuze), sym);
                bp
            }

            BuiltinProjectileBp::TandemHeat => {
                let mut bp = PixelBlueprint::new_empty("PG-7VR Tandem HEAT", 108, 57);
                let cy = 28;
                let sym = SymmetryMode::HorizontalAxis;

                // Rear Main HEAT Charge (x = 12..46, y = cy-13..cy+13)
                bp.paint_rect((12, cy - 13), (46, cy + 13), true, 1, Some(MaterialId::CompB), sym);
                bp.paint_wedge((47, cy - 9), (19, cy), true, 1, None, sym);
                bp.paint_wedge((46, cy - 8), (19, cy), false, 2, Some(MaterialId::Copper), sym);
                bp.paint_line((9, cy - 15), (62, cy - 15), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_line((9, cy - 14), (62, cy - 14), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_line((9, cy - 15), (9, cy - 2), 0, Some(MaterialId::MildSteel), sym);
                // Mild-steel blast baffle shielding rear main charge from precursor blast
                bp.paint_line((62, cy - 14), (62, cy - 1), 0, Some(MaterialId::MildSteel), sym);

                // Rear Delay Fuze well at (10..13, cy) initiated via flush outer piezo channel (12 µs after nose impact!)
                bp.paint_rect((9, cy - 1), (13, cy + 1), true, 1, Some(MaterialId::DelayFuze), sym);
                bp.paint_line((7, cy - 16), (66, cy - 16), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_line((7, cy - 16), (7, cy), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((7, cy - 1), (9, cy + 1), true, 1, Some(MaterialId::ImpactFuze), sym);

                // Front Precursor HEAT Charge (x = 68..84) to trigger & clear Kontakt-5 ERA!
                bp.paint_rect((68, cy - 7), (84, cy + 7), true, 1, Some(MaterialId::CompB), sym);
                bp.paint_wedge((85, cy - 5), (72, cy), true, 1, None, sym);
                bp.paint_wedge((84, cy - 4), (72, cy), false, 2, Some(MaterialId::Copper), sym);
                bp.paint_line((66, cy - 16), (98, cy - 5), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_line((65, cy - 16), (65, cy), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((65, cy - 1), (67, cy + 1), true, 1, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((98, cy - 5), (103, cy - 3), true, 1, Some(MaterialId::ImpactFuze), sym);
                bp
            }

            BuiltinProjectileBp::EfpDishWarhead => {
                // Explosively Formed Penetrator: heavy RHA confinement cylinder + Comp-B + shallow concave Copper dish
                let mut bp = PixelBlueprint::new_empty("EFP Self-Forging Dish", 92, 51);
                let cy = 25;
                let sym = SymmetryMode::HorizontalAxis;

                // Heavy RHA steel confinement sleeve & rear tamper
                bp.paint_rect((10, cy - 13), (45, cy + 13), true, 1, Some(MaterialId::RhaSteel), sym);
                bp.paint_rect((14, cy - 11), (45, cy + 11), true, 1, Some(MaterialId::CompB), sym);
                // Carve shallow 135° dish cavity on front of Comp-B
                bp.paint_wedge((46, cy - 11), (39, cy), true, 1, None, sym);
                // Shallow variable-thickness OFHC Copper dish liner (folds forward into a cohesive slug!)
                bp.paint_wedge((45, cy - 10), (39, cy), false, 4, Some(MaterialId::Copper), sym);

                // Flush standoff piezo probe routed along casing to rear centerline initiator
                bp.paint_line((46, cy - 14), (84, cy - 2), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_line((8, cy - 14), (46, cy - 14), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_line((8, cy - 14), (8, cy), 0, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((8, cy - 1), (14, cy + 1), true, 1, Some(MaterialId::ImpactFuze), sym);
                bp.paint_rect((83, cy - 2), (88, cy + 2), true, 1, Some(MaterialId::ImpactFuze), sym);
                bp
            }

            BuiltinProjectileBp::ApfsdsTungsten => {
                let mut bp = PixelBlueprint::new_empty("M829 Tungsten APFSDS", 96, 36);
                let cy = 18;
                let sym = SymmetryMode::HorizontalAxis;
                // Stabilizer fins
                bp.paint_line((4, cy - 11), (16, cy - 3), 1, Some(MaterialId::Aluminum), sym);
                // Long-rod WHA core
                bp.paint_rect((10, cy - 3), (76, cy + 3), true, 1, Some(MaterialId::Tungsten), sym);
                // Tapered WHA tip + ballistic Al cap
                bp.paint_wedge((76, cy - 3), (88, cy), true, 1, Some(MaterialId::Tungsten), sym);
                bp.paint_wedge((86, cy - 2), (93, cy), true, 1, Some(MaterialId::Aluminum), sym);
                bp
            }

            BuiltinProjectileBp::ApfsdsDepletedU => {
                let mut bp = PixelBlueprint::new_empty("M829A3 Depleted U APFSDS", 96, 36);
                let cy = 18;
                let sym = SymmetryMode::HorizontalAxis;
                bp.paint_line((4, cy - 11), (16, cy - 3), 1, Some(MaterialId::Aluminum), sym);
                bp.paint_rect(
                    (10, cy - 3),
                    (74, cy + 3),
                    true,
                    1,
                    Some(MaterialId::DepletedUranium),
                    sym,
                );
                bp.paint_wedge(
                    (74, cy - 3),
                    (86, cy),
                    true,
                    1,
                    Some(MaterialId::DepletedUranium),
                    sym,
                );
                bp.paint_wedge((84, cy - 2), (92, cy), true, 1, Some(MaterialId::Titanium), sym);
                bp
            }

            BuiltinProjectileBp::ApheDelayShell => {
                let mut bp = PixelBlueprint::new_empty("88mm PzGr.39 APHE", 72, 40);
                let cy = 20;
                let sym = SymmetryMode::HorizontalAxis;

                // Thick RHA penetrating shell body
                bp.paint_rect((10, cy - 10), (46, cy + 10), true, 1, Some(MaterialId::RhaSteel), sym);
                // Hardened HHA armor-piercing cap + Al ballistic windshield
                bp.paint_wedge((46, cy - 10), (60, cy), true, 1, Some(MaterialId::HhaSteel), sym);
                bp.paint_wedge((56, cy - 6), (67, cy), true, 1, Some(MaterialId::Aluminum), sym);
                // Copper driving band
                bp.paint_rect((14, cy - 11), (18, cy - 10), true, 1, Some(MaterialId::Copper), sym);
                // Internal Cast TNT burster cavity + Base Delay Fuze (18 µs post-impact!)
                bp.paint_rect((15, cy - 6), (40, cy + 6), true, 1, Some(MaterialId::TntExplosive), sym);
                bp.paint_rect((10, cy - 5), (15, cy + 5), true, 1, Some(MaterialId::DelayFuze), sym);
                bp
            }

            BuiltinProjectileBp::HeshSquashHead => {
                let mut bp = PixelBlueprint::new_empty("120mm HESH Squash-Head", 68, 42);
                let cy = 21;
                let sym = SymmetryMode::HorizontalAxis;

                // Thin malleable Copper/MildSteel outer shell filled with C4 plastic explosive putty
                bp.paint_ellipse((12, cy - 12), (62, cy + 12), true, 1, Some(MaterialId::C4Explosive), sym);
                bp.paint_rect((10, cy - 12), (38, cy + 12), true, 1, Some(MaterialId::C4Explosive), sym);
                bp.paint_ellipse((12, cy - 12), (62, cy + 12), false, 2, Some(MaterialId::Copper), sym);
                bp.paint_line((9, cy - 12), (38, cy - 12), 0, Some(MaterialId::MildSteel), sym);
                bp.paint_line((9, cy - 12), (9, cy), 0, Some(MaterialId::MildSteel), sym);
                // Base Delay Fuze (18 µs so C4 squashes flat onto armor plate before detonating!)
                bp.paint_rect((10, cy - 6), (16, cy + 6), true, 1, Some(MaterialId::DelayFuze), sym);
                bp
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinTargetBp {
    Kontakt5EraHull,
    ChobhamNeraCheek,
    TankHullAmmoRack,
    ArmoredGlassBlock,
    ConcreteBunker,
}

impl BuiltinTargetBp {
    pub const ALL: [BuiltinTargetBp; 5] = [
        BuiltinTargetBp::Kontakt5EraHull,
        BuiltinTargetBp::ChobhamNeraCheek,
        BuiltinTargetBp::TankHullAmmoRack,
        BuiltinTargetBp::ArmoredGlassBlock,
        BuiltinTargetBp::ConcreteBunker,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BuiltinTargetBp::Kontakt5EraHull => "Kontakt-5 ERA + Composite Glacis",
            BuiltinTargetBp::ChobhamNeraCheek => "Chobham NERA + Ceramic Turret Cheek",
            BuiltinTargetBp::TankHullAmmoRack => "Tank Hull Cutaway + Ammo Rack",
            BuiltinTargetBp::ArmoredGlassBlock => "Multi-Layer Ballistic Glass + Ceramic",
            BuiltinTargetBp::ConcreteBunker => "Reinforced Concrete + Rebar Bunker",
        }
    }

    pub fn build(self) -> PixelBlueprint {
        match self {
            BuiltinTargetBp::Kontakt5EraHull => {
                let mut bp = PixelBlueprint::new_empty("Kontakt-5 ERA Hull", 128, 128);
                // 3 sloped Kontakt-5 ERA bricks on the front (Heavy HHA flyer + C4 + MildSteel)
                for k in 0..3 {
                    let y0 = 18 + k * 32;
                    let x0 = 22 + k * 6;
                    bp.paint_line((x0, y0), (x0 + 16, y0 + 26), 2, Some(MaterialId::HhaSteel), SymmetryMode::None);
                    bp.paint_line((x0 + 4, y0), (x0 + 20, y0 + 26), 2, Some(MaterialId::C4Explosive), SymmetryMode::None);
                    bp.paint_line((x0 + 8, y0), (x0 + 24, y0 + 26), 1, Some(MaterialId::MildSteel), SymmetryMode::None);
                }
                // Main sloped composite glacis: RHA + Textolite + SiC Ceramic + RHA + Kevlar Spall Liner
                bp.paint_line((44, 10), (72, 118), 3, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_line((51, 10), (79, 118), 4, Some(MaterialId::Textolite), SymmetryMode::None);
                bp.paint_line((59, 10), (87, 118), 3, Some(MaterialId::SiliconCarbide), SymmetryMode::None);
                bp.paint_line((66, 10), (94, 118), 4, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_line((73, 10), (101, 118), 2, Some(MaterialId::AramidLiner), SymmetryMode::None);
                bp
            }

            BuiltinTargetBp::ChobhamNeraCheek => {
                let mut bp = PixelBlueprint::new_empty("Chobham NERA Cheek", 136, 128);
                // Outer HHA strike plate
                bp.paint_line((20, 12), (38, 116), 3, Some(MaterialId::HhaSteel), SymmetryMode::None);
                // Two spaced NERA bulging sandwiches (HHA + NERA Rubber + MildSteel) with air cavities!
                for n in 0..2 {
                    let x_off = 36 + n * 20;
                    bp.paint_line((x_off, 14), (x_off + 18, 114), 1, Some(MaterialId::HhaSteel), SymmetryMode::None);
                    bp.paint_line((x_off + 3, 14), (x_off + 21, 114), 2, Some(MaterialId::RubberNera), SymmetryMode::None);
                    bp.paint_line((x_off + 6, 14), (x_off + 24, 114), 1, Some(MaterialId::MildSteel), SymmetryMode::None);
                }
                // Boron Carbide ceramic matrix + RHA backing + Kevlar spall liner
                bp.paint_line((80, 12), (98, 116), 5, Some(MaterialId::BoronCarbide), SymmetryMode::None);
                bp.paint_line((90, 12), (108, 116), 4, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_line((98, 12), (116, 116), 2, Some(MaterialId::AramidLiner), SymmetryMode::None);
                bp
            }

            BuiltinTargetBp::TankHullAmmoRack => {
                let mut bp = PixelBlueprint::new_empty("Tank Hull + Ammo Rack", 142, 120);
                // Sloped front RHA glacis + roof + floor + rear wall
                bp.paint_line((24, 58), (48, 16), 2, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_line((24, 58), (44, 104), 2, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_rect((48, 13), (126, 19), true, 1, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_rect((44, 101), (126, 108), true, 1, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_rect((120, 13), (128, 108), true, 1, Some(MaterialId::RhaSteel), SymmetryMode::None);

                // Internal Brass/Copper-cased Comp-B Ammo Rack + Percussion Primers inside fighting compartment!
                for row in 0..3 {
                    let ay = 44 + row * 15;
                    bp.paint_rect((76, ay), (104, ay + 9), true, 1, Some(MaterialId::CompB), SymmetryMode::None);
                    bp.paint_rect((75, ay - 1), (105, ay + 10), false, 1, Some(MaterialId::Copper), SymmetryMode::None);
                    bp.paint_rect((74, ay + 2), (76, ay + 7), true, 1, Some(MaterialId::MildSteel), SymmetryMode::None);
                }
                // Aluminum crew witness dummy plate in front of ammo rack
                bp.paint_rect((60, 34), (63, 86), true, 1, Some(MaterialId::Aluminum), SymmetryMode::None);
                bp
            }

            BuiltinTargetBp::ArmoredGlassBlock => {
                let mut bp = PixelBlueprint::new_empty("Armored Vision Block", 110, 120);
                // Alternating Armored Glass + thin Rubber/Polycarbonate interlayers + Kevlar frame
                for k in 0..4 {
                    let x = 26 + k * 14;
                    bp.paint_rect((x, 16), (x + 9, 104), true, 1, Some(MaterialId::ArmorGlass), SymmetryMode::None);
                    if k < 3 {
                        bp.paint_rect((x + 10, 16), (x + 13, 104), true, 1, Some(MaterialId::RubberNera), SymmetryMode::None);
                    }
                }
                bp.paint_rect((22, 11), (86, 15), true, 1, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp.paint_rect((22, 105), (86, 109), true, 1, Some(MaterialId::RhaSteel), SymmetryMode::None);
                bp
            }

            BuiltinTargetBp::ConcreteBunker => {
                let mut bp = PixelBlueprint::new_empty("Reinforced Concrete Bunker", 120, 128);
                bp.paint_rect((28, 10), (92, 118), true, 1, Some(MaterialId::Concrete), SymmetryMode::None);
                // Internal mild-steel rebar mesh!
                for rx in [38, 60, 82] {
                    bp.paint_line((rx, 14), (rx, 114), 1, Some(MaterialId::MildSteel), SymmetryMode::None);
                }
                for ry in [26, 48, 70, 92] {
                    bp.paint_line((32, ry), (88, ry), 1, Some(MaterialId::MildSteel), SymmetryMode::None);
                }
                bp
            }
        }
    }
}
