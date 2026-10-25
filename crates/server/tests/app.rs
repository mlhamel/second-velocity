use std::path::Path;

use sv_server::{Content, TickMode, build_server_app};

#[test]
fn server_app_loads_base_pack() {
    let content_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let app = build_server_app(content_root, TickMode::Fixed, None).expect("base pack must load");
    assert!(app.world().contains_resource::<Content>());
}
