//! Mesh primitives: the shapes the engine can build itself, as triangle
//! meshes the GPU draws, like Godot's `SphereMesh` and `PlaneMesh` or
//! Bevy's `Sphere` and `Plane3d` primitives. Built once when a scene
//! loads.
//!
//! Texture coordinates are in *tiles*: a texture covering `tile_size`
//! metres is repeated across the surface, so a surface of any size keeps
//! the same texture density (Godot's `uv1_scale`, Unity's "Tiling").

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use crate::render::Mesh;

/// Quads around a sphere's equator, and from pole to pole: enough for a
/// smooth silhouette at the sizes we draw.
const SPHERE_SEGMENTS: u32 = 32;
const SPHERE_RINGS: u32 = 16;

/// A sphere of `radius` metres around the origin, in render axes (y up).
/// Its texture repeats every `tile_size` metres along the surface.
pub(crate) fn sphere(radius: f32, tile_size: f32) -> Mesh {
    let mut mesh = empty_mesh();
    // A grid of (rings + 1) × (segments + 1) vertices, by latitude then
    // longitude. The seam column is doubled (longitude 0 and 2π) so its
    // texture coordinates can differ on each side.
    for ring in 0..=SPHERE_RINGS {
        let latitude = PI / 2.0 - PI * ring as f32 / SPHERE_RINGS as f32; // top first
        for segment in 0..=SPHERE_SEGMENTS {
            let longitude = TAU * segment as f32 / SPHERE_SEGMENTS as f32;
            let normal = Vec3::new(
                latitude.cos() * longitude.cos(),
                latitude.sin(),
                -latitude.cos() * longitude.sin(),
            );
            mesh.positions.push(normal * radius);
            mesh.normals.push(normal);
            // Distances along the surface, in tiles: around the equator,
            // and down from the north pole.
            let around = longitude * radius / tile_size;
            let down = (PI / 2.0 - latitude) * radius / tile_size;
            mesh.uvs.push(Vec2::new(around, down));
        }
    }
    let row = SPHERE_SEGMENTS + 1;
    for ring in 0..SPHERE_RINGS {
        for segment in 0..SPHERE_SEGMENTS {
            // A quad: two triangles, counter-clockwise seen from outside.
            let top_left = ring * row + segment;
            let (top_right, bottom_left) = (top_left + 1, top_left + row);
            let bottom_right = bottom_left + 1;
            mesh.triangles.push([top_left, bottom_left, bottom_right]);
            mesh.triangles.push([top_left, bottom_right, top_right]);
        }
    }
    mesh
}

/// A horizontal square around the origin, in render axes (y up),
/// `half_size` metres from its centre to each side. Its texture repeats
/// every `tile_size` metres.
pub(crate) fn plane(half_size: f32, tile_size: f32) -> Mesh {
    let h = half_size;
    // North-west, south-west, south-east, north-east (z is south).
    let corners = [
        Vec3::new(-h, 0.0, -h),
        Vec3::new(-h, 0.0, h),
        Vec3::new(h, 0.0, h),
        Vec3::new(h, 0.0, -h),
    ];
    Mesh {
        positions: corners.to_vec(),
        normals: vec![Vec3::Y; corners.len()],
        uvs: corners
            .iter()
            .map(|c| Vec2::new(c.x, c.z) / tile_size)
            .collect(),
        // Counter-clockwise seen from above: the side GPUs draw as the
        // front, and light as facing up.
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

fn empty_mesh() -> Mesh {
    Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        triangles: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Float maths on f32 (trigonometry, products): within 1e-5 of exact.
    const TOLERANCE: f32 = 1e-5;

    /// The outward direction of each triangle: the cross product of two
    /// edges, by the right-hand rule (counter-clockwise = towards you).
    fn facing(mesh: &Mesh) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        mesh.triangles.iter().map(|&[a, b, c]| {
            let p = |i: u32| mesh.positions[i as usize];
            let centre = (p(a) + p(b) + p(c)) / 3.0;
            (centre, (p(b) - p(a)).cross(p(c) - p(a)))
        })
    }

    #[test]
    fn every_sphere_vertex_is_on_the_surface_with_an_outward_normal() {
        let mesh = sphere(2.0, 1.0);
        for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
            assert!((position.length() - 2.0).abs() < TOLERANCE, "{position}");
            assert!((*normal - *position / 2.0).length() < TOLERANCE);
        }
    }

    #[test]
    fn sphere_triangles_face_outwards() {
        // Except the slivers at the poles, where two corners coincide and
        // the triangle has no area (and no direction).
        let mesh = sphere(1.0, 1.0);
        let mut faces = 0;
        for (centre, direction) in facing(&mesh) {
            if direction.length() > TOLERANCE {
                assert!(direction.dot(centre) > 0.0, "inwards at {centre}");
                faces += 1;
            }
        }
        // Every quad but the pole ones gives two real triangles.
        let quads = SPHERE_SEGMENTS * SPHERE_RINGS;
        assert_eq!(faces, (2 * quads - 2 * SPHERE_SEGMENTS) as usize);
    }

    #[test]
    fn the_sphere_texture_repeats_every_tile_along_the_surface() {
        // Radius 1: the equator is 2π m round, pole to pole is π m. With
        // 0.5 m tiles: 4π tiles around and 2π from pole to pole.
        let mesh = sphere(1.0, 0.5);
        let max = mesh.uvs.iter().fold(Vec2::ZERO, |m, uv| m.max(*uv));
        assert!((max - Vec2::new(4.0 * PI, 2.0 * PI)).length() < TOLERANCE);
        assert!(mesh.uvs.iter().all(|uv| uv.min_element() >= 0.0));
    }

    #[test]
    fn the_plane_is_a_flat_square_facing_up() {
        let mesh = plane(100.0, 10.0);
        assert!(mesh.positions.iter().all(|p| p.y == 0.0));
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.x.abs() == 100.0 && p.z.abs() == 100.0)
        );
        assert!(mesh.normals.iter().all(|n| *n == Vec3::Y));
        for (_, direction) in facing(&mesh) {
            assert!(direction.y > 0.0, "{direction}");
        }
    }

    #[test]
    fn the_plane_texture_repeats_every_tile() {
        // 100 m to each side, 10 m tiles: from -10 to 10 tiles.
        let mesh = plane(100.0, 10.0);
        assert!(mesh.uvs.iter().all(|uv| uv.abs() == Vec2::splat(10.0)));
    }
}
