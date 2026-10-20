//! Golden render comparisons for ec.transition.cardwipe.
//! Exact hashes are platform-specific; other platforms check repeatability.

use effectcraft_effects::{Buf, EffectCtx, EffectEnv, EffectHost, LayerPixels, Params, apply, default_value, find};
use effectcraft_keyframe::Value;
use effectcraft_raster::Image;

const W: u32 = 64;
const H: u32 = 48;
/// Layer ids served by [`Host`]: a texture and a luminance map.
const TEX: u64 = 7;
const MAP: u64 = 8;

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
fn l(id: u64) -> Value {
    Value::Layer(Some(id))
}

type Set = Vec<(&'static str, Value)>;

/// (effect id, parameter sets) — the first set of each is the defaults.
fn cases() -> Vec<(&'static str, Vec<Set>)> {
    vec![
        (
            "ec.transition.cardwipe",
            vec![
                vec![],
                vec![("completion", n(40.0))],
                vec![
                    ("completion", n(60.0)),
                    ("flipAxis", e(2)),
                    ("flipOrder", e(8)),
                    ("gradientLayer", l(MAP)),
                    ("backLayer", l(TEX)),
                    ("timingRandomness", n(0.5)),
                    ("rotationJitter/xRotJitterAmount", n(20.0)),
                ],
                vec![("completion", n(50.0)), ("backSelf", Value::Bool(true)), ("cameraPosition/yRotation", n(20.0)), ("positionJitter/zJitterAmount", n(1.0))],
            ],
        ),
    ]
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
    let out_dir = std::env::var_os("CARD_WIPE_GOLDEN_OUT").map(std::path::PathBuf::from);
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
fn card_wipe_golden_hashes() {
    let got = hashes();
    if std::env::var_os("CARD_WIPE_GOLDEN_PRINT").is_some() {
        for (name, h) in &got {
            println!("    (\"{name}\", 0x{h:016x}),");
        }
    }
    // Deterministic: a second pass (warm caches) renders the same frames.
    assert_eq!(got, hashes(), "effects render deterministically");
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert_eq!(got.len(), GOLDEN.len(), "case count");
        let bad = mismatches(&got, GOLDEN, LIBM_VARIANTS);
        assert!(bad.is_empty(), "CPU output changed (if it may be libm, see the module docs):\n{}", bad.join("\n"));
    }
}

/// The cases whose hash is neither the pinned one nor one of the case's [`LIBM_VARIANTS`].
fn mismatches(got: &[(String, u64)], golden: &[(&str, u64)], variants: &[(&str, u64, &str)]) -> Vec<String> {
    got.iter()
        .zip(golden)
        .filter(|((n, h), (gn, gh))| n != gn || (h != gh && !variants.iter().any(|(vn, vh, _)| vn == n && vh == h)))
        .map(|((n, h), (_, gh))| format!("{n}: 0x{h:016x} (pinned 0x{gh:016x})"))
        .collect()
}

/// A libm variant passes for its own case only (#232).
#[test]
fn libm_variants_are_accepted_for_their_case_only() {
    let golden = [("a", 1), ("b", 2)];
    let variants = [("a", 9, "another libm")];
    let got = |a: u64, b: u64| vec![("a".to_string(), a), ("b".to_string(), b)];
    assert!(mismatches(&got(1, 2), &golden, &variants).is_empty());
    assert!(mismatches(&got(9, 2), &golden, &variants).is_empty(), "the variant");
    assert_eq!(mismatches(&got(9, 9), &golden, &variants), ["b: 0x0000000000000009 (pinned 0x0000000000000002)"], "not another case's");
    assert_eq!(mismatches(&got(3, 2), &golden, &variants).len(), 1);
    assert!(LIBM_VARIANTS.iter().all(|(n, ..)| GOLDEN.iter().any(|(g, _)| g == n)), "every variant names a pinned case");
}

/// Hashes also accepted for a case: the same frame rendered by the same code with another macOS
/// release's libm (case, hash, where it was seen).
const LIBM_VARIANTS: &[(&str, u64, &str)] = &[
    // #232: the pinning commit (ce6ece4) renders byte-identical frames there.
];

/// Pinned hashes (aarch64 macOS).
const GOLDEN: &[(&str, u64)] = &[
    ("ec.transition.cardwipe/0/8bpc/t0.5", 0x8ad88caae775ea83),
    ("ec.transition.cardwipe/0/8bpc/t1.5", 0x8ad88caae775ea83),
    ("ec.transition.cardwipe/0/8bpc/t0.8", 0x8ad88caae775ea83),
    ("ec.transition.cardwipe/0/8bpc/t2.2", 0x8ad88caae775ea83),
    ("ec.transition.cardwipe/0/32bpc/t0.5", 0xe2b8332b96a1c76e),
    ("ec.transition.cardwipe/0/32bpc/t1.5", 0xe2b8332b96a1c76e),
    ("ec.transition.cardwipe/0/32bpc/t0.8", 0xe2b8332b96a1c76e),
    ("ec.transition.cardwipe/0/32bpc/t2.2", 0xe2b8332b96a1c76e),
    ("ec.transition.cardwipe/0/8bpc/half/t0.5", 0x730351fffb20e0c8),
    ("ec.transition.cardwipe/0/8bpc/half/t1.5", 0x730351fffb20e0c8),
    ("ec.transition.cardwipe/0/8bpc/half/t0.8", 0x730351fffb20e0c8),
    ("ec.transition.cardwipe/0/8bpc/half/t2.2", 0x730351fffb20e0c8),
    ("ec.transition.cardwipe/1/8bpc/t0.5", 0x955dfd5e5761773a),
    ("ec.transition.cardwipe/1/8bpc/t1.5", 0x955dfd5e5761773a),
    ("ec.transition.cardwipe/1/8bpc/t0.8", 0x955dfd5e5761773a),
    ("ec.transition.cardwipe/1/8bpc/t2.2", 0x955dfd5e5761773a),
    ("ec.transition.cardwipe/1/32bpc/t0.5", 0x90476fca5ecce0d5),
    ("ec.transition.cardwipe/1/32bpc/t1.5", 0x90476fca5ecce0d5),
    ("ec.transition.cardwipe/1/32bpc/t0.8", 0x90476fca5ecce0d5),
    ("ec.transition.cardwipe/1/32bpc/t2.2", 0x90476fca5ecce0d5),
    ("ec.transition.cardwipe/2/8bpc/t0.5", 0xbd2f2ff54b9ec732),
    ("ec.transition.cardwipe/2/8bpc/t1.5", 0x65d542cd14443396),
    ("ec.transition.cardwipe/2/8bpc/t0.8", 0xdfbe7b6b78a0a3f0),
    ("ec.transition.cardwipe/2/8bpc/t2.2", 0x165d56d4a9d1335f),
    ("ec.transition.cardwipe/2/32bpc/t0.5", 0x9434385a35a2dd49),
    ("ec.transition.cardwipe/2/32bpc/t1.5", 0xfe39b6b0d8f82589),
    ("ec.transition.cardwipe/2/32bpc/t0.8", 0x48cd644a9101ebff),
    ("ec.transition.cardwipe/2/32bpc/t2.2", 0x5ffafba81e1a2add),
    ("ec.transition.cardwipe/3/8bpc/t0.5", 0x011acd70baced759),
    ("ec.transition.cardwipe/3/8bpc/t1.5", 0x4f3e12557fa83c91),
    ("ec.transition.cardwipe/3/8bpc/t0.8", 0x31ba3f7b9e98633a),
    ("ec.transition.cardwipe/3/8bpc/t2.2", 0x040a6d3bd233ce47),
    ("ec.transition.cardwipe/3/32bpc/t0.5", 0x5f3be2dae40280cc),
    ("ec.transition.cardwipe/3/32bpc/t1.5", 0x3abe44793029ef0f),
    ("ec.transition.cardwipe/3/32bpc/t0.8", 0xa729a1df32f69328),
    ("ec.transition.cardwipe/3/32bpc/t2.2", 0xc6083cd48e2bcc52),
];
