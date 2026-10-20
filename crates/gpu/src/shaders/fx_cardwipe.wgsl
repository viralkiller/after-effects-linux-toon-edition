fn fxcw_unpremul(p: vec4<f32>) -> vec4<f32> {
    if (p.w > 1e-6) {
        return vec4<f32>(p.xyz / p.w, p.w);
    }
    return vec4<f32>(0.0, 0.0, 0.0, max(p.w, 0.0));
}

struct CardShade {
    ok: bool,
    c: vec4<f32>,
}

fn fxcw_none() -> CardShade {
    return CardShade(false, vec4<f32>(0.0));
}

fn fxcw_piece(at: u32, x: f32, y: f32) -> CardShade {
    let m0 = vec3<f32>(data[at + 1u], data[at + 2u], data[at + 3u]);
    let m1 = vec3<f32>(data[at + 4u], data[at + 5u], data[at + 6u]);
    let m2 = vec3<f32>(data[at + 7u], data[at + 8u], data[at + 9u]);
    let w = m2.x * x + m2.y * y + m2.z;
    if (abs(w) < 1e-12) {
        return fxcw_none();
    }
    let c0 = vec2<f32>(data[at + 10u], data[at + 11u]);
    var u = (m0.x * x + m0.y * y + m0.z) / w + c0.x;
    var v = (m1.x * x + m1.y * y + m1.z) / w + c0.y;
    let n = u32(data[at + 24u]);
    var md = 3.4e38;
    for (var i = 0u; i < n; i++) {
        let j = (i + 1u) % n;
        let ax = data[at + 12u + i * 2u];
        let ay = data[at + 13u + i * 2u];
        let ex = data[at + 12u + j * 2u] - ax;
        let ey = data[at + 13u + j * 2u] - ay;
        let l = max(sqrt(ex * ex + ey * ey), 1e-6);
        md = min(md, (ex * (v - ay) - ey * (u - ax)) / l);
    }
    let cov = clamp(md + 1.0, 0.0, 1.0);
    if (cov <= 0.0) {
        return fxcw_none();
    }
    var c: vec3<f32>;
    var a = 1.0;
    if (data[at + 33u] != 0.0) {
        c = vec3<f32>(data[at + 34u], data[at + 35u], data[at + 36u]);
    } else {
        let back = data[at + 37u] != 0.0;
        let mirror = u32(data[at + 38u]);
        if (back && mirror == 1u) {
            v = 2.0 * c0.y - v;
        } else if (back && mirror == 2u) {
            u = 2.0 * c0.x - u;
        }
        var t: vec4<f32>;
        if (back && P.u[1].y != 0u) {
            t = sample_bilinear(aux, u, v);
        } else {
            t = sample_bilinear(src, u, v);
        }
        let s = fxcw_unpremul(t);
        c = s.xyz;
        a = s.w;
    }
    a = a * cov * data[at + 32u];
    if (a <= 1e-6) {
        return CardShade(false, vec4<f32>(0.0));
    }
    let tint = vec3<f32>(data[at + 26u], data[at + 27u], data[at + 28u]);
    let spec = vec3<f32>(data[at + 29u], data[at + 30u], data[at + 31u]);
    let o = c * data[at + 25u] * tint + spec;
    return CardShade(true, vec4<f32>(o * a, a));
}

@compute @workgroup_size(16, 16)
fn fxcw_raster(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = out_dims();
    let p = vec2<i32>(gid.xy);
    if (p.x >= dims.x || p.y >= dims.y) {
        return;
    }
    let tile = u32(p.y / 16) * P.u[1].x + u32(p.x / 16);
    let start = u32(data[tile]);
    let end = u32(data[tile + 1u]);
    let stride = P.u[0].w;
    let x = f32(p.x) + 0.5;
    let y = f32(p.y) + 0.5;
    var d = vec4<f32>(0.0);
    for (var k = start; k < end; k++) {
        let at = P.u[0].z + u32(data[k]) * stride;
        let bb = at + stride - 4u;
        if (f32(p.x) < data[bb] || f32(p.y) < data[bb + 1u] || f32(p.x) > data[bb + 2u] || f32(p.y) > data[bb + 3u]) {
            continue;
        }
        let s = fxcw_piece(at, x, y);
        if (!s.ok) {
            continue;
        }
        let c = s.c;
        d = c + d * (1.0 - c.w);
    }
    textureStore(out, p, d);
}
