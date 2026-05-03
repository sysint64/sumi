struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) coord: vec2<f32>,
    @location(2) width: f32,
    @location(3) radius: f32,
};

struct InstanceInput {
    @location(5) width: f32,
    @location(6) radius: f32,
    @location(7) color: vec4<f32>,
    @location(8) mvp_matrix_0: vec4<f32>,
    @location(9) mvp_matrix_1: vec4<f32>,
    @location(10) mvp_matrix_2: vec4<f32>,
    @location(11) mvp_matrix_3: vec4<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let mvp_matrix = mat4x4<f32>(
        instance.mvp_matrix_0,
        instance.mvp_matrix_1,
        instance.mvp_matrix_2,
        instance.mvp_matrix_3,
    );

    var out: VertexOutput;
    out.color = instance.color;
    out.width = instance.width;
    out.radius = instance.radius;
    out.coord = model.position.xy;
    out.clip_position = mvp_matrix * vec4<f32>(model.position, 1.0);
    return out;
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    } else {
        return pow((c + 0.055) / 1.055, 2.4);
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let R = 0.5;
    let R2 = 0.5 - (in.width / in.radius / 2.0);
    let dist = length(in.coord);
    let aa = fwidth(dist);
    let outer = smoothstep(R + aa, R - aa, dist);
    let inner = smoothstep(R2 - aa, R2 + aa, dist);
    let linear = vec3(srgb_to_linear(in.color.r), srgb_to_linear(in.color.g), srgb_to_linear(in.color.b));
    return vec4(linear, in.color.a * outer * inner);
}
