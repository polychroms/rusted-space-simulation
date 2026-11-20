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

    pub fn bounding_radius(&self) -> f32 {
        match &self.kind {
            TargetKind::SolidBox { width, height, .. }
            | TargetKind::TriangleWedge { width, height, .. } => {
                0.5 * (width * width + height * height).sqrt()
            }
            TargetKind::Ellipse { rx, ry, .. } => rx.max(*ry),
            TargetKind::HollowRing { outer_r, .. } => *outer_r,
            TargetKind::CompositePlate { height, layers } => {
                let w: f32 = layers.iter().map(|l| l.thickness).sum();
                0.5 * (w * w + height * height).sqrt()
            }
            TargetKind::LayeredPlanet { shells } => {
                shells.iter().map(|s| s.thickness).sum::<f32>().max(1.0)
            }
            TargetKind::Blueprint { bp } => bp.world_half_extents(0.35).length().max(1.0),
        }
    }

    pub fn has_gravity_material(&self) -> bool {
        match &self.kind {
            TargetKind::SolidBox { mat, .. }
            | TargetKind::TriangleWedge { mat, .. }
            | TargetKind::Ellipse { mat, .. }
            | TargetKind::HollowRing { mat, .. } => mat.is_gravity_material(),
            TargetKind::CompositePlate { layers, .. } => {
                layers.iter().any(|l| l.mat.is_gravity_material())
            }
            TargetKind::LayeredPlanet { shells } => {
                shells.iter().any(|s| s.mat.is_gravity_material())
            }
            TargetKind::Blueprint { bp } => bp.cells.iter().flatten().any(|m| m.is_gravity_material()),
        }
    }

    pub fn estimate_mass(&self) -> f32 {
        match &self.kind {
            TargetKind::SolidBox { width, height, mat } => {
                width * height * mat.props().rho0
            }
            TargetKind::TriangleWedge { width, height, mat } => {
                0.5 * width * height * mat.props().rho0
            }
            TargetKind::Ellipse { rx, ry, mat } => {
                std::f32::consts::PI * rx * ry * mat.props().rho0
            }
            TargetKind::HollowRing {
                outer_r,
                wall_thick,
                mat,
            } => {
                let inner = (outer_r - wall_thick).max(0.0);
                std::f32::consts::PI * (outer_r * outer_r - inner * inner) * mat.props().rho0
            }
            TargetKind::CompositePlate { height, layers } => layers
                .iter()
                .map(|l| l.thickness * height * l.mat.props().rho0)
                .sum(),
            TargetKind::LayeredPlanet { shells } => {
                let mut r_prev = 0.0f32;
                let mut m = 0.0f32;
                for s in shells {
                    let r_next = r_prev + s.thickness.max(0.5);
                    m += std::f32::consts::PI
                        * (r_next * r_next - r_prev * r_prev)
                        * s.mat.props().rho0;
                    r_prev = r_next;
                }
                m
            }
            TargetKind::Blueprint { bp } => {
                let cell_area = 0.35f32 * 0.35f32;
                let mut m = 0.0f32;
                for mat in bp.cells.iter().flatten() {
                    m += cell_area * mat.props().rho0;
                }
                m.max(1.0)
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

    pub fn bounding_radius(&self) -> f32 {
        match self.shape {
            ProjectileShape::Sphere
            | ProjectileShape::HollowRing
            | ProjectileShape::DifferentiatedAsteroid => self.radius_cm.max(0.8),
            ProjectileShape::CustomBlueprint => self
                .blueprint
                .as_ref()
                .map(|b| b.world_half_extents(0.35).length())
                .unwrap_or_else(|| (0.5 * self.length_cm).hypot(self.radius_cm))
                .max(0.8),
            _ => (0.5 * self.length_cm).hypot(self.radius_cm).max(0.8),
        }
    }

    pub fn has_gravity_material(&self) -> bool {
        self.mat.is_gravity_material()
            || (self.shape == ProjectileShape::DifferentiatedAsteroid
                && self.core_mat.is_gravity_material())
            || (self.shape == ProjectileShape::CustomBlueprint
                && self
                    .blueprint
                    .as_ref()
                    .is_some_and(|bp| bp.cells.iter().flatten().any(|m| m.is_gravity_material())))
    }

    pub fn estimate_mass(&self) -> f32 {
        let r = self.radius_cm.max(0.4);
        match self.shape {
            ProjectileShape::Sphere => std::f32::consts::PI * r * r * self.mat.props().rho0,
            ProjectileShape::HollowRing => {
                let inner = (r * (1.0 - self.core_frac.clamp(0.1, 0.9))).max(0.0);
                std::f32::consts::PI * (r * r - inner * inner) * self.mat.props().rho0
            }
            ProjectileShape::DifferentiatedAsteroid => {
                let rc = r * self.core_frac.clamp(0.1, 0.9);
                std::f32::consts::PI
                    * (rc * rc * self.core_mat.props().rho0
                        + (r * r - rc * rc) * self.mat.props().rho0)
            }
            ProjectileShape::Ellipse => {
                std::f32::consts::PI * 0.5 * self.length_cm * r * self.mat.props().rho0
            }
            ProjectileShape::Wedge => {
                0.5 * self.length_cm * 2.0 * r * self.mat.props().rho0
            }
            ProjectileShape::LongRodDart => {
                0.90 * self.length_cm * 2.0 * r * self.mat.props().rho0
            }
            ProjectileShape::CustomBlueprint => {
                if let Some(bp) = &self.blueprint {
                    let cell_area = 0.35f32 * 0.35f32;
                    let mut m = 0.0f32;
                    for mat in bp.cells.iter().flatten() {
                        m += cell_area * mat.props().rho0;
                    }
                    m.max(1.0)
                } else {
                    self.length_cm * 2.0 * r * self.mat.props().rho0
                }
            }
            _ => self.length_cm * 2.0 * r * self.mat.props().rho0,
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

#[derive(Clone, Debug)]
pub struct TrajectoryTrace {
    pub is_projectile: bool,
    pub points: Vec<Vec2>,
    pub impact: Option<Vec2>,
    pub impact_body_pos: Option<Vec2>,
}

#[derive(Clone)]
enum BodyShapeGeom {
    Circle {
        radius: f32,
    },
    Annulus {
        inner_r: f32,
        outer_r: f32,
    },
    Rect {
        hx: f32,
        hy: f32,
        rot: Vec2,
    },
    Ellipse {
        rx: f32,
        ry: f32,
        rot: Vec2,
    },
    Polygon {
        verts_rot: Vec<Vec2>,
    },
    Blueprint {
        bp: PixelBlueprint,
        dx: f32,
        rot: Vec2,
        mid_gx: f32,
        mid_gy: f32,
    },
}

impl BodyShapeGeom {
    fn inv_rotate(rot: Vec2, v: Vec2) -> Vec2 {
        Vec2::new(v.x * rot.x + v.y * rot.y, -v.x * rot.y + v.y * rot.x)
    }

    /// Returns `(is_inside, closest_world_boundary_point)` relative to body center at `body_pos`.
    fn closest_boundary_world(&self, body_pos: Vec2, world_pt: Vec2) -> (bool, Vec2) {
        let d = world_pt - body_pos;
        match self {
            BodyShapeGeom::Circle { radius } => {
                let r = *radius;
                let len = d.length();
                let inside = len <= r;
                let dir = if len > 1e-6 { d / len } else { Vec2::X };
                (inside, body_pos + dir * r)
            }
            BodyShapeGeom::Annulus { inner_r, outer_r } => {
                let len = d.length();
                let dir = if len > 1e-6 { d / len } else { Vec2::X };
                let inside = len >= *inner_r && len <= *outer_r;
                let r_surf = if inside {
                    if (len - *inner_r) < (*outer_r - len) {
                        *inner_r
                    } else {
                        *outer_r
                    }
                } else {
                    len.clamp(*inner_r, *outer_r)
                };
                (inside, body_pos + dir * r_surf)
            }
            BodyShapeGeom::Rect { hx, hy, rot } => {
                let lp = Self::inv_rotate(*rot, d);
                let inside = lp.x.abs() <= *hx && lp.y.abs() <= *hy;
                let q = if inside {
                    let dx_face = *hx - lp.x.abs();
                    let dy_face = *hy - lp.y.abs();
                    if dx_face < dy_face {
                        Vec2::new(if lp.x >= 0.0 { *hx } else { -*hx }, lp.y)
                    } else {
                        Vec2::new(lp.x, if lp.y >= 0.0 { *hy } else { -*hy })
                    }
                } else {
                    Vec2::new(lp.x.clamp(-*hx, *hx), lp.y.clamp(-*hy, *hy))
                };
                (inside, body_pos + rot.rotate(q))
            }
            BodyShapeGeom::Ellipse { rx, ry, rot } => {
                let lp = Self::inv_rotate(*rot, d);
                let u2 = (lp.x / *rx).powi(2) + (lp.y / *ry).powi(2);
                let inside = u2 <= 1.0;
                let px = lp.x.abs();
                let py = lp.y.abs();
                let mut t = (ry * py).atan2(rx * px).clamp(0.0, std::f32::consts::FRAC_PI_2);
                let e2 = rx * rx - ry * ry;
                for _ in 0..6 {
                    let (st, ct) = t.sin_cos();
                    let f = e2 * ct * st - px * rx * st + py * ry * ct;
                    let df = e2 * (ct * ct - st * st) - px * rx * ct - py * ry * st;
                    if df.abs() < 1e-8 {
                        break;
                    }
                    t = (t - f / df).clamp(0.0, std::f32::consts::FRAC_PI_2);
                }
                let (st, ct) = t.sin_cos();
                let q = Vec2::new(
                    if lp.x >= 0.0 { rx * ct } else { -rx * ct },
                    if lp.y >= 0.0 { ry * st } else { -ry * st },
                );
                (inside, body_pos + rot.rotate(q))
            }
            BodyShapeGeom::Polygon { verts_rot } => {
                let (inside, closest_rel) = closest_point_and_inside_poly(verts_rot, d);
                (inside, body_pos + closest_rel)
            }
            BodyShapeGeom::Blueprint {
                bp,
                dx,
                rot,
                mid_gx,
                mid_gy,
                ..
            } => {
                let lp = Self::inv_rotate(*rot, d);
                let dy = *dx * 0.866_025_4;
                let gx_f = lp.x / *dx + *mid_gx;
                let gy_f = *mid_gy - lp.y / dy;
                let cx = gx_f.round() as i32;
                let cy = gy_f.round() as i32;
                let inside = bp.get(cx, cy).is_some();
                let mut best_d2 = f32::INFINITY;
                let mut best_local = lp;
                let scan = (bp.width.max(bp.height) as i32).clamp(4, 12);
                for yy in (cy - scan)..=(cy + scan) {
                    if yy < 0 || (yy as usize) >= bp.height {
                        continue;
                    }
                    let row_off = if (yy as usize) % 2 == 1 { 0.5 * *dx } else { 0.0 };
                    let ly = (*mid_gy - yy as f32) * dy;
                    for xx in (cx - scan)..=(cx + scan) {
                        if bp.get(xx, yy).is_none() {
                            continue;
                        }
                        let lx = (xx as f32 - *mid_gx) * *dx + row_off;
                        let cell_p = Vec2::new(lx, ly);
                        let d2 = (cell_p - lp).length_squared();
                        if d2 < best_d2 {
                            best_d2 = d2;
                            best_local = cell_p;
                        }
                    }
                }
                if !best_d2.is_finite() {
                    best_local = Vec2::ZERO;
                }
                (inside, body_pos + rot.rotate(best_local))
            }
        }
    }
}

fn closest_point_and_inside_poly(verts: &[Vec2], p: Vec2) -> (bool, Vec2) {
    let n = verts.len();
    if n < 3 {
        return (false, p);
    }
    let mut inside = false;
    let mut best_d2 = f32::INFINITY;
    let mut best_q = verts[0];
    let mut j = n - 1;
    for i in 0..n {
        let vi = verts[i];
        let vj = verts[j];
        if ((vi.y > p.y) != (vj.y > p.y))
            && (p.x < (vj.x - vi.x) * (p.y - vi.y) / (vj.y - vi.y + 1e-12) + vi.x)
        {
            inside = !inside;
        }
        let ab = vi - vj;
        let len2 = ab.length_squared().max(1e-12);
        let t = ((p - vj).dot(ab) / len2).clamp(0.0, 1.0);
        let q = vj + ab * t;
        let d2 = (p - q).length_squared();
        if d2 < best_d2 {
            best_d2 = d2;
            best_q = q;
        }
        j = i;
    }
    (inside, best_q)
}

fn segments_intersect(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> Option<Vec2> {
    let r = a1 - a0;
    let s = b1 - b0;
    let denom = r.perp_dot(s);
    if denom.abs() < 1e-8 {
        return None;
    }
    let qp = b0 - a0;
    let t = qp.perp_dot(s) / denom;
    let u = qp.perp_dot(r) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a0 + r * t)
    } else {
        None
    }
}

fn sample_poly_perimeter_rot(verts: &[Vec2], max_seg_step: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    let n = verts.len();
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let seg_len = (b - a).length();
        let k = ((seg_len / max_seg_step).ceil() as usize).max(1);
        for s in 0..k {
            out.push(a.lerp(b, s as f32 / k as f32));
        }
    }
    out
}

fn sample_rect_mass_nodes(
    hx: f32,
    hy: f32,
    total_mass: f32,
    grav_q: f32,
    rot: Vec2,
) -> Vec<(Vec2, f32, f32)> {
    let nx = 4usize;
    let ny = 4usize;
    let dm = total_mass / (nx * ny) as f32;
    let dq = dm * grav_q;
    let mut out = Vec::with_capacity(nx * ny);
    for iy in 0..ny {
        let ly = -hy + (iy as f32 + 0.5) * (2.0 * hy / ny as f32);
        for ix in 0..nx {
            let lx = -hx + (ix as f32 + 0.5) * (2.0 * hx / nx as f32);
            out.push((rot.rotate(Vec2::new(lx, ly)), dm, dq));
        }
    }
    out
}

fn sample_disk_mass_nodes(
    radius: f32,
    shells: &[(f32, f32, f32)],
    total_mass: f32,
) -> Vec<(Vec2, f32, f32)> {
    let r_max = radius.max(0.5);
    let step = (r_max / 3.8).max(0.45);
    let n = (r_max / step).ceil() as i32;
    let dy = step * 0.866_025_4;
    let mut raw = Vec::with_capacity(64);
    for iy in -n..=n {
        let ro = if iy.rem_euclid(2) == 1 { 0.5 * step } else { 0.0 };
        let ly = iy as f32 * dy;
        for ix in -n..=n {
            let lx = ix as f32 * step + ro;
            let r = lx.hypot(ly);
            if r <= r_max {
                let (_, rho, q) = shells
                    .iter()
                    .find(|(sr, _, _)| r <= *sr)
                    .copied()
                    .unwrap_or_else(|| shells.last().copied().unwrap_or((r_max, 1.0, 1.0)));
                raw.push((Vec2::new(lx, ly), rho, rho * q));
            }
        }
    }
    if raw.is_empty() {
        let q = shells.first().map(|(_, _, q)| *q).unwrap_or(1.0);
        return vec![(Vec2::ZERO, total_mass, total_mass * q)];
    }
    let sum_w: f32 = raw.iter().map(|(_, w, _)| *w).sum();
    if sum_w > 0.0 {
        let scale = total_mass / sum_w;
        for (_, w, qm) in &mut raw {
            *w *= scale;
            *qm *= scale;
        }
    }
    raw
}

#[derive(Clone)]
struct BodyProxy {
    pos: Vec2,
    vel: Vec2,
    mass: f32,
    grav_charge_mass: f32,
    bound_r: f32,
    is_proj: bool,
    geom: BodyShapeGeom,
    boundary_rot: Vec<Vec2>,
    edges_rot: Vec<(Vec2, Vec2)>,
    mass_nodes_rot: Vec<(Vec2, f32, f32)>,
    inertia_gyr2: f32,
}

impl BodyProxy {
    fn from_target(t: &TargetSpec, sim_dx: f32) -> Self {
        let mass = t.estimate_mass().max(1.0);
        let bound_r = t.bounding_radius().max(0.6);
        let rot = Vec2::from_angle(t.angle_deg.to_radians());
        let mut edges_rot = Vec::new();
        let (geom, boundary_rot, mass_nodes_rot) = match &t.kind {
            TargetKind::SolidBox { width, height, mat } => {
                let hx = 0.5 * *width;
                let hy = 0.5 * *height;
                let verts = [
                    rot.rotate(Vec2::new(-hx, -hy)),
                    rot.rotate(Vec2::new(hx, -hy)),
                    rot.rotate(Vec2::new(hx, hy)),
                    rot.rotate(Vec2::new(-hx, hy)),
                ];
                edges_rot = (0..4).map(|k| (verts[k], verts[(k + 1) % 4])).collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.7);
                let mn = sample_rect_mass_nodes(hx, hy, mass, mat.grav_charge_mult(), rot);
                (BodyShapeGeom::Rect { hx, hy, rot }, bnd, mn)
            }
            TargetKind::CompositePlate { height, layers } => {
                let w: f32 = layers.iter().map(|l| l.thickness).sum();
                let hx = 0.5 * w;
                let hy = 0.5 * *height;
                let verts = [
                    rot.rotate(Vec2::new(-hx, -hy)),
                    rot.rotate(Vec2::new(hx, -hy)),
                    rot.rotate(Vec2::new(hx, hy)),
                    rot.rotate(Vec2::new(-hx, hy)),
                ];
                edges_rot = (0..4).map(|k| (verts[k], verts[(k + 1) % 4])).collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.7);
                let mut acc_x = -hx;
                let mut mn = Vec::new();
                for l in layers {
                    let cx = acc_x + 0.5 * l.thickness;
                    let lm = l.thickness * *height * l.mat.props().rho0;
                    let lq = lm * l.mat.grav_charge_mult();
                    for iy in 0..4 {
                        let ly = -hy + (iy as f32 + 0.5) * (*height * 0.25);
                        mn.push((rot.rotate(Vec2::new(cx, ly)), lm * 0.25, lq * 0.25));
                    }
                    acc_x += l.thickness;
                }
                if mn.is_empty() {
                    mn.push((Vec2::ZERO, mass, mass));
                }
                (BodyShapeGeom::Rect { hx, hy, rot }, bnd, mn)
            }
            TargetKind::Ellipse { rx, ry, mat } => {
                let q = mat.grav_charge_mult();
                let verts: Vec<Vec2> = (0..32)
                    .map(|k| {
                        let th = (k as f32 / 32.0) * std::f32::consts::TAU;
                        rot.rotate(Vec2::new(*rx * th.cos(), *ry * th.sin()))
                    })
                    .collect();
                let mut mn = Vec::with_capacity(24);
                let step_x = (*rx * 0.45).max(0.5);
                let step_y = (*ry * 0.45).max(0.5);
                for iy in -2..=2 {
                    let ly = iy as f32 * step_y;
                    for ix in -2..=2 {
                        let lx = ix as f32 * step_x;
                        if (lx / *rx).powi(2) + (ly / *ry).powi(2) <= 1.0 {
                            mn.push((rot.rotate(Vec2::new(lx, ly)), 1.0f32, q));
                        }
                    }
                }
                let n_len = mn.len().max(1) as f32;
                for (_, w, qm) in &mut mn {
                    *w = mass / n_len;
                    *qm = *w * q;
                }
                if mn.is_empty() {
                    mn.push((Vec2::ZERO, mass, mass * q));
                }
                (BodyShapeGeom::Ellipse { rx: *rx, ry: *ry, rot }, verts, mn)
            }
            TargetKind::HollowRing { outer_r, wall_thick, mat } => {
                let q = mat.grav_charge_mult();
                let inner_r = (*outer_r - *wall_thick).max(0.0);
                let mut verts = Vec::with_capacity(32);
                let r_mid = 0.5 * (*outer_r + inner_r);
                let mn: Vec<(Vec2, f32, f32)> = (0..16)
                    .map(|k| {
                        let th = (k as f32 / 16.0) * std::f32::consts::TAU;
                        let wm = mass / 16.0;
                        (Vec2::from_angle(th) * r_mid, wm, wm * q)
                    })
                    .collect();
                for k in 0..24 {
                    let th = (k as f32 / 24.0) * std::f32::consts::TAU;
                    verts.push(Vec2::from_angle(th) * *outer_r);
                }
                (
                    BodyShapeGeom::Annulus {
                        inner_r,
                        outer_r: *outer_r,
                    },
                    verts,
                    mn,
                )
            }
            TargetKind::TriangleWedge { width, height, mat } => {
                let hw = (0.5 * *width).max(1.0);
                let hh = (0.5 * *height).max(1.0);
                let verts = vec![
                    rot.rotate(Vec2::new(-hw, -hh)),
                    rot.rotate(Vec2::new(hw, 0.0)),
                    rot.rotate(Vec2::new(-hw, hh)),
                ];
                edges_rot = (0..3).map(|k| (verts[k], verts[(k + 1) % 3])).collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.7);
                let com = rot.rotate(Vec2::new(-hw / 3.0, 0.0));
                let mn = vec![(com, mass, mass * mat.grav_charge_mult())];
                (
                    BodyShapeGeom::Polygon {
                        verts_rot: verts,
                    },
                    bnd,
                    mn,
                )
            }
            TargetKind::LayeredPlanet { shells } => {
                let mut acc_r = 0.0f32;
                let mut cum = Vec::with_capacity(shells.len());
                for s in shells {
                    acc_r += s.thickness.max(0.5);
                    cum.push((acc_r, s.mat.props().rho0, s.mat.grav_charge_mult()));
                }
                let r_out = acc_r.max(1.0);
                let bnd: Vec<Vec2> = (0..24)
                    .map(|k| Vec2::from_angle((k as f32 / 24.0) * std::f32::consts::TAU) * r_out)
                    .collect();
                let mn = sample_disk_mass_nodes(r_out, &cum, mass);
                (BodyShapeGeom::Circle { radius: r_out }, bnd, mn)
            }
            TargetKind::Blueprint { bp } => Self::build_blueprint_proxy(bp, sim_dx, rot, mass),
        };
        Self::finish(t.pos, t.vel, mass, bound_r, false, geom, boundary_rot, edges_rot, mass_nodes_rot)
    }

    fn from_projectile(p: &ProjectileSpec, sim_dx: f32) -> Self {
        let mass = p.estimate_mass().max(0.5);
        let bound_r = p.bounding_radius().max(0.6);
        let vel = p.vel_cm_per_us();
        let rot = Vec2::from_angle(p.angle_deg.to_radians());
        let mut edges_rot = Vec::new();
        let r = p.radius_cm.max(0.45);
        let q = p.mat.grav_charge_mult();
        let (geom, boundary_rot, mass_nodes_rot) = match p.shape {
            ProjectileShape::Sphere => {
                let bnd = (0..20)
                    .map(|k| Vec2::from_angle((k as f32 / 20.0) * std::f32::consts::TAU) * r)
                    .collect();
                let mn = sample_disk_mass_nodes(r, &[(r, p.mat.props().rho0, q)], mass);
                (BodyShapeGeom::Circle { radius: r }, bnd, mn)
            }
            ProjectileShape::DifferentiatedAsteroid => {
                let rc = (r * p.core_frac.clamp(0.15, 0.85)).max(0.4);
                let bnd = (0..20)
                    .map(|k| Vec2::from_angle((k as f32 / 20.0) * std::f32::consts::TAU) * r)
                    .collect();
                let mn = sample_disk_mass_nodes(
                    r,
                    &[
                        (rc, p.core_mat.props().rho0, p.core_mat.grav_charge_mult()),
                        (r, p.mat.props().rho0, q),
                    ],
                    mass,
                );
                (BodyShapeGeom::Circle { radius: r }, bnd, mn)
            }
            ProjectileShape::HollowRing => {
                let inner_r = (r * (1.0 - p.core_frac.clamp(0.15, 0.85))).max(0.0);
                let r_mid = 0.5 * (r + inner_r);
                let bnd = (0..20)
                    .map(|k| Vec2::from_angle((k as f32 / 20.0) * std::f32::consts::TAU) * r)
                    .collect();
                let mn = (0..12)
                    .map(|k| {
                        let wm = mass / 12.0;
                        (
                            Vec2::from_angle((k as f32 / 12.0) * std::f32::consts::TAU) * r_mid,
                            wm,
                            wm * q,
                        )
                    })
                    .collect();
                (BodyShapeGeom::Annulus { inner_r, outer_r: r }, bnd, mn)
            }
            ProjectileShape::Ellipse => {
                let rx = (0.5 * p.length_cm).max(1.0);
                let bnd = (0..24)
                    .map(|k| {
                        let th = (k as f32 / 24.0) * std::f32::consts::TAU;
                        rot.rotate(Vec2::new(rx * th.cos(), r * th.sin()))
                    })
                    .collect();
                let mn = sample_rect_mass_nodes(0.75 * rx, 0.75 * r, mass, q, rot);
                (BodyShapeGeom::Ellipse { rx, ry: r, rot }, bnd, mn)
            }
            ProjectileShape::BluntSlug => {
                let hx = (0.5 * p.length_cm).max(1.0);
                let verts = [
                    rot.rotate(Vec2::new(-hx, -r)),
                    rot.rotate(Vec2::new(hx, -r)),
                    rot.rotate(Vec2::new(hx, r)),
                    rot.rotate(Vec2::new(-hx, r)),
                ];
                edges_rot = (0..4).map(|k| (verts[k], verts[(k + 1) % 4])).collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.65);
                let mn = sample_rect_mass_nodes(hx, r, mass, q, rot);
                (BodyShapeGeom::Rect { hx, hy: r, rot }, bnd, mn)
            }
            ProjectileShape::Wedge => {
                let hw = (0.5 * p.length_cm).max(1.0);
                let verts = vec![
                    rot.rotate(Vec2::new(-hw, -r)),
                    rot.rotate(Vec2::new(hw, 0.0)),
                    rot.rotate(Vec2::new(-hw, r)),
                ];
                edges_rot = (0..3).map(|k| (verts[k], verts[(k + 1) % 3])).collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.65);
                let mn = vec![(rot.rotate(Vec2::new(-hw / 3.0, 0.0)), mass, mass * q)];
                (BodyShapeGeom::Polygon { verts_rot: verts }, bnd, mn)
            }
            ProjectileShape::LongRodDart => {
                let hl = (0.5 * p.length_cm).max(2.0);
                let ns = hl * 0.58;
                let fs = -hl * 0.72;
                let verts = vec![
                    rot.rotate(Vec2::new(-hl, 2.15 * r)),
                    rot.rotate(Vec2::new(fs, r)),
                    rot.rotate(Vec2::new(ns, r)),
                    rot.rotate(Vec2::new(hl, 0.18 * r)),
                    rot.rotate(Vec2::new(hl, -0.18 * r)),
                    rot.rotate(Vec2::new(ns, -r)),
                    rot.rotate(Vec2::new(fs, -r)),
                    rot.rotate(Vec2::new(-hl, -2.15 * r)),
                ];
                edges_rot = (0..verts.len())
                    .map(|k| (verts[k], verts[(k + 1) % verts.len()]))
                    .collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.65);
                let mn = sample_rect_mass_nodes(0.85 * hl, r, mass, q, rot);
                (BodyShapeGeom::Polygon { verts_rot: verts }, bnd, mn)
            }
            ProjectileShape::ShapedJet => {
                let hl = (0.5 * p.length_cm).max(3.0);
                let verts = vec![
                    rot.rotate(Vec2::new(-hl, 1.45 * r)),
                    rot.rotate(Vec2::new(hl, 0.80 * r)),
                    rot.rotate(Vec2::new(hl, -0.80 * r)),
                    rot.rotate(Vec2::new(-hl, -1.45 * r)),
                ];
                edges_rot = (0..4).map(|k| (verts[k], verts[(k + 1) % 4])).collect();
                let bnd = sample_poly_perimeter_rot(&verts, 0.65);
                let mn = sample_rect_mass_nodes(0.85 * hl, r, mass, q, rot);
                (BodyShapeGeom::Polygon { verts_rot: verts }, bnd, mn)
            }
            ProjectileShape::CustomBlueprint => {
                if let Some(bp) = &p.blueprint {
                    Self::build_blueprint_proxy(bp, sim_dx, rot, mass)
                } else {
                    let hx = (0.5 * p.length_cm).max(1.0);
                    let verts = [
                        rot.rotate(Vec2::new(-hx, -r)),
                        rot.rotate(Vec2::new(hx, -r)),
                        rot.rotate(Vec2::new(hx, r)),
                        rot.rotate(Vec2::new(-hx, r)),
                    ];
                    edges_rot = (0..4).map(|k| (verts[k], verts[(k + 1) % 4])).collect();
                    let bnd = sample_poly_perimeter_rot(&verts, 0.65);
                    let mn = sample_rect_mass_nodes(hx, r, mass, q, rot);
                    (BodyShapeGeom::Rect { hx, hy: r, rot }, bnd, mn)
                }
            }
        };
        Self::finish(p.pos, vel, mass, bound_r, true, geom, boundary_rot, edges_rot, mass_nodes_rot)
    }

    fn build_blueprint_proxy(
        bp: &PixelBlueprint,
        dx: f32,
        rot: Vec2,
        total_mass: f32,
    ) -> (BodyShapeGeom, Vec<Vec2>, Vec<(Vec2, f32, f32)>) {
        let Some((min_x, min_y, max_x, max_y)) = bp.occupied_bounds() else {
            return (
                BodyShapeGeom::Circle { radius: 1.5 },
                vec![Vec2::X * 1.5, Vec2::NEG_X * 1.5],
                vec![(Vec2::ZERO, total_mass, total_mass)],
            );
        };
        let dy = dx * 0.866_025_4;
        let mid_gx = 0.5 * (min_x + max_x) as f32;
        let mid_gy = 0.5 * (min_y + max_y) as f32;

        let mut raw_bnd = Vec::new();
        let mut bins = vec![(Vec2::ZERO, 0.0f32, 0.0f32); 16];
        let mut sum_rho = 0.0f32;
        let w_span = (max_x - min_x + 1).max(1) as f32;
        let h_span = (max_y - min_y + 1).max(1) as f32;

        for y in min_y..=max_y {
            let row_off = if y % 2 == 1 { 0.5 * dx } else { 0.0 };
            let ly = (mid_gy - y as f32) * dy;
            for x in min_x..=max_x {
                if let Some(m) = bp.get(x as i32, y as i32) {
                    let lx = (x as f32 - mid_gx) * dx + row_off;
                    let lp = Vec2::new(lx, ly);
                    let rho = m.props().rho0;
                    sum_rho += rho;
                    let bx = (((x - min_x) as f32 / w_span) * 4.0).floor().clamp(0.0, 3.0) as usize;
                    let by = (((y - min_y) as f32 / h_span) * 4.0).floor().clamp(0.0, 3.0) as usize;
                    let bid = by * 4 + bx;
                    bins[bid].0 += lp * rho;
                    bins[bid].1 += rho;
                    bins[bid].2 += rho * m.grav_charge_mult();

                    let xi = x as i32;
                    let yi = y as i32;
                    if bp.get(xi - 1, yi).is_none()
                        || bp.get(xi + 1, yi).is_none()
                        || bp.get(xi, yi - 1).is_none()
                        || bp.get(xi, yi + 1).is_none()
                    {
                        raw_bnd.push(rot.rotate(lp));
                    }
                }
            }
        }
        let max_pts = 72usize;
        let bnd = if raw_bnd.len() > max_pts {
            let stride = raw_bnd.len().div_ceil(max_pts);
            raw_bnd.into_iter().step_by(stride).collect()
        } else {
            raw_bnd
        };
        let mut mn = Vec::with_capacity(16);
        let scale = if sum_rho > 0.0 { total_mass / sum_rho } else { 1.0 };
        for (com_sum, w, q_sum) in bins {
            if w > 0.0 {
                mn.push((rot.rotate(com_sum / w), w * scale, q_sum * scale));
            }
        }
        if mn.is_empty() {
            mn.push((Vec2::ZERO, total_mass, total_mass));
        }
        (
            BodyShapeGeom::Blueprint {
                bp: bp.clone(),
                dx,
                rot,
                mid_gx,
                mid_gy,
            },
            bnd,
            mn,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        pos: Vec2,
        vel: Vec2,
        mass: f32,
        bound_r: f32,
        is_proj: bool,
        geom: BodyShapeGeom,
        boundary_rot: Vec<Vec2>,
        edges_rot: Vec<(Vec2, Vec2)>,
        mass_nodes_rot: Vec<(Vec2, f32, f32)>,
    ) -> Self {
        let grav_charge_mass = mass_nodes_rot.iter().map(|(_, _, qm)| *qm).sum();
        let inertia_gyr2 = if mass > 0.0 {
            mass_nodes_rot
                .iter()
                .map(|(off, m, _)| m * off.length_squared())
                .sum::<f32>()
                / mass
        } else {
            0.5 * bound_r * bound_r
        };
        Self {
            pos,
            vel,
            mass,
            grav_charge_mass,
            bound_r,
            is_proj,
            geom,
            boundary_rot,
            edges_rot,
            mass_nodes_rot,
            inertia_gyr2,
        }
    }
}

fn collide_bodies(a: &BodyProxy, pos_a: Vec2, b: &BodyProxy, pos_b: Vec2) -> Option<Vec2> {
    if let (BodyShapeGeom::Circle { radius: ra }, BodyShapeGeom::Circle { radius: rb }) =
        (&a.geom, &b.geom)
    {
        let d = pos_b - pos_a;
        let r_sum = ra + rb;
        let d2 = d.length_squared();
        if d2 <= r_sum * r_sum {
            let dist = d2.sqrt();
            let dir = if dist > 1e-6 { d / dist } else { Vec2::X };
            return Some(pos_a + dir * (*ra).min(dist));
        }
        return None;
    }
    if let BodyShapeGeom::Circle { radius: ra } = &a.geom {
        let (inside, closest_b) = b.geom.closest_boundary_world(pos_b, pos_a);
        if inside || (pos_a - closest_b).length_squared() <= ra * ra {
            return Some(closest_b);
        }
        return None;
    }
    if let BodyShapeGeom::Circle { radius: rb } = &b.geom {
        let (inside, closest_a) = a.geom.closest_boundary_world(pos_a, pos_b);
        if inside || (pos_b - closest_a).length_squared() <= rb * rb {
            return Some(closest_a);
        }
        return None;
    }

    let bp_pad_a: f32 = if matches!(a.geom, BodyShapeGeom::Blueprint { .. }) { 0.35 } else { 0.12 };
    let bp_pad_b: f32 = if matches!(b.geom, BodyShapeGeom::Blueprint { .. }) { 0.35 } else { 0.12 };
    let tol = bp_pad_a.max(bp_pad_b);

    for &off_a in &a.boundary_rot {
        let pt_w = pos_a + off_a;
        let (inside, closest_b) = b.geom.closest_boundary_world(pos_b, pt_w);
        if inside || (pt_w - closest_b).length_squared() <= tol * tol {
            return Some(closest_b);
        }
    }
    for &off_b in &b.boundary_rot {
        let pt_w = pos_b + off_b;
        let (inside, closest_a) = a.geom.closest_boundary_world(pos_a, pt_w);
        if inside || (pt_w - closest_a).length_squared() <= tol * tol {
            return Some(pt_w);
        }
    }
    for &(a0, a1) in &a.edges_rot {
        for &(b0, b1) in &b.edges_rot {
            if let Some(hit) = segments_intersect(pos_a + a0, pos_a + a1, pos_b + b0, pos_b + b1) {
                return Some(hit);
            }
        }
    }
    None
}

fn compute_domain_soft_sq(bodies: &[BodyProxy], positions: &[Vec2], active: &[bool], dx: f32) -> f32 {
    let mut tot_m = 0.0f32;
    let mut com = Vec2::ZERO;
    for (i, b) in bodies.iter().enumerate() {
        if active[i] {
            tot_m += b.mass;
            com += positions[i] * b.mass;
        }
    }
    if tot_m <= 0.0 {
        return (2.4 * dx) * (2.4 * dx) + 0.6;
    }
    com /= tot_m;
    let mut r2_sum = 0.0f32;
    for (i, b) in bodies.iter().enumerate() {
        if active[i] {
            r2_sum += b.mass * ((positions[i] - com).length_squared() + b.inertia_gyr2);
        }
    }
    let r_rms = (r2_sum / tot_m).sqrt();
    let half_span = (3.4 * r_rms).clamp(28.0, 600.0);
    let cell_sz = (2.0 * half_span) / 32.0;
    (2.4 * dx) * (2.4 * dx) + 0.20 * cell_sz * cell_sz
}

fn compute_pair_gravity_acc(
    recv: &BodyProxy,
    pos_r: Vec2,
    src: &BodyProxy,
    pos_s: Vec2,
    grav: f32,
    soft_sq: f32,
) -> Vec2 {
    let d_com = pos_s - pos_r;
    let close_thresh = 4.2 * (recv.bound_r + src.bound_r);
    if d_com.length_squared() > close_thresh * close_thresh
        || (recv.mass_nodes_rot.len() <= 1 && src.mass_nodes_rot.len() <= 1)
    {
        let r2 = d_com.length_squared() + soft_sq;
        let q_recv = if recv.mass > 0.0 {
            recv.grav_charge_mass / recv.mass
        } else {
            1.0
        };
        return d_com * (grav * q_recv * src.grav_charge_mass / (r2 * r2.sqrt()));
    }
    let r_stride = recv.mass_nodes_rot.len().div_ceil(12).max(1);
    let mut acc_sum = Vec2::ZERO;
    let mut m_sum = 0.0f32;
    for &(ro, rm, rq) in recv.mass_nodes_rot.iter().step_by(r_stride) {
        let wp_r = pos_r + ro;
        let mut g_node = Vec2::ZERO;
        for &(so, _sm, sq) in &src.mass_nodes_rot {
            let d = (pos_s + so) - wp_r;
            let r2 = d.length_squared() + soft_sq;
            g_node += d * (grav * sq / (r2 * r2.sqrt()));
        }
        acc_sum += g_node * rq;
        m_sum += rm;
    }
    if m_sum > 0.0 {
        acc_sum / m_sum
    } else {
        Vec2::ZERO
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

    pub fn effective_self_gravity(&self) -> (bool, f32) {
        let active = self.self_gravity
            || self.targets.iter().any(TargetSpec::has_gravity_material)
            || self
                .projectiles
                .iter()
                .any(ProjectileSpec::has_gravity_material);
        if active {
            (true, if self.grav_g > 0.0 { self.grav_g } else { 7.5e-5 })
        } else {
            (false, 0.0)
        }
    }

    /// Compute circular or eccentric orbit `(speed_kms, angle_deg)` for `projectiles[proj_idx]`
    /// around the nearest attractive massive target body under softened extended-body 2D N-body gravity.
    pub fn compute_orbit_velocity(
        &self,
        proj_idx: usize,
        retrograde: bool,
        eccentricity_factor: f32,
    ) -> Option<(f32, f32)> {
        let p = self.projectiles.get(proj_idx)?;
        let (eff_grav, eff_g) = self.effective_self_gravity();
        if !eff_grav || eff_g <= 0.0 || self.targets.is_empty() {
            return None;
        }
        let dx = 0.35f32;
        let tgt_proxies: Vec<BodyProxy> = self
            .targets
            .iter()
            .map(|t| BodyProxy::from_target(t, dx))
            .collect();
        let proj_proxy = BodyProxy::from_projectile(p, dx);
        let proj_q = if proj_proxy.mass > 0.0 {
            proj_proxy.grav_charge_mass / proj_proxy.mass
        } else {
            1.0
        };
        let mut best_idx = 0usize;
        let mut best_score = f32::NEG_INFINITY;
        for (ti, t) in tgt_proxies.iter().enumerate() {
            let attract_q = t.grav_charge_mass * proj_q;
            let score = attract_q / (t.pos - proj_proxy.pos).length_squared().max(1.0);
            if score > best_score {
                best_score = score;
                best_idx = ti;
            }
        }
        let tgt_p = &tgt_proxies[best_idx];
        let tgt = &self.targets[best_idx];
        let rel = proj_proxy.pos - tgt_p.pos;
        let r = rel.length().max(1.0);
        let r_hat = rel / r;
        let tang = if retrograde {
            Vec2::new(r_hat.y, -r_hat.x)
        } else {
            Vec2::new(-r_hat.y, r_hat.x)
        };
        let mut all_proxies = tgt_proxies.clone();
        all_proxies.push(proj_proxy.clone());
        let positions: Vec<Vec2> = all_proxies.iter().map(|b| b.pos).collect();
        let active = vec![true; all_proxies.len()];
        let soft_sq = compute_domain_soft_sq(&all_proxies, &positions, &active, dx);

        let a_vec =
            compute_pair_gravity_acc(&proj_proxy, proj_proxy.pos, tgt_p, tgt_p.pos, eff_g, soft_sq);
        let a_inward = -a_vec.dot(r_hat);
        if a_inward <= 1e-12 {
            return None;
        }
        let reduced_scale = (tgt_p.mass + proj_proxy.mass) / tgt_p.mass.max(1e-4);
        let v_rel = (a_inward * r * reduced_scale).sqrt() * eccentricity_factor;
        let v_vec = tgt.vel + tang * v_rel;
        let speed_kms = (v_vec.length() * 10.0).clamp(0.0, 100.0);
        let angle_deg = if v_vec.length_squared() > 1e-12 {
            v_vec.y.atan2(v_vec.x).to_degrees()
        } else {
            0.0
        };
        Some((speed_kms, angle_deg))
    }

    /// Integrate extended-body softened N-body trajectories with continuous swept shape-vs-shape collision.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn predict_staging_trajectories(&self) -> Vec<TrajectoryTrace> {
        self.predict_staging_trajectories_with_dx(0.35)
    }

    /// Variant accepting active simulation spacing `dx` for blueprint cell scale parity.
    pub fn predict_staging_trajectories_with_dx(&self, sim_dx: f32) -> Vec<TrajectoryTrace> {
        let n_t = self.targets.len();
        let n_p = self.projectiles.len();
        let n = n_t + n_p;
        if n == 0 {
            return Vec::new();
        }
        let mut bodies = Vec::with_capacity(n);
        for t in &self.targets {
            bodies.push(BodyProxy::from_target(t, sim_dx));
        }
        for p in &self.projectiles {
            bodies.push(BodyProxy::from_projectile(p, sim_dx));
        }

        let (eff_grav, eff_g) = self.effective_self_gravity();
        let grav = if eff_grav { eff_g } else { 0.0 };

        let mut pos: Vec<Vec2> = bodies.iter().map(|b| b.pos).collect();
        let mut vel: Vec<Vec2> = bodies.iter().map(|b| b.vel).collect();
        let mut traces: Vec<TrajectoryTrace> = bodies
            .iter()
            .map(|b| TrajectoryTrace {
                is_projectile: b.is_proj,
                points: vec![b.pos],
                impact: None,
                impact_body_pos: None,
            })
            .collect();
        let mut active = vec![true; n];
        let mut last_recorded_pos: Vec<Vec2> = pos.clone();

        // Check initial t = 0 overlap immediately
        for i in 0..n {
            if !active[i] {
                continue;
            }
            for j in (i + 1)..n {
                if !active[j] {
                    continue;
                }
                if (pos[j] - pos[i]).length_squared() <= (bodies[i].bound_r + bodies[j].bound_r).powi(2)
                {
                    if let Some(contact) = collide_bodies(&bodies[i], pos[i], &bodies[j], pos[j]) {
                        let keep_tgt_i = !bodies[i].is_proj
                            && bodies[j].is_proj
                            && bodies[j].mass < 0.35 * bodies[i].mass;
                        let keep_tgt_j = !bodies[j].is_proj
                            && bodies[i].is_proj
                            && bodies[i].mass < 0.35 * bodies[j].mass;
                        if !keep_tgt_i {
                            traces[i].impact = Some(contact);
                            traces[i].impact_body_pos = Some(pos[i]);
                            active[i] = false;
                        }
                        if !keep_tgt_j {
                            traces[j].impact = Some(contact);
                            traces[j].impact_body_pos = Some(pos[j]);
                            active[j] = false;
                        }
                    }
                }
            }
        }

        let compute_acc = |positions: &[Vec2], alive: &[bool]| -> Vec<Vec2> {
            let mut acc = vec![Vec2::ZERO; n];
            if grav <= 0.0 {
                return acc;
            }
            let soft_sq = compute_domain_soft_sq(&bodies, positions, alive, sim_dx);
            for i in 0..n {
                if !alive[i] {
                    continue;
                }
                let mut g = Vec2::ZERO;
                for j in 0..n {
                    if i == j || !alive[j] {
                        continue;
                    }
                    g += compute_pair_gravity_acc(
                        &bodies[i],
                        positions[i],
                        &bodies[j],
                        positions[j],
                        grav,
                        soft_sq,
                    );
                }
                acc[i] = g;
            }
            acc
        };

        let steps = if grav > 0.0 { 520usize } else { 180usize };
        let mut acc = compute_acc(&pos, &active);

        for step_idx in 0..steps {
            if !active.iter().any(|&a| a) {
                break;
            }
            let mut v_max = 0.02f32;
            let mut a_max = 0.0f32;
            for i in 0..n {
                if active[i] {
                    v_max = v_max.max(vel[i].length());
                    a_max = a_max.max(acc[i].length());
                }
            }
            let dt = if grav > 0.0 {
                let dt_v = 0.85 / v_max.max(0.03);
                let dt_a = if a_max > 1e-7 {
                    (0.10 * v_max.max(0.05) / a_max).clamp(0.10, 4.2)
                } else {
                    4.2
                };
                dt_v.min(dt_a).clamp(0.08, 4.2)
            } else {
                (1.8 / v_max.max(0.04)).clamp(0.15, 2.5)
            };

            let prev_pos = pos.clone();
            for i in 0..n {
                if active[i] {
                    pos[i] += vel[i] * dt + 0.5 * acc[i] * dt * dt;
                }
            }

            // Swept continuous shape-vs-shape collision detection sorted earliest-first
            loop {
                let mut earliest: Option<(f32, usize, usize, Vec2)> = None;
                for i in 0..n {
                    if !active[i] {
                        continue;
                    }
                    for j in (i + 1)..n {
                        if !active[j] {
                            continue;
                        }
                        let r_rel0 = prev_pos[i] - prev_pos[j];
                        let r_rel1 = pos[i] - pos[j];
                        let seg = r_rel1 - r_rel0;
                        let seg_len2 = seg.length_squared();
                        let u_min = if seg_len2 > 1e-12 {
                            (-r_rel0.dot(seg) / seg_len2).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let min_d2 = (r_rel0 + seg * u_min).length_squared();
                        let br_sum = bodies[i].bound_r + bodies[j].bound_r + 0.4;
                        if min_d2 > br_sum * br_sum {
                            continue;
                        }

                        let n_sub = ((seg_len2.sqrt() / 0.35).ceil() as usize).clamp(1, 28);
                        let mut hit_span: Option<(f32, f32, Vec2)> = None;
                        for s in 1..=n_sub {
                            let u_hi = s as f32 / n_sub as f32;
                            let pa = prev_pos[i].lerp(pos[i], u_hi);
                            let pb = prev_pos[j].lerp(pos[j], u_hi);
                            if let Some(c) = collide_bodies(&bodies[i], pa, &bodies[j], pb) {
                                let u_lo = (s - 1) as f32 / n_sub as f32;
                                hit_span = Some((u_lo, u_hi, c));
                                break;
                            }
                        }
                        if let Some((mut lo, mut hi, mut contact)) = hit_span {
                            for _ in 0..6 {
                                let mid = 0.5 * (lo + hi);
                                let pa = prev_pos[i].lerp(pos[i], mid);
                                let pb = prev_pos[j].lerp(pos[j], mid);
                                if let Some(c) = collide_bodies(&bodies[i], pa, &bodies[j], pb) {
                                    hi = mid;
                                    contact = c;
                                } else {
                                    lo = mid;
                                }
                            }
                            if earliest.is_none_or(|(best_u, _, _, _)| hi < best_u) {
                                earliest = Some((hi, i, j, contact));
                            }
                        }
                    }
                }

                let Some((u_hit, i, j, contact)) = earliest else {
                    break;
                };
                let pi_hit = prev_pos[i].lerp(pos[i], u_hit);
                let pj_hit = prev_pos[j].lerp(pos[j], u_hit);
                let keep_tgt_i = !bodies[i].is_proj
                    && bodies[j].is_proj
                    && bodies[j].mass < 0.35 * bodies[i].mass;
                let keep_tgt_j = !bodies[j].is_proj
                    && bodies[i].is_proj
                    && bodies[i].mass < 0.35 * bodies[j].mass;
                if !keep_tgt_i {
                    traces[i].points.push(pi_hit);
                    traces[i].impact = Some(contact);
                    traces[i].impact_body_pos = Some(pi_hit);
                    active[i] = false;
                }
                if !keep_tgt_j {
                    traces[j].points.push(pj_hit);
                    traces[j].impact = Some(contact);
                    traces[j].impact_body_pos = Some(pj_hit);
                    active[j] = false;
                }
            }

            let next_acc = compute_acc(&pos, &active);
            for i in 0..n {
                if active[i] {
                    vel[i] += 0.5 * (acc[i] + next_acc[i]) * dt;
                }
            }
            acc = next_acc;

            for i in 0..n {
                if active[i]
                    && (traces[i].is_projectile || vel[i].length_squared() > 1e-8)
                    && ((pos[i] - last_recorded_pos[i]).length_squared() >= 0.72
                        || step_idx + 1 == steps)
                {
                    traces[i].points.push(pos[i]);
                    last_recorded_pos[i] = pos[i];
                }
            }
        }
        traces
    }

    pub fn build_world(&self, dx: f32) -> ContinuumWorld {
        let mut world = ContinuumWorld::new(dx);
        let (eff_grav, eff_g) = self.effective_self_gravity();
        world.self_gravity = eff_grav;
        world.grav_g = eff_g;

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
