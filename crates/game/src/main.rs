mod assets;
mod game;
mod inbox;
mod input;
mod track;
mod view;

use macroquad::prelude::*;

fn conf() -> Conf {
    Conf {
        window_title: "Zero Degrees".to_owned(),
        // Desktop dev window in portrait aspect; ignored on Android/iOS,
        // where the game always gets the full screen.
        window_width: 480,
        window_height: 854,
        high_dpi: true,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    // On desktop, load assets relative to the crate so `cargo run -p game`
    // works from any directory; mobile bundles resolve from the app package.
    set_pc_assets_folder(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));

    let assets = assets::Assets::load().await;
    let mut game = game::Game::new(assets);

    loop {
        game.frame();
        next_frame().await
    }
}
