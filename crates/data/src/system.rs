use serde::Deserialize;

pub const PLANET_RADIUS: f32 = 90.0;
pub const PLANET_OFFSET_X: f32 = 500.0;
pub const PLANET_SPACING: f32 = 600.0;

#[derive(Debug, Clone, Deserialize)]
pub struct SystemDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub government: String,
    pub position: (f32, f32),
    pub links: Vec<String>,
    pub planets: Vec<PlanetDef>,
    #[serde(default)]
    pub stations: Vec<StationDef>,
    #[serde(default)]
    pub star: Option<StarDef>,
    #[serde(default)]
    pub black_hole: Option<BlackHoleDef>,
    #[serde(default)]
    pub meteorites: Option<MeteoriteFieldDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShipyardDef {
    pub ships: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutfitterDef {
    pub outfits: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlackHoleDef {
    pub position: (f32, f32),
    pub radius: f32,
    pub influence: f32,
    pub pull_strength: f32,
    #[serde(default)]
    pub sprite: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StarDef {
    pub position: (f32, f32),
    pub radius: f32,
    #[serde(default)]
    pub sprite: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StationDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub radius: f32,
    pub placement: StationPlacement,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub master: Option<ChiefDef>,
    #[serde(default)]
    pub commodities: Vec<CommodityPrice>,
    #[serde(default)]
    pub shipyard: Option<ShipyardDef>,
    #[serde(default)]
    pub outfitter: Option<OutfitterDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum StationPlacement {
    Fixed {
        position: (f32, f32),
    },
    Orbit {
        planet: String,
        radius: f32,
        period_secs: f32,
        #[serde(default)]
        phase: f32,
    },
}

#[derive(Clone, Copy)]
pub enum DockableDef<'a> {
    Planet(&'a PlanetDef),
    Station(&'a StationDef),
}

impl<'a> DockableDef<'a> {
    pub fn shipyard(self) -> Option<&'a ShipyardDef> {
        match self {
            Self::Planet(p) => p.shipyard.as_ref(),
            Self::Station(s) => s.shipyard.as_ref(),
        }
    }
    pub fn outfitter(self) -> Option<&'a OutfitterDef> {
        match self {
            Self::Planet(p) => p.outfitter.as_ref(),
            Self::Station(s) => s.outfitter.as_ref(),
        }
    }
    pub fn name(self) -> &'a str {
        match self {
            Self::Planet(p) => &p.name,
            Self::Station(s) => &s.name,
        }
    }
    pub fn commodities(self) -> &'a [CommodityPrice] {
        match self {
            Self::Planet(p) => &p.commodities,
            Self::Station(s) => &s.commodities,
        }
    }
    pub fn host(self) -> Option<&'a ChiefDef> {
        match self {
            Self::Planet(p) => p.chief.as_ref(),
            Self::Station(s) => s.master.as_ref(),
        }
    }
}

impl SystemDef {
    pub fn dockable(&self, id: &str) -> Option<DockableDef<'_>> {
        self.planets
            .iter()
            .find(|p| p.id == id)
            .map(DockableDef::Planet)
            .or_else(|| {
                self.stations
                    .iter()
                    .find(|s| s.id == id)
                    .map(DockableDef::Station)
            })
    }

    pub fn planet_position(&self, id: &str) -> Option<(f32, f32)> {
        self.planets.iter().position(|p| p.id == id).map(|index| {
            (
                self.position.0 + PLANET_OFFSET_X + index as f32 * PLANET_SPACING,
                self.position.1,
            )
        })
    }
}

fn default_field_radius() -> f32 {
    1200.0
}

fn default_stationary_weight() -> u32 {
    2
}

fn default_slow_weight() -> u32 {
    5
}

fn default_fast_weight() -> u32 {
    3
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeteoriteFieldDef {
    pub count: u32,
    #[serde(default = "default_field_radius")]
    pub radius: f32,
    #[serde(default = "default_stationary_weight")]
    pub stationary_weight: u32,
    #[serde(default = "default_slow_weight")]
    pub slow_weight: u32,
    #[serde(default = "default_fast_weight")]
    pub fast_weight: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanetDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub chief: Option<ChiefDef>,
    pub commodities: Vec<CommodityPrice>,
    #[serde(default)]
    pub shipyard: Option<ShipyardDef>,
    #[serde(default)]
    pub outfitter: Option<OutfitterDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChiefDef {
    pub name: String,
    #[serde(default)]
    pub portrait: Option<String>,
    pub greeting: String,
    pub offer_text: String,
    pub tribute_text: String,
    pub quest_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommodityPrice {
    pub id: String,
    pub price: u32,
    #[serde(default)]
    pub icon: Option<String>,
}
