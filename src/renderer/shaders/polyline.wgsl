// Original solution: https://github.com/ForesightMiningSoftwareCorporation/bevy_polyline
struct Globals {
    viewport_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct Vertex {
    @location(0) depth_bias: f32,
    @location(1) width: f32,
    @location(2) I_Point0_: vec3<f32>,
    @location(3) I_Point1_: vec3<f32>,
    @location(4) color: vec4<f32>,
    @location(5) mvp_matrix_0: vec4<f32>,
    @location(6) mvp_matrix_1: vec4<f32>,
    @location(7) mvp_matrix_2: vec4<f32>,
    @location(8) mvp_matrix_3: vec4<f32>,
    @builtin(vertex_index) index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var positions: array<vec3<f32>, 6u> = array<vec3<f32>, 6u>(
        vec3<f32>(0.0, -0.5, 0.0),
        vec3<f32>(0.0, -0.5, 1.0),
        vec3<f32>(0.0, 0.5, 1.0),
        vec3<f32>(0.0, -0.5, 0.0),
        vec3<f32>(0.0, 0.5, 1.0),
        vec3<f32>(0.0, 0.5, 0.0)
    );
    let position = positions[vertex.index];

    // algorithm based on https://wwwtyro.net/2019/11/18/instanced-lines.html
    let mvp_matrix = mat4x4<f32>(
        vertex.mvp_matrix_0,
        vertex.mvp_matrix_1,
        vertex.mvp_matrix_2,
        vertex.mvp_matrix_3,
    );
    let clip0 = mvp_matrix * vec4<f32>(vertex.I_Point0_, 1.0);
    let clip1 = mvp_matrix * vec4<f32>(vertex.I_Point1_, 1.0);
    let clip = mix(clip0, clip1, position.z);

    let resolution = vec2<f32>(globals.viewport_size.x, globals.viewport_size.y);
    let screen0 = resolution * (0.5 * clip0.xy / clip0.w + 0.5);
    let screen1 = resolution * (0.5 * clip1.xy / clip1.w + 0.5);

    let xBasis = normalize(screen1 - screen0);
    let yBasis = vec2<f32>(-xBasis.y, xBasis.x);

    var line_width = vertex.width;
    var color = vertex.color;

    // #ifdef POLYLINE_PERSPECTIVE
    // line_width = line_width / clip.w;
    //     // Line thinness fade from https://acegikmo.com/shapes/docs/#anti-aliasing
    // if (line_width < 1.0) {
    //     color.a = color.a * line_width;
    //     line_width = 1.0;
    // }
    // #endif

    let pt0 = screen0 + line_width * (position.x * xBasis + position.y * yBasis);
    let pt1 = screen1 + line_width * (position.x * xBasis + position.y * yBasis);
    let pt = mix(pt0, pt1, position.z);

    var depth: f32 = clip.z;
    if (vertex.depth_bias >= 0.0) {
        depth = depth * (1.0 - vertex.depth_bias);
    } else {
        let epsilon = 4.88e-04;
        // depth * (clip.w / depth)^-depth_bias. So that when -depth_bias is 1.0, this is equal to clip.w
        // and when equal to 0.0, it is exactly equal to depth.
        // the epsilon is here to prevent the depth from exceeding clip.w when -depth_bias = 1.0
        // clip.w represents the near plane in homogenous clip space in bevy, having a depth
        // of this value means nothing can be in front of this
        // The reason this uses an exponential function is that it makes it much easier for the
        // user to chose a value that is convinient for them
        depth = depth * exp2(-vertex.depth_bias * log2(clip.w / depth - epsilon));
    }

    return VertexOutput(vec4<f32>(clip.w * ((2.0 * pt) / resolution - 1.0), depth, clip.w), color);
}

struct FragmentInput {
    @location(0) color: vec4<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

@fragment
fn fragment(in: FragmentInput) -> FragmentOutput {
    return FragmentOutput(in.color);
}
