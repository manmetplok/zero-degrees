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

    // Dev harness: ZD_SHOT=file.png saves a screenshot and exits, letting
    // tooling verify visuals. With ZD_DEMO=1 a scripted run (clear, skip,
    // mid-run ingest) plays and several numbered shots are saved.
    let shot: Option<String> = std::env::var("ZD_SHOT").ok();
    let demo = std::env::var("ZD_DEMO").is_ok();
    let mut frames: u32 = 0;

    loop {
        if demo {
            game.demo_tick(frames);
        }
        game.frame();
        frames += 1;
        if let Some(path) = &shot {
            if demo {
                if let Some(n) = [90u32, 210, 330].iter().position(|f| *f == frames) {
                    get_screen_data()
                        .export_png(&path.replace(".png", &format!("_{}.png", n + 1)));
                    if n == 2 {
                        return;
                    }
                }
            } else if frames == 90 {
                get_screen_data().export_png(path);
                return;
            }
        }
        next_frame().await
    }
}
