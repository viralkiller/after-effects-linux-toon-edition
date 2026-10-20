// ---------------------------------------------------------------- Curl Noise (noise2::curl_noise)
//
// u[0] = (seed, flow steps, wrap edges); f[0] = (offset x, offset y, scale, complexity);
// f[1] = (cos rotation, sin rotation, evolution · 4, step length).

// The fBm potential at each pixel (in .x).
@compute @workgroup_size(16, 16)
fn fxp_curl_pot(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = out_dims();
    let p = vec2<i32>(gid.xy);
    if (p.x >= dims.x || p.y >= dims.y) {
        return;
    }
    let dx = f32(p.x) + 0.5 - P.f[0].x;
    let dy = f32(p.y) + 0.5 - P.f[0].y;
    let cr = P.f[1].x;
    let sr = P.f[1].y;
    let u = (dx * cr + dy * sr) / P.f[0].z;
    let v = (-dx * sr + dy * cr) / P.f[0].z;
    textureStore(out, p, vec4<f32>(fxs_fbm(u, v, P.f[1].z, P.u[0].x, P.f[0].w), 0.0, 0.0, 0.0));
}

// The curl of the potential (src): (∂ψ/∂y, −∂ψ/∂x) · scale, edges clamped.
@compute @workgroup_size(16, 16)
fn fxp_curl_vel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = out_dims();
    let p = vec2<i32>(gid.xy);
    if (p.x >= dims.x || p.y >= dims.y) {
        return;
    }
    let gx = (tex_get_clamped(src, p.x + 1, p.y).x - tex_get_clamped(src, p.x - 1, p.y).x) * 0.5;
    let gy = (tex_get_clamped(src, p.x, p.y + 1).x - tex_get_clamped(src, p.x, p.y - 1).x) * 0.5;
    textureStore(out, p, vec4<f32>(gy * P.f[0].z, -gx * P.f[0].z, 0.0, 0.0));
}

// The Flow Field view of the velocities (src).
@compute @workgroup_size(16, 16)
fn fxp_curl_view(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = out_dims();
    let p = vec2<i32>(gid.xy);
    if (p.x >= dims.x || p.y >= dims.y) {
        return;
    }
    let v = textureLoad(src, p, 0);
    textureStore(out, p, vec4<f32>(clamp(0.5 + v.x * 0.5, 0.0, 1.0), clamp(0.5 + v.y * 0.5, 0.0, 1.0), 0.5, 1.0));
}

// The layer (src) advected backwards along the velocities (aux).
@compute @workgroup_size(16, 16)
fn fxp_curl_flow(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = out_dims();
    let p = vec2<i32>(gid.xy);
    if (p.x >= dims.x || p.y >= dims.y) {
        return;
    }
    var sx = f32(p.x) + 0.5;
    var sy = f32(p.y) + 0.5;
    let dt = P.f[1].w;
    for (var i = 0u; i < P.u[0].y; i++) {
        let vel = sample_bilinear_clamped(aux, sx, sy);
        sx -= vel.x * dt;
        sy -= vel.y * dt;
    }
    if (P.u[0].z != 0u) {
        let w = f32(dims.x);
        let h = f32(dims.y);
        sx = sx - w * floor(sx / w);
        sy = sy - h * floor(sy / h);
    }
    textureStore(out, p, sample_bilinear_clamped(src, sx, sy));
}
