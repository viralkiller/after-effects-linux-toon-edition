//! Textured card geometry and rasterization for Card Wipe.
use crate::card3d::{Lighting, Proj};
use crate::util::unpremul;
use crate::{Buf, EffectCtx};
use effectcraft_raster::{Image, Px};
use rayon::prelude::*;
const BAND: usize = 16;
pub(crate) fn raster<T: Sync>(img: &mut Image, items: &[T], bbox: impl Fn(&T) -> Option<[f32; 4]> + Sync, shade: impl Fn(&T, f32, f32) -> Option<Px> + Sync) {
    let (w, hgt) = (img.width as usize, img.height as usize);
    if w == 0 || hgt == 0 || items.is_empty() {
        return;
    }
    let nb = hgt.div_ceil(BAND);
    let boxes: Vec<Option<[i32; 4]>> = items
        .par_iter()
        .map(|it| {
            let b = bbox(it)?;
            if !(b[0].is_finite() && b[1].is_finite() && b[2].is_finite() && b[3].is_finite()) {
                return None;
            }
            let x0 = b[0].floor().max(0.0) as i32;
            let y0 = b[1].floor().max(0.0) as i32;
            let x1 = (b[2].ceil() as i32).min(w as i32 - 1);
            let y1 = (b[3].ceil() as i32).min(hgt as i32 - 1);
            if x1 < x0 || y1 < y0 || b[2] < 0.0 || b[3] < 0.0 {
                return None;
            }
            Some([x0, y0, x1, y1])
        })
        .collect();
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); nb];
    for (i, b) in boxes.iter().enumerate() {
        if let Some(b) = b {
            for band in (b[1] as usize / BAND)..=(b[3] as usize / BAND).min(nb - 1) {
                buckets[band].push(i as u32);
            }
        }
    }
    img.data.par_chunks_mut(w * BAND).enumerate().for_each(|(band, chunk)| {
        let by0 = band * BAND;
        let rows = chunk.len() / w;
        for &i in &buckets[band] {
            let Some(b) = boxes[i as usize] else { continue };
            let it = &items[i as usize];
            let ya = (b[1] as usize).max(by0);
            let yb = (b[3] as usize).min(by0 + rows - 1);
            for y in ya..=yb {
                let row = &mut chunk[(y - by0) * w..(y - by0 + 1) * w];
                for x in b[0] as usize..=b[2] as usize {
                    if let Some(c) = shade(it, x as f32 + 0.5, y as f32 + 0.5) {
                        let d = &mut row[x];
                        let k = 1.0 - c[3];
                        *d = [c[0] + d[0] * k, c[1] + d[1] * k, c[2] + d[2] * k, c[3] + d[3] * k];
                    }
                }
            }
        }
    });
}
pub(crate) type M3 = [[f64; 3]; 3];

/// Rotation by `angle` about unit `axis` (Rodrigues).
pub(crate) fn rot_axis(axis: [f64; 3], angle: f64) -> M3 {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if l < 1e-12 || angle == 0.0 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let (x, y, z) = (axis[0] / l, axis[1] / l, axis[2] / l);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

fn inv3(m: &M3) -> Option<M3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 {
        return None;
    }
    let d = 1.0 / det;
    Some([
        [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d],
        [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d],
        [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d],
    ])
}

/// A planar textured piece ready to draw.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    /// Screen (buffer px, homogeneous) → piece-local (a, b, w).
    pub inv: [[f64; 3]; 3],
    /// Rest centre of the piece in texture pixels.
    pub c0: [f32; 2],
    /// Convex polygon (texture pixels), counter-clockwise in y-down space.
    pub poly: [[f32; 2]; 6],
    pub n: u8,
    pub bbox: [f32; 4],
    pub shade: f32,
    /// Lighting: colour multiplier and additive specular highlight.
    pub tint: [f32; 3],
    pub spec: [f32; 3],
    pub alpha: f32,
    /// Flat straight colour instead of the texture.
    pub flat: Option<[f32; 3]>,
    /// Facing away from the camera (draw the back texture).
    pub back: bool,
    /// Back faces sample their texture mirrored about the piece centre: 0 no, 1 vertically
    /// (flipped about the X axis), 2 horizontally (about the Y axis).
    pub back_mirror: u8,
    depth: f64,
}

/// A piece texture: the layer itself or another image (buffer-aligned).
#[derive(Clone, Debug)]
pub enum PieceTex {
    Layer,
    Image(Image),
}

/// What Card Wipe draws this frame: pieces sorted
/// far to near over a transparent frame, textured by `front` (and `back` for pieces facing
/// away; none = they take `front`).
#[derive(Clone, Debug)]
pub struct PiecePlan {
    pub pieces: Vec<Piece>,
    pub front: PieceTex,
    pub back: Option<PieceTex>,
}

impl PiecePlan {
    /// Rasterise over a transparent frame of the layer `b`'s size.
    pub fn finish(&self, mut b: Buf) -> Buf {
        let mut out = Image::new(b.img.width, b.img.height);
        let front = match &self.front {
            PieceTex::Layer => &b.img,
            PieceTex::Image(i) => i,
        };
        let back = self.back.as_ref().map(|t| match t {
            PieceTex::Layer => &b.img,
            PieceTex::Image(i) => i,
        });
        draw_pieces(&mut out, &self.pieces, front, back);
        b.img = out;
        b
    }
}

/// Card Wipe geometry for the current frame.
pub fn piece_plan(id: &str, ctx: &EffectCtx, b: &Buf) -> Option<PiecePlan> {
    if id == "ec.transition.cardwipe" { crate::transition::card_wipe_plan(ctx, b) } else { None }
}

impl Piece {
    /// Place polygon `poly` (texture px, around rest centre `c0`) with rotation `r` and its
    /// centre at 3D `pos` (buffer px; z away from the camera), seen through `cam`.
    pub(crate) fn new(poly: &[[f32; 2]], c0: [f32; 2], r: &M3, pos: [f64; 3], cam: impl Into<Proj>) -> Option<Piece> {
        let cam: Proj = cam.into();
        let n = poly.len().min(6);
        if n < 3 {
            return None;
        }
        let m = &cam.m;
        let row = |i: usize| -> [f64; 3] {
            [
                m[i][0] * r[0][0] + m[i][1] * r[1][0] + m[i][2] * r[2][0],
                m[i][0] * r[0][1] + m[i][1] * r[1][1] + m[i][2] * r[2][1],
                m[i][0] * pos[0] + m[i][1] * pos[1] + m[i][2] * pos[2] + m[i][3],
            ]
        };
        let hm: M3 = [row(0), row(1), row(2)];
        let mut bb = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        for v in &poly[..n] {
            let (a, b) = ((v[0] - c0[0]) as f64, (v[1] - c0[1]) as f64);
            let w = hm[2][0] * a + hm[2][1] * b + hm[2][2];
            if w < cam.near {
                return None;
            }
            let sx = ((hm[0][0] * a + hm[0][1] * b + hm[0][2]) / w) as f32;
            let sy = ((hm[1][0] * a + hm[1][1] * b + hm[1][2]) / w) as f32;
            bb = [bb[0].min(sx), bb[1].min(sy), bb[2].max(sx), bb[3].max(sy)];
        }
        let inv = inv3(&hm)?;
        let mut pp = [[0.0f32; 2]; 6];
        pp[..n].copy_from_slice(&poly[..n]);
        // Normalise winding so "inside" is cross >= 0 for every edge.
        let area: f32 = (0..n).map(|i| pp[i][0] * pp[(i + 1) % n][1] - pp[(i + 1) % n][0] * pp[i][1]).sum();
        if area < 0.0 {
            pp[..n].reverse();
        }
        // The front faces -z at rest; it faces the camera when its normal points at the eye.
        let eye = cam.eye();
        let nrm = [-r[0][2], -r[1][2], -r[2][2]];
        let to_eye = [eye[0] - pos[0], eye[1] - pos[1], eye[2] - pos[2]];
        let facing = nrm[0] * to_eye[0] + nrm[1] * to_eye[1] + nrm[2] * to_eye[2];
        let depth = hm[2][2];
        Some(Piece {
            inv,
            c0,
            poly: pp,
            n: n as u8,
            bbox: [bb[0] - 1.0, bb[1] - 1.0, bb[2] + 1.0, bb[3] + 1.0],
            shade: 1.0,
            tint: [1.0; 3],
            spec: [0.0; 3],
            alpha: 1.0,
            flat: None,
            back: facing < 0.0,
            back_mirror: 0,
            depth,
        })
    }

    /// Light the piece (rotation `r`, centre `pos`) with Lighting / Material.
    pub(crate) fn light(&mut self, l: &Lighting, r: &M3, pos: [f64; 3], eye: [f64; 3]) {
        let (t, s) = l.shade([-r[0][2], -r[1][2], -r[2][2]], pos, eye);
        self.tint = t;
        self.spec = s;
    }
}

/// Draw pieces (already sorted far → near) over `out`.
pub(crate) fn draw_pieces(out: &mut Image, pieces: &[Piece], front: &Image, back: Option<&Image>) {
    raster(
        out,
        pieces,
        |q| Some(q.bbox),
        |q, x, y| {
            let (xf, yf) = (x as f64, y as f64);
            let m = &q.inv;
            let w = m[2][0] * xf + m[2][1] * yf + m[2][2];
            if w.abs() < 1e-12 {
                return None;
            }
            let u = ((m[0][0] * xf + m[0][1] * yf + m[0][2]) / w) as f32 + q.c0[0];
            let v = ((m[1][0] * xf + m[1][1] * yf + m[1][2]) / w) as f32 + q.c0[1];
            let n = q.n as usize;
            let mut md = f32::INFINITY;
            for i in 0..n {
                let a = q.poly[i];
                let b = q.poly[(i + 1) % n];
                let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
                let l = (ex * ex + ey * ey).sqrt().max(1e-6);
                md = md.min((ex * (v - a[1]) - ey * (u - a[0])) / l);
            }
            // Coverage extends half a pixel past the edge so abutting pieces leave no seams.
            let cov = (md + 1.0).clamp(0.0, 1.0);
            if cov <= 0.0 {
                return None;
            }
            let (c, a) = match q.flat {
                Some(c) => (c, 1.0),
                None => {
                    let tex = if q.back { back.unwrap_or(front) } else { front };
                    let (u, v) = match (q.back, q.back_mirror) {
                        (true, 1) => (u, 2.0 * q.c0[1] - v),
                        (true, 2) => (2.0 * q.c0[0] - u, v),
                        _ => (u, v),
                    };
                    unpremul(tex.sample_bilinear(u as f64, v as f64))
                }
            };
            let a = a * cov * q.alpha;
            if a <= 1e-6 {
                return None;
            }
            let k = q.shade;
            let o: [f32; 3] = std::array::from_fn(|i| c[i] * k * q.tint[i] + q.spec[i]);
            Some([o[0] * a, o[1] * a, o[2] * a, a])
        },
    );
}

pub(crate) fn sort_far_first(pieces: &mut [Piece]) {
    pieces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
}
