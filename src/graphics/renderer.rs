      
// src/graphics/renderer.rs
use crate::graphics::font::{Font};
use crate::graphics::texture::{self, TextureResource}; // Import texture module and TextureResource
use crate::graphics::vulkan_base::{BufferResource, UniformBufferObject, Vertex, VulkanBase};
use crate::state::PushConstantData;
use ash::{vk, Device};
use cgmath::{ortho, Matrix4, Rad, SquareMatrix, Vector3}; // Added SquareMatrix
use log::{info, trace, warn}; // Added info
use ash::util::read_spv;
use memoffset::offset_of;
use std::error::Error;
use std::{ffi::CString, mem};

// --- Add SolidColor variant ---
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DescriptorSetId {
    Font,
    Logo,
    Dancer,
    Gameplay,
    SolidColor, // NEW
}

pub struct Renderer {
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: std::collections::HashMap<DescriptorSetId, vk::DescriptorSet>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    quad_vertex_buffer: BufferResource,
    quad_index_buffer: BufferResource,
    quad_index_count: u32,
    projection_ubo: BufferResource,
    current_window_size: (f32, f32),
    solid_white_texture: TextureResource, // NEW: Store the white texture
}

impl Renderer {
    pub fn new(
        base: &VulkanBase,
        initial_window_size: (f32, f32),
    ) -> Result<Self, Box<dyn Error>> {
        info!("Initializing Renderer..."); // Use info log level

        // --- Create Common Quad Buffers --- (no change)
        // ... (as before) ...
        let quad_vertices: [Vertex; 4] = [
            Vertex { pos: [-0.5, -0.5], tex_coord: [0.0, 0.0] },
            Vertex { pos: [ 0.5, -0.5], tex_coord: [1.0, 0.0] },
            Vertex { pos: [ 0.5,  0.5], tex_coord: [1.0, 1.0] },
            Vertex { pos: [-0.5,  0.5], tex_coord: [0.0, 1.0] },
        ];
        let vertex_buffer_size = (quad_vertices.len() * mem::size_of::<Vertex>()) as vk::DeviceSize;
        let quad_vertex_buffer = base.create_buffer(
            vertex_buffer_size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        base.update_buffer(&quad_vertex_buffer, &quad_vertices)?;
        info!("Quad Vertex Buffer created and populated.");

        let quad_indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer_size = (quad_indices.len() * mem::size_of::<u32>()) as vk::DeviceSize;
        let quad_index_buffer = base.create_buffer(
            index_buffer_size,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        base.update_buffer(&quad_index_buffer, &quad_indices)?;
        let quad_index_count = quad_indices.len() as u32;
        info!("Quad Index Buffer created and populated.");


        // --- Create Projection UBO Buffer --- (no change)
        // ... (as before) ...
        let ubo_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
        let projection_ubo = base.create_buffer(
            ubo_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
         info!("Projection UBO buffer created.");


        // --- Create 1x1 White Texture ---
        info!("Creating solid white texture...");
        let solid_white_texture = texture::create_solid_color_texture(base, [255, 255, 255, 255])?;
        info!("Solid white texture created.");


        // --- Create Descriptor Set Layout (DSL) --- (no change)
        // ... (as before) ...
        let dsl_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        let dsl_create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&dsl_bindings);
        let descriptor_set_layout = unsafe {
            base.device.create_descriptor_set_layout(&dsl_create_info, None)?
        };
         info!("Descriptor Set Layout created.");


        // --- Create Descriptor Pool ---
        const MAX_SETS: u32 = 5; // UPDATED: Increased count by 1
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: MAX_SETS, // Enough for all sets
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: MAX_SETS, // Enough for all sets
            },
        ];
        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_SETS); // Use the updated count
        let descriptor_pool = unsafe {
            base.device.create_descriptor_pool(&pool_create_info, None)?
        };
        info!("Descriptor Pool created for {} sets.", MAX_SETS);


        // --- Allocate Descriptor Sets ---
        let set_layouts = vec![descriptor_set_layout; MAX_SETS as usize]; // Use updated count
        let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);
        let allocated_sets = unsafe { base.device.allocate_descriptor_sets(&desc_alloc_info)? };
        info!("Allocated {} descriptor sets.", allocated_sets.len());

        // Store sets in a map
        let mut descriptor_sets = std::collections::HashMap::new();
        descriptor_sets.insert(DescriptorSetId::Font, allocated_sets[0]);
        descriptor_sets.insert(DescriptorSetId::Logo, allocated_sets[1]);
        descriptor_sets.insert(DescriptorSetId::Dancer, allocated_sets[2]);
        descriptor_sets.insert(DescriptorSetId::Gameplay, allocated_sets[3]);
        descriptor_sets.insert(DescriptorSetId::SolidColor, allocated_sets[4]); // Add the new set


        // --- Create Pipeline Layout & Graphics Pipeline --- (no change)
        // ... (as before) ...
        let push_constant_ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, // Accessible by both
            offset: 0,
            size: mem::size_of::<PushConstantData>() as u32,
        }];
        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout)) // Use the DSL
            .push_constant_ranges(&push_constant_ranges); // Define push constants
        let pipeline_layout = unsafe {
            base.device.create_pipeline_layout(&pipeline_layout_create_info, None)?
        };
         info!("Pipeline Layout created.");

        let vert_shader_module = {
            let mut vert_shader_file = std::io::Cursor::new(&include_bytes!("../../shaders/vert.spv")[..]);
            let vert_code = read_spv(&mut vert_shader_file)?;
            let vert_module_info = vk::ShaderModuleCreateInfo::default().code(&vert_code);
            unsafe { base.device.create_shader_module(&vert_module_info, None)? }
        };
        let frag_shader_module = {
            let mut frag_shader_file = std::io::Cursor::new(&include_bytes!("../../shaders/frag.spv")[..]);
            let frag_code = read_spv(&mut frag_shader_file)?;
            let frag_module_info = vk::ShaderModuleCreateInfo::default().code(&frag_code);
            unsafe { base.device.create_shader_module(&frag_module_info, None)? }
        };
         info!("Shader modules created.");

        let shader_entry_name = CString::new("main").unwrap();

        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo::default()
                .module(vert_shader_module)
                .name(&shader_entry_name)
                .stage(vk::ShaderStageFlags::VERTEX),
            vk::PipelineShaderStageCreateInfo::default()
                .module(frag_shader_module)
                .name(&shader_entry_name)
                .stage(vk::ShaderStageFlags::FRAGMENT),
        ];
         let binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0, binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1, binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, tex_coord) as u32,
            },
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);

        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(std::slice::from_ref(&color_blend_attachment));

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(false)
            .depth_write_enable(false)
            .stencil_test_enable(false);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stage_create_infos)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .depth_stencil_state(&depth_stencil_state)
            .layout(pipeline_layout)
            .render_pass(base.render_pass)
            .subpass(0)
            .dynamic_state(&dynamic_state_info);

        let graphics_pipeline = unsafe {
            base.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .map_err(|(p, r)| { log::error!("Pipeline creation failed: {:?}", r); Box::new(r) as Box<dyn Error>})?[0]
        };
        info!("Graphics Pipeline created.");

        unsafe {
            base.device.destroy_shader_module(vert_shader_module, None);
            base.device.destroy_shader_module(frag_shader_module, None);
        }
         info!("Shader modules destroyed.");



        // --- Initial Projection Matrix Update & Bind White Texture ---
        let mut renderer = Self {
            pipeline_layout,
            graphics_pipeline,
            descriptor_pool,
            descriptor_sets,
            descriptor_set_layout,
            quad_vertex_buffer,
            quad_index_buffer,
            quad_index_count,
            projection_ubo,
            current_window_size: (0.0, 0.0),
            solid_white_texture, // Store the created texture
        };
        // Update projection first (needed for UBO binding in descriptor set update)
        renderer.update_projection_matrix(base, initial_window_size)?;

        // NOW bind the white texture to its dedicated descriptor set
        renderer.update_texture_descriptor(
            &base.device,
            DescriptorSetId::SolidColor,
            &renderer.solid_white_texture,
        );
         info!("Bound solid white texture to its descriptor set.");


        log::info!("Renderer initialization complete.");
        Ok(renderer)
    }

    // --- window_size, update_projection_matrix, update_texture_descriptor --- (no change)
    // ...
    pub fn window_size(&self) -> (f32, f32) {
        self.current_window_size
    }
    pub fn update_projection_matrix(
        &mut self,
        base: &VulkanBase,
        window_size: (f32, f32),
    ) -> Result<(), Box<dyn Error>> {
        if window_size.0 <= 0.0 || window_size.1 <= 0.0 {
            warn!("Attempted to update projection matrix with zero or negative size: {:?}", window_size);
            return Ok(());
        }
        self.current_window_size = window_size;
        let proj = ortho(0.0, window_size.0, 0.0, window_size.1, -1.0, 1.0);
        let ubo = UniformBufferObject { projection: proj };
        base.update_buffer(&self.projection_ubo, &[ubo])?;
        log::info!("Projection matrix UBO updated for size: {:?}", window_size);
        Ok(())
    }
     pub fn update_texture_descriptor(
         &self,
         device: &Device,
         set_id: DescriptorSetId,
         texture: &TextureResource,
     ) {
         let descriptor_set = self.descriptor_sets.get(&set_id)
             .unwrap_or_else(|| panic!("Invalid DescriptorSetId provided for update: {:?}", set_id));

         let ubo_buffer_info = vk::DescriptorBufferInfo::default()
             .buffer(self.projection_ubo.buffer)
             .offset(0)
             .range(vk::WHOLE_SIZE);
         let write_ubo = vk::WriteDescriptorSet::default()
             .dst_set(*descriptor_set)
             .dst_binding(0)
             .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
             .buffer_info(std::slice::from_ref(&ubo_buffer_info));

         let image_info = vk::DescriptorImageInfo::default()
             .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
             .image_view(texture.view)
             .sampler(texture.sampler);
         let write_sampler = vk::WriteDescriptorSet::default()
             .dst_set(*descriptor_set)
             .dst_binding(1)
             .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
             .image_info(std::slice::from_ref(&image_info));

         unsafe { device.update_descriptor_sets(&[write_ubo, write_sampler], &[]) };
         log::trace!("Updated descriptor set {:?} to use texture with view {:?}", set_id, texture.view);
     }

    // --- begin_frame, draw_quad --- (no change)
    // ...
     pub fn begin_frame(
        &self,
        device: &Device,
        cmd_buf: vk::CommandBuffer,
        surface_extent: vk::Extent2D,
    ) {
        unsafe {
            device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);

            let viewport = vk::Viewport {
                x: 0.0, y: 0.0,
                width: surface_extent.width as f32,
                height: surface_extent.height as f32,
                min_depth: 0.0, max_depth: 1.0,
            };
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: surface_extent,
            };
            device.cmd_set_viewport(cmd_buf, 0, &[viewport]);
            device.cmd_set_scissor(cmd_buf, 0, &[scissor]);

            device.cmd_bind_vertex_buffers(cmd_buf, 0, &[self.quad_vertex_buffer.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd_buf, self.quad_index_buffer.buffer, 0, vk::IndexType::UINT32);
        }
    }
    pub fn draw_quad(
        &self,
        device: &Device,
        cmd_buf: vk::CommandBuffer,
        set_id: DescriptorSetId,
        position: Vector3<f32>,
        size: (f32, f32),
        rotation_rad: Rad<f32>,
        tint: [f32; 4],
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
    ) {
         trace!("Drawing quad: pos={:?}, size={:?}, rot={:?}, tint={:?}, uv_off={:?}, uv_scl={:?}, set={:?}",
               position, size, rotation_rad, tint, uv_offset, uv_scale, set_id);

        let model_matrix = Matrix4::from_translation(position)
            * Matrix4::from_angle_z(rotation_rad)
            * Matrix4::from_nonuniform_scale(size.0, size.1, 1.0);

        let push_data = PushConstantData {
            model: model_matrix,
            color: tint,
            uv_offset,
            uv_scale,
        };

        unsafe {
            let descriptor_set = self.descriptor_sets.get(&set_id)
                .unwrap_or_else(|| panic!("Invalid DescriptorSetId provided for draw_quad: {:?}", set_id));
            device.cmd_bind_descriptor_sets(
                cmd_buf,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[*descriptor_set],
                &[],
            );

            let push_data_bytes = std::slice::from_raw_parts(
                &push_data as *const _ as *const u8,
                mem::size_of::<PushConstantData>(),
            );
            device.cmd_push_constants(
                cmd_buf,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                push_data_bytes,
            );

            device.cmd_draw_indexed(cmd_buf, self.quad_index_count, 1, 0, 0, 0);
        }
    }

    // --- draw_text --- (keep the version with scale argument from previous step)
    // ...
     #[allow(clippy::too_many_arguments)]
     pub fn draw_text(
         &self,
         device: &Device,
         cmd_buf: vk::CommandBuffer,
         font: &Font,
         text: &str,
         mut x: f32,
         mut y: f32,
         color: [f32; 4],
         scale: f32,
     ) {
        let font_set = self.descriptor_sets.get(&DescriptorSetId::Font)
            .expect("Font descriptor set not found");
         unsafe {
             device.cmd_bind_descriptor_sets(
                 cmd_buf,
                 vk::PipelineBindPoint::GRAPHICS,
                 self.pipeline_layout,
                 0,
                 &[*font_set],
                 &[],
             );
         }

         let start_x = x;

         for char_code in text.chars() {
             match char_code {
                 '\n' => {
                     x = start_x;
                     y += font.line_height * scale;
                 }
                 ' ' => {
                     x += font.space_width * scale;
                 }
                 _ => {
                     if let Some(glyph_info) = font.get_glyph(char_code) {
                         let scaled_quad_width = font.metrics.cell_width * scale;
                         let scaled_quad_height = font.metrics.cell_height * scale;

                         let scaled_bearing_x = glyph_info.bearing_x * scale;
                         let scaled_bearing_y = glyph_info.bearing_y * scale;
                         let final_top_left_x = x + scaled_bearing_x;
                         let final_top_left_y = y - scaled_bearing_y;

                         let final_center_x = final_top_left_x + scaled_quad_width / 2.0;
                         let final_center_y = final_top_left_y + scaled_quad_height / 2.0;

                         let model_matrix = Matrix4::from_translation(Vector3::new(final_center_x, final_center_y, 0.0))
                                          * Matrix4::from_nonuniform_scale(scaled_quad_width, scaled_quad_height, 1.0);


                         let uv_offset = [glyph_info.u0, glyph_info.v0];
                         let uv_scale_uv = [glyph_info.u1 - glyph_info.u0, glyph_info.v1 - glyph_info.v0];

                         let push_data = PushConstantData {
                             model: model_matrix,
                             color,
                             uv_offset,
                             uv_scale: uv_scale_uv,
                         };

                         unsafe {
                             let push_data_bytes = std::slice::from_raw_parts(
                                 &push_data as *const _ as *const u8,
                                 mem::size_of::<PushConstantData>(),
                             );
                             device.cmd_push_constants(
                                 cmd_buf,
                                 self.pipeline_layout,
                                 vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                 0,
                                 push_data_bytes,
                             );
                             device.cmd_draw_indexed(cmd_buf, self.quad_index_count, 1, 0, 0, 0);
                         }

                         x += glyph_info.advance * scale;

                     } else {
                         warn!("Glyph for '{}' and fallback '?' not found. Advancing by scaled space width.", char_code);
                         x += font.space_width * scale;
                     }
                 }
             }
         }
     }


    /// Cleans up renderer-specific Vulkan resources.
    pub fn destroy(&mut self, device: &Device) {
        log::info!("Destroying Renderer resources...");
        unsafe {
            log::debug!("Destroying quad vertex buffer...");
            self.quad_vertex_buffer.destroy(device);
            log::debug!("Destroying quad index buffer...");
            self.quad_index_buffer.destroy(device);
            log::debug!("Destroying projection UBO buffer...");
            self.projection_ubo.destroy(device);

            // NEW: Destroy the solid white texture
            log::debug!("Destroying solid white texture...");
            self.solid_white_texture.destroy(device);

            log::debug!("Destroying graphics pipeline...");
            if self.graphics_pipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.graphics_pipeline, None);
                self.graphics_pipeline = vk::Pipeline::null();
            }

            log::debug!("Destroying pipeline layout...");
            if self.pipeline_layout != vk::PipelineLayout::null() {
                device.destroy_pipeline_layout(self.pipeline_layout, None);
                self.pipeline_layout = vk::PipelineLayout::null();
            }

            log::debug!("Destroying descriptor pool...");
            if self.descriptor_pool != vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.descriptor_pool, None);
                self.descriptor_pool = vk::DescriptorPool::null();
            }
            self.descriptor_sets.clear();

            log::debug!("Destroying descriptor set layout...");
            if self.descriptor_set_layout != vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
                self.descriptor_set_layout = vk::DescriptorSetLayout::null();
            }
        }
        log::info!("Renderer resources destroyed.");
    }
} // End impl Renderer

    