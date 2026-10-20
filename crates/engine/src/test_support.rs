//! Shared test fixtures retained independently of removed feature test suites.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use effectcraft_color::Label;
use effectcraft_project::build;
use effectcraft_project::{Footage, FootageKind, ItemId, ItemKind, LayerId, LayerSource};
use effectcraft_raster::Image;
use effectcraft_render::FootageSource;
use effectcraft_time::{FrameRate, Tick};
use serde_json::json;

use crate::Session;

pub(crate) const W: u32 = 320;
pub(crate) const H: u32 = 240;
pub(crate) const FPS: u32 = 25;

pub(crate) fn texture(x: f64, y: f64, seed: u32) -> f32 {
    let mut v = 0.0f64;
    let mut s = seed.wrapping_mul(2654435761).wrapping_add(12345);
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        (s % 10_000) as f64 / 10_000.0
    };
    for _ in 0..6 {
        let ang = rnd() * std::f64::consts::TAU;
        let period = 9.0 + rnd() * 25.0;
        let phase = rnd() * std::f64::consts::TAU;
        let k = std::f64::consts::TAU / period;
        v += (k * (x * ang.cos() + y * ang.sin()) + phase).sin() * (0.4 + rnd()) * 0.1;
    }
    (0.5 + v).clamp(0.0, 1.0) as f32
}

/// Footage computed per frame (cached).
pub(crate) struct Synth {
    make: Box<dyn Fn(u32) -> Image + Send + Sync>,
    cache: Mutex<HashMap<u32, Arc<Image>>>,
}

impl FootageSource for Synth {
    fn frame(&self, _: ItemId, _: &Footage, t: Tick) -> Option<Arc<Image>> {
        let f = (t.seconds() * FPS as f64).round().max(0.0) as u32;
        let mut c = self.cache.lock().unwrap();
        Some(c.entry(f).or_insert_with(|| Arc::new((self.make)(f))).clone())
    }
}

/// Patch pose on frame `f`: centre, rotation (degrees), scale.
pub(crate) type Pose = fn(u32) -> ([f64; 2], f64, f64);

pub(crate) fn patch_frames(pose: Pose) -> impl Fn(u32) -> Image + Send + Sync {
    move |f| {
        let (c, rot, sc) = pose(f);
        let (sn, cs) = rot.to_radians().sin_cos();
        let mut img = Image::new(W, H);
        for y in 0..H {
            for x in 0..W {
                let p = [x as f64 + 0.5, y as f64 + 0.5];
                let d = [(p[0] - c[0]) / sc, (p[1] - c[1]) / sc];
                let l = [cs * d[0] + sn * d[1], -sn * d[0] + cs * d[1]];
                let v = if l[0].abs() <= 50.0 && l[1].abs() <= 50.0 { texture(l[0], l[1], 7) } else { 0.6 * texture(p[0] * 0.7, p[1] * 0.7, 3) };
                img.set(x, y, [v, v, v, 1.0]);
            }
        }
        img
    }
}

/// A comp with the synthetic clip (layer `clip`) and a 100 × 100 solid above it (layer `solid`).
pub(crate) fn setup(make: impl Fn(u32) -> Image + Send + Sync + 'static) -> (Session, LayerId, LayerId) {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name": "Track", "width": W, "height": H, "frameRate": FPS, "duration": 1})).unwrap();
    let cid = s.active_comp_id().unwrap();
    let clip = s
        .edit("clip", None, |p, _| {
            let fr = FrameRate::new(FPS as i64, 1);
            let footage = Footage {
                path: "synthetic.mov".into(),
                kind: FootageKind::Video,
                width: W,
                height: H,
                pixel_aspect: 1.0,
                frame_rate: fr,
                native_rate: None,
                duration: Tick::from_seconds_f64(1.0),
                has_video: true,
                has_audio: false,
                alpha: Default::default(),
                premul_color: [0.0; 3],
                loop_count: 1,
                codec: String::new(),
                missing: false,
                sequence: vec![],
                color_profile: None,
                ..Default::default()
            };
            let item = p.add_item("clip", Label::Aqua, None, ItemKind::Footage(footage));
            let comp = p.comp(cid).unwrap().clone();
            let l = build::layer(p, &comp, "Clip", LayerSource::Footage { item }, (W, H), None);
            let id = l.id;
            p.comp_mut(cid).unwrap().layers.insert(0, l);
            Ok(id)
        })
        .unwrap();
    s.footage = Arc::new(Synth { make: Box::new(make), cache: Mutex::default() });
    let solid = s.execute("layer.newSolid", json!({"width": 100, "height": 100, "color": "#ff0000"})).unwrap()["layer"].as_u64().unwrap();
    s.execute("layer.select", json!({"layers": [clip.0]})).unwrap();
    (s, clip, LayerId(solid))
}

pub(crate) fn frame_time(f: u32) -> Tick {
    FrameRate::new(FPS as i64, 1).tick_of(f as i64)
}

/// Serialize tests that purge shared caches. Hold the guard for the test's scope.
pub(crate) fn cache_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
