use std::sync::Arc;

use log::error;
use specs::{System, ReadStorage, Read, Write, Entities, Entity};
use vulkano::{command_buffer::{RenderPassBeginInfo, SubpassContents, AutoCommandBufferBuilder, CommandBufferUsage, allocator::{CommandBufferAllocator, StandardCommandBufferAllocator}, PrimaryAutoCommandBuffer}, descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet}, pipeline::{Pipeline, PipelineBindPoint}, buffer::TypedBufferAccess};

use crate::{ecs::{components::{general::{Transform, Renderable, Camera, Wireframe}, physics::ColliderRenderable}, resources::{ActiveCamera, RenderData, ProjectionMatrix, CommandBuffer, RenderDataFrameBuffer}}, shaders::default::vs::ty::{VPUniformBufferObject, ModelPushConstants}};

pub struct Render;

impl<'a> System<'a> for Render {
    type SystemData = (
        Entities<'a>,
        Option<Read<'a, ActiveCamera>>,
        Option<Read<'a, RenderData>>,
        Option<Read<'a, RenderDataFrameBuffer>>,
        Write<'a, CommandBuffer>,
        Read<'a, ProjectionMatrix>,
        ReadStorage<'a, Camera>,
        ReadStorage<'a, Transform>,
        ReadStorage<'a, Renderable>,
        ReadStorage<'a, ColliderRenderable>,
        ReadStorage<'a, Wireframe>
    );

    fn run(&mut self, (entities, active_cam, render_data, framebuffer, mut command_buffer, proj, _camera, transform, renderable, collider, wireframe): Self::SystemData) {
        use specs::Join;
        // Verify we have all dependencies
        // Abort if not
        let active_camera = match active_cam {
            Some(v) => v,
            None => {
                error!("Active camera was none");
                return
            }
        };

        let render_data = match render_data {
            Some(v) => v,
            None => {
                error!("Command buffer was none");
                return
            }
        };

        let framebuffer = match framebuffer {
            Some(v) => v,
            None => {
                error!("Framebuffer was none");
                return
            }
        };

        // Get camera view matrix from transform
        let view_matrix = match transform.get(active_camera.0) {
            Some(t) => {
                match t.transformation_matrix().try_inverse() {
                    Some(v) => v,
                    None => return error!("Somehow view matrix is not square, aborting rendering")
                }
            }
            // No transform on active camera
            None => return error!("No Transform on active camera, cannot render!")
        };

        // Create a command buffer
        let mut builder = AutoCommandBufferBuilder::primary(
            &render_data.command_buffer_allocator,
            render_data.queue_family_index,
            CommandBufferUsage::MultipleSubmit
        ).unwrap();

        // Setup ubo data
        let ubo_data = VPUniformBufferObject {
            view: view_matrix.into(),
            proj: proj.0.into()
        };
        let view_ubo = render_data.ubo_pool.from_data(ubo_data).unwrap();

        // Allocate and write model and view matrix to descriptor set
        let layout_view = render_data.pipeline.layout().set_layouts().get(0).unwrap();
        let descriptor_set_view = PersistentDescriptorSet::new(
            &render_data.descriptor_set_allocator,
            layout_view.clone(),
            [WriteDescriptorSet::buffer(0, view_ubo.clone())]
        ).unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into()), Some(1f32.into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer.0.clone())
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .bind_pipeline_graphics(render_data.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics, 
                render_data.pipeline.layout().clone(), 
                0, 
                descriptor_set_view.clone()
            );

        for (e, t, r, ()) in (&*entities, &transform, &renderable, !&wireframe).join() {
            self.render_entity(e, t, r, &mut builder, &render_data, true);
        }

        // Render wireframe pipeline
        builder
            .bind_pipeline_graphics(render_data.pipeline_wireframe.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics, 
                render_data.pipeline.layout().clone(), 
                0, 
                descriptor_set_view.clone()
            );

        // TODO: this is bad figure out a better way
        for (e, t, r) in (&*entities, &transform, &collider).join() {
            // TODO: this is horrible lmao
            self.render_entity(e, t, &Renderable { vertex_buffer: r.vertex_buffer.clone(), index_buffer: r.index_buffer.clone(), descriptor_set_texture: descriptor_set_view.clone() }, &mut builder, &render_data, false);
        }

        match builder.end_render_pass() {
            Ok(v) => v,
            Err(e) => return error!("Failed ending render pass: {:?}", e)
        };

        let buffer = match builder.build() {
            Ok(v) => Arc::new(v),
            Err(e) => return error!("Failed building command buffer: {:?}", e)
        };

        command_buffer.command_buffer = Some(buffer);
    }
}

impl Render {
    fn render_entity(
        &self,
        entity: Entity, 
        transform: &Transform, 
        renderable: &Renderable, 
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer, Arc<StandardCommandBufferAllocator>>, 
        render_data: &RenderData,
        has_texture: bool
    ) {
        // shorthands for convenience
        let e = entity;
        let t = transform;
        let r = renderable;

        // Insert the model matrix into a push constant
        let push_constants = ModelPushConstants {
            model: t.transformation_matrix().into()
        };
        // Bind everything required and render this entity
        if has_texture {
            builder.bind_descriptor_sets(PipelineBindPoint::Graphics, 
                render_data.pipeline.layout().clone(), 
                1, 
                r.descriptor_set_texture.clone()
            );
        }

        let result = builder
            .push_constants(render_data.pipeline.layout().clone(), 0, push_constants)
            .bind_vertex_buffers(0, r.vertex_buffer.clone())
            .bind_index_buffer(r.index_buffer.clone())
            .draw_indexed(r.index_buffer.len() as u32, 1, 0, 0, 0);

        if result.is_err() {
            error!("Building a command buffer failed for entity {:?}", e);
        }
    }
}
