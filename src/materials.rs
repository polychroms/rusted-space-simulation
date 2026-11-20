/// Material database in consistent Los Alamos shock-hydrocode units:
/// - Length: cm
/// - Time: µs
/// - Mass: g
/// - Velocity: cm/µs (= 10 km/s)
/// - Density: g/cm³ (= 1000 kg/m³)
/// - Stress / Pressure / Modulus: Mbar (= 100 GPa)
/// - Specific internal energy: (cm/µs)² (= 10^8 J/kg)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialCategory {
    Metals,
    CeramicsMinerals,
    PolymersNera,
    ExplosivesFuzes,
    GravitySpace,
}

impl MaterialCategory {
    pub const ALL: [MaterialCategory; 5] = [
        MaterialCategory::Metals,
        MaterialCategory::CeramicsMinerals,
        MaterialCategory::PolymersNera,
        MaterialCategory::ExplosivesFuzes,
        MaterialCategory::GravitySpace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MaterialCategory::Metals => "Metals",
            MaterialCategory::CeramicsMinerals => "Ceramics & Rock",
            MaterialCategory::PolymersNera => "Polymers & NERA",
            MaterialCategory::ExplosivesFuzes => "Explosives & Fuzes",
            MaterialCategory::GravitySpace => "Gravity & Space",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaterialId {
    Tungsten = 0,
    RhaSteel = 1,
    Copper = 2,
    Aluminum = 3,
    BoronCarbide = 4,
    AramidLiner = 5,
    BasaltRock = 6,
    PlanetaryIron = 7,
    WaterIce = 8,
    DepletedUranium = 9,
    HhaSteel = 10,
    MildSteel = 11,
    Titanium = 12,
    Lead = 13,
    SiliconCarbide = 14,
    ArmorGlass = 15,
    Concrete = 16,
    RubberNera = 17,
    Textolite = 18,
    CompB = 19,
    C4Explosive = 20,
    TntExplosive = 21,
    ImpactFuze = 22,
    DelayFuze = 23,
    Granite = 24,
    LiquidWater = 25,
    Gold = 26,
    MagnesiumAlloy = 27,
    TungstenCarbide = 28,
    UhMwPe = 29,
    OctolExplosive = 30,
    ThermitePyro = 31,
    AttractorMass = 32,
    RepulsorMass = 33,
}

impl MaterialId {
    pub const ALL: [MaterialId; 34] = [
        // Metals
        MaterialId::RhaSteel,
        MaterialId::HhaSteel,
        MaterialId::MildSteel,
        MaterialId::Tungsten,
        MaterialId::DepletedUranium,
        MaterialId::Copper,
        MaterialId::Gold,
        MaterialId::Aluminum,
        MaterialId::Titanium,
        MaterialId::MagnesiumAlloy,
        MaterialId::Lead,
        // Ceramics, Glass, Concrete & Space
        MaterialId::BoronCarbide,
        MaterialId::SiliconCarbide,
        MaterialId::TungstenCarbide,
        MaterialId::ArmorGlass,
        MaterialId::Concrete,
        MaterialId::Granite,
        MaterialId::BasaltRock,
        MaterialId::PlanetaryIron,
        MaterialId::WaterIce,
        MaterialId::LiquidWater,
        // Polymers & NERA
        MaterialId::AramidLiner,
        MaterialId::UhMwPe,
        MaterialId::RubberNera,
        MaterialId::Textolite,
        // Explosives & Fuzes
        MaterialId::CompB,
        MaterialId::OctolExplosive,
        MaterialId::C4Explosive,
        MaterialId::TntExplosive,
        MaterialId::ThermitePyro,
        MaterialId::ImpactFuze,
        MaterialId::DelayFuze,
        // Gravity & Space
        MaterialId::AttractorMass,
        MaterialId::RepulsorMass,
    ];

    #[inline]
    pub fn props(self) -> &'static MaterialProps {
        &MATERIALS[self as usize]
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.props().code == code)
    }

    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => MaterialId::Tungsten,
            1 => MaterialId::RhaSteel,
            2 => MaterialId::Copper,
            3 => MaterialId::Aluminum,
            4 => MaterialId::BoronCarbide,
            5 => MaterialId::AramidLiner,
            6 => MaterialId::BasaltRock,
            7 => MaterialId::PlanetaryIron,
            8 => MaterialId::WaterIce,
            9 => MaterialId::DepletedUranium,
            10 => MaterialId::HhaSteel,
            11 => MaterialId::MildSteel,
            12 => MaterialId::Titanium,
            13 => MaterialId::Lead,
            14 => MaterialId::SiliconCarbide,
            15 => MaterialId::ArmorGlass,
            16 => MaterialId::Concrete,
            17 => MaterialId::RubberNera,
            18 => MaterialId::Textolite,
            19 => MaterialId::CompB,
            20 => MaterialId::C4Explosive,
            21 => MaterialId::TntExplosive,
            22 => MaterialId::ImpactFuze,
            23 => MaterialId::DelayFuze,
            24 => MaterialId::Granite,
            25 => MaterialId::LiquidWater,
            26 => MaterialId::Gold,
            27 => MaterialId::MagnesiumAlloy,
            28 => MaterialId::TungstenCarbide,
            29 => MaterialId::UhMwPe,
            30 => MaterialId::OctolExplosive,
            31 => MaterialId::ThermitePyro,
            32 => MaterialId::AttractorMass,
            _ => MaterialId::RepulsorMass,
        }
    }

    /// Signed gravitational charge multiplier applied to source mass and passive particle acceleration.
    /// Repulsors share `-` sign so same-sign pairs attract internally (`- * - > 0`) while repelling normal/`+` matter.
    #[inline]
    pub const fn grav_charge_mult(self) -> f32 {
        match self {
            MaterialId::AttractorMass => 3.5,
            MaterialId::RepulsorMass => -3.5,
            _ => 1.0,
        }
    }

    #[inline]
    pub const fn is_gravity_material(self) -> bool {
        matches!(self, MaterialId::AttractorMass | MaterialId::RepulsorMass)
    }

    /// Johnson-Cook logarithmic strain-rate sensitivity constant C in [1 + C * ln(edot / edot_0)].
    #[inline]
    pub const fn jc_rate_c(self) -> f32 {
        match self {
            MaterialId::Tungsten | MaterialId::TungstenCarbide => 0.016,
            MaterialId::RhaSteel => 0.014,
            MaterialId::HhaSteel => 0.012,
            MaterialId::MildSteel => 0.022,
            MaterialId::DepletedUranium => 0.007,
            MaterialId::Copper | MaterialId::Gold => 0.025,
            MaterialId::Aluminum | MaterialId::MagnesiumAlloy => 0.010,
            MaterialId::Titanium => 0.019,
            MaterialId::Lead => 0.030,
            MaterialId::PlanetaryIron => 0.018,
            MaterialId::BoronCarbide | MaterialId::SiliconCarbide => 0.009,
            MaterialId::ArmorGlass => 0.011,
            MaterialId::Concrete | MaterialId::BasaltRock | MaterialId::Granite => 0.012,
            MaterialId::WaterIce | MaterialId::LiquidWater => 0.015,
            MaterialId::AramidLiner | MaterialId::UhMwPe => 0.020,
            MaterialId::Textolite => 0.016,
            MaterialId::RubberNera => 0.045,
            MaterialId::CompB
            | MaterialId::C4Explosive
            | MaterialId::TntExplosive
            | MaterialId::OctolExplosive => 0.010,
            MaterialId::ImpactFuze | MaterialId::DelayFuze | MaterialId::ThermitePyro => 0.014,
            MaterialId::AttractorMass | MaterialId::RepulsorMass => 0.012,
        }
    }

    /// Johnson-Cook homologous thermal softening exponent m in [1 - ((T - T0)/(Tm - T0))^m].
    /// Lower m on DU (0.78) & Ti-6Al-4V (0.85) promotes adiabatic shear band self-sharpening!
    #[inline]
    pub const fn thermal_m(self) -> f32 {
        match self {
            MaterialId::DepletedUranium => 0.78,
            MaterialId::Titanium => 0.85,
            MaterialId::Tungsten | MaterialId::TungstenCarbide => 1.15,
            MaterialId::Copper | MaterialId::Gold => 1.09,
            MaterialId::RhaSteel | MaterialId::HhaSteel | MaterialId::MildSteel => 1.03,
            MaterialId::Aluminum | MaterialId::MagnesiumAlloy => 1.00,
            MaterialId::Lead => 0.95,
            _ => 1.20,
        }
    }

    /// Programmed-burn Jones-Wilkins-Lee (JWL) EOS parameters `[A (Mbar), B (Mbar), R1, R2, omega]`.
    #[inline]
    pub const fn jwl_params(self) -> [f32; 5] {
        match self {
            // Comp-B: P_CJ ~ 27.7 GPa at V=1.0, D_CJ = 7.98 km/s
            MaterialId::CompB => [14.80, 0.180, 4.20, 1.15, 0.34],
            // Octol 75/25: P_CJ ~ 34.2 GPa at V=1.0, D_CJ = 8.48 km/s
            MaterialId::OctolExplosive => [18.20, 0.250, 4.35, 1.20, 0.28],
            // C4 / RDX: P_CJ ~ 28.1 GPa at V=1.0, D_CJ = 8.09 km/s
            MaterialId::C4Explosive => [16.50, 0.220, 4.30, 1.20, 0.28],
            // Cast TNT: P_CJ ~ 21.2 GPa at V=1.0, D_CJ = 6.90 km/s
            MaterialId::TntExplosive => [10.60, 0.130, 4.15, 1.10, 0.35],
            // Piezo Impact Fuze: electric primer signal trace (zero blast pressure / no false sidewall pinch)
            MaterialId::ImpactFuze => [0.05, 0.001, 4.20, 1.15, 0.22],
            MaterialId::DelayFuze => [12.00, 0.150, 4.20, 1.15, 0.32],
            _ => [0.0, 0.0, 4.20, 1.15, 0.30],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialProps {
    pub code: &'static str,
    pub name: &'static str,
    pub short_name: &'static str,
    pub category: MaterialCategory,
    /// Reference density (g/cm³)
    pub rho0: f32,
    /// Bulk sound speed c0 (cm/µs, 1.0 = 10 km/s)
    pub c0: f32,
    /// Hugoniot shock slope s in Us = c0 + s * up
    pub hugoniot_s: f32,
    /// Mie-Grüneisen gamma0
    pub gamma0: f32,
    /// Young's modulus E (Mbar, 1.0 = 100 GPa)
    pub youngs_mod: f32,
    /// Initial yield strength Y0 (Mbar, 0.01 = 1 GPa)
    pub yield_strength: f32,
    /// Isotropic plastic hardening multiplier
    pub hardening: f32,
    /// Dynamic tensile spall threshold (Mbar)
    pub spall_strength: f32,
    /// Total elongation strain at fracture (ductile vs brittle)
    pub fracture_strain: f32,
    /// Drucker-Prager compressive confinement strength multiplier (ceramics/rock)
    pub pressure_hardening: f32,
    /// Melting / decomposition temperature (Kelvin)
    pub melt_temp: f32,
    /// Specific heat capacity cv in (cm/µs)² / K  (1 J/(kg·K) = 1e-8)
    pub cv: f32,
    /// Coulomb dry friction coefficient for comminuted/unbonded contact
    pub friction: f32,
    /// Energetic / reactive material parameters
    pub is_energetic: bool,
    /// Chapman-Jouguet detonation wave speed D_CJ (cm/µs; 0.80 = 8.0 km/s)
    pub detonation_vel: f32,
    /// Specific chemical energy released upon detonation ((cm/µs)²; 0.05 = 5 MJ/kg)
    pub detonation_energy: f32,
    /// Shock pressure initiation threshold (Mbar; 0.01 = 1.0 GPa)
    pub initiation_pressure: f32,
    /// Pyrotechnic countdown delay before detonation after shock trigger (µs)
    pub delay_us: f32,
    /// Base sRGB material color
    pub color: [u8; 3],
}

impl MaterialProps {
    pub fn bulk_modulus_gpa(&self) -> f32 {
        self.rho0 * self.c0 * self.c0 * 100.0
    }
    pub fn sound_speed_kms(&self) -> f32 {
        self.c0 * 10.0
    }
    pub fn yield_gpa(&self) -> f32 {
        self.yield_strength * 100.0
    }
    pub fn spall_gpa(&self) -> f32 {
        self.spall_strength * 100.0
    }
    pub fn det_speed_kms(&self) -> f32 {
        self.detonation_vel * 10.0
    }
    pub fn init_gpa(&self) -> f32 {
        self.initiation_pressure * 100.0
    }
    /// Exact isotropic Poisson's ratio nu = (3 K0 - E) / (6 K0), clamped to physical solid bounds.
    #[inline]
    pub fn poisson_ratio(&self) -> f32 {
        let k0 = (self.rho0 * self.c0 * self.c0).max(1e-5);
        ((3.0 * k0 - self.youngs_mod) / (6.0 * k0)).clamp(0.14, 0.485)
    }
    /// Exact isotropic shear modulus G = E / (2 * (1 + nu)) in Mbar.
    #[inline]
    pub fn shear_modulus(&self) -> f32 {
        self.youngs_mod / (2.0 * (1.0 + self.poisson_ratio()))
    }
    /// Shock acoustic impedance Z0 = rho0 * c0 in MRayl (10^6 kg/(m^2 s) = 0.1 g/(cm^2 us) -> *10 for MRayl).
    #[inline]
    pub fn acoustic_impedance_mrayl(&self) -> f32 {
        self.rho0 * self.c0 * 10.0
    }
    pub fn is_ductile(&self) -> bool {
        self.fracture_strain >= 0.15
    }
}

/// Indexed by `MaterialId as usize` (0..34)
pub const MATERIALS: [MaterialProps; 34] = [
    // 0: Tungsten Heavy Alloy (WHA)
    MaterialProps {
        code: "WHA",
        name: "Tungsten Heavy Alloy (WHA)",
        short_name: "Tungsten",
        category: MaterialCategory::Metals,
        rho0: 17.50,
        c0: 0.403,
        hugoniot_s: 1.24,
        gamma0: 1.67,
        youngs_mod: 3.60,
        yield_strength: 0.0165,
        hardening: 0.35,
        spall_strength: 0.036,
        fracture_strain: 0.19,
        pressure_hardening: 0.06,
        melt_temp: 3695.0,
        cv: 1.34e-6,
        friction: 0.28,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [176, 188, 210],
    },
    // 1: Rolled Homogeneous Armor Steel (RHA 4340)
    MaterialProps {
        code: "RHA",
        name: "RHA Armor Steel (MIL-12560)",
        short_name: "RHA Steel",
        category: MaterialCategory::Metals,
        rho0: 7.85,
        c0: 0.457,
        hugoniot_s: 1.49,
        gamma0: 1.93,
        youngs_mod: 2.10,
        yield_strength: 0.0115,
        hardening: 0.55,
        spall_strength: 0.034,
        fracture_strain: 0.32,
        pressure_hardening: 0.05,
        melt_temp: 1810.0,
        cv: 4.65e-6,
        friction: 0.32,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [112, 128, 150],
    },
    // 2: OFHC Copper (Shaped-charge liners & HESH shells)
    MaterialProps {
        code: "CU",
        name: "OFHC Copper (HEAT Liner)",
        short_name: "Copper",
        category: MaterialCategory::Metals,
        rho0: 8.96,
        c0: 0.394,
        hugoniot_s: 1.49,
        gamma0: 2.02,
        youngs_mod: 1.24,
        yield_strength: 0.0030,
        hardening: 0.60,
        spall_strength: 0.024,
        fracture_strain: 0.65,
        pressure_hardening: 0.03,
        melt_temp: 1358.0,
        cv: 3.85e-6,
        friction: 0.22,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [224, 128, 74],
    },
    // 3: Aluminum 7075-T6
    MaterialProps {
        code: "AL",
        name: "Aluminum 7075-T6",
        short_name: "Al 7075",
        category: MaterialCategory::Metals,
        rho0: 2.81,
        c0: 0.524,
        hugoniot_s: 1.34,
        gamma0: 2.00,
        youngs_mod: 0.72,
        yield_strength: 0.0050,
        hardening: 0.35,
        spall_strength: 0.015,
        fracture_strain: 0.24,
        pressure_hardening: 0.04,
        melt_temp: 908.0,
        cv: 9.10e-6,
        friction: 0.26,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [195, 208, 222],
    },
    // 4: Boron Carbide Ceramic (B4C)
    MaterialProps {
        code: "B4C",
        name: "Boron Carbide Ceramic (B4C)",
        short_name: "B4C Ceramic",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 2.52,
        c0: 0.780,
        hugoniot_s: 1.15,
        gamma0: 1.45,
        youngs_mod: 4.40,
        yield_strength: 0.065,
        hardening: 0.05,
        spall_strength: 0.0110,
        fracture_strain: 0.036,
        pressure_hardening: 0.70,
        melt_temp: 2720.0,
        cv: 9.50e-6,
        friction: 0.55,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [52, 172, 166],
    },
    // 5: Aramid / Kevlar Composite Spall Liner
    MaterialProps {
        code: "KEV",
        name: "Aramid / Kevlar Spall Liner",
        short_name: "Kevlar Liner",
        category: MaterialCategory::PolymersNera,
        rho0: 1.44,
        c0: 0.310,
        hugoniot_s: 1.42,
        gamma0: 1.20,
        youngs_mod: 0.24,
        yield_strength: 0.0052,
        hardening: 0.95,
        spall_strength: 0.022,
        fracture_strain: 0.75,
        pressure_hardening: 0.08,
        melt_temp: 820.0,
        cv: 14.2e-6,
        friction: 0.38,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [216, 186, 68],
    },
    // 6: Basalt / Silicate Rock
    MaterialProps {
        code: "ROCK",
        name: "Basalt / Silicate Rock",
        short_name: "Basalt Rock",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 2.88,
        c0: 0.385,
        hugoniot_s: 1.32,
        gamma0: 1.50,
        youngs_mod: 0.85,
        yield_strength: 0.0075,
        hardening: 0.22,
        spall_strength: 0.0068,
        fracture_strain: 0.048,
        pressure_hardening: 0.62,
        melt_temp: 1520.0,
        cv: 8.40e-6,
        friction: 0.58,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [132, 114, 98],
    },
    // 7: Planetary Iron-Nickel
    MaterialProps {
        code: "FENI",
        name: "Planetary Iron-Nickel",
        short_name: "Fe-Ni Core",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 8.15,
        c0: 0.445,
        hugoniot_s: 1.52,
        gamma0: 1.85,
        youngs_mod: 1.95,
        yield_strength: 0.0080,
        hardening: 0.40,
        spall_strength: 0.024,
        fracture_strain: 0.36,
        pressure_hardening: 0.08,
        melt_temp: 1780.0,
        cv: 4.50e-6,
        friction: 0.30,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [188, 152, 126],
    },
    // 8: Crystalline Water Ice
    MaterialProps {
        code: "ICE",
        name: "Crystalline Water Ice",
        short_name: "Water Ice",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 0.92,
        c0: 0.295,
        hugoniot_s: 1.38,
        gamma0: 1.15,
        youngs_mod: 0.095,
        yield_strength: 0.0020,
        hardening: 0.12,
        spall_strength: 0.0022,
        fracture_strain: 0.045,
        pressure_hardening: 0.28,
        melt_temp: 273.15,
        cv: 20.5e-6,
        friction: 0.12,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [150, 215, 248],
    },
    // 9: Depleted Uranium (DU - self-sharpening adiabatic shear penetrator!)
    MaterialProps {
        code: "DU",
        name: "Depleted Uranium (DU-0.75Ti)",
        short_name: "Depleted U",
        category: MaterialCategory::Metals,
        rho0: 19.05,
        c0: 0.310,
        hugoniot_s: 1.51,
        gamma0: 2.15,
        youngs_mod: 1.90,
        yield_strength: 0.0155,
        hardening: 0.22,
        spall_strength: 0.030,
        fracture_strain: 0.16,
        pressure_hardening: 0.05,
        melt_temp: 1405.0, // Lower melt than WHA -> localized adiabatic shear self-sharpening!
        cv: 1.17e-6,
        friction: 0.26,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [108, 138, 102],
    },
    // 10: High-Hardness Armor Steel (HHA - MIL-DTL-46100)
    MaterialProps {
        code: "HHA",
        name: "High-Hardness Armor Steel (HHA)",
        short_name: "HHA Steel",
        category: MaterialCategory::Metals,
        rho0: 7.85,
        c0: 0.462,
        hugoniot_s: 1.49,
        gamma0: 1.93,
        youngs_mod: 2.12,
        yield_strength: 0.0175, // 1.75 GPa ultra-hard strike face
        hardening: 0.25,
        spall_strength: 0.025, // More prone to brittle spall than ductile RHA
        fracture_strain: 0.14,
        pressure_hardening: 0.06,
        melt_temp: 1800.0,
        cv: 4.65e-6,
        friction: 0.34,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [78, 92, 114],
    },
    // 11: Mild / Structural Steel (Shell casings, ERA containers)
    MaterialProps {
        code: "MILD",
        name: "Mild Structural Steel (1020)",
        short_name: "Mild Steel",
        category: MaterialCategory::Metals,
        rho0: 7.80,
        c0: 0.452,
        hugoniot_s: 1.49,
        gamma0: 1.93,
        youngs_mod: 2.00,
        yield_strength: 0.0042, // 0.42 GPa soft structural casing steel
        hardening: 0.65,
        spall_strength: 0.015,
        fracture_strain: 0.22,
        pressure_hardening: 0.04,
        melt_temp: 1815.0,
        cv: 4.70e-6,
        friction: 0.30,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [142, 150, 162],
    },
    // 12: Titanium Ti-6Al-4V
    MaterialProps {
        code: "TI",
        name: "Titanium Alloy (Ti-6Al-4V)",
        short_name: "Titanium",
        category: MaterialCategory::Metals,
        rho0: 4.43,
        c0: 0.513,
        hugoniot_s: 1.09,
        gamma0: 1.23,
        youngs_mod: 1.14,
        yield_strength: 0.0110,
        hardening: 0.32,
        spall_strength: 0.032,
        fracture_strain: 0.20,
        pressure_hardening: 0.05,
        melt_temp: 1933.0,
        cv: 5.26e-6,
        friction: 0.34,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [162, 170, 192],
    },
    // 13: Soft Lead
    MaterialProps {
        code: "PB",
        name: "Soft Lead (Pb)",
        short_name: "Lead",
        category: MaterialCategory::Metals,
        rho0: 11.34,
        c0: 0.205,
        hugoniot_s: 1.46,
        gamma0: 2.77,
        youngs_mod: 0.16,
        yield_strength: 0.0003,
        hardening: 0.10,
        spall_strength: 0.0020,
        fracture_strain: 0.50,
        pressure_hardening: 0.02,
        melt_temp: 600.6,
        cv: 1.29e-6,
        friction: 0.18,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [98, 102, 118],
    },
    // 14: Silicon Carbide Ceramic (SiC)
    MaterialProps {
        code: "SIC",
        name: "Silicon Carbide Ceramic (SiC)",
        short_name: "SiC Ceramic",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 3.21,
        c0: 0.800,
        hugoniot_s: 1.05,
        gamma0: 1.35,
        youngs_mod: 4.10,
        yield_strength: 0.058,
        hardening: 0.05,
        spall_strength: 0.0105,
        fracture_strain: 0.034,
        pressure_hardening: 0.68,
        melt_temp: 2970.0,
        cv: 7.50e-6,
        friction: 0.54,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [66, 134, 152],
    },
    // 15: Armored Silicate Glass
    MaterialProps {
        code: "GLASS",
        name: "Armored Ballistic Glass",
        short_name: "Armor Glass",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 2.52,
        c0: 0.530,
        hugoniot_s: 1.22,
        gamma0: 1.10,
        youngs_mod: 0.75,
        yield_strength: 0.012,
        hardening: 0.02,
        spall_strength: 0.0018, // Brittle radial crack webs!
        fracture_strain: 0.016,
        pressure_hardening: 0.50,
        melt_temp: 1680.0,
        cv: 8.40e-6,
        friction: 0.42,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [138, 224, 218],
    },
    // 16: High-Strength Bunker Concrete
    MaterialProps {
        code: "CONC",
        name: "Reinforced Bunker Concrete",
        short_name: "Concrete",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 2.40,
        c0: 0.320,
        hugoniot_s: 1.40,
        gamma0: 1.25,
        youngs_mod: 0.42,
        yield_strength: 0.0018,
        hardening: 0.08,
        spall_strength: 0.0009, // Heavy backface spalling & cratering
        fracture_strain: 0.018,
        pressure_hardening: 0.58,
        melt_temp: 1550.0,
        cv: 8.80e-6,
        friction: 0.62,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [148, 146, 140],
    },
    // 17: Vulcanized Rubber / Elastomer (NERA reactive bulging layer!)
    MaterialProps {
        code: "RUB",
        name: "NERA Vulcanized Elastomer Rubber",
        short_name: "NERA Rubber",
        category: MaterialCategory::PolymersNera,
        rho0: 1.18,
        c0: 0.185,
        hugoniot_s: 1.65,
        gamma0: 1.35,
        youngs_mod: 0.018, // Soft shear (nu = 0.43), nearly incompressible bulk -> bulges NERA flyer plates!
        yield_strength: 0.0020,
        hardening: 0.90,
        spall_strength: 0.0080,
        fracture_strain: 1.15, // Huge elastic/viscoelastic stretch
        pressure_hardening: 0.10,
        melt_temp: 650.0,
        cv: 18.5e-6,
        friction: 0.65,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [54, 58, 66],
    },
    // 18: Textolite / Fiberglass (Soviet T-72/T-80 composite glacis interlayer)
    MaterialProps {
        code: "TEX",
        name: "Textolite / Fiberglass Laminate",
        short_name: "Textolite",
        category: MaterialCategory::PolymersNera,
        rho0: 1.85,
        c0: 0.340,
        hugoniot_s: 1.45,
        gamma0: 1.30,
        youngs_mod: 0.38,
        yield_strength: 0.0055,
        hardening: 0.45,
        spall_strength: 0.012,
        fracture_strain: 0.18,
        pressure_hardening: 0.25,
        melt_temp: 880.0,
        cv: 11.5e-6,
        friction: 0.42,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [168, 118, 58],
    },
    // 19: Composition B High Explosive (HEAT Shaped Charges & Ammo Racks!)
    MaterialProps {
        code: "COMPB",
        name: "Composition B (RDX/TNT High Explosive)",
        short_name: "Comp-B HE",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 1.72,
        c0: 0.285,
        hugoniot_s: 1.60,
        gamma0: 1.50,
        youngs_mod: 0.12,
        yield_strength: 0.0012,
        hardening: 0.10,
        spall_strength: 0.0015,
        fracture_strain: 0.06,
        pressure_hardening: 0.12,
        melt_temp: 780.0,
        cv: 12.0e-6,
        friction: 0.25,
        is_energetic: true,
        detonation_vel: 0.798,        // 7.98 km/s CJ detonation speed
        detonation_energy: 0.056,     // ~5.6 MJ/kg -> ~29 GPa CJ detonation pressure!
        initiation_pressure: 0.030,   // Initiates at >= 3.0 GPa shock or adjacent fuze
        delay_us: 0.04,
        color: [228, 168, 62],
    },
    // 20: C4 / RDX Plastic Explosive (Kontakt-5 ERA & HESH Squash-Head!)
    MaterialProps {
        code: "C4",
        name: "C4 / RDX Plastic Explosive (ERA/HESH)",
        short_name: "C4 Explosive",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 1.60,
        c0: 0.260,
        hugoniot_s: 1.62,
        gamma0: 1.50,
        youngs_mod: 0.05,
        yield_strength: 0.0005, // Soft moldable putty — squashes cleanly in HESH!
        hardening: 0.15,
        spall_strength: 0.0020,
        fracture_strain: 0.55,
        pressure_hardening: 0.08,
        melt_temp: 760.0,
        cv: 12.5e-6,
        friction: 0.30,
        is_energetic: true,
        detonation_vel: 0.809,        // 8.09 km/s CJ detonation speed
        detonation_energy: 0.058,     // ~5.8 MJ/kg
        initiation_pressure: 0.036,   // 3.6 GPa threshold: squashes safely at 0.73 km/s HESH (~2.7 GPa), detonates immediately in Kontakt-5 ERA (~4.5 GPa)!
        delay_us: 0.04,
        color: [238, 108, 92],
    },
    // 21: Cast TNT (Insensitive APHE internal burster!)
    MaterialProps {
        code: "TNT",
        name: "Cast TNT (Insensitive APHE Filler)",
        short_name: "Cast TNT",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 1.65,
        c0: 0.257,
        hugoniot_s: 1.58,
        gamma0: 1.45,
        youngs_mod: 0.10,
        yield_strength: 0.0010,
        hardening: 0.10,
        spall_strength: 0.0015,
        fracture_strain: 0.08,
        pressure_hardening: 0.12,
        melt_temp: 820.0,
        cv: 12.2e-6,
        friction: 0.25,
        is_energetic: true,
        detonation_vel: 0.690,        // 6.90 km/s
        detonation_energy: 0.044,     // ~4.4 MJ/kg
        initiation_pressure: 0.110,   // 11.0 GPa shock-insensitive -> survives armor entry until Base Delay Fuze fires!
        delay_us: 0.05,
        color: [204, 182, 92],
    },
    // 22: Piezoelectric Impact Nose Fuze (Instant Contact Trigger Wire — does NOT plug shaped-charge muzzle!)
    MaterialProps {
        code: "FUZE",
        name: "Impact Contact Fuze (Piezo)",
        short_name: "Impact Fuze",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 1.35,
        c0: 0.420,
        hugoniot_s: 1.40,
        gamma0: 1.40,
        youngs_mod: 0.45,
        yield_strength: 0.0035,
        hardening: 0.10,
        spall_strength: 0.0040,
        fracture_strain: 0.08,
        pressure_hardening: 0.05,
        melt_temp: 620.0,
        cv: 10.0e-6,
        friction: 0.25,
        is_energetic: true,
        detonation_vel: 4.200,        // 42 km/s piezoelectric firing pulse reaches rear booster before cone crushes
        detonation_energy: 0.0004,    // Minimal-blast electric primer trace (leaves HEAT casing & muzzle unperturbed)
        initiation_pressure: 0.010,   // 1.0 GPa piezo crystal crush OR direct cross-body contact
        delay_us: 0.02,
        color: [255, 68, 68],
    },
    // 23: Base Pyrotechnic Delay Fuze (APHE / HESH Post-Impact Delay!)
    MaterialProps {
        code: "DFUZE",
        name: "Base Delay Fuze (Post-Pen Delay)",
        short_name: "Delay Fuze",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 5.80,
        c0: 0.420,
        hugoniot_s: 1.45,
        gamma0: 1.50,
        youngs_mod: 1.40,
        yield_strength: 0.0080,
        hardening: 0.25,
        spall_strength: 0.018,
        fracture_strain: 0.22,
        pressure_hardening: 0.05,
        melt_temp: 950.0,
        cv: 5.0e-6,
        friction: 0.28,
        is_energetic: true,
        detonation_vel: 0.780,
        detonation_energy: 0.036,
        initiation_pressure: 0.0025,  // 0.25 GPa impact shock / strong deceleration arms 12 µs delay train
        delay_us: 12.0,
        color: [255, 135, 38],
    },
    // 24: Coarse-Grained Granite
    MaterialProps {
        code: "GRAN",
        name: "Granite (Continental Silicate Rock)",
        short_name: "Granite",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 2.65,
        c0: 0.480,
        hugoniot_s: 1.42,
        gamma0: 1.15,
        youngs_mod: 0.65,
        yield_strength: 0.0018,
        hardening: 0.20,
        spall_strength: 0.0015,
        fracture_strain: 0.025,
        pressure_hardening: 0.72,
        melt_temp: 1520.0,
        cv: 7.9e-6,
        friction: 0.55,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [168, 152, 142],
    },
    // 25: Liquid Ocean Water (Hydrodynamic above 273 K!)
    MaterialProps {
        code: "H2O",
        name: "Liquid Water (Subsurface Ocean Fluid)",
        short_name: "Water",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 1.00,
        c0: 0.148,
        hugoniot_s: 1.75,
        gamma0: 1.10,
        youngs_mod: 0.022,
        yield_strength: 0.00001,
        hardening: 0.0,
        spall_strength: 0.0002,
        fracture_strain: 2.0,
        pressure_hardening: 0.0,
        melt_temp: 273.0,
        cv: 41.8e-6,
        friction: 0.01,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [46, 138, 224],
    },
    // 26: Pure Gold (High-Z Ductile Calibrated Target)
    MaterialProps {
        code: "AU",
        name: "Pure Gold (High-Z Ductile Foil)",
        short_name: "Gold",
        category: MaterialCategory::Metals,
        rho0: 19.30,
        c0: 0.306,
        hugoniot_s: 1.57,
        gamma0: 2.97,
        youngs_mod: 0.79,
        yield_strength: 0.0020,
        hardening: 0.35,
        spall_strength: 0.015,
        fracture_strain: 1.45,
        pressure_hardening: 0.02,
        melt_temp: 1337.0,
        cv: 1.29e-6,
        friction: 0.25,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [242, 198, 65],
    },
    // 27: AZ31B Magnesium Aerospace Alloy
    MaterialProps {
        code: "MG",
        name: "AZ31B Magnesium Aerospace Alloy",
        short_name: "Magnesium",
        category: MaterialCategory::Metals,
        rho0: 1.78,
        c0: 0.452,
        hugoniot_s: 1.26,
        gamma0: 1.54,
        youngs_mod: 0.45,
        yield_strength: 0.0026,
        hardening: 0.32,
        spall_strength: 0.009,
        fracture_strain: 0.28,
        pressure_hardening: 0.04,
        melt_temp: 923.0,
        cv: 10.2e-6,
        friction: 0.24,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [198, 212, 205],
    },
    // 28: Cemented Tungsten Carbide (WC-Co Core)
    MaterialProps {
        code: "WC",
        name: "Tungsten Carbide (WC-Co Hard Core)",
        short_name: "WC Carbide",
        category: MaterialCategory::CeramicsMinerals,
        rho0: 15.63,
        c0: 0.518,
        hugoniot_s: 1.28,
        gamma0: 1.50,
        youngs_mod: 6.20,
        yield_strength: 0.042,
        hardening: 0.15,
        spall_strength: 0.028,
        fracture_strain: 0.09,
        pressure_hardening: 0.55,
        melt_temp: 3143.0,
        cv: 2.1e-6,
        friction: 0.32,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [105, 112, 122],
    },
    // 29: UHMWPE Dyneema Ballistic Backing
    MaterialProps {
        code: "UHMWPE",
        name: "UHMWPE Fiber (Dyneema Liner)",
        short_name: "UHMWPE",
        category: MaterialCategory::PolymersNera,
        rho0: 0.97,
        c0: 0.290,
        hugoniot_s: 1.62,
        gamma0: 1.15,
        youngs_mod: 0.25,
        yield_strength: 0.0038,
        hardening: 0.30,
        spall_strength: 0.022,
        fracture_strain: 0.85,
        pressure_hardening: 0.08,
        melt_temp: 425.0,
        cv: 19.0e-6,
        friction: 0.18,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [228, 236, 240],
    },
    // 30: Octol 75/25 High Explosive
    MaterialProps {
        code: "OCTOL",
        name: "Octol 75/25 (HMX High Explosive)",
        short_name: "Octol",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 1.82,
        c0: 0.310,
        hugoniot_s: 1.75,
        gamma0: 1.50,
        youngs_mod: 0.16,
        yield_strength: 0.0014,
        hardening: 0.10,
        spall_strength: 0.0018,
        fracture_strain: 0.06,
        pressure_hardening: 0.12,
        melt_temp: 720.0,
        cv: 12.0e-6,
        friction: 0.25,
        is_energetic: true,
        detonation_vel: 0.848,
        detonation_energy: 0.065,
        initiation_pressure: 0.028,
        delay_us: 0.03,
        color: [236, 86, 148],
    },
    // 31: Condensed-Phase Pyrotechnic Thermite (Fe2O3 + Al Exothermic Burn, No Gas Explosion)
    MaterialProps {
        code: "THERM",
        name: "Thermite Pyrotechnic Charge (Fe2O3+Al)",
        short_name: "Thermite",
        category: MaterialCategory::ExplosivesFuzes,
        rho0: 4.15,
        c0: 0.390,
        hugoniot_s: 1.40,
        gamma0: 1.60,
        youngs_mod: 0.85,
        yield_strength: 0.0035,
        hardening: 0.15,
        spall_strength: 0.005,
        fracture_strain: 0.10,
        pressure_hardening: 0.20,
        melt_temp: 1810.0,
        cv: 7.2e-6,
        friction: 0.42,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.040,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [215, 92, 48],
    },
    // 32: Attractor Core (High-Gravity Mass, q = +3.5x)
    MaterialProps {
        code: "ATTR",
        name: "Attractor Core (High-Gravity Mass)",
        short_name: "Attractor",
        category: MaterialCategory::GravitySpace,
        rho0: 12.0,
        c0: 0.85,
        hugoniot_s: 1.35,
        gamma0: 1.40,
        youngs_mod: 4.50,
        yield_strength: 0.045,
        hardening: 0.25,
        spall_strength: 0.120,
        fracture_strain: 0.90,
        pressure_hardening: 0.60,
        melt_temp: 8500.0,
        cv: 6.0e-6,
        friction: 0.35,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [64, 198, 255],
    },
    // 33: Repulsor Mass (Anti-Gravity Field, q = -3.5x)
    MaterialProps {
        code: "REPL",
        name: "Repulsor Mass (Anti-Gravity Field)",
        short_name: "Repulsor",
        category: MaterialCategory::GravitySpace,
        rho0: 12.0,
        c0: 0.85,
        hugoniot_s: 1.35,
        gamma0: 1.40,
        youngs_mod: 4.50,
        yield_strength: 0.045,
        hardening: 0.25,
        spall_strength: 0.120,
        fracture_strain: 0.90,
        pressure_hardening: 0.60,
        melt_temp: 8500.0,
        cv: 6.0e-6,
        friction: 0.35,
        is_energetic: false,
        detonation_vel: 0.0,
        detonation_energy: 0.0,
        initiation_pressure: 0.0,
        delay_us: 0.0,
        color: [208, 92, 255],
    },
];
