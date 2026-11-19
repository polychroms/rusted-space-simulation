use std::fs;
use std::path::PathBuf;

use glam::Vec2;

use crate::blueprint::{blueprint_storage_dir, PixelBlueprint};
use crate::materials::MaterialId;
use crate::physics::ContinuumWorld;
use crate::scenarios::{
    spawn_composite_plate, spawn_ellipse, spawn_hollow_ring, spawn_layered_disk,
    spawn_oriented_box, spawn_projectile_custom, spawn_triangle_wedge, ProjectileShape,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ArmorLayer {
    pub thickness: f32,
    pub mat: MaterialId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanetShell {
    pub thickness: f32,
    pub mat: MaterialId,
    pub temp_k: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TargetKind {
    SolidBox {
        width: f32,
        height: f32,
        mat: MaterialId,
    },
    Ellipse {
        rx: f32,
        ry: f32,
        mat: MaterialId,
    },
    HollowRing {
        outer_r: f32,
        wall_thick: f32,
        mat: MaterialId,
    },
    TriangleWedge {
        width: f32,
        height: f32,
        mat: MaterialId,
    },
    CompositePlate {
        height: f32,
        layers: Vec<ArmorLayer>,
    },
    LayeredPlanet {
        shells: Vec<PlanetShell>,
    },
    Blueprint {
        bp: PixelBlueprint,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargetSpec {
    pub name: String,
    pub pos: Vec2,
    pub angle_deg: f32,
    pub vel: Vec2, // cm/us (0.1 = 1 km/s)
    pub temp_k: f32,
    pub kind: TargetKind,
}

impl TargetSpec {
    pub fn default_box(pos: Vec2, mat: MaterialId) -> Self {
        Self {
            name: format!("{} Plate", mat.props().short_name),
            pos,
            angle_deg: 0.0,
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::SolidBox {
                width: 8.0,
                height: 48.0,
                mat,
            },
        }
    }

    pub fn default_ellipse(pos: Vec2, mat: MaterialId) -> Self {
        Self {
            name: format!("{} Ellipse", mat.props().short_name),
            pos,
            angle_deg: 0.0,
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::Ellipse {
                rx: 10.0,
                ry: 16.0,
                mat,
            },
        }
    }

    pub fn default_ring(pos: Vec2, mat: MaterialId) -> Self {
        Self {
            name: format!("{} Ring", mat.props().short_name),
            pos,
            angle_deg: 0.0,
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::HollowRing {
                outer_r: 14.0,
                wall_thick: 3.2,
                mat,
            },
        }
    }

    pub fn default_wedge(pos: Vec2, mat: MaterialId) -> Self {
        Self {
            name: format!("{} Wedge", mat.props().short_name),
            pos,
            angle_deg: 180.0, // face incoming right-moving projectiles with pointed nose!
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::TriangleWedge {
                width: 16.0,
                height: 36.0,
                mat,
            },
        }
    }

    pub fn default_composite(pos: Vec2) -> Self {
        Self {
            name: "Chobham Armor".to_string(),
            pos,
            angle_deg: -20.0,
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::CompositePlate {
                height: 50.0,
                layers: vec![
                    ArmorLayer {
                        thickness: 4.0,
                        mat: MaterialId::BoronCarbide,
                    },
                    ArmorLayer {
                        thickness: 4.8,
                        mat: MaterialId::RhaSteel,
                    },
                    ArmorLayer {
                        thickness: 2.2,
                        mat: MaterialId::AramidLiner,
                    },
                ],
            },
        }
    }

    pub fn default_planet(pos: Vec2, mantle_mat: MaterialId) -> Self {
        Self {
            name: "Proto-Planet".to_string(),
            pos,
            angle_deg: 0.0,
            vel: Vec2::ZERO,
            temp_k: 300.0,
            kind: TargetKind::LayeredPlanet {
                shells: vec![
                    PlanetShell {
                        thickness: 5.5,
                        mat: MaterialId::PlanetaryIron,
                        temp_k: 950.0,
                    },
                    PlanetShell {
                        thickness: 7.5,
                        mat: mantle_mat,
                        temp_k: 420.0,
                    },
                ],
            },
        }
    }

    pub fn from_blueprint(pos: Vec2, bp: PixelBlueprint) -> Self {
        Self {
            name: bp.name.clone(),
            pos,
            angle_deg: 0.0,
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::Blueprint { bp },
        }
    }

    pub fn speed_kms(&self) -> f32 {
        self.vel.length() * 10.0
    }

    pub fn vel_angle_deg(&self) -> f32 {
        if self.vel.length_squared() > 1e-10 {
            self.vel.y.atan2(self.vel.x).to_degrees()
        } else {
            0.0
        }
    }

    pub fn set_speed_and_angle(&mut self, speed_kms: f32, angle_deg: f32) {
        let v = speed_kms.max(0.0) * 0.1;
        self.vel = Vec2::from_angle(angle_deg.to_radians()) * v;
    }

    pub fn hit_test(&self, p: Vec2) -> bool {
        let d = p - self.pos;
        match &self.kind {
            TargetKind::SolidBox { width, height, .. }
            | TargetKind::TriangleWedge { width, height, .. } => {
                let local = Vec2::from_angle(-self.angle_deg.to_radians()).rotate(d);
                local.x.abs() <= 0.5 * width + 1.5 && local.y.abs() <= 0.5 * height + 1.5
            }
            TargetKind::Ellipse { rx, ry, .. } => {
                let local = Vec2::from_angle(-self.angle_deg.to_radians()).rotate(d);
                (local.x / (rx + 1.5)).powi(2) + (local.y / (ry + 1.5)).powi(2) <= 1.0
            }
            TargetKind::HollowRing { outer_r, .. } => d.length() <= outer_r + 1.5,
            TargetKind::CompositePlate { height, layers } => {
                let w: f32 = layers.iter().map(|l| l.thickness).sum();
                let local = Vec2::from_angle(-self.angle_deg.to_radians()).rotate(d);
                local.x.abs() <= 0.5 * w + 1.5 && local.y.abs() <= 0.5 * height + 1.5
            }
            TargetKind::LayeredPlanet { shells } => {
                let r: f32 = shells.iter().map(|s| s.thickness).sum();
                d.length() <= r + 1.5
            }
            TargetKind::Blueprint { bp } => {
                let he = bp.world_half_extents(0.35);
                let local = Vec2::from_angle(-self.angle_deg.to_radians()).rotate(d);
                local.x.abs() <= he.x + 1.5 && local.y.abs() <= he.y + 1.5
            }
        }
    }

    pub fn spawn_into(&self, world: &mut ContinuumWorld) {
        match &self.kind {
            TargetKind::SolidBox { width, height, mat } => {
                spawn_oriented_box(
                    world,
                    self.pos,
                    Vec2::new(0.5 * *width, 0.5 * *height),
                    self.angle_deg.to_radians(),
                    self.vel,
                    *mat,
                    None,
                    Some(self.temp_k),
                );
            }
            TargetKind::Ellipse { rx, ry, mat } => {
                spawn_ellipse(
                    world,
                    self.pos,
                    *rx,
                    *ry,
                    self.angle_deg.to_radians(),
                    self.vel,
                    *mat,
                    Some(self.temp_k),
                );
            }
            TargetKind::HollowRing {
                outer_r,
                wall_thick,
                mat,
            } => {
                spawn_hollow_ring(
                    world,
                    self.pos,
                    *outer_r,
                    *wall_thick,
                    self.vel,
                    *mat,
                    Some(self.temp_k),
                );
            }
            TargetKind::TriangleWedge { width, height, mat } => {
                spawn_triangle_wedge(
                    world,
                    self.pos,
                    *width,
                    *height,
                    self.angle_deg.to_radians(),
                    self.vel,
                    *mat,
                    Some(self.temp_k),
                );
            }
            TargetKind::CompositePlate { height, layers } => {
                if layers.is_empty() {
                    return;
                }
                let layer_pairs: Vec<(f32, MaterialId)> =
                    layers.iter().map(|l| (l.thickness, l.mat)).collect();
                spawn_composite_plate(
                    world,
                    self.pos,
                    *height,
                    self.angle_deg.to_radians(),
                    self.vel,
                    &layer_pairs,
                    Some(self.temp_k),
                );
            }
            TargetKind::LayeredPlanet { shells } => {
                if shells.is_empty() {
                    return;
                }
                let mut acc_r = 0.0f32;
                let cumulative: Vec<(f32, MaterialId, Option<f32>)> = shells
                    .iter()
                    .map(|s| {
                        acc_r += s.thickness.max(0.5);
                        (acc_r, s.mat, Some(s.temp_k))
                    })
                    .collect();
                spawn_layered_disk(world, self.pos, &cumulative, self.vel);
            }
            TargetKind::Blueprint { bp } => {
                bp.spawn_into(
                    world,
                    self.pos,
                    self.angle_deg.to_radians(),
                    self.vel,
                    Some(self.temp_k),
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectileSpec {
    pub name: String,
    pub shape: ProjectileShape,
    pub pos: Vec2,
    pub speed_kms: f32,
    pub angle_deg: f32,
    pub mat: MaterialId,
    pub core_mat: MaterialId,
    pub length_cm: f32,
    pub radius_cm: f32,
    pub core_frac: f32,
    pub temp_k: f32,
    pub blueprint: Option<PixelBlueprint>,
}

impl ProjectileSpec {
    pub fn default_with_shape(shape: ProjectileShape, pos: Vec2, mat: MaterialId) -> Self {
        let (length_cm, radius_cm, speed_kms, temp_k) = match shape {
            ProjectileShape::LongRodDart => (23.0, 1.45, 2.2, 293.0),
            ProjectileShape::Sphere => (
                7.2,
                3.6,
                5.0,
                if mat == MaterialId::WaterIce {
                    150.0
                } else {
                    293.0
                },
            ),
            ProjectileShape::Ellipse => (12.0, 3.0, 4.5, 293.0),
            ProjectileShape::HollowRing => (9.0, 4.5, 3.8, 293.0),
            ProjectileShape::DifferentiatedAsteroid => (11.0, 5.5, 5.5, 280.0),
            ProjectileShape::BluntSlug => (9.6, 2.8, 1.15, 293.0),
            ProjectileShape::Wedge => (11.0, 3.4, 3.2, 293.0),
            ProjectileShape::ShapedJet => (26.0, 1.1, 6.4, 760.0),
            ProjectileShape::CustomBlueprint => (24.0, 4.5, 1.2, 293.0),
        };
        Self {
            name: format!("{} {}", mat.props().short_name, shape.name()),
            shape,
            pos,
            speed_kms,
            angle_deg: 0.0,
            mat,
            core_mat: MaterialId::PlanetaryIron,
            length_cm,
            radius_cm,
            core_frac: 0.45,
            temp_k,
            blueprint: None,
        }
    }

    pub fn from_blueprint(pos: Vec2, speed_kms: f32, angle_deg: f32, bp: PixelBlueprint) -> Self {
        let he = bp.world_half_extents(0.35);
        Self {
            name: bp.name.clone(),
            shape: ProjectileShape::CustomBlueprint,
            pos,
            speed_kms,
            angle_deg,
            mat: MaterialId::RhaSteel,
            core_mat: MaterialId::CompB,
            length_cm: (2.0 * he.x).max(2.0),
            radius_cm: he.y.max(1.0),
            core_frac: 0.45,
            temp_k: 293.0,
            blueprint: Some(bp),
        }
    }

    pub fn vel_cm_per_us(&self) -> Vec2 {
        let v = self.speed_kms.max(0.0) * 0.1;
        Vec2::from_angle(self.angle_deg.to_radians()) * v
    }

    pub fn set_vel_cm_per_us(&mut self, vel: Vec2) {
        self.speed_kms = (vel.length() * 10.0).clamp(0.0, 100.0);
        if vel.length_squared() > 1e-10 {
            self.angle_deg = vel.y.atan2(vel.x).to_degrees();
        }
    }

    pub fn hit_test(&self, p: Vec2) -> bool {
        let d = p - self.pos;
        match self.shape {
            ProjectileShape::Sphere
            | ProjectileShape::HollowRing
            | ProjectileShape::DifferentiatedAsteroid => d.length() <= self.radius_cm + 1.5,
            ProjectileShape::CustomBlueprint => {
                let he = self
                    .blueprint
                    .as_ref()
                    .map(|b| b.world_half_extents(0.35))
                    .unwrap_or(Vec2::new(0.5 * self.length_cm, self.radius_cm));
                let local = Vec2::from_angle(-self.angle_deg.to_radians()).rotate(d);
                local.x.abs() <= he.x + 1.5 && local.y.abs() <= he.y + 1.8
            }
            ProjectileShape::LongRodDart
            | ProjectileShape::BluntSlug
            | ProjectileShape::Ellipse
            | ProjectileShape::Wedge
            | ProjectileShape::ShapedJet => {
                let local = Vec2::from_angle(-self.angle_deg.to_radians()).rotate(d);
                local.x.abs() <= 0.5 * self.length_cm + 1.5
                    && local.y.abs() <= self.radius_cm + 2.0
            }
        }
    }

    pub fn spawn_into(&self, world: &mut ContinuumWorld) {
        let vel = self.vel_cm_per_us();
        if self.shape == ProjectileShape::CustomBlueprint {
            if let Some(bp) = &self.blueprint {
                bp.spawn_into(
                    world,
                    self.pos,
                    self.angle_deg.to_radians(),
                    vel,
                    Some(self.temp_k),
                );
                return;
            }
        }
        let dir = Vec2::from_angle(self.angle_deg.to_radians());
        spawn_projectile_custom(
            world,
            self.shape,
            self.pos,
            dir,
            vel,
            self.mat,
            self.core_mat,
            self.length_cm,
            self.radius_cm,
            self.core_frac,
            self.temp_k,
        );
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneSpec {
    pub targets: Vec<TargetSpec>,
    pub projectiles: Vec<ProjectileSpec>,
    pub self_gravity: bool,
    pub grav_g: f32,
}

impl SceneSpec {
    pub fn empty() -> Self {
        Self {
            targets: Vec::new(),
            projectiles: Vec::new(),
            self_gravity: false,
            grav_g: 7.0e-5,
        }
    }

    pub fn build_world(&self, dx: f32) -> ContinuumWorld {
        let mut world = ContinuumWorld::new(dx);
        world.self_gravity = self.self_gravity;
        world.grav_g = if self.self_gravity && self.grav_g <= 0.0 {
            7.0e-5
        } else {
            self.grav_g
        };

        for target in &self.targets {
            target.spawn_into(&mut world);
        }
        for proj in &self.projectiles {
            proj.spawn_into(&mut world);
        }
        world.equilibrate_self_gravity();

        world.record_telemetry();
        world
    }

    /// Deterministic ASCII `.rscene` serialization of all targets, projectiles, blueprints, and gravity!
    pub fn serialize_text(&self, scene_name: &str) -> String {
        let clean_name = scene_name.replace(['\n', '\r'], " ");
        let mut out = format!(
            "RSSIM_SCENE_V1\nname={}\nself_gravity={}\ngrav_g={:.8}\n",
            clean_name, self.self_gravity, self.grav_g
        );

        for t in &self.targets {
            let t_name = t.name.replace(['\n', '\r', '|'], " ");
            out.push_str(&format!(
                "TARGET|{}|{:.5}|{:.5}|{:.5}|{:.6}|{:.6}|{:.3}\n",
                t_name, t.pos.x, t.pos.y, t.angle_deg, t.vel.x, t.vel.y, t.temp_k
            ));
            match &t.kind {
                TargetKind::SolidBox { width, height, mat } => {
                    out.push_str(&format!(
                        "KIND_BOX|{:.5}|{:.5}|{}\n",
                        width,
                        height,
                        mat.props().code
                    ));
                }
                TargetKind::Ellipse { rx, ry, mat } => {
                    out.push_str(&format!(
                        "KIND_ELLIPSE|{:.5}|{:.5}|{}\n",
                        rx,
                        ry,
                        mat.props().code
                    ));
                }
                TargetKind::HollowRing {
                    outer_r,
                    wall_thick,
                    mat,
                } => {
                    out.push_str(&format!(
                        "KIND_RING|{:.5}|{:.5}|{}\n",
                        outer_r,
                        wall_thick,
                        mat.props().code
                    ));
                }
                TargetKind::TriangleWedge { width, height, mat } => {
                    out.push_str(&format!(
                        "KIND_WEDGE|{:.5}|{:.5}|{}\n",
                        width,
                        height,
                        mat.props().code
                    ));
                }
                TargetKind::CompositePlate { height, layers } => {
                    let parts: Vec<String> = layers
                        .iter()
                        .map(|l| format!("{:.5}:{}", l.thickness, l.mat.props().code))
                        .collect();
                    out.push_str(&format!("KIND_COMP|{:.5}|{}\n", height, parts.join(",")));
                }
                TargetKind::LayeredPlanet { shells } => {
                    let parts: Vec<String> = shells
                        .iter()
                        .map(|s| format!("{:.5}:{}:{:.2}", s.thickness, s.mat.props().code, s.temp_k))
                        .collect();
                    out.push_str(&format!("KIND_PLANET|{}\n", parts.join(",")));
                }
                TargetKind::Blueprint { bp } => {
                    let bp_txt = bp.serialize_text();
                    let n_lines = bp_txt.lines().count();
                    out.push_str(&format!("KIND_BP|{}\n{}", n_lines, bp_txt));
                    if !bp_txt.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
        }

        for p in &self.projectiles {
            let p_name = p.name.replace(['\n', '\r', '|'], " ");
            out.push_str(&format!(
                "PROJ|{}|{}|{:.5}|{:.5}|{:.5}|{:.5}|{}|{}|{:.5}|{:.5}|{:.5}|{:.3}\n",
                p_name,
                p.shape.code(),
                p.pos.x,
                p.pos.y,
                p.speed_kms,
                p.angle_deg,
                p.mat.props().code,
                p.core_mat.props().code,
                p.length_cm,
                p.radius_cm,
                p.core_frac,
                p.temp_k
            ));
            if let Some(bp) = &p.blueprint {
                let bp_txt = bp.serialize_text();
                let n_lines = bp_txt.lines().count();
                out.push_str(&format!("PROJ_BP|{}\n{}", n_lines, bp_txt));
                if !bp_txt.ends_with('\n') {
                    out.push('\n');
                }
            } else {
                out.push_str("PROJ_NOBP\n");
            }
        }

        out
    }

    pub fn deserialize_text(text: &str) -> Option<(String, Self)> {
        let mut lines = text.lines().peekable();
        if lines.next()?.trim() != "RSSIM_SCENE_V1" {
            return None;
        }
        let name = lines.next()?.strip_prefix("name=")?.to_string();
        let self_gravity: bool = lines.next()?.strip_prefix("self_gravity=")?.parse().ok()?;
        let grav_g: f32 = lines.next()?.strip_prefix("grav_g=")?.parse().ok()?;

        let mut scene = SceneSpec {
            targets: Vec::new(),
            projectiles: Vec::new(),
            self_gravity,
            grav_g,
        };

        while let Some(line) = lines.next() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("TARGET|") {
                let fields: Vec<&str> = rest.split('|').collect();
                if fields.len() < 7 {
                    return None;
                }
                let t_name = fields[0].to_string();
                let px: f32 = fields[1].parse().ok()?;
                let py: f32 = fields[2].parse().ok()?;
                let ang: f32 = fields[3].parse().ok()?;
                let vx: f32 = fields[4].parse().ok()?;
                let vy: f32 = fields[5].parse().ok()?;
                let temp_k: f32 = fields[6].parse().ok()?;

                let kind_line = lines.next()?.trim();
                let kind = if let Some(kr) = kind_line.strip_prefix("KIND_BOX|") {
                    let kf: Vec<&str> = kr.split('|').collect();
                    if kf.len() < 3 {
                        return None;
                    }
                    TargetKind::SolidBox {
                        width: kf[0].parse().ok()?,
                        height: kf[1].parse().ok()?,
                        mat: MaterialId::from_code(kf[2])?,
                    }
                } else if let Some(kr) = kind_line.strip_prefix("KIND_ELLIPSE|") {
                    let kf: Vec<&str> = kr.split('|').collect();
                    if kf.len() < 3 {
                        return None;
                    }
                    TargetKind::Ellipse {
                        rx: kf[0].parse().ok()?,
                        ry: kf[1].parse().ok()?,
                        mat: MaterialId::from_code(kf[2])?,
                    }
                } else if let Some(kr) = kind_line.strip_prefix("KIND_RING|") {
                    let kf: Vec<&str> = kr.split('|').collect();
                    if kf.len() < 3 {
                        return None;
                    }
                    TargetKind::HollowRing {
                        outer_r: kf[0].parse().ok()?,
                        wall_thick: kf[1].parse().ok()?,
                        mat: MaterialId::from_code(kf[2])?,
                    }
                } else if let Some(kr) = kind_line.strip_prefix("KIND_WEDGE|") {
                    let kf: Vec<&str> = kr.split('|').collect();
                    if kf.len() < 3 {
                        return None;
                    }
                    TargetKind::TriangleWedge {
                        width: kf[0].parse().ok()?,
                        height: kf[1].parse().ok()?,
                        mat: MaterialId::from_code(kf[2])?,
                    }
                } else if let Some(kr) = kind_line.strip_prefix("KIND_COMP|") {
                    let (h_s, l_s) = kr.split_once('|')?;
                    let height: f32 = h_s.parse().ok()?;
                    let mut layers = Vec::new();
                    if !l_s.is_empty() {
                        for item in l_s.split(',') {
                            let (th_s, m_s) = item.split_once(':')?;
                            layers.push(ArmorLayer {
                                thickness: th_s.parse().ok()?,
                                mat: MaterialId::from_code(m_s)?,
                            });
                        }
                    }
                    TargetKind::CompositePlate { height, layers }
                } else if let Some(kr) = kind_line.strip_prefix("KIND_PLANET|") {
                    let mut shells = Vec::new();
                    if !kr.is_empty() {
                        for item in kr.split(',') {
                            let sp: Vec<&str> = item.split(':').collect();
                            if sp.len() < 3 {
                                return None;
                            }
                            shells.push(PlanetShell {
                                thickness: sp[0].parse().ok()?,
                                mat: MaterialId::from_code(sp[1])?,
                                temp_k: sp[2].parse().ok()?,
                            });
                        }
                    }
                    TargetKind::LayeredPlanet { shells }
                } else {
                    let kr = kind_line.strip_prefix("KIND_BP|")?;
                    let n_lines: usize = kr.parse().ok()?;
                    let mut bp_buf = String::new();
                    for _ in 0..n_lines {
                        bp_buf.push_str(lines.next()?);
                        bp_buf.push('\n');
                    }
                    TargetKind::Blueprint {
                        bp: PixelBlueprint::deserialize_text(&bp_buf)?,
                    }
                };

                scene.targets.push(TargetSpec {
                    name: t_name,
                    pos: Vec2::new(px, py),
                    angle_deg: ang,
                    vel: Vec2::new(vx, vy),
                    temp_k,
                    kind,
                });
            } else if let Some(rest) = line.strip_prefix("PROJ|") {
                let pf: Vec<&str> = rest.split('|').collect();
                if pf.len() < 12 {
                    return None;
                }
                let p_name = pf[0].to_string();
                let shape = ProjectileShape::from_code(pf[1])?;
                let px: f32 = pf[2].parse().ok()?;
                let py: f32 = pf[3].parse().ok()?;
                let speed_kms: f32 = pf[4].parse().ok()?;
                let angle_deg: f32 = pf[5].parse().ok()?;
                let mat = MaterialId::from_code(pf[6])?;
                let core_mat = MaterialId::from_code(pf[7])?;
                let length_cm: f32 = pf[8].parse().ok()?;
                let radius_cm: f32 = pf[9].parse().ok()?;
                let core_frac: f32 = pf[10].parse().ok()?;
                let temp_k: f32 = pf[11].parse().ok()?;

                let bp_header = lines.next()?.trim();
                let blueprint = if let Some(kr) = bp_header.strip_prefix("PROJ_BP|") {
                    let n_lines: usize = kr.parse().ok()?;
                    let mut bp_buf = String::new();
                    for _ in 0..n_lines {
                        bp_buf.push_str(lines.next()?);
                        bp_buf.push('\n');
                    }
                    Some(PixelBlueprint::deserialize_text(&bp_buf)?)
                } else {
                    None
                };

                scene.projectiles.push(ProjectileSpec {
                    name: p_name,
                    shape,
                    pos: Vec2::new(px, py),
                    speed_kms,
                    angle_deg,
                    mat,
                    core_mat,
                    length_cm,
                    radius_cm,
                    core_frac,
                    temp_k,
                    blueprint,
                });
            }
        }

        Some((name, scene))
    }
}

pub fn save_scene_to_disk(name: &str, scene: &SceneSpec) -> Result<PathBuf, String> {
    let dir = blueprint_storage_dir("scenes");
    let safe_name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let file_name = if safe_name.trim_matches('_').is_empty() {
        "custom_scene.rscene".to_string()
    } else {
        format!("{}.rscene", safe_name)
    };
    let path = dir.join(file_name);
    fs::write(&path, scene.serialize_text(name)).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn list_saved_scenes() -> Vec<(String, SceneSpec)> {
    let dir = blueprint_storage_dir("scenes");
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "rscene") {
                if let Ok(txt) = fs::read_to_string(entry.path()) {
                    if let Some(pair) = SceneSpec::deserialize_text(&txt) {
                        out.push(pair);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneSelection {
    Target(usize),
    Projectile(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_editing_builds_with_edited_positions_and_velocities() {
        let mut scene = SceneSpec::empty();
        let mut target = TargetSpec::default_composite(Vec2::new(20.0, 5.0));
        target.angle_deg = -15.0;
        scene.targets.push(target);

        let mut proj = ProjectileSpec::default_with_shape(
            ProjectileShape::LongRodDart,
            Vec2::new(-10.0, 5.0),
            MaterialId::Tungsten,
        );
        proj.speed_kms = 4.2;
        proj.angle_deg = 0.0;
        scene.projectiles.push(proj);

        let world = scene.build_world(0.4);
        assert!(world.len() > 200);
        // Verify projectile particles received the exact edited velocity (4.2 km/s = 0.42 cm/us)
        let max_vx = world.vel.iter().map(|v| v.x).fold(0.0f32, f32::max);
        assert!((max_vx - 0.42).abs() < 1e-4);
    }

    #[test]
    fn custom_geometric_shapes_build_cleanly() {
        let mut scene = SceneSpec::empty();
        scene.targets.push(TargetSpec::default_ellipse(Vec2::new(0.0, 0.0), MaterialId::RhaSteel));
        scene.targets.push(TargetSpec::default_ring(Vec2::new(30.0, 0.0), MaterialId::Aluminum));
        scene.targets.push(TargetSpec::default_wedge(Vec2::new(60.0, 0.0), MaterialId::BasaltRock));
        for shape in [
            ProjectileShape::Ellipse,
            ProjectileShape::HollowRing,
            ProjectileShape::Wedge,
        ] {
            scene.projectiles.push(ProjectileSpec::default_with_shape(
                shape,
                Vec2::new(-25.0, 0.0),
                MaterialId::Tungsten,
            ));
        }
        let world = scene.build_world(0.45);
        assert!(world.len() > 1000);
        assert!(world.next_body_id >= 6);
    }

    #[test]
    fn scene_serialize_roundtrip_preserves_blueprints_and_specs() {
        let mut scene = SceneSpec::empty();
        scene.self_gravity = true;
        scene.grav_g = 3.2e-5;
        scene.targets.push(TargetSpec::default_composite(Vec2::new(14.0, -2.5)));
        scene.targets.push(TargetSpec::from_blueprint(
            Vec2::new(28.0, 0.0),
            crate::blueprint::BuiltinTargetBp::Kontakt5EraHull.build(),
        ));
        scene.projectiles.push(ProjectileSpec::from_blueprint(
            Vec2::new(-18.0, 1.0),
            0.32,
            -4.5,
            crate::blueprint::BuiltinProjectileBp::HeatWarhead.build(),
        ));

        let txt = scene.serialize_text("ERA Range Test");
        let (loaded_name, loaded_scene) =
            SceneSpec::deserialize_text(&txt).expect("valid .rscene deserialize");
        assert_eq!(loaded_name, "ERA Range Test");
        assert_eq!(loaded_scene.targets.len(), 2);
        assert_eq!(loaded_scene.projectiles.len(), 1);
        assert!((loaded_scene.projectiles[0].speed_kms - 0.32).abs() < 1e-4);
        assert!(loaded_scene.self_gravity);
    }
}
