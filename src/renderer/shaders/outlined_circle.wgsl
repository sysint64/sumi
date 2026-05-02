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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = vec2(in.radius);
    let R = 0.5;
    let R2 = 0.5 - (in.width / size.x / 2.);

    let dist = length(in.coord);
    let RR = R - 0.005;
    let RR2 = R2 + 0.005;
    let sm = smoothstep(R, RR, dist);
    let sm2 = smoothstep(R2, RR2, dist);
    let alpha = sm*sm2;

    if (dist < 0.5 && dist > R2) {
        return vec4(in.color.rgb, in.color.a + alpha);
    } else {
        return vec4(0., 0., 0., 0.);
    }
}
