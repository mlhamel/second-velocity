use bevy::math::Vec2;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position(pub Vec2);

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heading(pub f32);

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hull {
    pub shields: f32,
    pub max_shields: f32,
    pub armor: f32,
    pub max_armor: f32,
}

#[derive(Component, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EquippedOutfits {
    pub ids: Vec<String>,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuyShip {
    pub ship: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuyOutfit {
    pub outfit: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SellOutfit {
    pub outfit: String,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fuel {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipInfo {
    pub def_id: String,
    pub kind: ShipKind,
    pub owner: Option<u64>,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipKind {
    Player,
    Pirate,
    Police,
    Merchant,
    Defender,
    Explorer,
}

impl ShipKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Pirate => "pirate",
            Self::Police => "police",
            Self::Merchant => "merchant",
            Self::Defender => "defender",
            Self::Explorer => "explorer",
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Projectile {
    pub damage: f32,
    pub ttl: f32,
    pub owner: u64,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanetTag {
    pub system: String,
    pub id: String,
    pub name: String,
    pub radius: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarTag {
    pub system: String,
    pub radius: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlackHoleTag {
    pub system: String,
    pub radius: f32,
    pub influence: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemMembership(pub String);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockableKind {
    #[default]
    Planet,
    Station,
}

impl DockableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planet => "planet",
            Self::Station => "station",
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockableTag {
    pub system: String,
    pub id: String,
    pub name: String,
    pub radius: f32,
    pub kind: DockableKind,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockedAt {
    pub system: String,
    #[serde(alias = "planet")]
    pub location: String,
    #[serde(default)]
    pub kind: DockableKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketPrice {
    pub commodity: String,
    pub price: u32,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortMarket {
    pub prices: Vec<MarketPrice>,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountState {
    pub credits: u32,
    pub cargo: Vec<(String, u32)>,
    pub cargo_capacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChiefMood {
    Offer,
    Tribute,
    Quest,
    TurnIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterOption {
    pub id: String,
    pub label: String,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChiefEncounter {
    pub chief_name: String,
    pub portrait: Option<String>,
    pub mood: ChiefMood,
    pub text: String,
    pub options: Vec<EncounterOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QuestTarget {
    Delivery {
        commodity: String,
        qty: u32,
        destination_system: String,
        destination_planet: String,
    },
    Bounty {
        system: String,
        target: String,
        killed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveQuest {
    pub id: String,
    pub name: String,
    pub giver_system: String,
    pub giver_planet: String,
    pub target: QuestTarget,
    pub reward_credits: u32,
    pub remaining_secs: Option<f32>,
}

#[derive(Component, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActiveQuests {
    #[serde(default)]
    pub quests: Vec<ActiveQuest>,
    #[serde(default)]
    pub completed: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeteoriteSize {
    Large,
    Medium,
    Small,
}

impl MeteoriteSize {
    pub const fn radius(self) -> f32 {
        match self {
            Self::Large => 28.0,
            Self::Medium => 17.0,
            Self::Small => 10.0,
        }
    }

    pub const fn max_hp(self) -> f32 {
        match self {
            Self::Large => 27.0,
            Self::Medium => 18.0,
            Self::Small => 9.0,
        }
    }

    pub const fn collision_damage(self) -> f32 {
        match self {
            Self::Large => 18.0,
            Self::Medium => 10.0,
            Self::Small => 5.0,
        }
    }

    pub const fn child_size(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Large => "large",
            Self::Medium => "medium",
            Self::Small => "small",
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meteorite {
    pub size: MeteoriteSize,
    pub hp: f32,
    pub max_hp: f32,
    pub spin_rate: f32,
    pub variant: u8,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetThrottle(pub f32);

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetTurn(pub f32);

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetFiring(pub bool);

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestLand {
    pub planet: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestDock {
    pub system: String,
    pub location: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestLiftOff;

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuyCommodity {
    pub commodity: String,
    pub qty: u32,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SellCommodity {
    pub commodity: String,
    pub qty: u32,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetWarp(pub bool);

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RespondToEncounter {
    pub choice: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HailShip;

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub credits: u32,
    pub cargo: Vec<(String, u32)>,
    pub system: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    pub text: String,
}

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerDied;

#[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explosion {
    pub position: (f32, f32),
    pub system: String,
}

pub fn register(app: &mut App) {
    app.replicate::<Position>()
        .replicate::<Heading>()
        .replicate::<Hull>()
        .replicate::<Fuel>()
        .replicate::<ShipInfo>()
        .replicate::<Projectile>()
        .replicate::<PlanetTag>()
        .replicate::<DockableTag>()
        .replicate::<StarTag>()
        .replicate::<BlackHoleTag>()
        .replicate::<SystemMembership>()
        .replicate::<DockedAt>()
        .replicate::<AccountState>()
        .replicate::<EquippedOutfits>()
        .replicate::<ChiefEncounter>()
        .replicate::<ActiveQuests>()
        .replicate::<Meteorite>()
        .replicate::<PortMarket>()
        .add_client_event::<SetThrottle>(Channel::Ordered)
        .add_client_event::<SetTurn>(Channel::Ordered)
        .add_client_event::<SetFiring>(Channel::Ordered)
        .add_client_event::<RequestLand>(Channel::Ordered)
        .add_client_event::<RequestDock>(Channel::Ordered)
        .add_client_event::<RequestLiftOff>(Channel::Ordered)
        .add_client_event::<BuyCommodity>(Channel::Ordered)
        .add_client_event::<BuyShip>(Channel::Ordered)
        .add_client_event::<BuyOutfit>(Channel::Ordered)
        .add_client_event::<SellOutfit>(Channel::Ordered)
        .add_client_event::<SellCommodity>(Channel::Ordered)
        .add_client_event::<SetWarp>(Channel::Ordered)
        .add_client_event::<RespondToEncounter>(Channel::Ordered)
        .add_client_event::<HailShip>(Channel::Ordered)
        .add_client_event::<LoginRequest>(Channel::Ordered)
        .add_server_event::<LoginResponse>(Channel::Ordered)
        .add_server_event::<Notice>(Channel::Ordered)
        .add_server_event::<PlayerDied>(Channel::Ordered)
        .add_server_event::<Explosion>(Channel::Unordered);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
        let text = ron::to_string(&value).expect("must serialize");
        let parsed: T = ron::from_str(&text).expect("must deserialize");
        assert_eq!(parsed, value);
    }

    #[test]
    fn components_roundtrip() {
        roundtrip(EquippedOutfits {
            ids: vec!["light-blaster".into(), "shield-generator-1".into()],
        });
        roundtrip(DockableTag {
            system: "sol".into(),
            id: "lin-relay".into(),
            name: "Lin Relay".into(),
            radius: 36.0,
            kind: DockableKind::Station,
        });
        roundtrip(DockedAt {
            system: "sol".into(),
            location: "lin-relay".into(),
            kind: DockableKind::Station,
        });
        roundtrip(PortMarket {
            prices: vec![
                MarketPrice {
                    commodity: "food".into(),
                    price: 8,
                },
                MarketPrice {
                    commodity: "isotopes".into(),
                    price: 90,
                },
            ],
        });
        roundtrip(Position(Vec2::new(12.5, -3.25)));
        roundtrip(Heading(1.25));
        roundtrip(Hull {
            shields: 40.0,
            max_shields: 40.0,
            armor: 25.0,
            max_armor: 25.0,
        });
        roundtrip(ShipInfo {
            def_id: "shuttle".into(),
            kind: ShipKind::Pirate,
            owner: None,
            name: "Turncoat".into(),
        });
        roundtrip(ShipInfo {
            def_id: "vellari-clipper".into(),
            kind: ShipKind::Merchant,
            owner: None,
            name: "Sun-Sail Clipper".into(),
        });
        roundtrip(AccountState {
            credits: 500,
            cargo: vec![("food".into(), 3)],
            cargo_capacity: 20,
        });
        roundtrip(PlanetTag {
            system: "sol".into(),
            id: "earth".into(),
            name: "Earth".into(),
            radius: 90.0,
        });
        roundtrip(StarTag {
            system: "sol".into(),
            radius: 240.0,
        });
        roundtrip(BlackHoleTag {
            system: "cygnus-x1".into(),
            radius: 90.0,
            influence: 500.0,
        });
        roundtrip(ChiefEncounter {
            chief_name: "Port Director Lin".into(),
            portrait: Some("textures/chiefs/earth.png".into()),
            mood: ChiefMood::Quest,
            text: "Take this contract.".into(),
            options: vec![EncounterOption {
                id: "accept_quest".into(),
                label: "Accept".into(),
            }],
        });
        roundtrip(ActiveQuests {
            quests: vec![ActiveQuest {
                id: "newhaven-relief".into(),
                name: "Sealed Med-Crates for Newhaven".into(),
                giver_system: "sol".into(),
                giver_planet: "earth".into(),
                target: QuestTarget::Delivery {
                    commodity: "medicine".into(),
                    qty: 4,
                    destination_system: "alpha-centauri".into(),
                    destination_planet: "newhaven".into(),
                },
                reward_credits: 450,
                remaining_secs: Some(120.0),
            }],
            completed: vec!["old-quest".into()],
        });
        roundtrip(Meteorite {
            size: MeteoriteSize::Large,
            hp: 27.0,
            max_hp: 27.0,
            spin_rate: 1.25,
            variant: 2,
        });
    }

    #[test]
    fn events_roundtrip() {
        roundtrip(BuyShip {
            ship: "terran-courier".into(),
        });
        roundtrip(BuyOutfit {
            outfit: "pulse-blaster".into(),
        });
        roundtrip(SellOutfit {
            outfit: "light-blaster".into(),
        });
        roundtrip(RequestDock {
            system: "sol".into(),
            location: "lin-relay".into(),
        });
        roundtrip(SetThrottle(0.8));
        roundtrip(SetTurn(-1.0));
        roundtrip(SetFiring(true));
        roundtrip(RequestLand {
            planet: "earth".into(),
        });
        roundtrip(BuyCommodity {
            commodity: "food".into(),
            qty: 5,
        });
        roundtrip(SetWarp(true));
        roundtrip(RespondToEncounter {
            choice: "accept_quest".into(),
        });
        roundtrip(HailShip);
        roundtrip(LoginRequest {
            username: "alice".into(),
            password: "pass123".into(),
        });
        roundtrip(LoginResponse {
            success: true,
            message: "Welcome back.".into(),
            credits: 500,
            cargo: vec![("food".into(), 5)],
            system: "sol".into(),
        });
        roundtrip(Notice {
            text: "No fuel, no glory.".into(),
        });
        roundtrip(PlayerDied);
        roundtrip(Explosion {
            position: (10.0, -4.0),
            system: "sol".into(),
        });
    }
}
