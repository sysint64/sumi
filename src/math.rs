use glam::{Mat4, Quat, Vec2, Vec3};

/// Contains various matrices used for camera transformations.
#[derive(Debug, Clone, Copy)]
pub struct CameraMatrices {
    /// The view transformation matrix.
    pub view_matrix: Mat4,
    /// The projection transformation matrix.
    pub projection_matrix: Mat4,
    /// The model transformation matrix.
    pub model_matrix: Mat4,
    /// The combined model-view-projection transformation matrix.
    pub mvp_matrix: Mat4,
}

/// Represents the basic transforms for an orthographic camera.
#[derive(Default, Debug, Clone, Copy)]
pub struct OthroCameraTransforms {
    /// The size of the viewport (width and height).
    pub viewport_size: Vec2,
    /// The position of the camera in 2D space.
    pub position: Vec2,
    /// The zoom level of the camera.
    pub zoom: f32,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Transforms2D {
    pub position: Vec2,
    pub scaling: Vec2,
    pub rotation: f32,
}

impl Default for CameraMatrices {
    fn default() -> Self {
        CameraMatrices {
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            model_matrix: Mat4::IDENTITY,
            mvp_matrix: Mat4::IDENTITY,
        }
    }
}

pub fn create_ortho_camera_matrices(transforms: &OthroCameraTransforms) -> CameraMatrices {
    let eye = transforms.position.extend(1.0);
    let target = transforms.position.extend(0.0);
    let up = Vec3::new(0.0, 1.0, 0.0);

    let view_matrix = Mat4::look_at_rh(eye, target, up);
    let projection_matrix = Mat4::orthographic_rh(
        0.0,
        transforms.viewport_size.x,
        0.0,
        transforms.viewport_size.y,
        0.0,
        10.0,
    );

    let model_matrix = if transforms.zoom != 1.0 {
        Mat4::from_scale(Vec3::new(transforms.zoom, transforms.zoom, 1.0))
    } else {
        Mat4::IDENTITY
    };

    let mvp_matrix = model_matrix * projection_matrix * view_matrix;

    CameraMatrices {
        view_matrix,
        projection_matrix,
        model_matrix,
        mvp_matrix,
    }
}

/// Creates a set of camera matrices for a centered orthographic camera.
///
/// This function creates a set of matrices for an orthographic camera centered on the given position.
/// It takes into account the viewport size, camera position, and zoom level to create appropriate
/// view, projection, and model matrices. The function then combines these to create an MVP
/// (Model-View-Projection) matrix.
///
/// # Parameters
///
/// * `transforms` - A reference to an `OthroCameraTransforms` struct containing the basic camera transforms.
///
/// # Returns
///
/// Returns a `CameraMatrices` struct containing the calculated view, projection, model, and MVP matrices.
///
/// # Example
///
/// ```
/// use tech_paws_graphics::math::{create_centered_ortho_camera_matrices, OthroCameraTransforms};
/// use glam::Vec2;
///
/// let transforms = OthroCameraTransforms {
///     viewport_size: Vec2::new(800.0, 600.0),
///     position: Vec2::new(400.0, 300.0),
///     zoom: 1.0,
/// };
/// let matrices = create_centered_ortho_camera_matrices(&transforms);
/// ```
pub fn create_centered_ortho_camera_matrices(transforms: &OthroCameraTransforms) -> CameraMatrices {
    let eye = Vec3::new(
        transforms.position.x - transforms.viewport_size.x / 2.,
        transforms.position.y - transforms.viewport_size.y / 2.,
        1.0,
    );
    let target = Vec3::new(
        transforms.position.x - transforms.viewport_size.x / 2.,
        transforms.position.y - transforms.viewport_size.y / 2.,
        0.0,
    );
    let up = Vec3::new(0.0, 1.0, 0.0);

    let view_matrix = Mat4::look_at_rh(eye, target, up);
    let projection_matrix = Mat4::orthographic_rh(
        0.0,
        transforms.viewport_size.x,
        0.0,
        transforms.viewport_size.y,
        0.0,
        10.0,
    );

    let model_matrix = if transforms.zoom != 1.0 {
        Mat4::from_scale(Vec3::new(transforms.zoom, transforms.zoom, 1.0))
    } else {
        Mat4::IDENTITY
    };

    let mvp_matrix = model_matrix * projection_matrix * view_matrix;

    CameraMatrices {
        view_matrix,
        projection_matrix,
        model_matrix,
        mvp_matrix,
    }
}

pub fn transforms_create_2d_model_matrix(transforms: &Transforms2D) -> Mat4 {
    let scale = transforms.scaling.extend(1.0);
    let translation = transforms.position.extend(0.0);
    let rotation = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), transforms.rotation);

    Mat4::from_scale_rotation_translation(scale, rotation, translation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    // Helper function to compare Mat4 values
    fn assert_mat4_eq(a: &Mat4, b: &Mat4, epsilon: f32) {
        for i in 0..4 {
            for j in 0..4 {
                assert_relative_eq!(a.col(i)[j], b.col(i)[j], epsilon = epsilon);
            }
        }
    }

    #[test]
    fn test_create_centered_ortho_camera_matrices() {
        let transforms = OthroCameraTransforms {
            viewport_size: Vec2::new(800.0, 600.0),
            position: Vec2::new(400.0, 300.0),
            zoom: 1.0,
        };

        let matrices = create_centered_ortho_camera_matrices(&transforms);

        // Test view matrix (adjust expected values if necessary)
        let expected_view = Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        assert_mat4_eq(&matrices.view_matrix, &expected_view, 1e-5);

        // Test projection matrix
        let expected_projection = Mat4::orthographic_rh(0.0, 800.0, 0.0, 600.0, 0.0, 10.0);
        assert_mat4_eq(&matrices.projection_matrix, &expected_projection, 1e-5);

        // Test model matrix
        assert_mat4_eq(&matrices.model_matrix, &Mat4::IDENTITY, 1e-5);

        // Test MVP matrix
        let expected_mvp = expected_projection * expected_view;
        assert_mat4_eq(&matrices.mvp_matrix, &expected_mvp, 1e-5);
    }
}
