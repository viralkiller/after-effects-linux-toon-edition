use super::*;
use effectcraft_color::Label;
use effectcraft_project::{BitDepth, MaskMode, build};
use std::sync::atomic::{AtomicU64, Ordering};

struct Still {
    revision: AtomicU64,
    calls: AtomicU64,
}
impl FootageSource for Still {
    fn cache_revision(&self) -> Option<u64> {
        Some(self.revision.load(Ordering::Relaxed))
    }
    fn frame(&self, _: ItemId, f: &Footage, _: Tick) -> Option<Arc<Image>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let red = if self.revision.load(Ordering::Relaxed) == 0 { 0.25 } else { 0.75 };
        Some(Arc::new(Image::filled(f.width, f.height, [red, 0.0, 0.0, 1.0])))
    }
}
fn scene() -> (Project, ItemId) {
    let mut p = Project::default();
    p.settings.bit_depth = BitDepth::Bpc32;
    let c = Comp::new(64, 32, FrameRate::FPS_30, Tick::from_seconds_f64(2.0));
    let cid = p.add_item("Comp", Label::Sandstone, None, ItemKind::Comp(c.clone().into()));
    let fid = p.add_item(
        "Still",
        Label::Lavender,
        None,
        ItemKind::Footage(Footage { path: "still.png".into(), kind: FootageKind::Still, width: 16, height: 16, has_video: true, ..Default::default() }),
    );
    let l = build::layer(&mut p, &c, "Still", LayerSource::Footage { item: fid }, (16, 16), None);
    p.comp_mut(cid).unwrap().layers.push(l);
    (p, cid)
}
#[test]
fn still_pixels_reused_across_frames_and_invalidated_on_reload() {
    let (p, cid) = scene();
    let src = Still { revision: AtomicU64::new(0), calls: AtomicU64::new(0) };
    let cache = LayerCache::new(1 << 20);
    let mut r = Renderer::new(&p, &src, RenderOpts::default());
    r.cache = Some(&cache);
    let ctx = r.eval_ctx(cid, Tick::ZERO).unwrap();
    let l = &ctx.comp.layers[0];
    let a = r.content_buf(&ctx, l).unwrap();
    let b = r.content_buf(&ctx.at(FrameRate::FPS_30.tick_of(12)), l).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(src.calls.load(Ordering::Relaxed), 1);
    src.revision.store(1, Ordering::Relaxed);
    let c = r.content_buf(&ctx, l).unwrap();
    assert!(!Arc::ptr_eq(&a, &c));
    assert_ne!(a.img.data, c.img.data);
}
#[test]
fn still_keys_ignore_placement_but_include_masks_and_proxy_policy() {
    let (mut p, cid) = scene();
    let key = |p: &Project| {
        let c = p.comp(cid).unwrap();
        cache::layer_key(&EvalCtx::new(p, cid, c, Tick::ZERO), &c.layers[0], 1.0, false, false).unwrap()
    };
    let a = key(&p);
    p.comp_mut(cid).unwrap().layers[0].props.prop_mut("transform/position").unwrap().value = effectcraft_keyframe::Value::Vec3([2.25, 5.0, 0.0]);
    assert_eq!(a, key(&p));
    let src = Still { revision: AtomicU64::new(0), calls: AtomicU64::new(0) };
    let mut r = Renderer::new(&p, &src, RenderOpts::default());
    let ctx = r.eval_ctx(cid, Tick::ZERO).unwrap();
    let a = r.processed_key(&ctx, &ctx.comp.layers[0]);
    r.opts.proxy = effectcraft_project::render_queue::ProxyUse::UseNone;
    assert_ne!(a, r.processed_key(&ctx, &ctx.comp.layers[0]));
    let mut ids = p.next_id;
    let mask = build::mask(&mut build::Ids(&mut ids), "Mask", effectcraft_keyframe::ShapePath::rect([8.0, 8.0], 8.0, 8.0), MaskMode::Add, [255, 0, 0]);
    p.next_id = ids;
    p.comp_mut(cid).unwrap().layers[0].props.sub_mut("masks").unwrap().children.push(mask.into());
    assert_ne!(key(&p), key(&scene().0));
}
#[test]
fn movies_still_decline_the_processed_still_cache() {
    let (mut p, cid) = scene();
    let LayerSource::Footage { item } = p.comp(cid).unwrap().layers[0].source else { panic!() };
    let ItemKind::Footage(f) = &mut p.item_mut(item).unwrap().kind else { panic!() };
    f.kind = FootageKind::Video;
    let c = p.comp(cid).unwrap();
    assert!(cache::layer_key(&EvalCtx::new(&p, cid, c, Tick::ZERO), &c.layers[0], 1.0, false, false).is_none());
}

#[test]
fn custom_providers_without_revisions_keep_time_keyed_inputs() {
    struct Dynamic;
    impl FootageSource for Dynamic {
        fn frame(&self, _: ItemId, f: &Footage, t: Tick) -> Option<Arc<Image>> {
            Some(Arc::new(Image::filled(f.width, f.height, [t.seconds() as f32, 0.0, 0.0, 1.0])))
        }
    }
    let (p, cid) = scene();
    let cache = LayerCache::new(1 << 20);
    let mut r = Renderer::new(&p, &Dynamic, RenderOpts::default());
    r.cache = Some(&cache);
    let ctx = r.eval_ctx(cid, Tick::ZERO).unwrap();
    let layer = &ctx.comp.layers[0];
    assert!(r.processed_key(&ctx, layer).is_none());
    let a = r.layer_input(&ctx, layer, 0).unwrap();
    let b = r.layer_input(&ctx.at(FrameRate::FPS_30.tick_of(12)), layer, 0).unwrap();
    assert_ne!(a.img.data, b.img.data);
}
