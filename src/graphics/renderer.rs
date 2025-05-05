use crate::graphics::font::{Font};
use crate::graphics::texture::TextureResource;
use crate::graphics::vulkan_base::{BufferResource, UniformBufferObject, Vertex, VulkanBase};
use crate::state::PushConstantData; // Use the state definition
use ash::{vk, Device};
use cgmath::{ortho, Matrix4, Rad, Vector3};
use log::{trace, warn};
use ash::util::read_spv; 
use memoffset::offset_of;
use std::error::Error;
use std::{ffi::CString, mem}; // Added Arc potentially if Base is shared

// Vertex definition local or imported? Import from vulkan_base
// PushConstantData definition local or imported? Import from state

// Identifiers for Descriptor Sets (more robust than indices)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DescriptorSetId {
    Font,
    Logo,
    Dancer,
    Gameplay, // Arrows/Targets
}

pub struct Renderer {
    // Vulkan objects needed for rendering commands
    // Option 1: Hold references (requires lifetime management, good if App owns Base)
    // device: &'a Device,
    // Option 2: Hold Arc (good if Renderer might outlive initial App scope or be shared)
    // device: Arc<Device>, // Needs changes in VulkanBase to use Arc<Device>
    // Option 3: Hold necessary handles directly (simpler for now if owned by App)

    // Let's assume App owns VulkanBase and passes handles/refs to Renderer::new
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool, // Owns the pool
    // Store the allocated sets
    descriptor_sets: std::collections::HashMap<DescriptorSetId, vk::DescriptorSet>,
    descriptor_set_layout: vk::DescriptorSetLayout, // Store the layout used by the sets/pipeline

    // Common Resources (owned or references?) - Let's make Renderer own copies/handles
    quad_vertex_buffer: BufferResource,
    quad_index_buffer: BufferResource,
    quad_index_count: u32,
    projection_ubo: BufferResource, // UBO buffer itself

    // Keep track of current window size for projection updates
    current_window_size: (f32, f32),
}

impl Renderer {
    pub fn new(
        base: &VulkanBase, // Pass VulkanBase to get device, properties etc.
        initial_window_size: (f32, f32),
    ) -> Result<Self, Box<dyn Error>> {
        log::info!("Initializing Renderer...");

        // --- Create Common Quad Buffers ---
        let quad_vertices: [Vertex; 4] = [
            Vertex { pos: [-0.5, -0.5], tex_coord: [0.0, 0.0] }, // Top-left UV
            Vertex { pos: [ 0.5, -0.5], tex_coord: [1.0, 0.0] }, // Top-right UV
            Vertex { pos: [ 0.5,  0.5], tex_coord: [1.0, 1.0] }, // Bottom-right UV
            Vertex { pos: [-0.5,  0.5], tex_coord: [0.0, 1.0] }, // Bottom-left UV
        ];
        let vertex_buffer_size = (quad_vertices.len() * mem::size_of::<Vertex>()) as vk::DeviceSize;
        let quad_vertex_buffer = base.create_buffer(
            vertex_buffer_size,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        base.update_buffer(&quad_vertex_buffer, &quad_vertices)?;
        log::info!("Quad Vertex Buffer created and populated.");

        let quad_indices: [u32; 6] = [0, 1, 2, 2, 3, 0]; // Defines two triangles for the quad
        let index_buffer_size = (quad_indices.len() * mem::size_of::<u32>()) as vk::DeviceSize;
        let quad_index_buffer = base.create_buffer(
            index_buffer_size,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        base.update_buffer(&quad_index_buffer, &quad_indices)?;
        let quad_index_count = quad_indices.len() as u32;
        log::info!("Quad Index Buffer created and populated.");

        // --- Create Projection UBO Buffer ---
        let ubo_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
        let projection_ubo = base.create_buffer(
            ubo_size,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
         log::info!("Projection UBO buffer created.");


        // --- Create Descriptor Set Layout (DSL) ---
        // Defines the structure of the descriptor sets used by the pipeline
        let dsl_bindings = [
            // Binding 0: Uniform Buffer Object (Projection Matrix)
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX), // Used in vertex shader
            // Binding 1: Combined Image Sampler (Texture)
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT), // Used in fragment shader
        ];
        let dsl_create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&dsl_bindings);
        let descriptor_set_layout = unsafe {
            base.device.create_descriptor_set_layout(&dsl_create_info, None)?
        };
         log::info!("Descriptor Set Layout created.");

        // --- Create Descriptor Pool ---
        // Allocates memory for descriptor sets
        const MAX_SETS: u32 = 4; // One for each usage type (Font, Logo, Dancer, Gameplay)
        let pool_sizes = [
            // Enough UBO descriptors for all sets
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: MAX_SETS,
            },
            // Enough Sampler descriptors for all sets
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: MAX_SETS,
            },
        ];
        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_SETS)
            // Allow individual sets to be freed if needed (though we don't here)
            // .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);
            ;
        let descriptor_pool = unsafe {
            base.device.create_descriptor_pool(&pool_create_info, None)?
        };
         log::info!("Descriptor Pool created.");


        // --- Allocate Descriptor Sets ---
        let set_layouts = [descriptor_set_layout; MAX_SETS as usize]; // Use the same layout for all sets
        let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);
        let allocated_sets = unsafe { base.device.allocate_descriptor_sets(&desc_alloc_info)? };
         log::info!("Allocated {} descriptor sets.", allocated_sets.len());

        // Store sets in a map for easy access by ID
        let mut descriptor_sets = std::collections::HashMap::new();
        descriptor_sets.insert(DescriptorSetId::Font, allocated_sets[0]);
        descriptor_sets.insert(DescriptorSetId::Logo, allocated_sets[1]);
        descriptor_sets.insert(DescriptorSetId::Dancer, allocated_sets[2]);
        descriptor_sets.insert(DescriptorSetId::Gameplay, allocated_sets[3]);


        // --- Create Pipeline Layout ---
        // Connects descriptor sets and push constants to the pipeline
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
         log::info!("Pipeline Layout created.");


        // --- Create Graphics Pipeline ---
        // Load shader modules (consider moving shader loading elsewhere, e.g., AssetManager)
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
         log::info!("Shader modules created.");

        let shader_entry_name = CString::new("main").unwrap(); // Entry point function name

        // Define shader stages
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

        // Define vertex input state (matches `Vertex` struct)
        let binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0, // Input binding 0
            stride: mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX, // Per-vertex data
        }];
        let attribute_descriptions = [
            // Position (location = 0)
            vk::VertexInputAttributeDescription {
                location: 0, binding: 0,
                format: vk::Format::R32G32_SFLOAT, // vec2
                offset: offset_of!(Vertex, pos) as u32,
            },
            // Texture Coordinate (location = 1)
            vk::VertexInputAttributeDescription {
                location: 1, binding: 0,
                format: vk::Format::R32G32_SFLOAT, // vec2
                offset: offset_of!(Vertex, tex_coord) as u32,
            },
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);

        // Define input assembly (how vertices form primitives)
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST) // Draw triangles
            .primitive_restart_enable(false);

        // Define viewport and scissor (dynamic states, actual values set later)
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        // Define rasterization state
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL) // Fill triangles
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE) // No backface culling for 2D sprites
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE); // Standard front face

        // Define multisampling state (disabled)
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // Define color blending state (enable standard alpha blending)
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA) // Blend based on source alpha
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE) // Don't blend alpha channel itself
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false) // Use blend factors, not logic op
            .attachments(std::slice::from_ref(&color_blend_attachment));

        // Define depth/stencil state (depth test disabled for simple 2D)
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(false) // No depth testing
            .depth_write_enable(false) // No writing to depth buffer
            .stencil_test_enable(false); // No stencil testing

        // Define dynamic states (viewport and scissor will be set per frame)
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        // Create the pipeline
        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stage_create_infos)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .depth_stencil_state(&depth_stencil_state)
            .layout(pipeline_layout) // Use the created layout
            .render_pass(base.render_pass) // Compatible render pass
            .subpass(0) // Index of the subpass to use
            .dynamic_state(&dynamic_state_info); // Enable dynamic states

        let graphics_pipeline = unsafe {
            base.device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(), // No pipeline cache for simplicity
                    &[pipeline_info], // Array of one pipeline create info
                    None,
                )
                .map_err(|(pipelines, result)| {
                    // Proper error handling for pipeline creation failure
                    log::error!("Pipeline creation failed: {:?}", result);
                    // It returns a tuple (Vec<Pipelines>, Result) on error
                    for p in pipelines { // Clean up any partially created pipelines
                        if p != vk::Pipeline::null() {
                            base.device.destroy_pipeline(p, None);
                        }
                    }
                    Box::new(result) as Box<dyn Error> // Convert vk::Result to Box<dyn Error>
                })?[0] // Get the first pipeline from the resulting vec
        };
        log::info!("Graphics Pipeline created.");

        // --- Cleanup Shader Modules ---
        // Modules are not needed after pipeline creation
        unsafe {
            base.device.destroy_shader_module(vert_shader_module, None);
            base.device.destroy_shader_module(frag_shader_module, None);
        }
         log::info!("Shader modules destroyed.");


        // --- Initial Projection Matrix Update ---
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
            current_window_size: (0.0, 0.0), // Will be set by update_projection
        };
        renderer.update_projection_matrix(base, initial_window_size)?;


        log::info!("Renderer initialization complete.");
        Ok(renderer)
    }

    pub fn window_size(&self) -> (f32, f32) {
        self.current_window_size
    }

    /// Updates the projection UBO based on window size. Call on init and resize.
    pub fn update_projection_matrix(
        &mut self,
        base: &VulkanBase, // Need base to call update_buffer
        window_size: (f32, f32),
    ) -> Result<(), Box<dyn Error>> {
        if window_size.0 <= 0.0 || window_size.1 <= 0.0 {
            warn!("Attempted to update projection matrix with zero or negative size: {:?}", window_size);
            return Ok(()); // Avoid division by zero or invalid matrix
        }
        self.current_window_size = window_size;
        // Create orthographic projection matrix:
        // Maps x from 0..width to -1..1
        // Maps y from 0..height to -1..1 (Vulkan NDC Y is downwards)
        // Maps z from -1..1 to 0..1
        let proj = ortho(0.0, window_size.0, 0.0, window_size.1, -1.0, 1.0); // Correct Y for Vulkan NDC
        let ubo = UniformBufferObject { projection: proj };
        base.update_buffer(&self.projection_ubo, &[ubo])?;
        log::info!("Projection matrix UBO updated for size: {:?}", window_size);
        Ok(())
    }

     /// Updates the specified descriptor set to point to the given texture.
     pub fn update_texture_descriptor(
         &self,
         device: &Device, // Need device to update set
         set_id: DescriptorSetId,
         texture: &TextureResource,
     ) {
         let descriptor_set = self.descriptor_sets.get(&set_id)
             .expect("Invalid DescriptorSetId provided for update"); // Should not happen if IDs are managed correctly

         // 1. Update UBO Binding (Binding 0) - Point to the common projection UBO
         let ubo_buffer_info = vk::DescriptorBufferInfo::default()
             .buffer(self.projection_ubo.buffer)
             .offset(0)
             .range(vk::WHOLE_SIZE); // Use the whole buffer range
         let write_ubo = vk::WriteDescriptorSet::default()
             .dst_set(*descriptor_set)
             .dst_binding(0)
             .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
             .buffer_info(std::slice::from_ref(&ubo_buffer_info));

         // 2. Update Sampler Binding (Binding 1) - Point to the specific texture
         let image_info = vk::DescriptorImageInfo::default()
             .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) // Must match layout used in shader
             .image_view(texture.view)
             .sampler(texture.sampler);
         let write_sampler = vk::WriteDescriptorSet::default()
             .dst_set(*descriptor_set)
             .dst_binding(1)
             .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
             .image_info(std::slice::from_ref(&image_info));

         // Perform the update
         unsafe { device.update_descriptor_sets(&[write_ubo, write_sampler], &[]) };
         log::trace!("Updated descriptor set {:?} to use texture with view {:?}", set_id, texture.view);
     }


    /// Called at the start of drawing a frame (inside VulkanBase::draw_frame closure).
    /// Sets up common render state for the frame.
    pub fn begin_frame(
        &self,
        device: &Device,
        cmd_buf: vk::CommandBuffer,
        surface_extent: vk::Extent2D,
    ) {
        unsafe {
            // Bind the common graphics pipeline
            device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);

            // Set dynamic viewport and scissor state
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

            // Bind common vertex and index buffers
            device.cmd_bind_vertex_buffers(cmd_buf, 0, &[self.quad_vertex_buffer.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd_buf, self.quad_index_buffer.buffer, 0, vk::IndexType::UINT32);
        }
    }

    /// Draws a textured quad.
    /// Assumes begin_frame has been called.
    pub fn draw_quad(
        &self,
        device: &Device,
        cmd_buf: vk::CommandBuffer,
        set_id: DescriptorSetId, // Which texture set to use
        position: Vector3<f32>,
        size: (f32, f32),
        rotation_rad: Rad<f32>,
        tint: [f32; 4],
        uv_offset: [f32; 2],
        uv_scale: [f32; 2],
    ) {
         trace!("Drawing quad: pos={:?}, size={:?}, rot={:?}, tint={:?}, uv_off={:?}, uv_scl={:?}, set={:?}",
               position, size, rotation_rad, tint, uv_offset, uv_scale, set_id);

        // Calculate model matrix
        let model_matrix = Matrix4::from_translation(position)
            * Matrix4::from_angle_z(rotation_rad)
            * Matrix4::from_nonuniform_scale(size.0, size.1, 1.0);

        // Prepare push constants
        let push_data = PushConstantData {
            model: model_matrix,
            color: tint,
            uv_offset,
            uv_scale,
        };

        unsafe {
            // Bind the descriptor set for the specified texture
            let descriptor_set = self.descriptor_sets.get(&set_id)
                .expect("Invalid DescriptorSetId provided for draw_quad");
            device.cmd_bind_descriptor_sets(
                cmd_buf,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0, // First set
                &[*descriptor_set],
                &[], // No dynamic offsets
            );

            // Update push constants
            let push_data_bytes = std::slice::from_raw_parts(
                &push_data as *const _ as *const u8,
                mem::size_of::<PushConstantData>(),
            );
            device.cmd_push_constants(
                cmd_buf,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0, // Offset
                push_data_bytes,
            );

            // Issue the draw call
            device.cmd_draw_indexed(cmd_buf, self.quad_index_count, 1, 0, 0, 0);
        }
    }

     /// Draws text using the specified font.
     /// Assumes begin_frame has been called.
     pub fn draw_text(
         &self,
         device: &Device,
         cmd_buf: vk::CommandBuffer,
         font: &Font,        // Reference to the loaded font data
         text: &str,
         mut x: f32,       // Starting X position (cursor)
         mut y: f32,           // Baseline Y position for the first line
         color: [f32; 4],
         // Maybe add scale/size later? For now, use font's native size.
     ) {
        // Ensure the Font descriptor set is bound before calling this function repeatedly for performance,
        // or bind it here if it's a one-off call. Let's bind it here for simplicity, assuming
        // text rendering might be interspersed with other drawing.
        let font_set = self.descriptor_sets.get(&DescriptorSetId::Font)
            .expect("Font descriptor set not found");
         unsafe {
             device.cmd_bind_descriptor_sets(
                 cmd_buf,
                 vk::PipelineBindPoint::GRAPHICS,
                 self.pipeline_layout,
                 0, // First set
                 &[*font_set],
                 &[], // No dynamic offsets
             );
         }

         let start_x = x; // Remember starting X for newlines

         for char_code in text.chars() {
             match char_code {
                 '\n' => {
                     x = start_x;
                     y += font.line_height; // Move down one line
                 }
                 ' ' => {
                     x += font.space_width; // Advance by space width
                 }
                 _ => {
                     // Get glyph info (handles fallback to '?')
                     if let Some(glyph_info) = font.get_glyph(char_code) {
                         // Calculate quad position based on cursor and glyph bearings
                         // Quad top-left corner X = cursor_x + bearing_x
                         // Quad top-left corner Y = baseline_y - bearing_y (since Y is down)
                         let quad_pos_x = x + glyph_info.bearing_x;
                         let quad_pos_y = y - glyph_info.bearing_y;

                         // Calculate size for the rendering quad (usually font cell size)
                         // This ensures consistent spacing even if glyph bitmap is smaller
                         let quad_width = font.metrics.cell_width;
                         let quad_height = font.metrics.cell_height;

                         // Center the translation point on the quad for scaling/rotation
                         let center_x = quad_pos_x + quad_width / 2.0;
                         let center_y = quad_pos_y + quad_height / 2.0;

                         // Calculate UV scale and offset from GlyphInfo
                         // uv_offset = [u0, v0]
                         // uv_scale = [u1-u0, v1-v0]
                         let uv_offset = [glyph_info.u0, glyph_info.v0];
                         let uv_scale = [glyph_info.u1 - glyph_info.u0, glyph_info.v1 - glyph_info.v0];

                         // Calculate model matrix for this specific glyph quad
                         let model_matrix = Matrix4::from_translation(Vector3::new(center_x, center_y, 0.0))
                                          * Matrix4::from_nonuniform_scale(quad_width, quad_height, 1.0);

                         // Prepare push constants for this glyph
                         let push_data = PushConstantData {
                             model: model_matrix,
                             color,
                             uv_offset,
                             uv_scale,
                         };

                         // Update push constants and draw
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

                         // Advance cursor position
                         x += glyph_info.advance;

                     } else {
                         // Glyph (and fallback '?') not found, advance by space width as last resort
                         warn!("Glyph for '{}' and fallback '?' not found. Advancing by space width.", char_code);
                         x += font.space_width;
                     }
                 }
             }
         }
     }


    /// Cleans up renderer-specific Vulkan resources.
    pub fn destroy(&mut self, device: &Device) {
         log::info!("Destroying Renderer resources...");
         unsafe {
             // Buffers need to be destroyed *before* the memory they might use
             log::debug!("Destroying quad vertex buffer...");
             self.quad_vertex_buffer.destroy(device);
             log::debug!("Destroying quad index buffer...");
             self.quad_index_buffer.destroy(device);
             log::debug!("Destroying projection UBO buffer...");
             self.projection_ubo.destroy(device); // UBO uses its own memory

             // Destroy pipeline first, as it depends on layout
              log::debug!("Destroying graphics pipeline...");
             if self.graphics_pipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.graphics_pipeline, None);
                self.graphics_pipeline = vk::Pipeline::null();
             }

             // Destroy pipeline layout
              log::debug!("Destroying pipeline layout...");
              if self.pipeline_layout != vk::PipelineLayout::null() {
                 device.destroy_pipeline_layout(self.pipeline_layout, None);
                 self.pipeline_layout = vk::PipelineLayout::null();
              }


             // Descriptor sets are implicitly destroyed when the pool is destroyed
              log::debug!("Destroying descriptor pool...");
             if self.descriptor_pool != vk::DescriptorPool::null() {
                 device.destroy_descriptor_pool(self.descriptor_pool, None);
                 self.descriptor_pool = vk::DescriptorPool::null();
             }
             self.descriptor_sets.clear(); // Clear the map


             // Destroy descriptor set layout
              log::debug!("Destroying descriptor set layout...");
              if self.descriptor_set_layout != vk::DescriptorSetLayout::null() {
                 device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
                 self.descriptor_set_layout = vk::DescriptorSetLayout::null();
             }
         }
         log::info!("Renderer resources destroyed.");
    }
}