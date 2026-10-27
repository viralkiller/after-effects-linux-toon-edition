use super::*;
use crate::{
    context::Enc,
    ops,
    tests::{Pattern, Scene, gpu, pattern},
};
use effectcraft_geom::{Mat3, vec2};
use effectcraft_project::BitDepth;
use effectcraft_raster::Sampling;

#[test]
fn bounded_composite_matches_full_canvas_and_preserves_shared_images() {
    let Some(g) = gpu() else { return };
    for levels in [None, Some(255.0), Some(32768.0)] {
        let mut e = Enc::new(&g.ctx);
        let mut canvas = e.image(97, 61);
        canvas.levels = levels;
        let src = g.ctx.upload_image(&pattern(1, 13, 9)).unwrap();
        let m = Mat3::translate(vec2(17.25, 9.75));
        let reference = ops::warp_q(&mut e, &canvas, &src, &m, Sampling::Bilinear, effectcraft_color::BlendMode::Normal, 0.7, 1, None, levels);
        let held = canvas.clone();
        assert!(!ops::warp_in_place(&mut e, &mut canvas, &src, &m, Sampling::Bilinear, 0.7, 1, levels));
        drop(held);
        assert!(ops::warp_in_place(&mut e, &mut canvas, &src, &m, Sampling::Bilinear, 0.7, 1, levels));
        let a = e.download(&reference).unwrap();
        let b = e.download(&canvas).unwrap();
        assert_eq!(a.data, b.data);
        assert_eq!(b.get(0, 0), [0.0; 4]);
    }
}
#[test]
fn half_textures_convert_on_gpu_and_full_precision_encoders_stay_full_precision() {
    let Some(g) = gpu() else { return };
    let mut e = Enc::new(&g.ctx);
    if !e.enable_half() {
        eprintln!("FP16 storage unsupported; skipping");
        return;
    }
    let src = g.ctx.upload_image(&pattern(2, 17, 13)).unwrap();
    let out = e.scratch(17, 13);
    e.pixels("copy_pixels", &context::Params::default(), &src, None, &out, None);
    assert_eq!(out.texture.format(), wgpu::TextureFormat::Rgba16Float);
    let read = e.download(&out).unwrap();
    let reference = pattern(2, 17, 13);
    for (a, b) in read.data.iter().flatten().zip(reference.data.iter().flatten()) {
        assert!((a - b).abs() <= 0.0005);
    }
    let mut full = Enc::new(&g.ctx);
    assert_eq!(full.scratch(17, 13).texture.format(), context::FORMAT);
}
#[test]
fn gpu_layer_effects_avoid_cpu_readback_and_match_cpu() {
    let Some(g) = gpu() else { return };
    let mut s = Scene::new(BitDepth::Bpc32);
    let mut l = s.solid([0.25, 0.5, 0.75], 17, 13);
    s.effect(&mut l, "ec.blur.gaussian", &[("blurriness", effectcraft_keyframe::Value::Scalar(2.0))]);
    s.push(l);
    let cache = effectcraft_render::LayerCache::new(1 << 20);
    let mut r = Renderer::new(&s.p, &Pattern, effectcraft_render::RenderOpts::default());
    r.opts.backend = effectcraft_render::Backend::Gpu;
    r.cache = Some(&cache);
    r.accel = Some(g);
    let before = g.ctx.transfer_stats();
    let display = g.render_display(&r, s.cid, Tick::ZERO).unwrap();
    let after = g.ctx.transfer_stats();
    assert_eq!(before.readbacks, after.readbacks);
    let warm = g.render_display(&r, s.cid, Tick::ZERO).unwrap();
    assert_eq!(g.ctx.transfer_stats().readbacks, after.readbacks);
    assert_eq!(g.ctx.transfer_stats().uploads, after.uploads);
    let pixels = g.read_display(&display).unwrap();
    assert_eq!(pixels, g.read_display(&warm).unwrap());
    let cpu = Renderer::new(&s.p, &Pattern, effectcraft_render::RenderOpts::default()).comp_frame_cpu(s.cid, Tick::ZERO);
    let cpu: Vec<u8> = cpu
        .data
        .iter()
        .flat_map(|p| {
            let a = p[3].clamp(0.0, 1.0);
            [p[0].clamp(0.0, a), p[1].clamp(0.0, a), p[2].clamp(0.0, a), a].map(|v| (v * 255.0 + 0.5) as u8)
        })
        .collect();
    for (a, b) in pixels.iter().zip(cpu.iter()) {
        assert!((*a as i16 - *b as i16).abs() <= 2);
    }
}

#[test]
fn half_preview_gate_keeps_hdr_and_motion_accumulation_full_precision() {
    let mut s = Scene::new(BitDepth::Bpc16);
    s.p.comp_mut(s.cid).unwrap().enable_motion_blur = false;
    let mut opts = effectcraft_render::RenderOpts { motion_blur: false, ..Default::default() };
    assert!(preview_half_eligible(&Renderer::new(&s.p, &Pattern, opts), s.cid, 0));
    s.p.settings.bit_depth = BitDepth::Bpc32;
    assert!(!preview_half_eligible(&Renderer::new(&s.p, &Pattern, opts), s.cid, 0));
    s.p.settings.bit_depth = BitDepth::Bpc16;
    s.p.comp_mut(s.cid).unwrap().enable_motion_blur = true;
    opts.motion_blur = true;
    assert!(!preview_half_eligible(&Renderer::new(&s.p, &Pattern, opts), s.cid, 0));
}
