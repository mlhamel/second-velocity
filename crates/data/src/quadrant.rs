use serde::{Deserialize, Serialize};

pub const SYSTEM_EDGE_CLEARANCE: f32 = 800.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuadrantDef {
    pub id: String,
    pub name: String,
    pub origin: (f32, f32),
    pub size: (f32, f32),
    #[serde(default)]
    pub systems: Vec<String>,
    #[serde(default)]
    pub quests: Vec<String>,
    #[serde(default)]
    pub pilots: Vec<String>,
}

impl QuadrantDef {
    pub fn min_x(&self) -> f32 {
        self.origin.0 - self.size.0 * 0.5
    }

    pub fn max_x(&self) -> f32 {
        self.origin.0 + self.size.0 * 0.5
    }

    pub fn min_y(&self) -> f32 {
        self.origin.1 - self.size.1 * 0.5
    }

    pub fn max_y(&self) -> f32 {
        self.origin.1 + self.size.1 * 0.5
    }

    pub fn contains_with_clearance(&self, pos: (f32, f32), clearance: f32) -> bool {
        pos.0 > self.min_x() + clearance
            && pos.0 < self.max_x() - clearance
            && pos.1 > self.min_y() + clearance
            && pos.1 < self.max_y() - clearance
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.min_x() < other.max_x()
            && self.max_x() > other.min_x()
            && self.min_y() < other.max_y()
            && self.max_y() > other.min_y()
    }
}
