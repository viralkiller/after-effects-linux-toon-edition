//! Plain pixel-layer XCF, from GIMP's public format specification, not decoder source.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
use effectcraft_color::{linear_to_srgb, srgb_to_linear};
use std::collections::BTreeSet;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XCF: {}", self.0)
    }
}
impl std::error::Error for Error {}
fn bad(s: &str) -> Error {
    Error(s.into())
}
fn unsupported(s: &str) -> Error {
    Error(format!(
        "{s} is outside plain XCF support. Save an RGB 8/16-bit integer, built-in sRGB copy with ungrouped Normal pixel layers; apply masks and filters in GIMP first."
    ))
}
pub const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_PIXELS: usize = 16 * 1024 * 1024;
const MAX_LAYERS: usize = 1024;
const MAX_PROPERTIES: usize = 4096;
pub fn is_xcf(b: &[u8]) -> bool {
    b.starts_with(b"gimp xcf ")
}
/// Limit native reads both before allocation and while reading a file that may be growing.
pub fn read_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let n = file.metadata()?.len();
    if n > MAX_FILE_BYTES as u64 {
        return Err(std::io::Error::other("XCF exceeds 256 MiB import limit"));
    }
    let mut b = Vec::new();
    b.try_reserve_exact(n as usize).map_err(std::io::Error::other)?;
    file.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut b)?;
    if b.len() > MAX_FILE_BYTES {
        return Err(std::io::Error::other("XCF exceeds 256 MiB import limit"));
    }
    Ok(b)
}
struct Reader<'a> {
    b: &'a [u8],
    p: usize,
    wide: bool,
}
impl<'a> Reader<'a> {
    fn new(b: &'a [u8], p: usize, wide: bool) -> Result<Self> {
        if p > b.len() {
            return Err(bad("offset outside file"));
        }
        Ok(Self { b, p, wide })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.p.checked_add(n).ok_or_else(|| bad("offset overflow"))?;
        let b = self.b.get(self.p..end).ok_or_else(|| bad("truncated file"))?;
        self.p = end;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8> {
        self.take(1)?.first().copied().ok_or_else(|| bad("truncated byte"))
    }
    fn word(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| bad("truncated word"))?))
    }
    fn ptr(&mut self) -> Result<usize> {
        let n = if self.wide { u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| bad("truncated pointer"))?) } else { u64::from(self.word()?) };
        let n = usize::try_from(n).map_err(|_| bad("pointer too large"))?;
        if n >= self.b.len() && n != 0 {
            return Err(bad("pointer outside file"));
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<&'a str> {
        let n = self.word()? as usize;
        if n == 0 {
            return Ok("");
        }
        if n > 65536 {
            return Err(bad("string too long"));
        }
        let b = self.take(n)?;
        if b.last() != Some(&0) {
            return Err(bad("unterminated string"));
        }
        std::str::from_utf8(b.get(..n - 1).ok_or_else(|| bad("bad string"))?).map_err(|_| bad("invalid UTF-8"))
    }
    fn ptrs(&mut self, limit: usize) -> Result<Vec<usize>> {
        let mut v = Vec::new();
        for _ in 0..=limit {
            let p = self.ptr()?;
            if p == 0 {
                return Ok(v);
            }
            if v.len() == limit {
                return Err(bad("too many records"));
            }
            v.push(p);
        }
        Err(bad("unterminated pointer list"))
    }
    fn props(&mut self, version: u32, mut visit: impl FnMut(u32, &'a [u8]) -> Result<()>) -> Result<()> {
        for _ in 0..MAX_PROPERTIES {
            let ty = self.word()?;
            let n = self.word()? as usize;
            if ty == 0 {
                return if n == 0 { Ok(()) } else { Err(bad("invalid terminator")) };
            }
            // Historical colormap and custom-unit length words are unreliable.
            if ty == 1 {
                let count = self.word()? as usize;
                if count > 256 {
                    return Err(bad("invalid colormap"));
                }
                self.take(count * 3)?;
            } else if ty == 24 {
                self.take(8)?;
                for _ in 0..if version >= 21 { 3 } else { 5 } {
                    self.string()?;
                }
            } else {
                visit(ty, self.take(n)?)?;
            }
        }
        Err(bad("too many properties"))
    }
}
fn word(b: &[u8]) -> Result<u32> {
    Ok(u32::from_be_bytes(b.try_into().map_err(|_| bad("incorrect property length"))?))
}
fn pixels(w: u32, h: u32) -> Result<usize> {
    let n = (w as usize).checked_mul(h as usize).ok_or_else(|| bad("pixel count overflow"))?;
    if w == 0 || h == 0 || w > 16384 || h > 16384 || n > MAX_PIXELS {
        return Err(bad("image exceeds 16 megapixels / 16384 pixels per axis"));
    }
    Ok(n)
}
#[derive(Debug)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub data: Vec<[f32; 4]>,
}
fn blank(w: u32, h: u32) -> Result<Pixels> {
    let n = pixels(w, h)?;
    let mut data = Vec::new();
    data.try_reserve_exact(n).map_err(|_| bad("not enough memory for image"))?;
    data.resize(n, [0.; 4]);
    Ok(Pixels { width: w, height: h, data })
}
#[derive(Debug)]
pub struct Layer {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub opacity: f32,
    pub visible: bool,
    pub tattoo: Option<u32>,
    pub linear_composite: bool,
    channels: usize,
    tiles: Vec<usize>,
}
#[derive(Debug)]
pub struct Document<'a> {
    pub width: u32,
    pub height: u32,
    /// Top to bottom, matching the timeline.
    pub layers: Vec<Layer>,
    bytes: &'a [u8],
    compression: u8,
    sample_bytes: usize,
    linear_samples: bool,
}
impl<'a> Document<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(bad("file exceeds 256 MiB"));
        }
        let mut r = Reader::new(bytes, 0, false)?;
        if r.take(9)? != b"gimp xcf " {
            return Err(bad("not XCF"));
        }
        let tag = r.take(4)?;
        let version = if tag == b"file" {
            0
        } else {
            if tag.first() != Some(&b'v') {
                return Err(bad("invalid version"));
            }
            let digits = tag.get(1..).ok_or_else(|| bad("invalid version"))?;
            if !digits.iter().all(u8::is_ascii_digit) {
                return Err(bad("invalid version"));
            }
            digits.iter().fold(0, |n, b| n * 10 + u32::from(*b - b'0'))
        };
        if version > 26 || (4..=6).contains(&version) {
            return Err(unsupported("this XCF version"));
        }
        if r.byte()? != 0 {
            return Err(bad("invalid header terminator"));
        }
        r.wide = version >= 11;
        let (width, height) = (r.word()?, r.word()?);
        pixels(width, height)?;
        if r.word()? != 0 {
            return Err(unsupported("grayscale/indexed colour"));
        }
        let precision = if version >= 4 { r.word()? } else { 150 };
        let (sample_bytes, linear_samples) = match precision {
            100 => (1, true),
            150 => (1, false),
            200 if version >= 12 => (2, true),
            250 if version >= 12 => (2, false),
            _ => return Err(unsupported("this pixel precision")),
        };
        let mut compression = 1;
        r.props(version, |ty, b| {
            match ty {
                17 => {
                    if b.len() != 1 {
                        return Err(bad("invalid compression property"));
                    }
                    compression = *b.first().ok_or_else(|| bad("missing compression"))?;
                }
                21 => check_profile(b, linear_samples)?,
                _ => {}
            }
            Ok(())
        })?;
        if compression > 2 {
            return Err(unsupported("compression"));
        }
        let offsets = r.ptrs(MAX_LAYERS)?;
        r.ptrs(MAX_LAYERS)?;
        if version >= 18 {
            r.ptrs(MAX_LAYERS)?;
        }
        let mut seen = BTreeSet::new();
        let mut tattoos = BTreeSet::new();
        let mut total = 0usize;
        let mut layers = Vec::new();
        for offset in offsets {
            if !seen.insert(offset) {
                return Err(bad("duplicate layer offset"));
            }
            let mut lr = Reader::new(bytes, offset, r.wide)?;
            let (w, h, ty) = (lr.word()?, lr.word()?, lr.word()?);
            total = total.checked_add(pixels(w, h)?).ok_or_else(|| bad("pixel count overflow"))?;
            if total > 64 * 1024 * 1024 {
                return Err(bad("layers exceed combined 64 megapixel limit"));
            }
            let channels = match ty {
                0 => 3,
                1 => 4,
                _ => return Err(unsupported("non-RGB layer")),
            };
            let mut l = Layer {
                name: lr.string()?.into(),
                width: w,
                height: h,
                x: 0,
                y: 0,
                opacity: 1.,
                visible: true,
                tattoo: None,
                linear_composite: false,
                channels,
                tiles: Vec::new(),
            };
            let mut mode = 0;
            let mut space = None;
            let mut float_opacity = None;
            lr.props(version, |ty, b| {
                match ty {
                    6 => {
                        let n = word(b)?;
                        if n > 255 {
                            return Err(bad("invalid opacity"));
                        }
                        l.opacity = n as f32 / 255.;
                    }
                    7 => {
                        mode = word(b)?;
                        if !matches!(mode, 0 | 28) {
                            return Err(unsupported("non-Normal layer blending"));
                        }
                    }
                    8 => l.visible = word(b)? != 0,
                    15 => {
                        if b.len() != 8 {
                            return Err(bad("invalid offsets"));
                        }
                        let mut p = Reader::new(b, 0, false)?;
                        l.x = p.word()? as i32;
                        l.y = p.word()? as i32;
                    }
                    20 => l.tattoo = Some(word(b)?).filter(|n| *n != 0),
                    33 => {
                        let f = f32::from_bits(word(b)?);
                        if !f.is_finite() || !(0.0..=1.0).contains(&f) {
                            return Err(bad("invalid opacity"));
                        }
                        float_opacity = Some(f);
                    }
                    35 => {
                        if (word(b)? as i32).unsigned_abs() != 1 {
                            return Err(unsupported("non-Union compositing"));
                        }
                    }
                    36 => {
                        let n = (word(b)? as i32).unsigned_abs();
                        if !matches!(n, 1 | 2 | 4) {
                            return Err(unsupported("composite colour space"));
                        }
                        space = Some(n);
                    }
                    37 => {
                        // Normal blending uses the source colour directly, so Auto (0)
                        // needs no blend-space conversion. GIMP saves it in plain files.
                        if !matches!((word(b)? as i32).unsigned_abs(), 0 | 1 | 2 | 4) {
                            return Err(unsupported("blend colour space"));
                        }
                    }
                    5 | 29 | 30 | 43.. => return Err(unsupported("groups, floating selections, vector/linked layers or unknown layer properties")),
                    // Known editing metadata. Text's saved pixels import as raster footage.
                    2 | 9..=14 | 21 | 26 | 28 | 31 | 32 | 34 | 41 | 42 => {}
                    _ => return Err(unsupported("unknown layer property")),
                }
                Ok(())
            })?;
            l.opacity = float_opacity.unwrap_or(l.opacity);
            l.linear_composite = space.map_or(mode == 28, |n| n == 1 || (n == 2 && linear_samples));
            if let Some(t) = l.tattoo
                && !tattoos.insert(t)
            {
                return Err(bad("duplicate layer identifier"));
            }
            let hierarchy = lr.ptr()?;
            if lr.ptr()? != 0 {
                return Err(unsupported("layer masks"));
            }
            if version >= 20 && lr.ptr()? != 0 {
                return Err(unsupported("live layer effects"));
            }
            if hierarchy == 0 {
                return Err(bad("missing pixel hierarchy"));
            }
            l.tiles = read_tiles(bytes, hierarchy, r.wide, w, h, channels * sample_bytes)?;
            layers.push(l);
        }
        if layers.is_empty() {
            return Err(bad("no pixel layers"));
        }
        Ok(Self { width, height, layers, bytes, compression, sample_bytes, linear_samples })
    }
    /// Tattoos are authoritative; never substitute a new layer at a deleted layer's index.
    pub fn find_layer(&self, tattoo: Option<u32>, name: &str) -> Result<usize> {
        let mut found = self.layers.iter().enumerate().filter(|(_, l)| match tattoo {
            Some(t) => l.tattoo == Some(t),
            None => l.name == name,
        });
        let index = found.next().map(|(i, _)| i).ok_or_else(|| bad("source layer removed; reimport XCF"))?;
        if found.next().is_some() {
            return Err(bad("ambiguous layer name; give layers unique names in GIMP"));
        }
        Ok(index)
    }
    /// Validate compressed tiles without allocating a full image.
    pub fn validate_pixels(&self) -> Result<()> {
        for l in &self.layers {
            self.visit(l, |_, _, _| {})?;
        }
        Ok(())
    }
    /// Straight sRGB pixels, without layer opacity/visibility (owned by the timeline).
    pub fn layer_pixels(&self, index: usize, canvas: bool) -> Result<Pixels> {
        let l = self.layers.get(index).ok_or_else(|| bad("missing layer"))?;
        let (w, h) = if canvas { (self.width, self.height) } else { (l.width, l.height) };
        let mut out = blank(w, h)?;
        self.visit(l, |x, y, p| {
            let (x, y) = if canvas { (i64::from(x) + i64::from(l.x), i64::from(y) + i64::from(l.y)) } else { (i64::from(x), i64::from(y)) };
            if x >= 0
                && y >= 0
                && x < i64::from(w)
                && y < i64::from(h)
                && let Some(d) = out.data.get_mut(y as usize * w as usize + x as usize)
            {
                *d = p;
            }
        })?;
        Ok(out)
    }
    /// Flatten Normal-over bottom to top using each layer's saved compositing space.
    pub fn composite(&self) -> Result<Pixels> {
        let mut out = blank(self.width, self.height)?;
        for l in self.layers.iter().rev().filter(|l| l.visible && l.opacity > 0.) {
            self.visit(l, |x, y, mut s| {
                let (x, y) = (i64::from(x) + i64::from(l.x), i64::from(y) + i64::from(l.y));
                if x < 0 || y < 0 || x >= i64::from(out.width) || y >= i64::from(out.height) {
                    return;
                }
                let Some(d) = out.data.get_mut(y as usize * out.width as usize + x as usize) else {
                    return;
                };
                s[3] *= l.opacity;
                let a = s[3] + d[3] * (1. - s[3]);
                if a > 0. {
                    for c in 0..3 {
                        let (sv, dv) = if l.linear_composite { (srgb_to_linear(s[c]), srgb_to_linear(d[c])) } else { (s[c], d[c]) };
                        let v = (sv * s[3] + dv * d[3] * (1. - s[3])) / a;
                        d[c] = if l.linear_composite { linear_to_srgb(v) } else { v };
                    }
                }
                d[3] = a;
            })?;
        }
        Ok(out)
    }
    fn visit(&self, l: &Layer, mut visit: impl FnMut(u32, u32, [f32; 4])) -> Result<()> {
        let cols = l.width.div_ceil(64) as usize;
        let bpp = l.channels * self.sample_bytes;
        for (i, offset) in l.tiles.iter().copied().enumerate() {
            let (tx, ty) = ((i % cols) as u32 * 64, (i / cols) as u32 * 64);
            let (tw, th) = ((l.width - tx).min(64) as usize, (l.height - ty).min(64) as usize);
            let count = tw * th;
            let end = l.tiles.get(i + 1).copied().unwrap_or(self.bytes.len()).min(offset.saturating_add(count * bpp * 4 + 1024));
            let input = self.bytes.get(offset..end).ok_or_else(|| bad("invalid tile offsets"))?;
            let raw = decode_tile(input, count, bpp, self.compression)?;
            for (n, pixel) in raw.chunks_exact(bpp).enumerate() {
                let mut p = [1.; 4];
                for (c, s) in pixel.chunks_exact(self.sample_bytes).enumerate() {
                    let v = match s {
                        [v] => f32::from(*v) / 255.,
                        [a, b] => f32::from(u16::from_be_bytes([*a, *b])) / 65535.,
                        _ => return Err(bad("bad sample")),
                    };
                    if let Some(d) = p.get_mut(c) {
                        *d = if c < 3 && self.linear_samples { linear_to_srgb(v) } else { v };
                    }
                }
                visit(tx + (n % tw) as u32, ty + (n / tw) as u32, p);
            }
        }
        Ok(())
    }
}
fn read_tiles(bytes: &[u8], offset: usize, wide: bool, w: u32, h: u32, bpp: usize) -> Result<Vec<usize>> {
    let mut r = Reader::new(bytes, offset, wide)?;
    if r.word()? != w || r.word()? != h || r.word()? as usize != bpp {
        return Err(bad("hierarchy dimensions/type disagree"));
    }
    let levels = r.ptrs(32)?;
    let level = levels.first().copied().ok_or_else(|| bad("missing pixel level"))?;
    let mut r = Reader::new(bytes, level, wide)?;
    if r.word()? != w || r.word()? != h {
        return Err(bad("level dimensions disagree"));
    }
    let count = w.div_ceil(64) as usize * h.div_ceil(64) as usize;
    let tiles = r.ptrs(count)?;
    if tiles.len() != count || tiles.first().is_some_and(|p| *p < r.p) || tiles.windows(2).any(|p| p[0] >= p[1]) {
        return Err(bad("invalid tile table"));
    }
    Ok(tiles)
}
fn decode_tile(input: &[u8], count: usize, bpp: usize, compression: u8) -> Result<Vec<u8>> {
    let size = count * bpp; // Caller bounds count <= 4096 and bpp <= 8.
    match compression {
        0 => Ok(input.get(..size).ok_or_else(|| bad("truncated raw tile"))?.to_vec()),
        2 => {
            let out = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(input, size).map_err(|_| bad("invalid/oversized zlib tile"))?;
            if out.len() != size {
                return Err(bad("short zlib tile"));
            }
            Ok(out)
        }
        1 => {
            let mut r = Reader::new(input, 0, false)?;
            let mut out = vec![0; size];
            for c in 0..bpp {
                let mut n = 0;
                while n < count {
                    let op = r.byte()?;
                    let run = match op {
                        0..=126 => usize::from(op) + 1,
                        127 | 128 => usize::from(u16::from_be_bytes([r.byte()?, r.byte()?])),
                        _ => 256 - usize::from(op),
                    };
                    if run == 0 || run > count - n {
                        return Err(bad("RLE run crosses plane boundary"));
                    }
                    if op <= 127 {
                        let v = r.byte()?;
                        for i in n..n + run {
                            *out.get_mut(i * bpp + c).ok_or_else(|| bad("RLE bounds"))? = v;
                        }
                    } else {
                        for (i, v) in r.take(run)?.iter().enumerate() {
                            *out.get_mut((n + i) * bpp + c).ok_or_else(|| bad("RLE bounds"))? = *v;
                        }
                    }
                    n += run;
                }
            }
            Ok(out)
        }
        _ => Err(unsupported("compression")),
    }
}
fn check_profile(bytes: &[u8], linear: bool) -> Result<()> {
    let mut r = Reader::new(bytes, 0, false)?;
    let mut count = 0;
    while r.p < bytes.len() {
        count += 1;
        if count > MAX_PROPERTIES {
            return Err(bad("too much metadata"));
        }
        let name = r.string()?;
        r.word()?;
        let n = r.word()? as usize;
        let data = r.take(n)?;
        if name == "icc-profile" {
            if n > 1024 * 1024 {
                return Err(unsupported("large ICC profile"));
            }
            let p = effectcraft_color::icc::parse(data).map_err(|_| unsupported("this ICC profile"))?;
            let primaries = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]];
            let chroma = p.primaries.iter().flatten().zip(primaries.iter().flatten()).all(|(a, b)| (a - b).abs() < 0.003)
                && p.white.iter().zip([0.3127, 0.3290]).all(|(a, b)| (a - b).abs() < 0.003);
            let trc = p.trc.iter().all(|t| t.is_srgb() || (linear && (0..=16).all(|i| (t.eval(i as f64 / 16.) - i as f64 / 16.).abs() < 0.001)));
            if !chroma || !trc {
                return Err(unsupported("non-sRGB colour profile (convert to built-in sRGB first)"));
            }
        }
    }
    Ok(())
}
