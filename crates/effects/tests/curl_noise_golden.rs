//! Golden render comparisons for ec.noise.curlnoise.
//! Exact hashes are platform-specific; other platforms check repeatability.

use effectcraft_effects::{Buf, EffectCtx, EffectEnv, EffectHost, LayerPixels, Params, apply, default_value, find};
use effectcraft_keyframe::Value;
use effectcraft_raster::Image;

const W: u32 = 64;
const H: u32 = 48;
/// Layer ids served by [`Host`]: a texture and a luminance map.
const TEX: u64 = 7;

/// The effect's own layer: a colour ramp with a clear hole and a half-transparent band
/// (`hdr` makes part of it overbright).
fn layer(hdr: bool) -> Image {
    let mut img = Image::new(W, H);
    for y in 0..H {
        for x in 0..W {
            let (u, v) = (x as f32 / W as f32, y as f32 / H as f32);
            let (dx, dy) = (x as f32 - 40.0, y as f32 - 20.0);
            let a = if dx * dx + dy * dy < 64.0 {
                0.0
            } else if (30..36).contains(&y) {
                0.45
            } else {
                1.0
            };
            let gain = if hdr && x < 20 { 1.6 } else { 1.0 };
            let c = [u * gain, v * gain, (0.3 + 0.4 * (u * 9.0).sin().abs()) * gain];
            img.set(x, y, [c[0] * a, c[1] * a, c[2] * a, a]);
        }
    }
    img
}

/// Another layer: a checker texture (`TEX`) or a radial luminance map (`MAP`).
fn other(id: u64) -> Image {
    let mut img = Image::new(W, H);
    for y in 0..H {
        for x in 0..W {
            let p = if id == TEX {
                let k = ((x / 8 + y / 8) % 2) as f32;
                [0.9 * k + 0.1, 0.3 + 0.5 * (1.0 - k), 0.6, 1.0]
            } else {
                let d = ((x as f32 - 32.0).powi(2) + (y as f32 - 24.0).powi(2)).sqrt() / 40.0;
                let l = (1.0 - d).clamp(0.0, 1.0);
                [l, l, l, 1.0]
            };
            img.set(x, y, p);
        }
    }
    img
}

struct Host;
impl EffectHost for Host {
    fn layer(&self, id: u64, _: bool) -> Option<LayerPixels> {
        let img = other(id);
        Some(LayerPixels { buf: Buf { img, offset: [0.0; 2], scale: 1.0 }, size: [W as f64, H as f64] })
    }
    fn layer_at(&self, id: u64, comp_time: f64, _: bool) -> Option<LayerPixels> {
        // A frame that changes with time (Particle Playground's Layer Map times).
        let mut img = other(id);
        let k = ((comp_time * 30.0).round() as i64).rem_euclid(5) as f32 * 0.15;
        for p in img.data.iter_mut() {
            p[0] = (p[0] + k).min(1.0) * p[3];
        }
        Some(LayerPixels { buf: Buf { img, offset: [0.0; 2], scale: 1.0 }, size: [W as f64, H as f64] })
    }
    fn audio(&self, _: u64, _: f64, _: usize, _: u32) -> Option<Vec<f32>> {
        None
    }
}

fn n(v: f64) -> Value {
    Value::Scalar(v)
}
fn e(v: u32) -> Value {
    Value::Enum(v)
}

type Set = Vec<(&'static str, Value)>;

/// (effect id, parameter sets) — the first set of each is the defaults.
fn cases() -> Vec<(&'static str, Vec<Set>)> {
    vec![(
        "ec.noise.curlnoise",
        vec![
            vec![],
            vec![("view", e(1))],
            vec![("edgeBehavior", e(1)), ("steps", n(8.0)), ("displacementAmount", n(60.0)), ("scale", n(20.0)), ("rotation", n(30.0)), ("evolution", n(90.0))],
        ],
    )]
}

/// Layer times, in order (the third seeks backwards).
const TIMES: [f64; 4] = [0.5, 1.5, 0.8, 2.2];

fn fnv(img: &Image) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in [img.width, img.height].iter().flat_map(|v| v.to_le_bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    for p in &img.data {
        for c in p {
            for b in c.to_bits().to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    h
}

fn render(id: &str, set: &Set, time: f64, bpc8: bool, half: bool) -> Image {
    let spec = find(id).unwrap_or_else(|| panic!("no effect {id}"));
    let size = [W as f64, H as f64];
    let mut params = Params { values: spec.params.iter().map(|p| (p.id.to_string(), default_value(p, size))).collect() };
    for (k, v) in set {
        assert!(params.values.contains_key(*k), "{id}: unknown param {k}");
        params.values.insert(k.to_string(), v.clone());
    }
    let mut img = layer(!bpc8);
    if bpc8 {
        img.clamp01();
        img.quantize(255.0);
    }
    let mut buf = Buf { img, offset: [0.0; 2], scale: 1.0 };
    if half {
        // A half-resolution preview buffer padded by 3 px.
        let (w2, h2) = (W / 2, H / 2);
        let mut small = Image::new(w2, h2);
        for y in 0..h2 {
            for x in 0..w2 {
                small.set(x, y, buf.img.sample_bilinear(x as f64 * 2.0 + 1.0, y as f64 * 2.0 + 1.0));
            }
        }
        buf = Buf { img: small, offset: [0.0; 2], scale: 0.5 };
        buf.pad(3);
    }
    let host = Host;
    let env = EffectEnv { host: Some(&host), frame_rate: 30.0, comp_time: time, ..Default::default() };
    let ctx = EffectCtx { params: &params, time, layer_size: size, seed: 1, adjustment: false, env };
    let mut out = apply(spec, &ctx, buf).img;
    if bpc8 {
        out.clamp01();
        out.quantize(255.0);
    }
    out
}

/// Every case's name and hash, in a fixed order.
fn hashes() -> Vec<(String, u64)> {
    let out_dir = std::env::var_os("CURL_NOISE_GOLDEN_OUT").map(std::path::PathBuf::from);
    if let Some(d) = &out_dir {
        std::fs::create_dir_all(d).unwrap();
    }
    let mut v = Vec::new();
    for (id, sets) in cases() {
        for (si, set) in sets.iter().enumerate() {
            for (bpc8, half) in [(true, false), (false, false), (true, true)] {
                if half && si > 0 {
                    continue;
                }
                for t in TIMES {
                    let img = render(id, set, t, bpc8, half);
                    let name = format!("{id}/{si}/{}{}/t{t}", if bpc8 { "8bpc" } else { "32bpc" }, if half { "/half" } else { "" });
                    if let Some(d) = &out_dir {
                        let bytes: Vec<u8> = img.data.iter().flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes())).collect();
                        let file = format!("{}_{}x{}.f32", name.replace('/', "_"), img.width, img.height);
                        std::fs::write(d.join(file), bytes).unwrap();
                    }
                    v.push((name, fnv(&img)));
                }
            }
        }
    }
    v
}

#[test]
fn curl_noise_golden_hashes() {
    let got = hashes();
    if std::env::var_os("CURL_NOISE_GOLDEN_PRINT").is_some() {
        for (name, h) in &got {
            println!("    (\"{name}\", 0x{h:016x}),");
        }
    }
    // Deterministic: a second pass (warm caches) renders the same frames.
    assert_eq!(got, hashes(), "effects render deterministically");
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        let bad: Vec<String> = got
            .iter()
            .zip(GOLDEN)
            .filter(|((n, h), (gn, gh))| n != gn || h != gh)
            .map(|((n, h), (_, gh))| format!("{n}: 0x{h:016x} (pinned 0x{gh:016x})"))
            .collect();
        assert_eq!(got.len(), GOLDEN.len(), "case count");
        assert!(bad.is_empty(), "CPU output changed:\n{}", bad.join("\n"));
    }
}

/// Pinned hashes (aarch64 macOS).
const GOLDEN: &[(&str, u64)] = &[
    ("ec.noise.curlnoise/0/8bpc/t0.5", 0xce5b0d9a6bbe529c),
    ("ec.noise.curlnoise/0/8bpc/t1.5", 0xce5b0d9a6bbe529c),
    ("ec.noise.curlnoise/0/8bpc/t0.8", 0xce5b0d9a6bbe529c),
    ("ec.noise.curlnoise/0/8bpc/t2.2", 0xce5b0d9a6bbe529c),
    ("ec.noise.curlnoise/0/32bpc/t0.5", 0x18de53fb08fbec6c),
    ("ec.noise.curlnoise/0/32bpc/t1.5", 0x18de53fb08fbec6c),
    ("ec.noise.curlnoise/0/32bpc/t0.8", 0x18de53fb08fbec6c),
    ("ec.noise.curlnoise/0/32bpc/t2.2", 0x18de53fb08fbec6c),
    ("ec.noise.curlnoise/0/8bpc/half/t0.5", 0x00124f8f5a0bb582),
    ("ec.noise.curlnoise/0/8bpc/half/t1.5", 0x00124f8f5a0bb582),
    ("ec.noise.curlnoise/0/8bpc/half/t0.8", 0x00124f8f5a0bb582),
    ("ec.noise.curlnoise/0/8bpc/half/t2.2", 0x00124f8f5a0bb582),
    ("ec.noise.curlnoise/1/8bpc/t0.5", 0x73e04e5ca0589de4),
    ("ec.noise.curlnoise/1/8bpc/t1.5", 0x73e04e5ca0589de4),
    ("ec.noise.curlnoise/1/8bpc/t0.8", 0x73e04e5ca0589de4),
    ("ec.noise.curlnoise/1/8bpc/t2.2", 0x73e04e5ca0589de4),
    ("ec.noise.curlnoise/1/32bpc/t0.5", 0x111d2c248e7b334b),
    ("ec.noise.curlnoise/1/32bpc/t1.5", 0x111d2c248e7b334b),
    ("ec.noise.curlnoise/1/32bpc/t0.8", 0x111d2c248e7b334b),
    ("ec.noise.curlnoise/1/32bpc/t2.2", 0x111d2c248e7b334b),
    ("ec.noise.curlnoise/2/8bpc/t0.5", 0xe62ab777e41bf94e),
    ("ec.noise.curlnoise/2/8bpc/t1.5", 0xe62ab777e41bf94e),
    ("ec.noise.curlnoise/2/8bpc/t0.8", 0xe62ab777e41bf94e),
    ("ec.noise.curlnoise/2/8bpc/t2.2", 0xe62ab777e41bf94e),
    ("ec.noise.curlnoise/2/32bpc/t0.5", 0xd477a194b7a2d123),
    ("ec.noise.curlnoise/2/32bpc/t1.5", 0xd477a194b7a2d123),
    ("ec.noise.curlnoise/2/32bpc/t0.8", 0xd477a194b7a2d123),
    ("ec.noise.curlnoise/2/32bpc/t2.2", 0xd477a194b7a2d123),
];
