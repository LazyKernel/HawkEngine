use nalgebra::{Vector3, DMatrix};
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder, RigidBodyType, RigidBody, Collider};

use crate::{ecs::components::{general::Renderable}, graphics::{models::{create_terrain_vertices, create_height_field}, vulkan::Vulkan}};



pub fn create_terrain(height_map_name: &str, texture_name: &str, vulkan: &Vulkan) -> (
    Result<Renderable, String>,
    RigidBody,
    Collider
) {
    let height_field = create_height_field(&format!("resources/heightmaps/{}.png", height_map_name), 128, 128);
    let (vertices, indices) = create_terrain_vertices(&height_field);

    let collider = ColliderBuilder::heightfield(
        DMatrix::from_iterator(128, 128, height_field.into_iter().flatten()), 
        Vector3::<f32>::zeros()
    ).build();

    let rigid_body = RigidBodyBuilder::new(RigidBodyType::Fixed).build();

    let renderable = vulkan.create_renderable_from_vertices(vertices, indices, texture_name, None);

    (renderable, rigid_body, collider)
}
