//! GPU rasterization for the Card Wipe transition.
use effectcraft_effects::{EffectCtx, PieceTex};
use crate::context::{Enc, GpuImage, Params};
use crate::effects::GBuf;
pub(crate) const KERNELS: &[&str] = &["fxcw_raster"];
pub(crate) const IDS: &[&str] = &["ec.transition.cardwipe"];
const MAX_FLOATS: usize = 1 << 24;
const TILE: i32 = 16;
pub(crate) fn apply(e: &mut Enc, id: &str, ctx: &EffectCtx, b: GBuf) -> Option<GBuf> { pieces(e, id, ctx, b) }
pub(crate) struct Items {
    stride: usize,
    recs: Vec<f32>,
    w: i32,
    h: i32,
    tiles: Vec<Vec<u32>>,
    pub(crate) tx: i32,
    /// Item indices in all tiles.
    n_idx: usize,
}

impl Items {
    pub(crate) fn new(stride: usize, w: u32, h: u32) -> Items {
        let (w, h) = (w as i32, h as i32);
        let tx = (w + TILE - 1) / TILE;
        let ty = (h + TILE - 1) / TILE;
        Items { stride, recs: vec![], w, h, tiles: vec![vec![]; (tx * ty).max(0) as usize], tx, n_idx: 0 }
    }

    /// The integer pixel bounds of float bounds `bb` (`None` = skipped, as card rasterizer).
    fn bounds(&self, bb: Option<[f32; 4]>) -> Option<[i32; 4]> {
        let b = bb?;
        if !(b[0].is_finite() && b[1].is_finite() && b[2].is_finite() && b[3].is_finite()) {
            return None;
        }
        let x0 = b[0].floor().max(0.0) as i32;
        let y0 = b[1].floor().max(0.0) as i32;
        let x1 = (b[2].ceil() as i32).min(self.w - 1);
        let y1 = (b[3].ceil() as i32).min(self.h - 1);
        if x1 < x0 || y1 < y0 || b[2] < 0.0 || b[3] < 0.0 {
            return None;
        }
        Some([x0, y0, x1, y1])
    }

    /// Whether adding an item covering `bb` keeps the kernel data within [`MAX_FLOATS`].
    pub(crate) fn fits(&self, bb: Option<[f32; 4]>) -> bool {
        let Some([x0, y0, x1, y1]) = self.bounds(bb) else { return true };
        let tiles = ((x1 / TILE - x0 / TILE + 1) * (y1 / TILE - y0 / TILE + 1)) as usize;
        self.tiles.len() + 1 + self.n_idx + tiles + self.recs.len() + self.stride <= MAX_FLOATS
    }

    /// Add an item (`rec` = its fields, without the bounds) covering float bounds `bb`
    /// (`None` or empty = skipped, as card rasterizer).
    pub(crate) fn push(&mut self, bb: Option<[f32; 4]>, rec: &[f32]) {
        let Some([x0, y0, x1, y1]) = self.bounds(bb) else { return };
        let i = (self.recs.len() / self.stride) as u32;
        let start = self.recs.len();
        self.recs.extend_from_slice(rec);
        self.recs.resize(start + self.stride - 4, 0.0);
        self.recs.extend([x0 as f32, y0 as f32, x1 as f32, y1 as f32]);
        for ty in y0 / TILE..=y1 / TILE {
            for tx in x0 / TILE..=x1 / TILE {
                self.tiles[(ty * self.tx + tx) as usize].push(i);
                self.n_idx += 1;
            }
        }
    }

    /// Kernel data: tile offsets (tiles + 1), item indices, items. `None` when too large.
    pub(crate) fn data(self) -> Option<(Vec<f32>, u32, u32)> {
        let n_tiles = self.tiles.len();
        let n_idx: usize = self.tiles.iter().map(Vec::len).sum();
        let total = n_tiles + 1 + n_idx + self.recs.len();
        if total > MAX_FLOATS {
            return None;
        }
        let mut d = Vec::with_capacity(total);
        let base = (n_tiles + 1) as f32;
        let mut at = 0usize;
        for t in &self.tiles {
            d.push(base + at as f32);
            at += t.len();
        }
        d.push(base + at as f32);
        for t in &self.tiles {
            d.extend(t.iter().map(|&i| i as f32));
        }
        let items_at = d.len() as u32;
        d.extend(self.recs);
        Some((d, items_at, self.stride as u32))
    }
}

/// Rasterise textured cards over a transparent frame.
#[allow(clippy::too_many_arguments)]
fn raster(e: &mut Enc, items: Items, kind: u32, add: bool, src: &GpuImage, aux: Option<&GpuImage>, mut p: Params, w: u32, h: u32) -> Option<GpuImage> {
    let tx = items.tx as u32;
    let (data, items_at, stride) = items.data()?;
    let buf = e.data(&data);
    p.u[0] = [kind, add as u32, items_at, stride];
    p.u[1][0] = tx;
    let out = e.scratch(w, h);
    e.pixels("fxcw_raster", &p, src, aux, &out, Some(&buf));
    Some(out)
}

fn pieces(e: &mut Enc, id: &str, ctx: &EffectCtx, b: GBuf) -> Option<GBuf> {
    // Planning reads the layer (gradient maps and textures default to it).
    let img = e.download(&b.img)?;
    let cpu = effectcraft_effects::Buf { img, offset: b.offset, scale: b.scale };

    let Some(plan) = effectcraft_effects::piece_plan(id, ctx, &cpu) else {
        // Card Wipe before anything moves: the layer as is.
        return (id == "ec.transition.cardwipe").then_some(b);
    };
    piece_pass(e, b, &plan)
}
pub(crate) fn piece_pass(e: &mut Enc, b: GBuf, plan: &effectcraft_effects::PiecePlan) -> Option<GBuf> {
    let (w, h) = (b.img.width, b.img.height);
    let tex = |t: &PieceTex| -> Option<GpuImage> {
        match t {
            PieceTex::Layer => Some(b.img.clone()),
            PieceTex::Image(i) => e.g.upload_image(i),
        }
    };
    let front = tex(&plan.front)?;
    let back = match &plan.back {
        Some(t) => Some(tex(t)?),
        None => None,
    };
    let mut items = Items::new(48, w, h);
    for q in &plan.pieces {
        let m = q.inv;
        let mut rec = vec![11.0];
        for r in m {
            rec.extend(r.map(|v| v as f32));
        }
        rec.extend(q.c0);
        for v in q.poly {
            rec.extend(v);
        }
        rec.extend([q.n as f32, q.shade]);
        rec.extend(q.tint);
        rec.extend(q.spec);
        rec.push(q.alpha);
        match q.flat {
            Some(c) => rec.extend([1.0, c[0], c[1], c[2]]),
            None => rec.extend([0.0; 4]),
        }
        rec.extend([q.back as u32 as f32, q.back_mirror as f32]);
        if !items.fits(Some(q.bbox)) { return None; }
        items.push(Some(q.bbox), &rec);
    }

    let mut p = Params::default();
    p.u[1][1] = back.is_some() as u32;
    let img = raster(e, items, 3, false, &front, back.as_ref(), p, w, h)?;
    Some(GBuf { img, ..b })
}
