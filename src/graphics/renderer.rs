use crate::graphics::font::{Font}; // Removed GlyphInfo if truly unused elsewhere
use crate::graphics::texture::{self, TextureResource};
use crate::graphics::vulkan_base::{BufferResource, UniformBufferObject, Vertex, VulkanBase};
use crate::state::PushConstantData;
use ash::util::read_spv;
use ash::{vk, Device};
use cgmath::{ortho, Matrix4, Rad, Vector3};
use log::{debug, info, trace, warn};
use memoffset::offset_of;
use std::collections::HashMap; // Import HashMap explicitly
use std::error::Error;
use std::{ffi::CString, mem};


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DescriptorSetId {
    FontWendy,
    FontMiso,
    FontCjk,
    Logo,
    Dancer,
    Gameplay,
    SolidColor,
    FallbackBanner,
    DynamicBanner,
}

pub struct Renderer {
    main_pipeline_layout: vk::PipelineLayout,
    main_pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: HashMap<DescriptorSetId, vk::DescriptorSet>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    quad_vertex_buffer: BufferResource,
    quad_index_buffer: BufferResource,
    quad_index_count: u32,
    projection_ubo: BufferResource,
    current_window_size: (f32, f32),
    solid_white_texture: TextureResource,
}

impl Renderer {
    pub fn new(base: &VulkanBase, initial_window_size: (f32, f32)) -> Result<Self, Box<dyn Error>> {
        info!("Initializing Renderer...");

        let quad_vertices: [Vertex; 4] = [
            Vertex {
                pos: [-0.5, -0.5],
                tex_coord: [0.0, 0.0],
            },
            Vertex {
                pos: [0.5, -0.5],
                tex_coord: [1.0, 0.0],
            },
            Vertex {
                pos: [0.5, 0.5],
                tex_coord: [1.0, 1.0],
            },
            Vertex {
                pos: [-0.5, 0.5],
                tex_coord: [0.0, 1.0],
            },
        ];
        let vertex_buffer_size = (quad_vertices.len() * mem::size_of::<Vertex>()) as vk::DeviceSize;
        let quad_vertex_buffer = base.create_buffer( vertex_buffer_size, vk::BufferUsageFlags::VERTEX_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )?;
        base.update_buffer(&quad_vertex_buffer, &quad_vertices)?;
        let quad_indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        let index_buffer_size = (quad_indices.len() * mem::size_of::<u32>()) as vk::DeviceSize;
        let quad_index_buffer = base.create_buffer( index_buffer_size, vk::BufferUsageFlags::INDEX_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )?;
        base.update_buffer(&quad_index_buffer, &quad_indices)?;
        let quad_index_count = quad_indices.len() as u32;
        let ubo_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
        let projection_ubo = base.create_buffer( ubo_size, vk::BufferUsageFlags::UNIFORM_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )?;
        let solid_white_texture = texture::create_solid_color_texture(base, [255, 255, 255, 255])?;

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
            base.device
                .create_descriptor_set_layout(&dsl_create_info, None)?
        };

        const MAX_SETS: u32 = 9; // Increased from 8
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: MAX_SETS,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: MAX_SETS,
            },
        ];
        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_SETS); // Use the constant
        let descriptor_pool = unsafe {
            base.device
                .create_descriptor_pool(&pool_create_info, None)?
        };

        let set_layouts_vec = vec![descriptor_set_layout; MAX_SETS as usize]; // Use the constant
        let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts_vec);
        let allocated_sets = unsafe { base.device.allocate_descriptor_sets(&desc_alloc_info)? };

        // --- Store Descriptor Sets in HashMap ---
        let mut descriptor_sets = HashMap::new(); // Use HashMap
        descriptor_sets.insert(DescriptorSetId::FontWendy, allocated_sets[0]);
        descriptor_sets.insert(DescriptorSetId::Logo, allocated_sets[1]);
        descriptor_sets.insert(DescriptorSetId::Dancer, allocated_sets[2]);
        descriptor_sets.insert(DescriptorSetId::Gameplay, allocated_sets[3]);
        descriptor_sets.insert(DescriptorSetId::SolidColor, allocated_sets[4]);
        descriptor_sets.insert(DescriptorSetId::FontMiso, allocated_sets[5]);
        descriptor_sets.insert(DescriptorSetId::FontCjk, allocated_sets[6]); // Keep even if unused for now
        descriptor_sets.insert(DescriptorSetId::FallbackBanner, allocated_sets[7]);
        descriptor_sets.insert(DescriptorSetId::DynamicBanner, allocated_sets[8]); // Assign the new set

        let push_constant_ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: mem::size_of::<PushConstantData>() as u32,
        }];
        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout))
            .push_constant_ranges(&push_constant_ranges);
        let main_pipeline_layout = unsafe {
            base.device
                .create_pipeline_layout(&pipeline_layout_create_info, None)?
        };
        info!("Main Pipeline Layout created.");

        let vert_shader_module = {
            let mut file = std::io::Cursor::new(&include_bytes!("../../shaders/msdf_vert.spv")[..]); // USE MSDF VERT
            let code = read_spv(&mut file)?;
            let info = vk::ShaderModuleCreateInfo::default().code(&code);
            unsafe { base.device.create_shader_module(&info, None)? }
        };
        let frag_shader_module = {
            let mut file = std::io::Cursor::new(&include_bytes!("../../shaders/msdf_frag.spv")[..]); // USE MSDF FRAG
            let code = read_spv(&mut file)?;
            let info = vk::ShaderModuleCreateInfo::default().code(&code);
            unsafe { base.device.create_shader_module(&info, None)? }
        };
        info!("MSDF shader modules loaded for main pipeline.");

        let shader_entry_name = CString::new("main").unwrap();
        let binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, tex_coord) as u32,
            },
        ];
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
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
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA) // Common for UI
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA) // Common for UI
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA) // CHANGED for straight alpha
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA) // CHANGED for straight alpha
            .alpha_blend_op(vk::BlendOp::ADD);
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));
        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default();
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let shader_stages_for_main_pipeline = [
            vk::PipelineShaderStageCreateInfo::default()
                .module(vert_shader_module)
                .name(&shader_entry_name)
                .stage(vk::ShaderStageFlags::VERTEX),
            vk::PipelineShaderStageCreateInfo::default()
                .module(frag_shader_module)
                .name(&shader_entry_name)
                .stage(vk::ShaderStageFlags::FRAGMENT),
        ];

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages_for_main_pipeline)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .depth_stencil_state(&depth_stencil_state)
            .layout(main_pipeline_layout)
            .render_pass(base.render_pass)
            .subpass(0)
            .dynamic_state(&dynamic_state_info);

        let main_pipeline = unsafe {
            base.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .map_err(|(_p, r)| {
                    log::error!("Main pipeline creation failed: {:?}", r);
                    Box::new(r) as Box<dyn Error>
                })?[0]
        };
        info!("Main Graphics Pipeline (using MSDF shaders) created.");

        unsafe {
            base.device.destroy_shader_module(vert_shader_module, None);
            base.device.destroy_shader_module(frag_shader_module, None);
        }
        info!("Shader modules destroyed.");

        let mut renderer = Self {
            main_pipeline_layout,
            main_pipeline,
            descriptor_pool,
            descriptor_sets, // Store the HashMap
            descriptor_set_layout,
            quad_vertex_buffer,
            quad_index_buffer,
            quad_index_count,
            projection_ubo,
            current_window_size: (0.0, 0.0),
            solid_white_texture,
        };
        renderer.update_projection_matrix(base, initial_window_size)?;
        renderer.update_texture_descriptor(
            &base.device,
            DescriptorSetId::SolidColor,
            &renderer.solid_white_texture,
        );
        // --- Initialize DynamicBanner with fallback initially ---
        // Need access to the fallback banner texture resource later in AssetManager
        // For now, we assume it will be updated by AssetManager shortly after creation.

        info!("Renderer initialization complete.");
        Ok(renderer)
    }

    pub fn window_size(&self) -> (f32, f32) {
        self.current_window_size
    }

    pub fn update_projection_matrix(
        &mut self,
        base: &VulkanBase,
        window_size: (f32, f32),
    ) -> Result<(), Box<dyn Error>> {
        if window_size.0 <= 0.0 || window_size.1 <= 0.0 {
            warn!(
                "Attempted to update projection matrix with zero or negative size: {:?}",
                window_size
            );
            return Ok(());
        }
        self.current_window_size = window_size;
        // Y-DOWN: top=0, bottom=height
        let proj = ortho(0.0, window_size.0, 0.0, window_size.1, -1.0, 1.0);
        let ubo = UniformBufferObject { projection: proj };
        base.update_buffer(&self.projection_ubo, &[ubo])?;
        debug!(
            "Projection matrix UBO updated for size: {:?}, Y-DOWN (0,0 top-left)",
            window_size
        );
        Ok(())
    }

    pub fn update_texture_descriptor(
        &self,
        device: &Device,
        set_id: DescriptorSetId,
        texture: &TextureResource,
    ) {
        let descriptor_set = self
            .descriptor_sets
            .get(&set_id)
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
        trace!(
            "Updated descriptor set {:?} for texture view {:?}",
            set_id,
            texture.view
        );
    }

    pub fn begin_frame(
        &self,
        device: &Device,
        cmd_buf: vk::CommandBuffer,
        surface_extent: vk::Extent2D,
    ) {
        unsafe {
            device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.main_pipeline);

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: surface_extent.width as f32,
                height: surface_extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: surface_extent,
            };
            device.cmd_set_viewport(cmd_buf, 0, &[viewport]);
            device.cmd_set_scissor(cmd_buf, 0, &[scissor]);

            device.cmd_bind_vertex_buffers(cmd_buf, 0, &[self.quad_vertex_buffer.buffer], &[0]);
            device.cmd_bind_index_buffer(
                cmd_buf,
                self.quad_index_buffer.buffer,
                0,
                vk::IndexType::UINT32,
            );
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
        trace!(
            "Drawing quad: pos={:?}, size={:?}, set={:?}",
            position,
            size,
            set_id
        );
        let model_matrix = Matrix4::from_translation(position)
            * Matrix4::from_angle_z(rotation_rad)
            * Matrix4::from_nonuniform_scale(size.0, size.1, 1.0);

        let push_data = PushConstantData {
            model: model_matrix,
            color: tint,
            uv_offset,
            uv_scale,
            px_range: 0.0, // Set to 0.0 for non-MSDF quads (shader will use texture alpha)
        };

        unsafe {
            let descriptor_set = self
                .descriptor_sets
                .get(&set_id)
                .unwrap_or_else(|| panic!("Invalid DescriptorSetId: {:?}", set_id));
            device.cmd_bind_descriptor_sets(
                cmd_buf,
                vk::PipelineBindPoint::GRAPHICS,
                self.main_pipeline_layout,
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
                self.main_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                push_data_bytes,
            );
            device.cmd_draw_indexed(cmd_buf, self.quad_index_count, 1, 0, 0, 0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &self,
        device: &Device,
        cmd_buf: vk::CommandBuffer,
        font: &Font,
        text: &str,
        mut pen_x: f32,
        pen_y: f32, // This is the baseline Y
        color: [f32; 4],
        scale: f32,
        // NEW: Optional letter spacing adjustment factor
        // 1.0 = normal spacing from font
        // < 1.0 = tighter spacing (e.g., 0.95 for 5% tighter)
        // > 1.0 = looser spacing
        letter_spacing_factor: Option<f32>,
    ) {
        let start_pen_x = pen_x;
        // Use provided factor or default to 1.0 (normal spacing)
        let actual_letter_spacing_factor = letter_spacing_factor.unwrap_or(1.0);

        for char_code in text.chars() {
            if char_code == '\n' {
                pen_x = start_pen_x;
                debug!("Newline in draw_text. Resetting pen_x. Caller handles pen_y advance.");
                continue;
            }

            let advance_amount;
            if char_code == ' ' {
                // Apply spacing factor to space width as well, if desired
                advance_amount = font.space_width * scale * actual_letter_spacing_factor;
                pen_x += advance_amount;
                continue;
            }

            if let Some(glyph_info) = font.get_glyph(char_code) {
                let quad_width = (glyph_info.plane_right - glyph_info.plane_left) * scale;
                let quad_height = (glyph_info.plane_top - glyph_info.plane_bottom) * scale;

                if char_code == 'A' {
                    debug!("DRAW_TEXT 'A': scale: {:.2}", scale);
                    debug!("  GlyphInfo: plane_left={:.2}, plane_bottom={:.2}, plane_right={:.2}, plane_top={:.2}, advance={:.2}",
                        glyph_info.plane_left, glyph_info.plane_bottom, glyph_info.plane_right, glyph_info.plane_top, glyph_info.advance);
                    debug!(
                        "  GlyphInfo UVs: u0={:.3}, v0={:.3}, u1={:.3}, v1={:.3}",
                        glyph_info.u0, glyph_info.v0, glyph_info.u1, glyph_info.v1
                    );
                    debug!(
                        "  Calculated quad_width={:.2}, quad_height={:.2}",
                        quad_width, quad_height
                    );
                }

                if quad_width <= 0.0 || quad_height <= 0.0 {
                    // Still advance even if not drawing (e.g., for zero-width glyphs with advance)
                    advance_amount = glyph_info.advance * scale * actual_letter_spacing_factor;
                    pen_x += advance_amount;
                    continue;
                }

                let quad_visual_left_x = pen_x + (glyph_info.plane_left * scale);
                let quad_actual_top_y = pen_y - (glyph_info.plane_top * scale);
                let quad_center_x = quad_visual_left_x + quad_width / 2.0;
                let quad_center_y = quad_actual_top_y + quad_height / 2.0;

                if char_code == 'A' {
                    debug!("  Pen_x={:.2}, Pen_y (baseline)={:.2}", pen_x, pen_y);
                    debug!(
                        "  Quad visual_left_x={:.2}, actual_top_y={:.2}",
                        quad_visual_left_x, quad_actual_top_y
                    );
                    debug!(
                        "  Quad center_x={:.2}, center_y={:.2}",
                        quad_center_x, quad_center_y
                    );
                    debug!(
                        "  UV offset=[{:.3}, {:.3}], UV scale=[{:.3}, {:.3}]",
                        glyph_info.u0,
                        glyph_info.v0,
                        glyph_info.u1 - glyph_info.u0,
                        glyph_info.v1 - glyph_info.v0
                    );
                }

                let model_matrix =
                    Matrix4::from_translation(Vector3::new(quad_center_x, quad_center_y, 0.0))
                        * Matrix4::from_nonuniform_scale(quad_width, quad_height, 1.0);

                if char_code == 'A' {
                    debug!("  Model Matrix for 'A': {:?}", model_matrix);
                }

                let uv_offset_vals = [glyph_info.u0, glyph_info.v0];
                let uv_scale_vals = [glyph_info.u1 - glyph_info.u0, glyph_info.v1 - glyph_info.v0];

                if char_code == 'A' || text.starts_with("Scaled Text") {
                    debug!(
                        "DRAW_TEXT (char: {}): px_range for push_data: {:.2}",
                        char_code, font.metrics.msdf_pixel_range
                    );
                }

                let push_data = PushConstantData {
                    model: model_matrix,
                    color,
                    uv_offset: uv_offset_vals,
                    uv_scale: uv_scale_vals,
                    px_range: font.metrics.msdf_pixel_range,
                };

                unsafe {
                    let descriptor_set = self
                        .descriptor_sets
                        .get(&font.descriptor_set_id)
                        .unwrap_or_else(|| {
                            panic!("Font descriptor set {:?} not found", font.descriptor_set_id)
                        });
                    device.cmd_bind_descriptor_sets(
                        cmd_buf,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.main_pipeline_layout,
                        0,
                        &[*descriptor_set],
                        &[],
                    );
                    let push_data_bytes = std::slice::from_raw_parts(
                        &push_data as *const _ as *const u8,
                        std::mem::size_of::<PushConstantData>(),
                    );
                    device.cmd_push_constants(
                        cmd_buf,
                        self.main_pipeline_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        push_data_bytes,
                    );
                    device.cmd_draw_indexed(cmd_buf, self.quad_index_count, 1, 0, 0, 0);
                }

                advance_amount = glyph_info.advance * scale * actual_letter_spacing_factor; // Apply factor
                pen_x += advance_amount;

            } else {
                warn!(
                    "Glyph for '{}' not found in MSDF font. Advancing by space width.",
                    char_code
                );
                // Apply spacing factor to space width if fallback used
                advance_amount = font.space_width * scale * actual_letter_spacing_factor;
                pen_x += advance_amount;
            }
        }
    }

    pub fn destroy(&mut self, device: &Device) {
        log::info!("Destroying Renderer resources...");
        unsafe {
            self.quad_vertex_buffer.destroy(device);
            self.quad_index_buffer.destroy(device);
            self.projection_ubo.destroy(device);
            self.solid_white_texture.destroy(device);

            if self.main_pipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.main_pipeline, None);
            }
            if self.main_pipeline_layout != vk::PipelineLayout::null() {
                device.destroy_pipeline_layout(self.main_pipeline_layout, None);
            }

            if self.descriptor_pool != vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.descriptor_pool, None);
            }
            self.descriptor_sets.clear();
            if self.descriptor_set_layout != vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            }
        }
        log::info!("Renderer resources destroyed.");
    }
}
