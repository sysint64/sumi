struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) coord: vec2<f32>,
    @location(1) fill_color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) border_widths: vec4<f32>,
    @location(4) size: vec2<f32>,
    @location(5) border_radius: f32,
};

struct InstanceInput {
    @location(5)  mvp_matrix_0:  vec4<f32>,
    @location(6)  mvp_matrix_1:  vec4<f32>,
    @location(7)  mvp_matrix_2:  vec4<f32>,
    @location(8)  mvp_matrix_3:  vec4<f32>,
    @location(9)  fill_color:    vec4<f32>,
    @location(10) border_color:  vec4<f32>,
    @location(11) border_widths: vec4<f32>,
    @location(12) size:          vec2<f32>,
    @location(13) border_radius: f32,
};

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    } else {
        return pow((c + 0.055) / 1.055, 2.4);
    }
}

// Returns negative inside, positive outside.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + r;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    let mvp_matrix = mat4x4<f32>(
        instance.mvp_matrix_0,
        instance.mvp_matrix_1,
        instance.mvp_matrix_2,
        instance.mvp_matrix_3,
    );

    var out: VertexOutput;
    out.coord         = model.position.xy;
    out.fill_color    = instance.fill_color;
    out.border_color  = instance.border_color;
    out.border_widths = instance.border_widths;
    out.size          = instance.size;
    out.border_radius = instance.border_radius;
    out.clip_position = mvp_matrix * vec4<f32>(model.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let half_size = in.size * 0.5;

    // coord is in [-0.5, 0.5]; scale to local pixel space.
    let p = in.coord * in.size;

    // Outer rounded rect boundary.
    let outer_dist  = sdf_rounded_rect(p, half_size, in.border_radius);
    let outer_aa    = 0.5 * fwidth(outer_dist);
    let outer_alpha = 1.0 - smoothstep(-outer_aa, outer_aa, outer_dist);

    // Inner fill region, inset by per-side border widths.
    // border_widths: (top, right, bottom, left)
    let inner_offset = vec2(
        (in.border_widths.w - in.border_widths.y) * 0.5,
        (in.border_widths.x - in.border_widths.z) * 0.5,
    );
    let inner_half_size = max(
        half_size - vec2(
            (in.border_widths.w + in.border_widths.y) * 0.5,
            (in.border_widths.x + in.border_widths.z) * 0.5,
        ),
        vec2(0.0),
    );
    let min_border   = min(min(in.border_widths.x, in.border_widths.y), min(in.border_widths.z, in.border_widths.w));
    let inner_radius = max(0.0, in.border_radius - min_border);

    let inner_dist  = sdf_rounded_rect(p - inner_offset, inner_half_size, inner_radius);
    let inner_aa    = 0.5 * fwidth(inner_dist);
    let fill_factor = 1.0 - smoothstep(-inner_aa, inner_aa, inner_dist);

    let color = mix(in.border_color, in.fill_color, fill_factor);
    let linear_rgb = vec3(srgb_to_linear(color.r), srgb_to_linear(color.g), srgb_to_linear(color.b));

    return vec4(linear_rgb, color.a * outer_alpha);
}
