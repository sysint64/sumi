struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @location(0) color: vec4<f32>,
    @builtin(position) clip_position: vec4<f32>,
};

struct InstanceInput {
    @location(3) mvp_matrix_0: vec4<f32>,
    @location(4) mvp_matrix_1: vec4<f32>,
    @location(5) mvp_matrix_2: vec4<f32>,
    @location(6) mvp_matrix_3: vec4<f32>,
    @location(7) color: vec4<f32>,
};

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    } else {
        return pow((c + 0.055) / 1.055, 2.4);
    }
}

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

    out.clip_position = mvp_matrix * vec4<f32>(model.position, 1.0);
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(
        srgb_to_linear(color.r),
        srgb_to_linear(color.g),
        srgb_to_linear(color.b),
        color.a,
    );
}
