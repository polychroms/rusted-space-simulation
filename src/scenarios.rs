use glam::Vec2;

use crate::blueprint::{BuiltinProjectileBp, BuiltinTargetBp};
use crate::materials::MaterialId;
use crate::physics::ContinuumWorld;
use crate::scene::{ArmorLayer, PlanetShell, ProjectileSpec, SceneSpec, TargetKind, TargetSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioCategory {
    All,
    ArmorShapedCharges,
    NavalBunker,
    PlanetaryOrbital,
}

impl ScenarioCategory {
    pub const ALL: [ScenarioCategory; 4] = [
        ScenarioCategory::All,
        ScenarioCategory::ArmorShapedCharges,
        ScenarioCategory::NavalBunker,
        ScenarioCategory::PlanetaryOrbital,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All Scenarios",
            Self::ArmorShapedCharges => "Armor & Shaped Charges",
            Self::NavalBunker => "Naval, Bunker & Structural",
            Self::PlanetaryOrbital => "Planetary & Orbital",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioPreset {
    HeatVsRhaArmor,
    ApfsdsVsKontakt5Era,
    TandemHeatVsEra,
    EfpVsSpacedArmor,
    ApheTankAmmoRack,
    HeshSquashSpall,
    ApfsdsVsChobhamNera,
    ApheVsConcreteBunker,
    PlanetaryImpact,
    IcyCometCollision,
    HypervelocityWhipple,
    LowSpeedPlasticRicochet,
    RocheLimitRingFormation,
    BinaryPlanetInspiral,
    KineticRodBunkerBuster,
    OceanMoonHyperImpact,
    EmptySpace,
}

impl ScenarioPreset {
    pub const ALL: [ScenarioPreset; 17] = [
        ScenarioPreset::HeatVsRhaArmor,
        ScenarioPreset::ApfsdsVsKontakt5Era,
        ScenarioPreset::TandemHeatVsEra,
        ScenarioPreset::EfpVsSpacedArmor,
        ScenarioPreset::ApheTankAmmoRack,
        ScenarioPreset::HeshSquashSpall,
        ScenarioPreset::ApfsdsVsChobhamNera,
        ScenarioPreset::ApheVsConcreteBunker,
        ScenarioPreset::PlanetaryImpact,
        ScenarioPreset::IcyCometCollision,
        ScenarioPreset::HypervelocityWhipple,
        ScenarioPreset::LowSpeedPlasticRicochet,
        ScenarioPreset::RocheLimitRingFormation,
        ScenarioPreset::BinaryPlanetInspiral,
        ScenarioPreset::KineticRodBunkerBuster,
        ScenarioPreset::OceanMoonHyperImpact,
        ScenarioPreset::EmptySpace,
    ];

    pub fn category(self) -> ScenarioCategory {
        match self {
            Self::HeatVsRhaArmor
            | Self::ApfsdsVsKontakt5Era
            | Self::TandemHeatVsEra
            | Self::EfpVsSpacedArmor
            | Self::HeshSquashSpall
            | Self::ApfsdsVsChobhamNera => ScenarioCategory::ArmorShapedCharges,
            Self::ApheTankAmmoRack
            | Self::ApheVsConcreteBunker
            | Self::LowSpeedPlasticRicochet
            | Self::KineticRodBunkerBuster => ScenarioCategory::NavalBunker,
            Self::PlanetaryImpact
            | Self::IcyCometCollision
            | Self::HypervelocityWhipple
            | Self::RocheLimitRingFormation
            | Self::BinaryPlanetInspiral
            | Self::OceanMoonHyperImpact => ScenarioCategory::PlanetaryOrbital,
            Self::EmptySpace => ScenarioCategory::All,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            ScenarioPreset::HeatVsRhaArmor => "1. HEAT Shaped-Charge vs RHA Armor (Munroe Jet)",
            ScenarioPreset::ApfsdsVsKontakt5Era => "2. APFSDS Dart vs Kontakt-5 ERA Hull (1.75 km/s)",
            ScenarioPreset::TandemHeatVsEra => "3. Tandem HEAT vs Kontakt-5 ERA (Anti-ERA)",
            ScenarioPreset::EfpVsSpacedArmor => "4. EFP Self-Forging Slug vs Spaced Roof Armor",
            ScenarioPreset::ApheTankAmmoRack => "5. 88mm APHE vs Tank Hull + Ammo Rack (Cook-Off!)",
            ScenarioPreset::HeshSquashSpall => "6. 120mm HESH Squash-Head Spallation (0.73 km/s)",
            ScenarioPreset::ApfsdsVsChobhamNera => "7. Depleted U APFSDS vs Chobham NERA Cheek",
            ScenarioPreset::ApheVsConcreteBunker => "8. APHE Shell vs Rebar Concrete Bunker (1.08 km/s)",
            ScenarioPreset::PlanetaryImpact => "9. Giant Planetary Core-Mantle Impact (5.6 km/s)",
            ScenarioPreset::IcyCometCollision => "10. Icy Comet vs Rocky Moon (6.8 km/s)",
            ScenarioPreset::HypervelocityWhipple => "11. Spacecraft Whipple Shield (7.2 km/s)",
            ScenarioPreset::LowSpeedPlasticRicochet => "12. Sub-Hydrodynamic Plastic Dent (0.42 km/s)",
            ScenarioPreset::RocheLimitRingFormation => "14. Tidal Roche-Limit Ring Disruption (Orbital)",
            ScenarioPreset::BinaryPlanetInspiral => "15. Binary Proto-Planet Inspiral Merger",
            ScenarioPreset::KineticRodBunkerBuster => "16. Tungsten Bunker-Buster vs Reinforced Vault",
            ScenarioPreset::OceanMoonHyperImpact => "17. Europa Ocean-Moon Hyper-Impact (9.2 km/s)",
            ScenarioPreset::EmptySpace => "13. Empty Continuum Space",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ScenarioPreset::HeatVsRhaArmor => {
                "Classic shaped-charge test! Nose piezoelectric Impact Fuze triggers along the firing wire to the rear apex of the Comp-B block. A 7.98 km/s Chapman-Jouguet detonation wave collapses the V-shaped OFHC Copper liner into an emergent hypersonic Munroe jet!"
            }
            ScenarioPreset::ApfsdsVsKontakt5Era => {
                "M829 Tungsten APFSDS long-rod strikes Kontakt-5 heavy ERA bricks over a composite RHA/Textolite/SiC glacis. Impact shock initiates the C4 interlayer, blasting the heavy HHA flyer plate obliquely into the rod to yaw and fracture it!"
            }
            ScenarioPreset::TandemHeatVsEra => {
                "PG-7VR Tandem HEAT warhead: the small front precursor shaped charge triggers and clears the Kontakt-5 ERA brick first, while an 18 µs pyrotechnic Delay Fuze waits before firing the main rear shaped charge into the exposed glacis!"
            }
            ScenarioPreset::EfpVsSpacedArmor => {
                "Explosively Formed Penetrator (EFP): standoff piezo probe trips against a thin Al 7075 slat screen, initiating the RHA-confined Comp-B block so the shallow 135° OFHC Copper dish folds forward in mid-air into a ~2.3 km/s slug against sloped RHA roof armor!"
            }
            ScenarioPreset::ApheTankAmmoRack => {
                "88mm PzGr.39 APHE punches through a sloped tank hull cutaway. Its shock-insensitive TNT burster survives entry until the 18 µs Base Delay Fuze detonates inside the crew compartment, sympathetically cooking off the internal Comp-B ammo rack!"
            }
            ScenarioPreset::HeshSquashSpall => {
                "120mm HESH shell squashes malleable C4 plastic explosive putty flat across sloped RHA steel before the Base Delay Fuze detonates it. A massive compressive shockwave reflects off the rear face as tension, ripping off a deadly spall scab!"
            }
            ScenarioPreset::ApfsdsVsChobhamNera => {
                "M829A3 Depleted Uranium penetrator vs Chobham NERA turret cheek: spaced HHA/Rubber/Mild-Steel sandwiches bulge non-explosively into air cavities to destabilize the dart ahead of a Boron Carbide ceramic matrix and Kevlar spall liner."
            }
            ScenarioPreset::ApheVsConcreteBunker => {
                "Armor-piercing HE shell burrows deep into a steel-rebar reinforced concrete bunker wall (Drucker-Prager pressure-hardening brittle concrete + ductile mild-steel rebar mesh) before its delay fuze detonates inside the masonry!"
            }
            ScenarioPreset::PlanetaryImpact => {
                "Differentiated proto-planet (Fe-Ni core + basalt mantle + icy crust) under self-gravity struck obliquely at 5.6 km/s. Shockwaves wrap through the core, trigger antipodal spalling, excavate a magma ocean, and rain ejecta back down."
            }
            ScenarioPreset::IcyCometCollision => {
                "Cryogenic crystalline water-ice comet (150 K) slams into a rocky silicate-iron moon at 6.8 km/s under self-gravity. The ice shatters brittlely at the rear while instantaneously shock-melting into a hydrodynamic plume at the crater."
            }
            ScenarioPreset::HypervelocityWhipple => {
                "At 7.2 km/s, an orbital debris sphere hits a thin sacrificial bumper plate. Shock heating instantly melts and fluidizes projectile + bumper into an expanding debris cloud that spreads impulse across the Kevlar-backed hull."
            }
            ScenarioPreset::LowSpeedPlasticRicochet => {
                "Contrast regime at 420 m/s: impact pressure stays near material strength instead of hydrodynamic fluid flow. The sloped steel plate permanently bends plastically while the deformed projectile ricochets."
            }
            ScenarioPreset::RocheLimitRingFormation => {
                "A fragile cryogenic water-ice moon skims inside a dense iron-silicate super-planet's Roche limit under self-gravity. Tidal differential acceleration shears the moon into elongated orbital ring arcs!"
            }
            ScenarioPreset::BinaryPlanetInspiral => {
                "Two differentiated Fe-Ni core + basalt planets orbit on a decaying binary trajectory under self-gravity, tidally stretching into prolate ellipsoids before coalescing into a hot post-impact body."
            }
            ScenarioPreset::KineticRodBunkerBuster => {
                "Heavy 2.4 km/s Tungsten penetrator rod dives through multi-story steel-rebar Concrete vault roof slabs and spall liners to breach a buried Comp-B munitions chamber."
            }
            ScenarioPreset::OceanMoonHyperImpact => {
                "An ice-shelled Europa-like ocean moon (Fe-Ni core + Granite mantle + deep Liquid Water subsurface ocean + cryogenic Water Ice crust) under self-gravity is punched open by a hypervelocity 9.2 km/s metallic impactor, venting massive hydrodynamic ocean plumes!"
            }
            ScenarioPreset::EmptySpace => {
                "Empty borderless continuum space. Design custom warheads & targets in Targets Builder & Projectiles Builder, place & aim them in STAGING, or pick a ballistic/astrophysics preset above."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileShape {
    CustomBlueprint,
    DifferentiatedAsteroid,
    Sphere,
    Ellipse,
    HollowRing,
    LongRodDart,
    BluntSlug,
    Wedge,
    ShapedJet,
}

impl ProjectileShape {
    pub const ALL: [ProjectileShape; 9] = [
        ProjectileShape::CustomBlueprint,
        ProjectileShape::Sphere,
        ProjectileShape::Ellipse,
        ProjectileShape::HollowRing,
        ProjectileShape::BluntSlug,
        ProjectileShape::Wedge,
        ProjectileShape::LongRodDart,
        ProjectileShape::DifferentiatedAsteroid,
        ProjectileShape::ShapedJet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ProjectileShape::CustomBlueprint => "Pixel Blueprint Warhead",
            ProjectileShape::DifferentiatedAsteroid => "Core-Mantle Asteroid",
            ProjectileShape::Sphere => "Circle / Sphere",
            ProjectileShape::Ellipse => "Ellipse / Oval",
            ProjectileShape::HollowRing => "Hollow Ring / Shell",
            ProjectileShape::LongRodDart => "APFSDS Long Rod",
            ProjectileShape::BluntSlug => "Rectangle / Cylinder Slug",
            ProjectileShape::Wedge => "Triangle / Wedge",
            ProjectileShape::ShapedJet => "Hypervelocity Jet",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            ProjectileShape::CustomBlueprint => "🎨 Blueprint",
            ProjectileShape::Sphere => "● Circle",
            ProjectileShape::Ellipse => "⬭ Ellipse",
            ProjectileShape::HollowRing => "◯ Ring",
            ProjectileShape::BluntSlug => "▭ Rect",
            ProjectileShape::Wedge => "▶ Wedge",
            ProjectileShape::LongRodDart => "➵ Rod",
            ProjectileShape::DifferentiatedAsteroid => "☄ Asteroid",
            ProjectileShape::ShapedJet => "⚡ Jet",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            ProjectileShape::CustomBlueprint => "BLUEPRINT",
            ProjectileShape::Sphere => "SPHERE",
            ProjectileShape::Ellipse => "ELLIPSE",
            ProjectileShape::HollowRing => "RING",
            ProjectileShape::BluntSlug => "SLUG",
            ProjectileShape::Wedge => "WEDGE",
            ProjectileShape::LongRodDart => "ROD",
            ProjectileShape::DifferentiatedAsteroid => "ASTEROID",
            ProjectileShape::ShapedJet => "JET",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        match s {
            "BLUEPRINT" => Some(ProjectileShape::CustomBlueprint),
            "SPHERE" => Some(ProjectileShape::Sphere),
            "ELLIPSE" => Some(ProjectileShape::Ellipse),
            "RING" => Some(ProjectileShape::HollowRing),
            "SLUG" => Some(ProjectileShape::BluntSlug),
            "WEDGE" => Some(ProjectileShape::Wedge),
            "ROD" => Some(ProjectileShape::LongRodDart),
            "ASTEROID" => Some(ProjectileShape::DifferentiatedAsteroid),
            "JET" => Some(ProjectileShape::ShapedJet),
            _ => None,
        }
    }
}

#[inline]
fn hash_jitter(seed: u64) -> Vec2 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    let fx = ((z & 0xFFFF) as f32 / 65535.0) - 0.5;
    let fy = (((z >> 16) & 0xFFFF) as f32 / 65535.0) - 0.5;
    Vec2::new(fx, fy)
}

/// Spawn an oriented rectangular solid block/plate with hexagonal close-packed continuum particles.
#[allow(clippy::too_many_arguments)]
pub fn spawn_oriented_box(
    world: &mut ContinuumWorld,
    center: Vec2,
    half_extents: Vec2,
    angle_rad: f32,
    vel: Vec2,
    mat: MaterialId,
    weld_existing_body: Option<u16>,
    temp_override: Option<f32>,
) -> u16 {
    let body_id = weld_existing_body.unwrap_or_else(|| world.alloc_body_id());
    let start_idx = world.len();
    let dx = world.dx;
    let dy = dx * 0.866_025_4;
    let rot = Vec2::from_angle(angle_rad);

    let nx = ((2.0 * half_extents.x) / dx).floor() as i32;
    let ny = ((2.0 * half_extents.y) / dy).floor() as i32;

    for iy in 0..=ny {
        let row_offset = if iy.rem_euclid(2) == 1 { 0.5 * dx } else { 0.0 };
        let ly = -half_extents.y + iy as f32 * dy;
        if ly > half_extents.y + 0.01 * dx {
            continue;
        }
        for ix in 0..=nx {
            let lx = -half_extents.x + ix as f32 * dx + row_offset;
            if lx > half_extents.x - 0.05 * dx {
                continue;
            }
            let is_edge = ix == 0 || ix >= nx - 1 || iy == 0 || iy >= ny - 1;
            let j_amp = if is_edge { 0.035 * dx } else { 0.16 * dx };
            let jit = hash_jitter(((ix as u64) << 20) ^ (iy as u64) ^ ((body_id as u64) << 40))
                * j_amp;
            let local = Vec2::new(lx, ly) + jit;
            let world_p = center + rot.rotate(local);
            world.push_particle(world_p, vel, mat, body_id, temp_override);
        }
    }

    let end_idx = world.len();
    if weld_existing_body.is_none() {
        world.bond_range(start_idx, end_idx, false);
    }
    body_id
}

/// Spawn a multi-layer welded composite armor stack centered at `center`.
pub fn spawn_composite_plate(
    world: &mut ContinuumWorld,
    center: Vec2,
    height: f32,
    normal_angle_rad: f32,
    vel: Vec2,
    layers: &[(f32, MaterialId)],
    temp_override: Option<f32>,
) {
    let start_idx = world.len();
    let dx = world.dx;
    let dy = dx * 0.866_025_4;
    let total_thick: f32 = layers.iter().map(|(t, _)| *t).sum();
    let rot = Vec2::from_angle(normal_angle_rad);
    let front_center = center - rot.rotate(Vec2::new(0.5 * total_thick, 0.0));

    let nx = (total_thick / dx).round() as i32;
    let ny = (height / dy).round() as i32;

    let body_id = world.alloc_body_id();
    let layer_bodies: Vec<(f32, f32, MaterialId, u16)> = {
        let mut acc = 0.0f32;
        let mut out = Vec::with_capacity(layers.len());
        for &(t, m) in layers {
            out.push((acc, acc + t, m, body_id));
            acc += t;
        }
        out
    };

    for iy in 0..=ny {
        let row_offset = if iy.rem_euclid(2) == 1 { 0.5 * dx } else { 0.0 };
        let ly = -0.5 * height + iy as f32 * dy;
        for ix in 0..=nx {
            let lx = ix as f32 * dx + row_offset;
            if lx > total_thick {
                continue;
            }
            let mut chosen = layer_bodies[layer_bodies.len() - 1];
            for &seg in &layer_bodies {
                if lx >= seg.0 && lx <= seg.1 + 1e-4 {
                    chosen = seg;
                    break;
                }
            }
            let jit = hash_jitter(((ix as u64) << 18) ^ (iy as u64) ^ 0xA5A5) * (0.015 * dx);
            let local = Vec2::new(lx, ly) + jit;
            let world_p = front_center + rot.rotate(local);
            world.push_particle(world_p, vel, chosen.2, chosen.3, temp_override);
        }
    }

    let end_idx = world.len();
    world.bond_range(start_idx, end_idx, true);
}

/// Spawn a solid disk / sphere (or differentiated multi-shell planet/asteroid).
pub fn spawn_layered_disk(
    world: &mut ContinuumWorld,
    center: Vec2,
    shells: &[(f32, MaterialId, Option<f32>)], // sorted inner -> outer radius: (outer_r, mat, temp)
    vel: Vec2,
) {
    let Some(&(max_r, _, _)) = shells.last() else {
        return;
    };
    let start_idx = world.len();
    let body_id = world.alloc_body_id();
    let dx = world.dx;
    let dy = dx * 0.866_025_4;

    let nx = ((2.0 * max_r) / dx).ceil() as i32;
    let ny = ((2.0 * max_r) / dy).ceil() as i32;

    for iy in -ny..=ny {
        let row_offset = if iy.rem_euclid(2) == 1 { 0.5 * dx } else { 0.0 };
        let ly = iy as f32 * dy;
        for ix in -nx..=nx {
            let lx = ix as f32 * dx + row_offset;
            let jit = hash_jitter(((ix + 4096) as u64) << 16 ^ ((iy + 4096) as u64)) * (0.025 * dx);
            let local = Vec2::new(lx, ly) + jit;
            let r = local.length();
            if r <= max_r {
                let mut mat = shells[shells.len() - 1].1;
                let mut temp = shells[shells.len() - 1].2;
                for &(sr, sm, st) in shells {
                    if r <= sr {
                        mat = sm;
                        temp = st;
                        break;
                    }
                }
                world.push_particle(center + local, vel, mat, body_id, temp);
            }
        }
    }

    let end_idx = world.len();
    world.bond_range(start_idx, end_idx, true);
}

#[allow(clippy::too_many_arguments)]
fn spawn_local_shape(
    world: &mut ContinuumWorld,
    center: Vec2,
    half_bounds: Vec2,
    angle_rad: f32,
    vel: Vec2,
    mat: MaterialId,
    temp_override: Option<f32>,
    inside: impl Fn(Vec2) -> bool,
) {
    let start_idx = world.len();
    let body_id = world.alloc_body_id();
    let dx = world.dx;
    let dy = dx * 0.866_025_4;
    let rot = Vec2::from_angle(angle_rad);

    let nx = (half_bounds.x / dx).ceil() as i32;
    let ny = (half_bounds.y / dy).ceil() as i32;

    for iy in -ny..=ny {
        let row_offset = if iy.rem_euclid(2) == 1 { 0.5 * dx } else { 0.0 };
        let ly = iy as f32 * dy;
        for ix in -nx..=nx {
            let lx = ix as f32 * dx + row_offset;
            let jit = hash_jitter(
                ((ix + 4096) as u64) << 16 ^ ((iy + 4096) as u64) ^ ((body_id as u64) << 36),
            ) * (0.025 * dx);
            let local = Vec2::new(lx, ly) + jit;
            if inside(local) {
                world.push_particle(center + rot.rotate(local), vel, mat, body_id, temp_override);
            }
        }
    }

    let end_idx = world.len();
    world.bond_range(start_idx, end_idx, false);
}

/// Spawn a solid ellipse (or circle when `rx == ry`) oriented at `angle_rad`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_ellipse(
    world: &mut ContinuumWorld,
    center: Vec2,
    rx: f32,
    ry: f32,
    angle_rad: f32,
    vel: Vec2,
    mat: MaterialId,
    temp_override: Option<f32>,
) {
    let rx = rx.max(0.6);
    let ry = ry.max(0.6);
    spawn_local_shape(
        world,
        center,
        Vec2::new(rx, ry),
        angle_rad,
        vel,
        mat,
        temp_override,
        |local| (local.x / rx).powi(2) + (local.y / ry).powi(2) <= 1.0,
    );
}

/// Spawn a hollow ring / circular shell tube.
pub fn spawn_hollow_ring(
    world: &mut ContinuumWorld,
    center: Vec2,
    outer_r: f32,
    wall_thick: f32,
    vel: Vec2,
    mat: MaterialId,
    temp_override: Option<f32>,
) {
    let outer_r = outer_r.max(1.2);
    let inner_r = (outer_r - wall_thick.max(world.dx * 1.6)).max(0.0);
    let r_out_sq = outer_r * outer_r;
    let r_in_sq = inner_r * inner_r;
    spawn_local_shape(
        world,
        center,
        Vec2::splat(outer_r),
        0.0,
        vel,
        mat,
        temp_override,
        |local| {
            let r2 = local.length_squared();
            r2 <= r_out_sq && r2 >= r_in_sq
        },
    );
}

/// Spawn an isosceles triangle / wedge pointing toward `+X` at `angle_rad = 0`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_triangle_wedge(
    world: &mut ContinuumWorld,
    center: Vec2,
    width: f32,
    height: f32,
    angle_rad: f32,
    vel: Vec2,
    mat: MaterialId,
    temp_override: Option<f32>,
) {
    let hw = (0.5 * width).max(1.0);
    let hh = (0.5 * height).max(1.0);
    spawn_local_shape(
        world,
        center,
        Vec2::new(hw, hh),
        angle_rad,
        vel,
        mat,
        temp_override,
        |local| {
            if local.x < -hw || local.x > hw {
                return false;
            }
            let s = (local.x + hw) / (2.0 * hw);
            local.y.abs() <= hh * (1.0 - s)
        },
    );
}

/// Spawn a fully parameterized projectile (used by Projectile Builder & SceneSpec).
#[allow(clippy::too_many_arguments)]
pub fn spawn_projectile_custom(
    world: &mut ContinuumWorld,
    shape: ProjectileShape,
    origin: Vec2,
    dir: Vec2,
    vel: Vec2,
    mat: MaterialId,
    core_mat: MaterialId,
    length_cm: f32,
    radius_cm: f32,
    core_frac: f32,
    temp_k: f32,
) {
    let dir = if dir.length_squared() > 1e-8 {
        dir.normalize()
    } else if vel.length_squared() > 1e-8 {
        vel.normalize()
    } else {
        Vec2::X
    };
    let angle = dir.y.atan2(dir.x);
    let rot = Vec2::from_angle(angle);
    let dx = world.dx;
    let dy = dx * 0.866_025_4;
    let start_idx = world.len();
    let body_id = world.alloc_body_id();

    match shape {
        ProjectileShape::DifferentiatedAsteroid => {
            let r_outer = radius_cm.max(1.2);
            let r_core = (r_outer * core_frac.clamp(0.15, 0.85)).max(0.6);
            let core_t = temp_k.max(220.0) + if core_mat == MaterialId::PlanetaryIron { 180.0 } else { 0.0 };
            spawn_layered_disk(
                world,
                origin,
                &[
                    (r_core, core_mat, Some(core_t)),
                    (r_outer, mat, Some(temp_k)),
                ],
                vel,
            );
        }
        ProjectileShape::LongRodDart => {
            let half_len = (0.5 * length_cm).max(2.0);
            let half_rad = radius_cm.max(0.5);
            let nose_start = half_len * 0.58;
            let nx = ((2.2 * half_len) / dx).ceil() as i32;
            let ny = ((4.5 * half_rad) / dy).ceil() as i32;

            for iy in -ny..=ny {
                let row_offset = if iy.rem_euclid(2) == 1 { 0.5 * dx } else { 0.0 };
                let ly = iy as f32 * dy;
                for ix in -nx..=nx {
                    let lx = ix as f32 * dx + row_offset;
                    if lx < -half_len || lx > half_len {
                        continue;
                    }
                    let allowed_r = if lx > nose_start {
                        let t = (lx - nose_start) / (half_len - nose_start);
                        half_rad * (1.0 - 0.82 * t)
                    } else if lx < -half_len * 0.72 {
                        let t = (-lx - half_len * 0.72) / (half_len * 0.28);
                        half_rad * (1.0 + 1.15 * t)
                    } else {
                        half_rad
                    };
                    if ly.abs() <= allowed_r {
                        let jit = hash_jitter(((ix + 2048) as u64) << 14 ^ ((iy + 2048) as u64))
                            * (0.03 * dx);
                        let world_p = origin + rot.rotate(Vec2::new(lx, ly) + jit);
                        world.push_particle(world_p, vel, mat, body_id, Some(temp_k));
                    }
                }
            }
            let end_idx = world.len();
            world.bond_range(start_idx, end_idx, false);
        }
        ProjectileShape::Sphere => {
            spawn_layered_disk(world, origin, &[(radius_cm.max(0.8), mat, Some(temp_k))], vel);
        }
        ProjectileShape::Ellipse => {
            spawn_ellipse(
                world,
                origin,
                (0.5 * length_cm).max(1.0),
                radius_cm.max(0.8),
                angle,
                vel,
                mat,
                Some(temp_k),
            );
        }
        ProjectileShape::HollowRing => {
            let outer_r = radius_cm.max(1.8);
            let wall = (outer_r * core_frac.clamp(0.15, 0.85)).max(world.dx * 2.0);
            spawn_hollow_ring(world, origin, outer_r, wall, vel, mat, Some(temp_k));
        }
        ProjectileShape::BluntSlug => {
            spawn_oriented_box(
                world,
                origin,
                Vec2::new((0.5 * length_cm).max(1.0), radius_cm.max(0.8)),
                angle,
                vel,
                mat,
                None,
                Some(temp_k),
            );
        }
        ProjectileShape::Wedge => {
            spawn_triangle_wedge(
                world,
                origin,
                length_cm.max(2.0),
                (2.0 * radius_cm).max(2.0),
                angle,
                vel,
                mat,
                Some(temp_k),
            );
        }
        ProjectileShape::CustomBlueprint => {
            let bp = BuiltinProjectileBp::HeatWarhead.build();
            bp.spawn_into(world, origin, angle, vel, Some(temp_k));
        }
        ProjectileShape::ShapedJet => {
            let half_len = (0.5 * length_cm).max(3.0);
            let base_rad = radius_cm.max(0.45);
            let v_mag = vel.length().max(0.25);
            let nx = ((2.0 * half_len) / dx).ceil() as i32;
            let ny = ((3.5 * base_rad) / dy).ceil() as i32;

            for iy in -ny..=ny {
                let row_offset = if iy.rem_euclid(2) == 1 { 0.5 * dx } else { 0.0 };
                let ly = iy as f32 * dy;
                for ix in -nx..=nx {
                    let lx = ix as f32 * dx + row_offset;
                    if lx < -half_len || lx > half_len {
                        continue;
                    }
                    let s = (lx + half_len) / (2.0 * half_len);
                    let jet_r = base_rad * (1.45 - 0.65 * s);
                    if ly.abs() <= jet_r {
                        let local_v = dir * (v_mag * (0.48 + 0.52 * s));
                        let world_p = origin + rot.rotate(Vec2::new(lx, ly));
                        world.push_particle(world_p, local_v, mat, body_id, Some(temp_k));
                    }
                }
            }
            let end_idx = world.len();
            world.bond_range(start_idx, end_idx, false);
        }
    }
}

/// Convert any built-in scenario preset into an editable `SceneSpec`.
pub fn preset_to_scene(preset: ScenarioPreset) -> SceneSpec {
    let mut scene = SceneSpec::empty();

    match preset {
        ScenarioPreset::HeatVsRhaArmor => {
            scene.targets.push(TargetSpec {
                name: "Deep RHA Steel Armor".to_string(),
                pos: Vec2::new(14.5, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 16.0,
                    height: 50.0,
                    mat: MaterialId::RhaSteel,
                },
            });
            scene.targets.push(TargetSpec {
                name: "Crew Aluminum Witness Plate".to_string(),
                pos: Vec2::new(31.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 2.0,
                    height: 48.0,
                    mat: MaterialId::Aluminum,
                },
            });
            let bp = BuiltinProjectileBp::HeatWarhead.build();
            let proj = ProjectileSpec::from_blueprint(
                Vec2::new(-9.5, 0.0),
                BuiltinProjectileBp::HeatWarhead.default_speed_kms(),
                0.0,
                bp,
            );
            scene.projectiles.push(proj);
        }

        ScenarioPreset::ApfsdsVsKontakt5Era => {
            let hull_bp = BuiltinTargetBp::Kontakt5EraHull.build();
            scene
                .targets
                .push(TargetSpec::from_blueprint(Vec2::new(14.0, 0.0), hull_bp));

            let dart_bp = BuiltinProjectileBp::ApfsdsTungsten.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-12.5, 2.0),
                BuiltinProjectileBp::ApfsdsTungsten.default_speed_kms(),
                0.0,
                dart_bp,
            ));
        }

        ScenarioPreset::TandemHeatVsEra => {
            let hull_bp = BuiltinTargetBp::Kontakt5EraHull.build();
            scene
                .targets
                .push(TargetSpec::from_blueprint(Vec2::new(15.5, 0.0), hull_bp));

            let tandem_bp = BuiltinProjectileBp::TandemHeat.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-16.2, 2.0),
                BuiltinProjectileBp::TandemHeat.default_speed_kms(),
                0.0,
                tandem_bp,
            ));
        }

        ScenarioPreset::ApheTankAmmoRack => {
            let tank_bp = BuiltinTargetBp::TankHullAmmoRack.build();
            scene
                .targets
                .push(TargetSpec::from_blueprint(Vec2::new(16.0, 0.0), tank_bp));

            let aphe_bp = BuiltinProjectileBp::ApheDelayShell.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-13.2, 0.0),
                1.22,
                0.0,
                aphe_bp,
            ));
        }

        ScenarioPreset::HeshSquashSpall => {
            scene.targets.push(TargetSpec {
                name: "18° Sloped RHA Turret Wall".to_string(),
                pos: Vec2::new(8.5, 0.0),
                angle_deg: -18.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 9.0,
                    height: 50.0,
                    mat: MaterialId::RhaSteel,
                },
            });
            scene.targets.push(TargetSpec {
                name: "Interior Crew Witness Plate".to_string(),
                pos: Vec2::new(25.5, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 1.8,
                    height: 46.0,
                    mat: MaterialId::Aluminum,
                },
            });
            let hesh_bp = BuiltinProjectileBp::HeshSquashHead.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-6.2, 0.0),
                BuiltinProjectileBp::HeshSquashHead.default_speed_kms(),
                0.0,
                hesh_bp,
            ));
        }

        ScenarioPreset::ApfsdsVsChobhamNera => {
            let cheek_bp = BuiltinTargetBp::ChobhamNeraCheek.build();
            scene
                .targets
                .push(TargetSpec::from_blueprint(Vec2::new(15.0, 0.0), cheek_bp));

            let du_bp = BuiltinProjectileBp::ApfsdsDepletedU.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-13.2, 0.0),
                BuiltinProjectileBp::ApfsdsDepletedU.default_speed_kms(),
                0.0,
                du_bp,
            ));
        }

        ScenarioPreset::ApheVsConcreteBunker => {
            let bunker_bp = BuiltinTargetBp::ConcreteBunker.build();
            scene
                .targets
                .push(TargetSpec::from_blueprint(Vec2::new(13.0, 0.0), bunker_bp));

            let aphe_bp = BuiltinProjectileBp::ApheDelayShell.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-9.8, 0.0),
                1.08,
                0.0,
                aphe_bp,
            ));
        }

        ScenarioPreset::HypervelocityWhipple => {
            scene.targets.push(TargetSpec {
                name: "Al 7075 Whipple Bumper".to_string(),
                pos: Vec2::new(-8.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 1.7,
                    height: 58.0,
                    mat: MaterialId::Aluminum,
                },
            });
            scene.targets.push(TargetSpec {
                name: "Kevlar + Al Hull Wall".to_string(),
                pos: Vec2::new(22.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::CompositePlate {
                    height: 56.0,
                    layers: vec![
                        ArmorLayer {
                            thickness: 1.8,
                            mat: MaterialId::AramidLiner,
                        },
                        ArmorLayer {
                            thickness: 2.2,
                            mat: MaterialId::Aluminum,
                        },
                    ],
                },
            });
            let mut sphere = ProjectileSpec::default_with_shape(
                ProjectileShape::Sphere,
                Vec2::new(-16.0, 0.0),
                MaterialId::Aluminum,
            );
            sphere.name = "Orbital Debris Sphere".to_string();
            sphere.speed_kms = 7.20;
            sphere.radius_cm = 3.78;
            scene.projectiles.push(sphere);
        }

        ScenarioPreset::PlanetaryImpact => {
            scene.self_gravity = true;
            scene.grav_g = 7.5e-5;
            scene.targets.push(TargetSpec {
                name: "Proto-Planet Target".to_string(),
                pos: Vec2::new(8.0, -2.0),
                angle_deg: 0.0,
                vel: Vec2::new(-0.015, 0.0),
                temp_k: 520.0,
                kind: TargetKind::LayeredPlanet {
                    shells: vec![
                        PlanetShell {
                            thickness: 9.5,
                            mat: MaterialId::PlanetaryIron,
                            temp_k: 1150.0,
                        },
                        PlanetShell {
                            thickness: 11.0,
                            mat: MaterialId::BasaltRock,
                            temp_k: 520.0,
                        },
                        PlanetShell {
                            thickness: 1.7,
                            mat: MaterialId::WaterIce,
                            temp_k: 155.0,
                        },
                    ],
                },
            });
            let mut ast = ProjectileSpec::default_with_shape(
                ProjectileShape::DifferentiatedAsteroid,
                Vec2::new(-21.5, 9.5),
                MaterialId::BasaltRock,
            );
            ast.name = "Iron-Core Asteroid".to_string();
            ast.core_mat = MaterialId::PlanetaryIron;
            ast.radius_cm = 6.8;
            ast.core_frac = 0.47;
            ast.temp_k = 260.0;
            ast.set_vel_cm_per_us(Vec2::new(0.54, -0.14));
            scene.projectiles.push(ast);
        }

        ScenarioPreset::IcyCometCollision => {
            scene.self_gravity = true;
            scene.grav_g = 6.5e-5;
            scene.targets.push(TargetSpec {
                name: "Silicate-Iron Moon".to_string(),
                pos: Vec2::new(9.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::new(-0.010, 0.0),
                temp_k: 340.0,
                kind: TargetKind::LayeredPlanet {
                    shells: vec![
                        PlanetShell {
                            thickness: 7.0,
                            mat: MaterialId::PlanetaryIron,
                            temp_k: 880.0,
                        },
                        PlanetShell {
                            thickness: 12.5,
                            mat: MaterialId::BasaltRock,
                            temp_k: 340.0,
                        },
                    ],
                },
            });
            let mut comet = ProjectileSpec::default_with_shape(
                ProjectileShape::DifferentiatedAsteroid,
                Vec2::new(-19.5, 4.0),
                MaterialId::WaterIce,
            );
            comet.name = "Cryogenic Ice Comet".to_string();
            comet.core_mat = MaterialId::BasaltRock;
            comet.radius_cm = 6.6;
            comet.core_frac = 0.33;
            comet.temp_k = 145.0;
            comet.set_vel_cm_per_us(Vec2::new(0.67, -0.08));
            scene.projectiles.push(comet);
        }

        ScenarioPreset::LowSpeedPlasticRicochet => {
            scene.targets.push(TargetSpec {
                name: "38° Sloped RHA Plate".to_string(),
                pos: Vec2::new(4.0, -2.0),
                angle_deg: -38.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 4.4,
                    height: 52.0,
                    mat: MaterialId::RhaSteel,
                },
            });
            let mut slug = ProjectileSpec::default_with_shape(
                ProjectileShape::BluntSlug,
                Vec2::new(-9.0, 2.5),
                MaterialId::Tungsten,
            );
            slug.name = "Sub-Hydrodynamic Slug".to_string();
            slug.length_cm = 10.1;
            slug.radius_cm = 2.94;
            slug.set_vel_cm_per_us(Vec2::new(0.045, -0.004));
            scene.projectiles.push(slug);
        }

        ScenarioPreset::EfpVsSpacedArmor => {
            scene.targets.push(TargetSpec {
                name: "Al 7075 Standoff Slat Screen".to_string(),
                pos: Vec2::new(6.8, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 1.5,
                    height: 44.0,
                    mat: MaterialId::Aluminum,
                },
            });
            scene.targets.push(TargetSpec {
                name: "Sloped RHA Roof Armor".to_string(),
                pos: Vec2::new(22.0, 0.0),
                angle_deg: 18.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::CompositePlate {
                    height: 48.0,
                    layers: vec![
                        ArmorLayer {
                            thickness: 4.2,
                            mat: MaterialId::RhaSteel,
                        },
                        ArmorLayer {
                            thickness: 1.6,
                            mat: MaterialId::AramidLiner,
                        },
                    ],
                },
            });
            scene.targets.push(TargetSpec {
                name: "Interior Witness Bulkhead".to_string(),
                pos: Vec2::new(33.5, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 1.6,
                    height: 44.0,
                    mat: MaterialId::MildSteel,
                },
            });
            let efp_bp = BuiltinProjectileBp::EfpDishWarhead.build();
            scene.projectiles.push(ProjectileSpec::from_blueprint(
                Vec2::new(-8.5, 0.0),
                BuiltinProjectileBp::EfpDishWarhead.default_speed_kms(),
                0.0,
                efp_bp,
            ));
        }

        ScenarioPreset::RocheLimitRingFormation => {
            scene.self_gravity = true;
            scene.grav_g = 9.0e-5;
            scene.targets.push(TargetSpec {
                name: "Super-Planet Core".to_string(),
                pos: Vec2::new(0.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 520.0,
                kind: TargetKind::LayeredPlanet {
                    shells: vec![
                        PlanetShell {
                            thickness: 9.0,
                            mat: MaterialId::PlanetaryIron,
                            temp_k: 1200.0,
                        },
                        PlanetShell {
                            thickness: 9.0,
                            mat: MaterialId::BasaltRock,
                            temp_k: 520.0,
                        },
                    ],
                },
            });
            let mut moon = ProjectileSpec::default_with_shape(
                ProjectileShape::DifferentiatedAsteroid,
                Vec2::new(0.0, 26.5),
                MaterialId::WaterIce,
            );
            moon.name = "Roche Icy Moonlet".to_string();
            moon.core_mat = MaterialId::BasaltRock;
            moon.radius_cm = 5.2;
            moon.core_frac = 0.25;
            moon.temp_k = 145.0;
            scene.projectiles.push(moon);
            if let Some((spd, ang)) = scene.compute_orbit_velocity(0, false, 0.94) {
                scene.projectiles[0].speed_kms = spd;
                scene.projectiles[0].angle_deg = ang;
            }
        }

        ScenarioPreset::BinaryPlanetInspiral => {
            scene.self_gravity = true;
            scene.grav_g = 8.5e-5;
            scene.targets.push(TargetSpec {
                name: "Proto-Planet Alpha".to_string(),
                pos: Vec2::new(-14.5, -2.5),
                angle_deg: 0.0,
                vel: Vec2::new(0.025, 0.055),
                temp_k: 450.0,
                kind: TargetKind::LayeredPlanet {
                    shells: vec![
                        PlanetShell {
                            thickness: 6.5,
                            mat: MaterialId::PlanetaryIron,
                            temp_k: 1050.0,
                        },
                        PlanetShell {
                            thickness: 8.5,
                            mat: MaterialId::BasaltRock,
                            temp_k: 450.0,
                        },
                    ],
                },
            });
            scene.targets.push(TargetSpec {
                name: "Proto-Planet Beta".to_string(),
                pos: Vec2::new(15.0, 2.5),
                angle_deg: 0.0,
                vel: Vec2::new(-0.025, -0.068),
                temp_k: 430.0,
                kind: TargetKind::LayeredPlanet {
                    shells: vec![
                        PlanetShell {
                            thickness: 5.5,
                            mat: MaterialId::PlanetaryIron,
                            temp_k: 950.0,
                        },
                        PlanetShell {
                            thickness: 7.5,
                            mat: MaterialId::BasaltRock,
                            temp_k: 430.0,
                        },
                    ],
                },
            });
        }

        ScenarioPreset::KineticRodBunkerBuster => {
            scene.targets.push(TargetSpec {
                name: "Reinforced Vault Overburden".to_string(),
                pos: Vec2::new(8.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::CompositePlate {
                    height: 46.0,
                    layers: vec![
                        ArmorLayer {
                            thickness: 7.5,
                            mat: MaterialId::Concrete,
                        },
                        ArmorLayer {
                            thickness: 1.5,
                            mat: MaterialId::MildSteel,
                        },
                        ArmorLayer {
                            thickness: 6.5,
                            mat: MaterialId::Concrete,
                        },
                        ArmorLayer {
                            thickness: 2.2,
                            mat: MaterialId::RhaSteel,
                        },
                        ArmorLayer {
                            thickness: 1.4,
                            mat: MaterialId::AramidLiner,
                        },
                    ],
                },
            });
            scene.targets.push(TargetSpec {
                name: "Underground Munitions Cache".to_string(),
                pos: Vec2::new(23.5, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::SolidBox {
                    width: 5.0,
                    height: 22.0,
                    mat: MaterialId::CompB,
                },
            });
            let mut rod = ProjectileSpec::default_with_shape(
                ProjectileShape::LongRodDart,
                Vec2::new(-16.0, 0.0),
                MaterialId::Tungsten,
            );
            rod.name = "Heavy Bunker-Buster Rod".to_string();
            rod.length_cm = 18.0;
            rod.radius_cm = 1.65;
            rod.speed_kms = 2.40;
            rod.angle_deg = 0.0;
            scene.projectiles.push(rod);
        }

        ScenarioPreset::OceanMoonHyperImpact => {
            scene.self_gravity = true;
            scene.grav_g = 7.5e-5;
            scene.targets.push(TargetSpec {
                name: "Ocean-Moon Target".to_string(),
                pos: Vec2::new(6.0, 0.0),
                angle_deg: 0.0,
                vel: Vec2::ZERO,
                temp_k: 293.0,
                kind: TargetKind::LayeredPlanet {
                    shells: vec![
                        PlanetShell {
                            thickness: 6.5,
                            mat: MaterialId::PlanetaryIron,
                            temp_k: 1050.0,
                        },
                        PlanetShell {
                            thickness: 7.0,
                            mat: MaterialId::Granite,
                            temp_k: 450.0,
                        },
                        PlanetShell {
                            thickness: 5.5,
                            mat: MaterialId::LiquidWater,
                            temp_k: 280.0,
                        },
                        PlanetShell {
                            thickness: 2.2,
                            mat: MaterialId::WaterIce,
                            temp_k: 145.0,
                        },
                    ],
                },
            });
            let mut imp = ProjectileSpec::default_with_shape(
                ProjectileShape::Sphere,
                Vec2::new(-22.5, 5.0),
                MaterialId::TungstenCarbide,
            );
            imp.name = "9.2 km/s Hyper-Impactor".to_string();
            imp.radius_cm = 4.2;
            imp.temp_k = 310.0;
            imp.set_vel_cm_per_us(Vec2::new(0.91, -0.14));
            scene.projectiles.push(imp);
        }

        ScenarioPreset::EmptySpace => {}
    }

    scene
}

/// Build a full preset world at chosen particle spacing `dx`.
pub fn build_scenario(preset: ScenarioPreset, dx: f32) -> ContinuumWorld {
    preset_to_scene(preset).build_world(dx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::PixelBlueprint;

    #[test]
    fn empty_space_starts_empty() {
        let world = build_scenario(ScenarioPreset::EmptySpace, 0.35);
        assert_eq!(world.len(), 0);
        assert_eq!(world.fractured_count, 0);
    }

    #[test]
    fn unbounded_space_has_no_borders() {
        // Spawn a target + projectile far outside the old (-65..65, -42..42) arena
        let mut world = ContinuumWorld::new(0.4);
        let far = Vec2::new(15_000.0, -9_000.0);
        spawn_oriented_box(
            &mut world,
            far + Vec2::new(4.0, 0.0),
            Vec2::new(2.0, 6.0),
            0.0,
            Vec2::ZERO,
            MaterialId::RhaSteel,
            None,
            None,
        );
        spawn_projectile_custom(
            &mut world,
            ProjectileShape::Sphere,
            far - Vec2::new(5.0, 0.0),
            Vec2::X,
            Vec2::new(0.45, 0.0),
            MaterialId::Tungsten,
            MaterialId::PlanetaryIron,
            5.8,
            2.9,
            0.45,
            293.0,
        );
        for _ in 0..250 {
            let dt = world.compute_cfl_dt();
            world.step(dt);
        }
        // Impact occurred at ~15000 cm and debris/penetrator kept moving freely past the plate with zero wall clamp
        let max_x = world.pos.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let max_ep = world
            .plastic_strain
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        assert!(max_x > far.x + 6.5);
        assert!(max_ep > 0.05);
    }

    #[test]
    fn heat_warhead_detonates_and_forms_hypervelocity_copper_jet() {
        let mut world = build_scenario(ScenarioPreset::HeatVsRhaArmor, 0.35);
        let mut peak_cu_vx = 0.0f32;
        for _ in 0..1450 {
            let dt = world.compute_cfl_dt();
            world.step(dt);
            for (i, m) in world.mat.iter().enumerate() {
                if *m == MaterialId::Copper && world.vel[i].x > peak_cu_vx {
                    peak_cu_vx = world.vel[i].x;
                }
            }
        }
        // Comp-B charge + ImpactFuze must have detonated after impact!
        assert!(
            world.detonated_count > 120,
            "Expected Comp-B detonation, got {}",
            world.detonated_count
        );
        // Emergent Munroe-effect collapse: warhead launched at 0.32 km/s (0.032 cm/us)
        // must accelerate OFHC Copper liner jet particles above 4.5 km/s (> 0.45 cm/us)!
        assert!(
            peak_cu_vx > 0.45,
            "Expected emergent hypervelocity copper Munroe jet (>4.5 km/s), got {:.2} km/s",
            peak_cu_vx * 10.0
        );
    }

    #[test]
    fn blueprint_serialize_roundtrip_preserves_cells() {
        let bp = BuiltinProjectileBp::ApheDelayShell.build();
        let text = bp.serialize_text();
        let loaded = PixelBlueprint::deserialize_text(&text).expect("valid blueprint text");
        assert_eq!(bp, loaded);
    }

    #[test]
    fn era_and_delay_fuze_warheads_detonate_cleanly() {
        for (preset, steps, min_det) in [
            (ScenarioPreset::ApfsdsVsKontakt5Era, 650, 60),
            (ScenarioPreset::ApheTankAmmoRack, 750, 80),
            (ScenarioPreset::HeshSquashSpall, 900, 80),
        ] {
            let mut world = build_scenario(preset, 0.35);
            for _ in 0..steps {
                let dt = world.compute_cfl_dt();
                world.step(dt);
            }
            let armed = world.det_state.iter().filter(|&&s| s == 1).count();
            let dfuze_max_v = world
                .mat
                .iter()
                .enumerate()
                .filter(|(_, m)| **m == MaterialId::DelayFuze)
                .map(|(i, _)| world.vel[i].x)
                .fold(0.0f32, f32::max);
            assert!(
                world.detonated_count >= min_det,
                "{:?}: t={:.1} us, armed={}, dfuze_vx={:.4}, expected >= {} detonated, got {}",
                preset,
                world.time_us,
                armed,
                dfuze_max_v,
                min_det,
                world.detonated_count
            );
        }
    }

    #[test]
    fn collision_invariants_and_bounded_cells() {
        for preset in [
            ScenarioPreset::HypervelocityWhipple,
            ScenarioPreset::PlanetaryImpact,
            ScenarioPreset::ApfsdsVsKontakt5Era,
        ] {
            let mut world = build_scenario(preset, 0.35);
            for _ in 0..700 {
                let dt = world.compute_cfl_dt();
                world.step(dt);
            }
            let any_nan = world
                .pos
                .iter()
                .any(|p| !p.x.is_finite() || !p.y.is_finite())
                || world
                    .vel
                    .iter()
                    .any(|v| !v.x.is_finite() || !v.y.is_finite());
            assert!(!any_nan);
            assert!(world.debug_max_cell_count() < 32);
        }
    }

    #[test]
    fn vulkan_gpu_and_ductile_metals_do_not_disintegrate_on_impact() {
        let mut vk_engine =
            crate::vulkan_gpu::VulkanPhysicsEngine::new().expect("Vulkan GPU engine should init");
        assert!(!vk_engine.device_name.is_empty());

        // 1. Sub-hydrodynamic impact on Vulkan GPU: metal bends plastically with ZERO disintegration!
        let mut world_dent = build_scenario(ScenarioPreset::LowSpeedPlasticRicochet, 0.35);
        for _ in 0..45 {
            let dt = world_dent.compute_cfl_dt();
            vk_engine.step_batch(&mut world_dent, 10, dt);
        }
        let max_ep = world_dent
            .plastic_strain
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        assert!(
            max_ep > 0.02,
            "Expected permanent plastic denting on Vulkan GPU, got max_ep={max_ep}"
        );
        assert_eq!(
            world_dent.fractured_count, 0,
            "Ductile metals should bend/dent without disintegrating on sub-spall impact!"
        );

        // 2. Reactive HEAT warhead on Vulkan GPU: emergent hypervelocity copper jet + intact solid RHA!
        let mut world_heat = build_scenario(ScenarioPreset::HeatVsRhaArmor, 0.35);
        let mut peak_cu_vx = 0.0f32;
        for _ in 0..145 {
            let dt = world_heat.compute_cfl_dt();
            vk_engine.step_batch(&mut world_heat, 10, dt);
            for (i, m) in world_heat.mat.iter().enumerate() {
                if *m == MaterialId::Copper && world_heat.vel[i].x > peak_cu_vx {
                    peak_cu_vx = world_heat.vel[i].x;
                }
            }
        }
        assert!(world_heat.detonated_count > 120);
        assert!(
            peak_cu_vx > 0.45,
            "Expected Vulkan GPU Munroe copper jet > 4.5 km/s, got {:.2} km/s",
            peak_cu_vx * 10.0
        );
        let intact_rha = world_heat
            .mat
            .iter()
            .enumerate()
            .filter(|(i, m)| **m == MaterialId::RhaSteel && world_heat.damage[*i] < 0.25)
            .count();
        let total_rha = world_heat
            .mat
            .iter()
            .filter(|m| **m == MaterialId::RhaSteel)
            .count();
        assert!(
            intact_rha * 100 > total_rha * 85,
            "Expected RHA steel block to remain coherent solid metal ({intact_rha}/{total_rha} intact)"
        );
    }

    #[test]
    fn efp_warhead_gpu_gravity_parity_and_timeline_scrub() {
        let mut vk_engine =
            crate::vulkan_gpu::VulkanPhysicsEngine::new().expect("Vulkan GPU engine should init");

        // 1. EFP Self-Forging Dish Liner vs RHA Armor (1 cm air gap so it flies cleanly intact before striking!)
        let mut efp_scene = SceneSpec::empty();
        efp_scene
            .targets
            .push(TargetSpec::default_box(Vec2::new(22.0, 0.0), MaterialId::RhaSteel));
        efp_scene.projectiles.push(ProjectileSpec::from_blueprint(
            Vec2::new(2.2, 0.0),
            BuiltinProjectileBp::EfpDishWarhead.default_speed_kms(),
            0.0,
            BuiltinProjectileBp::EfpDishWarhead.build(),
        ));
        let mut efp_world = efp_scene.build_world(0.36);
        let mut timeline = crate::analysis::TimelineRecorder::new();
        timeline.reset_with_world(&efp_world);
        let init_pos0 = efp_world.pos[0];

        let mut peak_efp_vx = 0.0f32;
        for _ in 0..140 {
            let dt = efp_world.compute_cfl_dt();
            vk_engine.step_batch(&mut efp_world, 10, dt);
            timeline.maybe_record(&efp_world);
            for (i, &m) in efp_world.mat.iter().enumerate() {
                if m == MaterialId::Copper && efp_world.vel[i].x > peak_efp_vx {
                    peak_efp_vx = efp_world.vel[i].x;
                }
            }
        }
        assert!(efp_world.detonated_count > 100, "EFP Comp-B must detonate");
        assert!(
            peak_efp_vx > 0.18,
            "EFP copper dish must self-forge > 1.8 km/s, got {:.2} km/s",
            peak_efp_vx * 10.0
        );
        assert!(timeline.keyframes.len() >= 5);

        // Verify TimelineScrubber rewinds cleanly to t=0 and branches cleanly!
        timeline.scrub_to(0, &mut efp_world);
        assert!(efp_world.time_us.abs() < 1e-5);
        assert!((efp_world.pos[0] - init_pos0).length() < 1e-5);
        timeline.commit_branch(&mut efp_world);
        assert_eq!(timeline.keyframes.len(), 1);

        // 2. Verify GPU-Resident 32x32 Self-Gravity vs CPU Rayon Self-Gravity parity!
        let mut w_cpu = build_scenario(ScenarioPreset::IcyCometCollision, 0.46);
        let mut w_gpu = build_scenario(ScenarioPreset::IcyCometCollision, 0.46);
        let dt = 0.04;
        for _ in 0..80 {
            w_cpu.step(dt);
        }
        vk_engine.step_batch(&mut w_gpu, 80, dt);

        let com_v = |w: &ContinuumWorld| -> Vec2 {
            let mut m_tot = 0.0f32;
            let mut p_tot = Vec2::ZERO;
            for i in 0..w.len() {
                m_tot += w.mass[i];
                p_tot += w.vel[i] * w.mass[i];
            }
            p_tot / m_tot.max(1e-5)
        };
        let diff_v = (com_v(&w_cpu) - com_v(&w_gpu)).length();
        assert!(
            diff_v < 0.005,
            "CPU vs GPU self-gravity COM velocity drifted: diff={diff_v:.6}"
        );
    }

    #[test]
    fn rankine_hugoniot_shock_and_tillotson_vapor_eos_accuracy() {
        let mut vk_engine =
            crate::vulkan_gpu::VulkanPhysicsEngine::new().expect("Vulkan GPU engine should init");

        // 1. Symmetric planar RHA Steel flyer-plate impact at 2.0 km/s -> u_p = 1.0 km/s (0.10 cm/us)
        // Analytical Rankine-Hugoniot pressure: P_H = rho0 * u_p * (c0 + s * u_p)
        let rha = MaterialId::RhaSteel.props();
        let up = 0.10f32; // 1.0 km/s
        let p_hugoniot_gpa = rha.rho0 * up * (rha.c0 + rha.hugoniot_s * up) * 100.0; // ~47.6 GPa

        let mut flyer_world = ContinuumWorld::new(0.32);
        spawn_oriented_box(
            &mut flyer_world,
            Vec2::new(-8.2, 0.0),
            Vec2::new(8.0, 14.0),
            0.0,
            Vec2::new(2.0 * up, 0.0),
            MaterialId::RhaSteel,
            None,
            None,
        );
        spawn_oriented_box(
            &mut flyer_world,
            Vec2::new(8.2, 0.0),
            Vec2::new(8.0, 14.0),
            0.0,
            Vec2::ZERO,
            MaterialId::RhaSteel,
            None,
            None,
        );

        let dt = 0.024;
        for _ in 0..15 {
            vk_engine.step_batch(&mut flyer_world, 12, dt);
        }

        // Sample shocked interior plateau inside target plate (0.7 cm < x < 1.55 cm, |y| < 5 cm)
        // Longitudinal Rankine-Hugoniot stress sigma_xx_comp = (-sigma_xx) = sph_pressure - s_xx
        let mut core_sig: Vec<f32> = flyer_world
            .pos
            .iter()
            .enumerate()
            .filter(|(_, p)| p.y.abs() < 5.0 && p.x > 0.7 && p.x < 1.55)
            .map(|(i, _)| (flyer_world.sph_pressure[i] - flyer_world.s_xx[i]) * 100.0)
            .collect();
        core_sig.sort_by(|a, b| a.total_cmp(b));
        assert!(!core_sig.is_empty(), "Planar shock plateau must form");
        let median_gpa = core_sig[core_sig.len() / 2];
        let rel_err = (median_gpa - p_hugoniot_gpa).abs() / p_hugoniot_gpa;
        assert!(
            rel_err < 0.14,
            "Expected Rankine-Hugoniot longitudinal shock ~{p_hugoniot_gpa:.1} GPa, got median {median_gpa:.1} GPa (rel_err={rel_err:.3})"
        );

        // 2. Verify EfpVsSpacedArmor preset waits cleanly across standoff gap, detonates on slat screen, and forges > 2.0 km/s slug
        let mut efp_preset = build_scenario(ScenarioPreset::EfpVsSpacedArmor, 0.35);
        for _ in 0..85 {
            let dt_cfl = efp_preset.compute_cfl_dt();
            vk_engine.step_batch(&mut efp_preset, 12, dt_cfl);
        }
        assert!(efp_preset.detonated_count > 100);
        let max_cu_kms = efp_preset
            .mat
            .iter()
            .enumerate()
            .filter(|(_, m)| **m == MaterialId::Copper)
            .map(|(i, _)| efp_preset.vel[i].length() * 10.0)
            .fold(0.0f32, f32::max);
        assert!(
            max_cu_kms > 1.8,
            "Expected EFP preset copper slug > 1.8 km/s, got {max_cu_kms:.2} km/s"
        );
    }

    #[test]
    fn smaller_particle_hitboxes_and_non_premature_explosive_damage() {
        // 1. Solid collision hitbox radius (r_hit = 0.51*dx, pair cutoff d_hit = 1.02*dx) is strictly
        //    smaller than the ~0.76*dx rendered crumb radius: visibly overlapping rims at 1.08*dx must
        //    produce zero ghost repulsion or fuze trigger (even for fractured debris!), while 0.94*dx triggers contact.
        let dx = 0.35_f32;
        let mut w_far = ContinuumWorld::new(dx);
        w_far.push_particle(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.03, 0.0),
            MaterialId::ImpactFuze,
            1,
            None,
        );
        w_far.push_particle(
            Vec2::new(1.08 * dx, 0.0),
            Vec2::ZERO,
            MaterialId::RhaSteel,
            2,
            None,
        );
        for _ in 0..3 {
            let dt = w_far.compute_cfl_dt();
            w_far.step(dt);
        }
        assert_eq!(w_far.detonated_count, 0, "fuze must not trigger at r=1.08*dx rim overlap");
        assert!(
            (w_far.vel[0].x - 0.03).abs() < 1e-4 && w_far.vel[1].x.abs() < 1e-4,
            "zero ghost contact force expected before hitbox overlap: vel={:?}",
            w_far.vel
        );

        // Fractured same-body debris crumbs sitting 1.08*dx apart must NOT blow each other apart across empty space!
        let mut w_debris = ContinuumWorld::new(dx);
        let d0 = w_debris.push_particle(Vec2::ZERO, Vec2::ZERO, MaterialId::RhaSteel, 1, None);
        let d1 = w_debris.push_particle(
            Vec2::new(1.08 * dx, 0.0),
            Vec2::ZERO,
            MaterialId::RhaSteel,
            1,
            None,
        );
        w_debris.damage[d0] = 1.0;
        w_debris.damage[d1] = 1.0;
        w_debris.rho[d0] *= 1.05;
        w_debris.rho[d1] *= 1.05;
        for _ in 0..5 {
            let dt = w_debris.compute_cfl_dt();
            w_debris.step(dt);
        }
        assert!(
            w_debris.vel[d0].length() < 1e-5 && w_debris.vel[d1].length() < 1e-5,
            "fractured debris outside d_hit must stay compact without ghost repulsion: vel={:?}",
            w_debris.vel
        );

        let mut w_near = ContinuumWorld::new(dx);
        w_near.push_particle(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.10, 0.0),
            MaterialId::ImpactFuze,
            1,
            None,
        );
        w_near.push_particle(
            Vec2::new(0.94 * dx, 0.0),
            Vec2::ZERO,
            MaterialId::RhaSteel,
            2,
            None,
        );
        for _ in 0..40 {
            let dt = w_near.compute_cfl_dt();
            w_near.step(dt);
        }
        assert!(
            w_near.detonated_count >= 1 && w_near.vel[1].x > 1e-4,
            "contact inside r_hit must detonate fuze and transfer impulse: det={}, vel={:?}",
            w_near.detonated_count,
            w_near.vel
        );

        // 2. Shaped-charge HEAT warhead must NOT detonate in free flight, waits cleanly across
        //    its preset standoff gap for nose impact, then jets >2 km/s and craters RHA.
        let mut vk_engine =
            crate::vulkan_gpu::VulkanPhysicsEngine::new().expect("Vulkan GPU engine should init");
        let mut free_heat = ContinuumWorld::new(dx);
        BuiltinProjectileBp::HeatWarhead.build().spawn_into(
            &mut free_heat,
            Vec2::ZERO,
            0.0,
            Vec2::new(0.032, 0.0),
            None,
        );
        for _ in 0..160 {
            let dt = free_heat.compute_cfl_dt();
            vk_engine.step_batch(&mut free_heat, 12, dt);
        }
        assert!(
            free_heat.time_us > 40.0 && free_heat.detonated_count == 0,
            "HEAT warhead must remain 100% un-detonated in free flight (t={:.2}us, det={})",
            free_heat.time_us,
            free_heat.detonated_count
        );

        let mut heat = build_scenario(ScenarioPreset::HeatVsRhaArmor, dx);
        let mut checked_flight = false;
        let mut peak_fast_cu = 0usize;
        for _ in 0..500 {
            let dt = heat.compute_cfl_dt();
            vk_engine.step_batch(&mut heat, 12, dt);
            if !checked_flight && heat.time_us >= 6.0 && heat.time_us < 10.5 {
                checked_flight = true;
                assert_eq!(
                    heat.detonated_count, 0,
                    "HEAT warhead must not detonate prematurely before crossing gap (t={:.2}us)",
                    heat.time_us
                );
            }
            let fast_cu = heat
                .mat
                .iter()
                .enumerate()
                .filter(|(i, m)| **m == MaterialId::Copper && heat.vel[*i].x > 0.20)
                .count();
            peak_fast_cu = peak_fast_cu.max(fast_cu);
        }
        let rha_dmg_heat = heat
            .mat
            .iter()
            .enumerate()
            .filter(|(i, m)| **m == MaterialId::RhaSteel && heat.damage[*i] > 0.5)
            .count();
        assert!(
            checked_flight && heat.detonated_count > 120 && peak_fast_cu >= 25 && rha_dmg_heat > 40,
            "HEAT must detonate on impact, form >2km/s Cu jet, and damage RHA (checked_flight={}, det={}, peak_cu={}, rha_dmg={})",
            checked_flight,
            heat.detonated_count,
            peak_fast_cu,
            rha_dmg_heat
        );

        // 3. Bare Comp-B explosive slug flies cleanly across the air gap, detonates on contact,
        //    and craters/spalls the RHA target plate.
        let mut he = ContinuumWorld::new(dx);
        spawn_oriented_box(
            &mut he,
            Vec2::new(9.0, 0.0),
            Vec2::new(3.0, 18.0),
            0.0,
            Vec2::ZERO,
            MaterialId::RhaSteel,
            None,
            None,
        );
        spawn_projectile_custom(
            &mut he,
            ProjectileShape::BluntSlug,
            Vec2::new(0.5, 0.0),
            Vec2::X,
            Vec2::new(0.12, 0.0),
            MaterialId::CompB,
            MaterialId::CompB,
            5.0,
            2.2,
            0.0,
            293.0,
        );
        let mut checked_air_gap = false;
        for _ in 0..190 {
            let dt = he.compute_cfl_dt();
            vk_engine.step_batch(&mut he, 10, dt);
            if !checked_air_gap && he.time_us >= 15.0 && he.time_us < 22.0 {
                checked_air_gap = true;
                assert_eq!(
                    he.detonated_count, 0,
                    "Comp-B slug must not self-detonate before crossing the 3cm gap (t={:.2}us)",
                    he.time_us
                );
            }
        }
        let rha_dmg_he = he
            .mat
            .iter()
            .enumerate()
            .filter(|(i, m)| **m == MaterialId::RhaSteel && he.damage[*i] > 0.5)
            .count();
        let rha_ep_he = he
            .mat
            .iter()
            .enumerate()
            .filter(|(i, m)| **m == MaterialId::RhaSteel && he.plastic_strain[*i] > 0.05)
            .count();

        assert!(
            checked_air_gap && he.detonated_count > 60 && rha_dmg_he > 80 && rha_ep_he > 200,
            "HE slug should impact cleanly and damage RHA (det={}, rha_dmg={}, rha_ep={})",
            he.detonated_count,
            rha_dmg_he,
            rha_ep_he
        );

        // 4. Tandem HEAT vs Kontakt-5 ERA remains un-detonated during approach (t < 60us) and fires both stages on impact.
        let mut tandem = build_scenario(ScenarioPreset::TandemHeatVsEra, dx);
        let mut checked_tandem_flight = false;
        for _ in 0..320 {
            let dt = tandem.compute_cfl_dt();
            vk_engine.step_batch(&mut tandem, 12, dt);
            if !checked_tandem_flight && tandem.time_us >= 35.0 && tandem.time_us < 62.0 {
                checked_tandem_flight = true;
                assert_eq!(
                    tandem.detonated_count, 0,
                    "Tandem HEAT must not detonate prematurely in approach (t={:.2}us)",
                    tandem.time_us
                );
            }
        }
        assert!(
            checked_tandem_flight && tandem.detonated_count > 800,
            "Tandem HEAT should complete two-stage detonation on impact (det={})",
            tandem.detonated_count
        );
    }

    #[test]
    fn low_speed_planetary_collision_preserves_planet_and_100kms_launch() {
        let mut vk =
            crate::vulkan_gpu::VulkanPhysicsEngine::new().expect("Vulkan GPU engine should init");

        // 1. Equilibrated idle layered planet stays cohesive under self-gravity (no free-fall collapse or fake core melt)
        let mut idle_scene = preset_to_scene(ScenarioPreset::PlanetaryImpact);
        idle_scene.projectiles.clear();
        idle_scene.targets[0].vel = Vec2::ZERO;
        let mut idle = idle_scene.build_world(0.45);
        let idle_n = idle.len().max(1);
        for _ in 0..140 {
            let dt = idle.compute_cfl_dt();
            vk.step_batch(&mut idle, 12, dt);
        }
        let idle_peak_gpa = idle.sph_pressure.iter().copied().fold(0.0f32, f32::max) * 100.0;
        let m_ice = idle.mat.iter().enumerate().filter(|(i, m)| **m == MaterialId::WaterIce && idle.temp[*i] >= m.props().melt_temp).count();
        let m_rock = idle.mat.iter().enumerate().filter(|(i, m)| **m == MaterialId::BasaltRock && idle.temp[*i] >= m.props().melt_temp).count();
        let m_iron = idle.mat.iter().enumerate().filter(|(i, m)| **m == MaterialId::PlanetaryIron && idle.temp[*i] >= m.props().melt_temp).count();
        assert!(
            idle.melted_count == 0 && idle.fractured_count * 100 < idle_n * 5 && idle_peak_gpa < 35.0,
            "Idle self-gravitating planet must remain stable (spall={}/{}, melt={}, peakP={idle_peak_gpa:.1} GPa, m_ice={m_ice}, m_rock={m_rock}, m_iron={m_iron})",
            idle.fractured_count,
            idle_n,
            idle.melted_count
        );

        // 2. Low-speed (0.6 km/s) collision craters locally without disintegrating the whole planet
        let mut low_scene = preset_to_scene(ScenarioPreset::PlanetaryImpact);
        low_scene.projectiles[0].speed_kms = 0.6;
        low_scene.projectiles[0].pos = Vec2::new(-14.5, 4.5);
        low_scene.projectiles[0].angle_deg = -14.0;
        let mut low = low_scene.build_world(0.45);
        let low_n = low.len().max(1);
        for _ in 0..140 {
            let dt = low.compute_cfl_dt();
            vk.step_batch(&mut low, 12, dt);
        }
        let intact_planet = low.damage.iter().filter(|&&d| d < 0.5).count();
        let low_peak_gpa = low.sph_pressure.iter().copied().fold(0.0f32, f32::max) * 100.0;
        assert!(
            intact_planet * 100 > low_n * 55 && low.melted_count < 400 && low_peak_gpa < 60.0,
            "Low-speed planetary impact must not disintegrate whole planet (intact={intact_planet}/{low_n}, melt={}, peakP={low_peak_gpa:.1} GPa)",
            low.melted_count
        );

        // 3. Max launch speed up to 100 km/s (10.0 cm/us) is preserved and stays numerically finite
        let mut hyper_proj = ProjectileSpec::default_with_shape(
            ProjectileShape::Sphere,
            Vec2::new(-24.0, 0.0),
            MaterialId::Tungsten,
        );
        hyper_proj.set_vel_cm_per_us(Vec2::new(10.0, 0.0));
        assert!((hyper_proj.speed_kms - 100.0).abs() < 1e-3);
        let mut hyper_scene = preset_to_scene(ScenarioPreset::EmptySpace);
        hyper_scene.targets.push(TargetSpec {
            name: "Plate".to_string(),
            pos: Vec2::new(4.0, 0.0),
            angle_deg: 0.0,
            vel: Vec2::ZERO,
            temp_k: 293.0,
            kind: TargetKind::SolidBox {
                width: 3.0,
                height: 20.0,
                mat: MaterialId::RhaSteel,
            },
        });
        hyper_scene.projectiles.push(hyper_proj);
        let mut hyper = hyper_scene.build_world(0.40);
        for _ in 0..45 {
            let dt = hyper.compute_cfl_dt();
            vk.step_batch(&mut hyper, 10, dt);
        }
        assert!(
            hyper.pos.iter().all(|p| p.x.is_finite() && p.y.is_finite())
                && hyper.vel.iter().all(|v| v.x.is_finite() && v.y.is_finite()),
            "100 km/s hypervelocity launch must stay finite without NaNs"
        );
    }

    #[test]
    fn radiative_cooling_and_parallel_starfield_render_invariants() {
        let mut vk =
            crate::vulkan_gpu::VulkanPhysicsEngine::new().expect("Vulkan GPU engine should init");

        // 1. Hot exposed silicate droplet cools via Stefan-Boltzmann radiation; cold 150K ice stays 150K.
        let mut w_cpu = ContinuumWorld::new(0.40);
        w_cpu.push_particle(Vec2::ZERO, Vec2::ZERO, MaterialId::BasaltRock, 1, Some(3200.0));
        w_cpu.push_particle(Vec2::new(8.0, 0.0), Vec2::ZERO, MaterialId::WaterIce, 2, None);

        let mut w_gpu = ContinuumWorld::new(0.40);
        w_gpu.push_particle(Vec2::ZERO, Vec2::ZERO, MaterialId::BasaltRock, 1, Some(3200.0));
        w_gpu.push_particle(Vec2::new(8.0, 0.0), Vec2::ZERO, MaterialId::WaterIce, 2, None);

        for _ in 0..40 {
            w_cpu.step(0.05);
        }
        vk.step_batch(&mut w_gpu, 40, 0.05);

        assert!(
            w_cpu.temp[0] < 3190.0 && w_gpu.temp[0] < 3190.0,
            "Surface magma should radiate heat (cpu={:.1}K, gpu={:.1}K)",
            w_cpu.temp[0],
            w_gpu.temp[0]
        );
        assert!(
            (w_cpu.temp[0] - w_gpu.temp[0]).abs() < 2.0,
            "CPU/GPU radiative cooling parity drifted: cpu={:.1}, gpu={:.1}",
            w_cpu.temp[0],
            w_gpu.temp[0]
        );
        assert!(
            (w_cpu.temp[1] - 150.0).abs() < 1e-3 && (w_gpu.temp[1] - 150.0).abs() < 1e-3,
            "Cryogenic 150K water ice must not warm spuriously (cpu={:.2}, gpu={:.2})",
            w_cpu.temp[1],
            w_gpu.temp[1]
        );

        // 2. Parallel band renderer + starfield determinism & rounded molten drops
        let mut canvas = crate::render::PixelCanvas::new();
        let cam = crate::render::Camera2D::default();
        canvas.render_with_opts(&w_gpu, &cam, 320, 240, crate::render::VisMode::MaterialThermal, false, true);
        let first = canvas.pixels.clone();
        canvas.render_with_opts(&w_gpu, &cam, 320, 240, crate::render::VisMode::MaterialThermal, false, true);
        assert_eq!(first, canvas.pixels, "Parallel starfield render must be deterministic");
    }

    #[test]
    fn live_interactive_tools_paint_erase_laser_gravity_and_blast_invariants() {
        let mut world = crate::physics::ContinuumWorld::new(0.45);

        // 1. Matter Brush non-overlapping honeycomb + second-pass idempotency
        let n1 = world.paint_matter_disc(
            Vec2::new(0.0, 0.0),
            2.6,
            MaterialId::RhaSteel,
            Vec2::ZERO,
            Some(293.0),
            1,
        );
        let n2 = world.paint_matter_disc(
            Vec2::new(0.0, 0.0),
            2.6,
            MaterialId::RhaSteel,
            Vec2::ZERO,
            Some(293.0),
            1,
        );
        assert!(n1 >= 60, "Brush disc should populate honeycomb grains (got {n1})");
        assert_eq!(n2, 0, "Brush disc must not double-stack overlapping grains");

        let n_he = world.paint_matter_disc(
            Vec2::new(9.0, 0.0),
            2.2,
            MaterialId::CompB,
            Vec2::ZERO,
            Some(293.0),
            2,
        );
        assert!(n_he >= 40);

        // 2. Witness Gauge pin + Erase Matter + Index Remapping
        let mut gauges = crate::analysis::GaugeBank::new();
        gauges.toggle_or_pin_at(&world, Vec2::new(0.0, 0.0));
        gauges.toggle_or_pin_at(&world, Vec2::new(9.0, 0.0));
        assert_eq!(gauges.gauges.len(), 2);

        let (removed, remap) = world.erase_matter_disc(Vec2::new(0.0, 0.0), 1.3);
        assert!(removed > 5, "Eraser must remove center grains");
        gauges.remap_indices(&remap);
        assert_eq!(
            gauges.gauges.len(),
            1,
            "Erased gauge must drop while downstream gauge survives"
        );
        let survived_idx = gauges.gauges[0].particle_idx;
        assert!(survived_idx < world.len());
        assert_eq!(world.mat[survived_idx], MaterialId::CompB);

        // 3. Front-surface stopping Thermal Laser: heats steel without penetrating behind to CompB
        let initial_he_max_t = world
            .mat
            .iter()
            .zip(world.temp.iter())
            .filter(|(m, _)| **m == MaterialId::CompB)
            .map(|(_, t)| *t)
            .fold(0.0f32, f32::max);
        for _ in 0..15 {
            let hit = world.apply_thermal_laser(
                Vec2::new(-12.0, 0.0),
                Vec2::X,
                1.2,
                6.0,
                0.25,
            );
            assert!(hit.is_some());
            assert!(hit.unwrap().x < 4.0, "Laser must stop on front steel shield");
        }
        let post_shield_he_t = world
            .mat
            .iter()
            .zip(world.temp.iter())
            .filter(|(m, _)| **m == MaterialId::CompB)
            .map(|(_, t)| *t)
            .fold(0.0f32, f32::max);
        assert!(
            (post_shield_he_t - initial_he_max_t).abs() < 1e-3,
            "Front steel must shield downstream CompB from thermal laser"
        );

        // Now shoot directly at CompB (+Y down or erase front steel) to verify laser cook-off!
        let (rem_all_steel, remap2) = world.erase_matter_disc(Vec2::new(0.0, 0.0), 5.0);
        assert!(rem_all_steel > 0);
        gauges.remap_indices(&remap2);
        for _ in 0..45 {
            world.apply_thermal_laser(Vec2::new(-5.0, 0.0), Vec2::X, 1.4, 8.0, 0.30);
        }
        let cooked = world.det_state.iter().filter(|&&d| d != 0).count();
        assert!(cooked > 0, "Thermal laser must cook off directly irradiated CompB");

        // 4. Gravity Vortex pull + Tangential Swirl
        world.apply_gravity_well(Vec2::new(24.0, 0.0), 0.35, 0.15, 0.12);
        let avg_vx: f32 = world.vel.iter().map(|v| v.x).sum::<f32>() / world.len().max(1) as f32;
        assert!(avg_vx > 1e-4, "Gravity well at +X must accelerate matter +X");

        // 5. Shock Blast impulse & damage + clean CPU continuum integration steps
        let blast_hits = world.trigger_shock_blast(Vec2::new(9.0, 0.0), 5.0, 0.35, 3600.0);
        assert!(blast_hits > 10);
        for _ in 0..5 {
            world.step(0.04);
        }
        for i in 0..world.len() {
            assert!(
                world.pos[i].is_finite() && world.vel[i].is_finite(),
                "Live tool mutations must remain numerically finite after integration"
            );
        }
    }

    #[test]
    fn orbital_velocity_trajectory_preview_and_astrophysics_presets_invariants() {
        // 1. Orbit assist computation produces orthogonal circular prograde & retrograde vectors
        let mut sc = preset_to_scene(ScenarioPreset::RocheLimitRingFormation);
        let (spd_pro, ang_pro) = sc
            .compute_orbit_velocity(0, false, 1.0)
            .expect("Self-gravitating scene must solve orbital velocity");
        let (spd_ret, ang_ret) = sc
            .compute_orbit_velocity(0, true, 1.0)
            .expect("Retrograde orbit velocity must exist");
        assert!(spd_pro > 0.1 && (spd_pro - spd_ret).abs() < 1e-3);
        let r_hat = (sc.projectiles[0].pos - sc.targets[0].pos).normalize();
        let v_pro = Vec2::from_angle(ang_pro.to_radians());
        let v_ret = Vec2::from_angle(ang_ret.to_radians());
        assert!(r_hat.dot(v_pro).abs() < 1e-3, "Prograde orbit must be tangential");
        assert!(v_pro.dot(v_ret) < -0.99, "Prograde and retrograde must be opposite");

        // 2. Curved N-body trajectory prediction for circular orbit vs direct radial impact
        sc.projectiles[0].speed_kms = spd_pro;
        sc.projectiles[0].angle_deg = ang_pro;
        let traces_orbit = sc.predict_staging_trajectories();
        assert!(traces_orbit.len() == 2);
        assert!(traces_orbit[1].points.len() > 20);
        assert!(
            traces_orbit[1].impact.is_none(),
            "Circular orbit at r=26.5 around R=18 planet should not impact"
        );

        sc.projectiles[0].set_vel_cm_per_us(Vec2::new(0.0, -0.35));
        let traces_hit = sc.predict_staging_trajectories();
        assert!(
            traces_hit[1].impact.is_some(),
            "Inward dive trajectory must predict impact intersection marker"
        );

        // 3. New presets build cleanly and step with finite states
        for preset in [
            ScenarioPreset::RocheLimitRingFormation,
            ScenarioPreset::BinaryPlanetInspiral,
            ScenarioPreset::KineticRodBunkerBuster,
        ] {
            let mut w = build_scenario(preset, 0.55);
            assert!(w.len() > 200, "{:?} must spawn continuum particles", preset);
            for _ in 0..25 {
                let dt = w.compute_cfl_dt();
                w.step(dt);
            }
            assert!(w.pos.iter().all(|p| p.is_finite()));
        }
    }

    #[test]
    fn thirty_two_materials_thermite_octol_gpu_parity_and_ocean_moon() {
        // 1. All 32 materials roundtrip cleanly by code and index
        assert_eq!(MaterialId::ALL.len(), 32);
        for (idx, m) in crate::materials::MATERIALS.iter().enumerate() {
            let id = MaterialId::from_index(idx);
            assert_eq!(id as usize, idx);
            assert_eq!(MaterialId::from_code(m.code), Some(id));
        }

        // 2. Condensed-phase Thermite exothermic burn CPU vs Vulkan GPU parity
        let make_thermite = || {
            let mut w = ContinuumWorld::new(0.40);
            w.paint_matter_disc(
                Vec2::ZERO,
                1.8,
                MaterialId::ThermitePyro,
                Vec2::ZERO,
                Some(1220.0), // ignited above 1150 K threshold
                1,
            );
            w
        };
        let mut w_cpu = make_thermite();
        let mut w_gpu = make_thermite();
        for _ in 0..64 {
            w_cpu.step(0.12);
        }
        let mut vk_opt = crate::vulkan_gpu::VulkanPhysicsEngine::new().ok();
        if let Some(vk) = &mut vk_opt {
            vk.step_batch(&mut w_gpu, 64, 0.12);
        } else {
            for _ in 0..64 {
                w_gpu.step(0.12);
            }
        }
        let cpu_peak_t: f32 = w_cpu.temp.iter().sum::<f32>() / w_cpu.len() as f32;
        let gpu_peak_t: f32 = w_gpu.temp.iter().sum::<f32>() / w_gpu.len() as f32;
        assert!(
            cpu_peak_t > 2400.0 && gpu_peak_t > 2400.0,
            "Active Thermite burn must exceed 2400 K (cpu={cpu_peak_t:.0}, gpu={gpu_peak_t:.0})"
        );
        assert!(
            (cpu_peak_t - gpu_peak_t).abs() < 35.0,
            "CPU/GPU peak Thermite parity drifted (cpu={cpu_peak_t:.1}, gpu={gpu_peak_t:.1})"
        );

        for _ in 0..16 {
            w_cpu.step(0.12);
        }
        if let Some(vk) = &mut vk_opt {
            vk.step_batch(&mut w_gpu, 16, 0.12);
        } else {
            for _ in 0..16 {
                w_gpu.step(0.12);
            }
        }
        let cpu_avg_t: f32 = w_cpu.temp.iter().sum::<f32>() / w_cpu.len() as f32;
        let gpu_avg_t: f32 = w_gpu.temp.iter().sum::<f32>() / w_gpu.len() as f32;
        assert!(
            cpu_avg_t > 1900.0 && gpu_avg_t > 1900.0,
            "Completed Thermite slag must stay above 1810 K melt point (cpu={cpu_avg_t:.0}, gpu={gpu_avg_t:.0})"
        );
        assert!(
            w_cpu.detonated_count == 0 && w_gpu.detonated_count == 0,
            "Thermite must remain condensed phase (det_state != 2)"
        );
        assert!(
            (cpu_avg_t - gpu_avg_t).abs() < 35.0,
            "CPU/GPU Thermite parity drifted (cpu={cpu_avg_t:.1}, gpu={gpu_avg_t:.1})"
        );

        // 3. Europa Ocean-Moon Hyper-Impact builds with Granite/LiquidWater/WaterIce/WC and integrates cleanly
        let mut ocean = build_scenario(ScenarioPreset::OceanMoonHyperImpact, 0.55);
        assert!(ocean.mat.contains(&MaterialId::Granite));
        assert!(ocean.mat.contains(&MaterialId::LiquidWater));
        assert!(ocean.mat.contains(&MaterialId::TungstenCarbide));
        for _ in 0..20 {
            let dt = ocean.compute_cfl_dt();
            ocean.step(dt);
        }
        assert!(ocean.pos.iter().all(|p| p.is_finite()));
    }
}
