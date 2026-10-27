//! Original synthetic XCF fixtures, generated from the published specification.
#![allow(dead_code)]
pub struct Layer<'a> {
    pub name: &'a str,
    pub w: u32,
    pub h: u32,
    pub xy: (i32, i32),
    pub rgba: Vec<[u16; 4]>,
    pub channels: u32,
    pub tattoo: u32,
    pub props: Vec<(u32, Vec<u8>)>,
    pub mask: bool,
    pub effect: bool,
}
impl<'a> Layer<'a> {
    pub fn solid(name: &'a str, w: u32, h: u32, rgba: [u8; 4], tattoo: u32) -> Self {
        Self {
            name,
            w,
            h,
            xy: (0, 0),
            rgba: vec![rgba.map(|n| u16::from(n) * 257); (w * h) as usize],
            channels: 4,
            tattoo,
            props: vec![],
            mask: false,
            effect: false,
        }
    }
}
fn word(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_be_bytes());
}
fn ptr(b: &mut Vec<u8>, n: usize, wide: bool) {
    if wide {
        b.extend((n as u64).to_be_bytes());
    } else {
        word(b, n as u32);
    }
}
fn fix(b: &mut [u8], slot: usize, n: usize, wide: bool) {
    if wide {
        b[slot..slot + 8].copy_from_slice(&(n as u64).to_be_bytes());
    } else {
        b[slot..slot + 4].copy_from_slice(&(n as u32).to_be_bytes());
    }
}
fn string(b: &mut Vec<u8>, s: &str) {
    word(b, s.len() as u32 + 1);
    b.extend(s.as_bytes());
    b.push(0);
}
fn prop(b: &mut Vec<u8>, ty: u32, v: &[u8]) {
    word(b, ty);
    word(b, v.len() as u32);
    b.extend(v);
}
pub fn fixture(w: u32, h: u32, version: u32, precision: u32, compression: u8, layers: &[Layer<'_>], zip: Option<fn(&[u8]) -> Vec<u8>>) -> Vec<u8> {
    let wide = version >= 11;
    let two = matches!(precision, 200 | 250);
    let mut b = b"gimp xcf ".to_vec();
    b.extend(if version == 0 { "file".into() } else { format!("v{version:03}") }.as_bytes());
    b.push(0);
    word(&mut b, w);
    word(&mut b, h);
    word(&mut b, 0);
    if version >= 4 {
        word(&mut b, precision);
    }
    prop(&mut b, 17, &[compression]);
    prop(&mut b, 0, &[]);
    let mut slots = vec![];
    for _ in layers {
        slots.push(b.len());
        ptr(&mut b, 0, wide);
    }
    ptr(&mut b, 0, wide);
    ptr(&mut b, 0, wide);
    if version >= 18 {
        ptr(&mut b, 0, wide);
    }
    for (slot, l) in slots.into_iter().zip(layers) {
        let at = b.len();
        fix(&mut b, slot, at, wide);
        word(&mut b, l.w);
        word(&mut b, l.h);
        word(&mut b, if l.channels == 4 { 1 } else { 0 });
        string(&mut b, l.name);
        let mut xy = vec![];
        word(&mut xy, l.xy.0 as u32);
        word(&mut xy, l.xy.1 as u32);
        prop(&mut b, 15, &xy);
        prop(&mut b, 20, &l.tattoo.to_be_bytes());
        prop(&mut b, 6, &255u32.to_be_bytes());
        prop(&mut b, 8, &1u32.to_be_bytes());
        prop(&mut b, 7, &0u32.to_be_bytes());
        for (ty, v) in &l.props {
            prop(&mut b, *ty, v);
        }
        prop(&mut b, 0, &[]);
        let hs = b.len();
        ptr(&mut b, 0, wide);
        ptr(&mut b, if l.mask { 1 } else { 0 }, wide);
        if version >= 20 {
            if l.effect {
                ptr(&mut b, 1, wide);
            }
            ptr(&mut b, 0, wide);
        }
        let at = b.len();
        fix(&mut b, hs, at, wide);
        word(&mut b, l.w);
        word(&mut b, l.h);
        word(&mut b, l.channels * if two { 2 } else { 1 });
        let ls = b.len();
        ptr(&mut b, 0, wide);
        ptr(&mut b, 0, wide);
        let at = b.len();
        fix(&mut b, ls, at, wide);
        word(&mut b, l.w);
        word(&mut b, l.h);
        let mut tiles = vec![];
        for _ in 0..l.w.div_ceil(64) * l.h.div_ceil(64) {
            tiles.push(b.len());
            ptr(&mut b, 0, wide);
        }
        ptr(&mut b, 0, wide);
        for (i, slot) in tiles.into_iter().enumerate() {
            let at = b.len();
            fix(&mut b, slot, at, wide);
            let (tx, ty) = ((i as u32 % l.w.div_ceil(64)) * 64, (i as u32 / l.w.div_ceil(64)) * 64);
            let mut raw = vec![];
            for y in ty..(ty + 64).min(l.h) {
                for x in tx..(tx + 64).min(l.w) {
                    for v in l.rgba[(y * l.w + x) as usize].iter().take(l.channels as usize) {
                        if two {
                            raw.extend(v.to_be_bytes());
                        } else {
                            raw.push((v / 257) as u8);
                        }
                    }
                }
            }
            match compression {
                0 => b.extend(raw),
                1 => {
                    let bpp = l.channels as usize * if two { 2 } else { 1 };
                    for c in 0..bpp {
                        let plane: Vec<_> = raw.iter().skip(c).step_by(bpp).copied().collect();
                        for chunk in plane.chunks(127) {
                            b.push((256 - chunk.len()) as u8);
                            b.extend(chunk);
                        }
                    }
                }
                2 => b.extend(zip.unwrap()(&raw)),
                _ => panic!("test compression"),
            }
        }
    }
    b
}
pub fn sample() -> Vec<u8> {
    let mut front = Layer::solid("Ink", 2, 2, [255, 0, 0, 128], 11);
    front.xy = (1, 1);
    let mut hidden = Layer::solid("Hidden", 4, 3, [0, 255, 0, 255], 12);
    hidden.props.push((8, 0u32.to_be_bytes().to_vec()));
    fixture(4, 3, 12, 150, 1, &[front, hidden, Layer::solid("Paper", 4, 3, [0, 0, 255, 255], 13)], None)
}
