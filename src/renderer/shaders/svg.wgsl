struct SvgSize {
    width: f32,
    height: f32,
    pad: vec2<u32>,
}

struct PrimitivesBuck {
    data: array<Primitive, 1024>,
}

struct Primitive {
    transform: u32,
    color: u32,
    pad: vec2<u32>,
};

struct TransformsBuck {
     data: array<Transform, 8192>,
}

struct Transform {
    data0: vec4<f32>,
    data1: vec4<f32>,
};

struct Primitives {
    data: array<PrimitivesBuck>,
};

struct Transforms {
    data: array<TransformsBuck>,
};

struct Sizes {
    data: array<SvgSize>,
};

@group(0) @binding(1) var<storage, read> primitives: Primitives;
@group(0) @binding(2) var<storage, read> transforms: Transforms;
@group(0) @binding(3) var<storage, read> svg_sizes: Sizes;

struct InstanceInput {
    @location(2) mvp_matrix_0: vec4<f32>,
    @location(3) mvp_matrix_1: vec4<f32>,
    @location(4) mvp_matrix_2: vec4<f32>,
    @location(5) mvp_matrix_3: vec4<f32>,
    @location(6) id: u32,
};

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) primitive_id: u32
};

struct VertexOutput {
    @location(0) v_color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var id = instance.id;
    var prim: Primitive = primitives.data[id].data[model.primitive_id];
    var t: Transform = transforms.data[id].data[prim.transform];
    var size: SvgSize = svg_sizes.data[id];

    var transform = mat3x3<f32>(
        vec3<f32>(t.data0.x, t.data0.y, 0.0),
        vec3<f32>(t.data0.z, t.data0.w, 0.0),
        vec3<f32>(t.data1.x, t.data1.y, 1.0)
    );

    var pos: vec2<f32> = (transform * vec3<f32>(model.position, 1.0)).xy;

    let mvp_matrix = mat4x4<f32>(
        instance.mvp_matrix_0,
        instance.mvp_matrix_1,
        instance.mvp_matrix_2,
        instance.mvp_matrix_3,
    );

    var invert_y = vec2<f32>(1.0, -1.0);
    var zoom = vec2<f32>(size.width, size.height);
    var offset = vec2<f32>(size.width / -2.0, size.height / -2.0);
    var position: vec4<f32> = mvp_matrix * vec4<f32>(((pos.xy + offset) / zoom) * invert_y, 0.0, 1.0);

    var mask: u32 = 255u;
    var color = vec4<f32>(
        f32(((prim.color >> u32(24)) & mask)),
        f32(((prim.color >> u32(16)) & mask)),
        f32(((prim.color >> u32(8)) & mask)),
        f32((prim.color & mask))
    ) / vec4<f32>(255.0);

    return VertexOutput(color, position);
}

struct FragmentOutput {
    @location(0) out_color: vec4<f32>,
};

@fragment
fn fs_main(@location(0) v_color: vec4<f32>) -> FragmentOutput {
    return FragmentOutput(v_color);
}
