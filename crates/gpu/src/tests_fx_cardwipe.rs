//! Card Wipe and Curl Noise CPU/GPU comparisons.
use effectcraft_keyframe::Value;
use effectcraft_project::BitDepth;
use effectcraft_render::RenderOpts;
use effectcraft_time::Tick;

use crate::tests::{Scene, check, compare_at, diff, gpu, n, opts, pattern, set};

pub(crate) fn e(v: u32) -> Value {
    Value::Enum(v)
}

/// The effect alone on a buffer at layer time `t` (`allow` = fraction of pixels over 1e-3).
pub(crate) fn direct(id: &str, vals: &[(&str, Value)], t: f64, allow: f64) {
    let Some(g) = gpu() else { return };
    let spec = effectcraft_effects::find(id).unwrap();
    let size = [70.0, 44.0];
    let mut params =
        effectcraft_effects::Params { values: spec.params.iter().map(|ps| (ps.id.to_string(), effectcraft_effects::default_value(ps, size))).collect() };
    for (k, v) in vals {
        params.values.insert(k.to_string(), v.clone());
    }
    for scale in [1.0, 0.5] {
        let img = pattern(7, (size[0] * scale) as u32, (size[1] * scale) as u32);
        let buf = effectcraft_effects::Buf { img, offset: [0.0, 0.0], scale };
        let ctx = || effectcraft_effects::EffectCtx { params: &params, time: t, layer_size: size, seed: 11, adjustment: false, env: Default::default() };
        let cpu = (spec.render)(&ctx(), buf.clone());
        let out = effectcraft_render::Accelerator::effects(g, &[effectcraft_render::FxStep { spec, ctx: ctx() }], &buf, None).expect("the GPU runs the effect");
        assert_eq!((out.offset, out.scale), (cpu.offset, cpu.scale), "{id}: geometry");
        let d = diff(&cpu.img, &out.img, 1e-3);
        assert!(
            d.over as f64 <= allow * d.total as f64,
            "{id} {vals:?} scale {scale}: {} of {} pixels over 1e-3 (max {}); worst {:?}",
            d.over,
            d.total,
            d.max,
            d.worst
        );
    }
}

/// A footage layer with the effect over a background and a second footage layer (the target of
/// `Value::Layer(Some(0))` parameters), composited at 8 and 32 bpc at comp time `t`.
pub(crate) fn composited(id: &str, vals: &[(&str, Value)], t: f64, allow: f64) {
    for depth in [BitDepth::Bpc8, BitDepth::Bpc32] {
        let mut s = Scene::new(depth);
        let bg = s.solid([0.15, 0.1, 0.2], 97, 61);
        s.push(bg);
        let other = s.footage(60, 50);
        let other_id = s.push(other);
        let mut l = s.footage(70, 44);
        let vals: Vec<(&str, Value)> =
            vals.iter().map(|(k, v)| (*k, if *v == Value::Layer(Some(0)) { Value::Layer(Some(other_id.0)) } else { v.clone() })).collect();
        s.effect(&mut l, id, &vals);
        set(&mut l, "transform/rotation", Value::Scalar(8.0));
        s.push(l);
        let at = Tick::from_seconds_f64(t);
        check(&format!("{id} {vals:?} {depth:?}"), compare_at(&s, opts(), at), allow);
        check(&format!("{id} {vals:?} {depth:?} half"), compare_at(&s, RenderOpts { scale: 0.5, ..opts() }, at), allow);
    }
}

pub(crate) fn case(id: &str, vals: &[(&str, Value)], t: f64, allow: f64) {
    direct(id, vals, t, allow);
    composited(id, vals, t, allow);
}

#[test]
fn card_wipe() {
    case("ec.transition.cardwipe", &[("completion", n(40.0))], 0.0, 0.0);
    composited("ec.transition.cardwipe", &[("completion", n(60.0)), ("backLayer", Value::Layer(Some(0))), ("flipAxis", e(1))], 0.0, 0.0);
}
#[test]
fn curl_noise() {
    case("ec.noise.curlnoise", &[], 0.0, 0.0);
    case("ec.noise.curlnoise", &[("view", e(1))], 0.0, 0.0);
    case(
        "ec.noise.curlnoise",
        &[("edgeBehavior", e(1)), ("steps", n(8.0)), ("displacementAmount", n(60.0)), ("scale", n(20.0)), ("rotation", n(30.0)), ("evolution", n(90.0))],
        0.0,
        0.0,
    );
    case("ec.noise.curlnoise", &[("displacementAmount", n(0.0))], 0.0, 0.0);
}
