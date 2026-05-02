struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) coord: vec2<f32>,
};

struct InstanceInput {
    @location(5) color: vec4<f32>,
    @location(6) mvp_matrix_0: vec4<f32>,
    @location(7) mvp_matrix_1: vec4<f32>,
    @location(8) mvp_matrix_2: vec4<f32>,
    @location(9) mvp_matrix_3: vec4<f32>,
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
    out.coord = model.position.xy;
    out.clip_position = mvp_matrix * vec4<f32>(model.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let R = 0.5;

    let dist = length(in.coord);
    let RR = R - 0.002;
    let sm = smoothstep(R, RR, dist);
    let alpha = sm;

    if (dist < R) {
        return vec4(in.color.rgb, min(1.0, in.color.a + alpha));
    } else {
        return vec4(0., 0., 0., 0.);
    }
}
