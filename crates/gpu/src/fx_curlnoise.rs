//! GPU Curl Noise potential and streamline tracing.
use crate::context::{Enc, Params};
use crate::effects::GBuf;
use effectcraft_effects::EffectCtx;
pub(crate) const IDS: &[&str] = &["ec.noise.curlnoise"];
pub(crate) const KERNELS: &[&str] = &["fxp_curl_pot", "fxp_curl_vel", "fxp_curl_view", "fxp_curl_flow"];
pub(crate) fn apply(e: &mut Enc, _id: &str, ctx: &EffectCtx, b: GBuf) -> Option<GBuf> {
    curl_noise(e, ctx, b)
}
fn curl_noise(e: &mut Enc, ctx: &EffectCtx, b: GBuf) -> Option<GBuf> {
    let pr = ctx.params;
    let amount = pr.f("displacementAmount") * b.scale;
    let view = pr.e("view");
    if amount.abs() < 1e-6 && view == 0 {
        return Some(b);
    }
    let scale = (pr.f("scale").max(1.0) * b.scale) as f32;
    let rot = pr.f("rotation").to_radians() as f32;
    let off = pr.v2("offset");
    let (ox, oy) = ((off[0] * b.scale + b.offset[0]) as f32, (off[1] * b.scale + b.offset[1]) as f32);
    let evo = (pr.f("evolution") / 360.0) as f32;
    let seed = pr.f("randomSeed") as i64 as u32 ^ 0x51_7cc1;
    let steps = pr.f("steps").clamp(1.0, 16.0) as usize;
    let (w, h) = (b.img.width, b.img.height);
    let mut p = Params::default();
    p.u[0] = [seed, steps as u32, (pr.e("edgeBehavior") == 1) as u32, 0];
    p.f[0] = [ox, oy, scale, pr.f("complexity") as f32];
    p.f[1] = [rot.cos(), rot.sin(), evo * 4.0, amount as f32 / steps as f32];
    let pot = e.scratch(w, h);
    e.pixels("fxp_curl_pot", &p, &b.img, None, &pot, None);
    let vel = e.scratch(w, h);
    e.pixels("fxp_curl_vel", &p, &pot, None, &vel, None);
    let out = e.scratch(w, h);
    if view == 1 {
        e.pixels("fxp_curl_view", &p, &vel, None, &out, None);
    } else {
        e.pixels("fxp_curl_flow", &p, &b.img, Some(&vel), &out, None);
    }
    Some(GBuf { img: out, ..b })
}
