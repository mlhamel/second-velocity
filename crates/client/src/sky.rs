use bevy::camera::ClearColor;
use bevy::prelude::*;
use sv_protocol::{ShipInfo, SystemMembership};

use crate::MyNetworkId;
use crate::input::ClientContent;

pub const SKY_Z: f32 = -30.0;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentSky>()
            .add_systems(Update, update_sky_color);
    }
}

#[derive(Resource, Default)]
struct CurrentSky {
    system: Option<String>,
}

fn update_sky_color(
    my_id: Res<MyNetworkId>,
    mut current: ResMut<CurrentSky>,
    mut clear: ResMut<ClearColor>,
    content: Res<ClientContent>,
    my_ships: Query<(&ShipInfo, &SystemMembership)>,
) {
    let system = my_ships
        .iter()
        .find(|(info, _)| info.owner == Some(my_id.0))
        .map(|(_, membership)| membership.0.clone());

    let Some(system) = system else {
        return;
    };
    if current.system.as_deref() == Some(system.as_str()) {
        return;
    }
    current.system = Some(system.clone());

    let Some(def) = content.0.system(&system) else {
        return;
    };
    clear.0 = crate::sky_gen::sky_clear_color(&def.id);
}
