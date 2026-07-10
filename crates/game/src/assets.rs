//! Asset loading. Sprite strips are pre-rendered from assets/models/runner.glb
//! by tools/render_sprites.py (macroquad has no skeletal animation, so the
//! glTF clips are baked to 2D frames offline).

use macroquad::prelude::*;

/// Fraction of a sprite frame below the character's foot line, set by the
/// render script's ground-plane pinning.
pub const GROUND_FRAC: f32 = 0.1;

pub struct Strip {
    pub texture: Texture2D,
    pub frames: u32,
    pub fps: f32,
    pub looped: bool,
}

impl Strip {
    async fn load(path: &str, frames: u32, fps: f32, looped: bool) -> Self {
        let texture = load_texture(path).await.expect(path);
        texture.set_filter(FilterMode::Linear);
        Self {
            texture,
            frames,
            fps,
            looped,
        }
    }

    /// Source rect for the frame at animation time `t` (seconds).
    pub fn source(&self, t: f32) -> Rect {
        let raw = (t * self.fps) as u32;
        let frame = if self.looped {
            raw % self.frames
        } else {
            raw.min(self.frames - 1)
        };
        let side = self.texture.height();
        Rect::new(frame as f32 * side, 0.0, side, side)
    }

    /// Source rect at normalized progress `p` in [0, 1], for animations that
    /// must sync to gameplay (e.g. the jump arc) rather than run on a clock.
    pub fn source_at(&self, p: f32) -> Rect {
        let frame = ((p.clamp(0.0, 1.0) * self.frames as f32) as u32).min(self.frames - 1);
        let side = self.texture.height();
        Rect::new(frame as f32 * side, 0.0, side, side)
    }
}

pub struct Assets {
    pub run: Strip,
    pub idle: Strip,
    pub jump: Strip,
    pub wave: Strip,
}

impl Assets {
    pub async fn load() -> Self {
        // Frame counts match tools/render_sprites.py invocation (see its JSON
        // sidecars in assets/sprites/).
        Self {
            run: Strip::load("sprites/run.png", 12, 18.0, true).await,
            idle: Strip::load("sprites/idle.png", 8, 8.0, true).await,
            jump: Strip::load("sprites/jump.png", 10, 12.0, false).await,
            wave: Strip::load("sprites/wave.png", 10, 10.0, true).await,
        }
    }
}
