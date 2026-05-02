use super::resources::mesh_2d::{
    Mesh2DGpuData, Mesh2DGpuPrimitive, Mesh2DGpuTransform, Mesh2DVertex,
};
use lyon::math::Point;
use lyon::path::PathEvent;
use lyon::tessellation::geometry_builder::*;
use lyon::tessellation::{self, FillOptions, FillTessellator, StrokeOptions, StrokeTessellator};

const FALLBACK_COLOR: usvg::Color = usvg::Color {
    red: 0,
    green: 0,
    blue: 0,
};

pub struct VertexCtor {
    pub prim_id: u32,
}

impl FillVertexConstructor<Mesh2DVertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> Mesh2DVertex {
        Mesh2DVertex {
            position: vertex.position().to_array(),
            prim_id: self.prim_id,
        }
    }
}

impl StrokeVertexConstructor<Mesh2DVertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex) -> Mesh2DVertex {
        Mesh2DVertex {
            position: vertex.position().to_array(),
            prim_id: self.prim_id,
        }
    }
}

fn new_primitive(transform_idx: u32, color: usvg::Color, alpha: f32) -> Mesh2DGpuPrimitive {
    Mesh2DGpuPrimitive {
        transform: transform_idx,
        color: ((color.red as u32) << 24)
            + ((color.green as u32) << 16)
            + ((color.blue as u32) << 8)
            + (alpha * 255.0) as u32,
        _pad: [0; 2],
    }
}

/// Some glue between usvg's iterators and lyon's.
pub struct PathConvIter<'a> {
    iter: tiny_skia_path::PathSegmentsIter<'a>,
    prev: Point,
    first: Point,
    needs_end: bool,
    deferred: Option<PathEvent>,
}

impl Iterator for PathConvIter<'_> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        if self.deferred.is_some() {
            return self.deferred.take();
        }

        let next = self.iter.next();
        match next {
            Some(tiny_skia_path::PathSegment::MoveTo(pt)) => {
                if self.needs_end {
                    let last = self.prev;
                    let first = self.first;
                    self.needs_end = false;
                    self.prev = Point::new(pt.x, pt.y);
                    self.deferred = Some(PathEvent::Begin { at: self.prev });
                    self.first = self.prev;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    self.first = Point::new(pt.x, pt.y);
                    self.needs_end = true;
                    Some(PathEvent::Begin { at: self.first })
                }
            }
            Some(tiny_skia_path::PathSegment::LineTo(pt)) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = Point::new(pt.x, pt.y);
                Some(PathEvent::Line {
                    from,
                    to: self.prev,
                })
            }
            Some(tiny_skia_path::PathSegment::CubicTo(p1, p2, p0)) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = Point::new(p0.x, p0.y);
                Some(PathEvent::Cubic {
                    from,
                    ctrl1: Point::new(p1.x, p1.y),
                    ctrl2: Point::new(p2.x, p2.y),
                    to: self.prev,
                })
            }
            Some(tiny_skia_path::PathSegment::QuadTo(p0, p1)) => {
                self.needs_end = true;
                let from = self.prev;
                self.prev = Point::new(p1.x, p1.y);
                Some(PathEvent::Quadratic {
                    from,
                    ctrl: Point::new(p0.x, p0.y),
                    to: self.prev,
                })
            }
            Some(tiny_skia_path::PathSegment::Close) => {
                self.needs_end = false;
                self.prev = self.first;
                Some(PathEvent::End {
                    last: self.prev,
                    first: self.first,
                    close: true,
                })
            }
            None => {
                if self.needs_end {
                    self.needs_end = false;
                    let last = self.prev;
                    let first = self.first;
                    Some(PathEvent::End {
                        last,
                        first,
                        close: false,
                    })
                } else {
                    None
                }
            }
        }
    }
}

fn convert_path(p: &usvg::Path) -> PathConvIter<'_> {
    PathConvIter {
        iter: p.data().segments(),
        first: Point::new(0.0, 0.0),
        prev: Point::new(0.0, 0.0),
        deferred: None,
        needs_end: false,
    }
}

fn convert_stroke(s: &usvg::Stroke) -> (usvg::Color, StrokeOptions) {
    let color = match s.paint() {
        usvg::Paint::Color(c) => *c,
        _ => FALLBACK_COLOR,
    };
    let linecap = match s.linecap() {
        usvg::LineCap::Butt => tessellation::LineCap::Butt,
        usvg::LineCap::Square => tessellation::LineCap::Square,
        usvg::LineCap::Round => tessellation::LineCap::Round,
    };
    let linejoin = match s.linejoin() {
        usvg::LineJoin::Miter => tessellation::LineJoin::Miter,
        usvg::LineJoin::MiterClip => tessellation::LineJoin::MiterClip,
        usvg::LineJoin::Bevel => tessellation::LineJoin::Bevel,
        usvg::LineJoin::Round => tessellation::LineJoin::Round,
    };

    let opt = StrokeOptions::tolerance(0.01)
        .with_line_width(s.width().get())
        .with_line_cap(linecap)
        .with_line_join(linejoin);

    (color, opt)
}

pub fn collect_geom(
    group: &usvg::Group,
    prev_transform: &mut usvg::Transform,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    geometry: &mut Mesh2DGpuData,
) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => {
                collect_geom(group, prev_transform, fill_tess, stroke_tess, geometry)
            }
            usvg::Node::Path(p) => {
                let t = node.abs_transform();
                if t != *prev_transform {
                    geometry.transforms.push(Mesh2DGpuTransform {
                        data0: [t.sx, t.kx, t.ky, t.sy],
                        data1: [t.tx, t.ty, 0.0, 0.0],
                    });
                }
                *prev_transform = t;

                let transform_idx = geometry.transforms.len() as u32 - 1;

                if let Some(fill) = p.fill() {
                    // fall back to always use color fill
                    // no gradients (yet?)
                    let color = match fill.paint() {
                        usvg::Paint::Color(c) => *c,
                        _ => FALLBACK_COLOR,
                    };

                    geometry.primitives.push(new_primitive(
                        transform_idx,
                        color,
                        fill.opacity().get(),
                    ));

                    fill_tess
                        .tessellate(
                            convert_path(p),
                            &FillOptions::tolerance(0.01),
                            &mut BuffersBuilder::new(
                                &mut geometry.data,
                                VertexCtor {
                                    prim_id: geometry.primitives.len() as u32 - 1,
                                },
                            ),
                        )
                        .expect("Error during tessellation!");
                }

                if let Some(stroke) = p.stroke() {
                    let (stroke_color, stroke_opts) = convert_stroke(stroke);

                    geometry.primitives.push(new_primitive(
                        transform_idx,
                        stroke_color,
                        stroke.opacity().get(),
                    ));
                    let _ = stroke_tess.tessellate(
                        convert_path(p),
                        &stroke_opts.with_tolerance(0.01),
                        &mut BuffersBuilder::new(
                            &mut geometry.data,
                            VertexCtor {
                                prim_id: geometry.primitives.len() as u32 - 1,
                            },
                        ),
                    );
                }
            }
            _ => continue,
        }
    }
}
