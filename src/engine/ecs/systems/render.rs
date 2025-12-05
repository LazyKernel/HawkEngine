use log::error;
use specs::{Entities, Entity, Read, ReadStorage, System, Write};
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        RenderPassBeginInfo, SubpassBeginInfo, SubpassContents, SubpassEndInfo,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::{Pipeline, PipelineBindPoint},
};

use crate::{
    ecs::{
        components::{
            general::{Camera, Renderable, Transform, Wireframe},
            physics::ColliderRenderable,
        },
        resources::{
            ActiveCamera, CommandBuffer, ProjectionMatrix, RenderData, RenderDataFrameBuffer,
        },
    },
    shaders::default::vs::{ModelPushConstants, VPUniformBufferObject},
};

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
        ReadStorage<'a, Wireframe>,
    );

    fn run(
        &mut self,
        (
            entities,
            active_cam,
            render_data,
            framebuffer,
            mut command_buffer,
            proj,
            _camera,
            transform,
            renderable,
            collider,
            wireframe,
        ): Self::SystemData,
    ) {
        use specs::Join;
        // Verify we have all dependencies
        // Abort if not
        let active_camera = match active_cam {
            Some(v) => v,
            None => {
                error!("Active camera was none");
                return;
            }
        };

        let render_data = match render_data {
            Some(v) => v,
            None => {
                error!("Render data was none");
                return;
            }
        };

        let framebuffer = match framebuffer {
            Some(v) => v,
            None => {
                error!("Framebuffer was none");
                return;
            }
        };

        // Get camera view matrix from transform
        let view_matrix = match transform.get(active_camera.0) {
            Some(t) => match t.transformation_matrix().try_inverse() {
                Some(v) => v,
                None => return error!("Somehow view matrix is not square, aborting rendering"),
            },
            // No transform on active camera
            None => return error!("No Transform on active camera, cannot render!"),
        };

        // Create a command buffer
        let mut builder = AutoCommandBufferBuilder::primary(
            render_data.command_buffer_allocator.clone(),
            render_data.queue_family_index,
            CommandBufferUsage::MultipleSubmit,
        )
        .unwrap();

        // Setup ubo data
        let ubo_data = VPUniformBufferObject {
            view: view_matrix.into(),
            proj: proj.0.into(),
        };
        let ubo_host_buffer = Buffer::from_data(
            render_data.buffer_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            ubo_data,
        )
        .unwrap();

        // Allocate and write model and view matrix to descriptor set
        let layout_view = render_data.pipeline.layout().set_layouts().get(0).unwrap();
        let descriptor_set_view = DescriptorSet::new(
            render_data.descriptor_set_allocator.clone(),
            layout_view.clone(),
            [WriteDescriptorSet::buffer(0, ubo_host_buffer.clone())],
            [],
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into()), Some(1f32.into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer.0.clone())
                },
                SubpassBeginInfo {
                    contents: SubpassContents::Inline,
                    ..SubpassBeginInfo::default()
                },
            )
            .unwrap()
            .bind_pipeline_graphics(render_data.pipeline.clone())
            .expect("Could not bind graphics pipeline")
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                render_data.pipeline.layout().clone(),
                0,
                descriptor_set_view.clone(),
            );

        for (e, t, r, ()) in (&*entities, &transform, &renderable, !&wireframe).join() {
            self.render_entity(e, t, r, &mut builder, &render_data, true);
        }

        // Render wireframe pipeline
        builder
            .bind_pipeline_graphics(render_data.pipeline_wireframe.clone())
            .expect("Could not bind pipeline graphics for wireframe")
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                render_data.pipeline.layout().clone(),
                0,
                descriptor_set_view.clone(),
            );

        // TODO: this is bad figure out a better way
        for (e, t, r) in (&*entities, &transform, &collider).join() {
            // TODO: this is horrible lmao
            self.render_entity(
                e,
                t,
                &Renderable {
                    vertex_buffer: r.vertex_buffer.clone(),
                    index_buffer: r.index_buffer.clone(),
                    descriptor_set_texture: descriptor_set_view.clone(),
                },
                &mut builder,
                &render_data,
                false,
            );
        }

        match builder.end_render_pass(SubpassEndInfo::default()) {
            Ok(v) => v,
            Err(e) => return error!("Failed ending render pass: {:?}", e),
        };

        let buffer = match builder.build() {
            Ok(v) => v,
            Err(e) => return error!("Failed building command buffer: {:?}", e),
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
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        render_data: &RenderData,
        has_texture: bool,
    ) {
        // shorthands for convenience
        let e = entity;
        let t = transform;
        let r = renderable;

        // Insert the model matrix into a push constant
        let push_constants = ModelPushConstants {
            model: t.transformation_matrix().into(),
        };
        // Bind everything required and render this entity
        if has_texture {
            builder.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                render_data.pipeline.layout().clone(),
                1,
                r.descriptor_set_texture.clone(),
            );
        }

        // NOTE: the gpu can do inherently unsafe things outside our control
        unsafe {
            let result = builder
                .push_constants(render_data.pipeline.layout().clone(), 0, push_constants)
                .expect("Pushing constants failed")
                .bind_vertex_buffers(0, r.vertex_buffer.clone())
                .expect("Binding vertex buffers failed")
                .bind_index_buffer(r.index_buffer.clone())
                .expect("Binding index buffers failed")
                .draw_indexed(r.index_buffer.len() as u32, 1, 0, 0, 0);

            if result.is_err() {
                error!("Building a command buffer failed for entity {:?}", e);
            }
        }
    }
}
