use crate::vulkan_base::{find_memorytype_index, record_submit_commandbuffer, VulkanBase};
use ash::{vk, Device};
use image::RgbaImage;
use std::error::Error;
use std::path::Path;

pub struct TextureResource {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub width: u32,
    pub height: u32,
}

impl TextureResource {
    pub fn destroy(&mut self, device: &Device) {
        unsafe {
            device.destroy_sampler(self.sampler, None);
            device.destroy_image_view(self.view, None);
            device.destroy_image(self.image, None);
            device.free_memory(self.memory, None);
        }
    }
}

// --- load_texture function remains the same ---
pub fn load_texture(base: &VulkanBase, path: &Path) -> Result<TextureResource, Box<dyn Error>> {
    // --- 1. Load Image with `image` crate ---
    log::info!("Starting to load texture from: {:?}", path);
    let img = image::open(path).map_err(|e| format!("Failed to open image {:?}: {}", path, e))?;
    log::info!("Image file opened successfully: {:?}", path);

    // No need to flip Y for Vulkan texture coordinates if UVs are handled correctly
    let img_rgba: RgbaImage = img.to_rgba8(); // Ensure RGBA format
    let (width, height) = img_rgba.dimensions();
    log::info!("Image converted to RGBA8, dimensions: {}x{}", width, height);
    let image_data = img_rgba.into_raw();
    let image_data_size = (width * height * 4) as vk::DeviceSize; // 4 bytes per pixel (RGBA)

    // --- 2. Create Staging Buffer ---
    log::info!("Creating staging buffer for image data");
    let mut staging_buffer = base.create_buffer(
        image_data_size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    log::info!("Staging buffer created successfully");

    base.update_buffer(&staging_buffer, &image_data)?;
    log::info!("Staging buffer updated with image data");

    // --- 3. Create Vulkan Image ---
    let format = vk::Format::R8G8B8A8_UNORM; // Standard RGBA format
    let image_extent = vk::Extent3D {
        width,
        height,
        depth: 1,
    };

    let image_create_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(image_extent)
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);

    log::info!("Creating Vulkan image");
    let image = unsafe { base.device.create_image(&image_create_info, None)? };
    log::info!("Vulkan image created successfully");

    // --- 4. Allocate Memory for Image ---
    let mem_requirements = unsafe { base.device.get_image_memory_requirements(image) };
    log::info!("Got memory requirements for image");

    let mem_type_index = find_memorytype_index(
        &mem_requirements,
        &base.device_memory_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or("Failed to find suitable memory type for image")?;
    log::info!("Found suitable memory type index: {}", mem_type_index);

    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_requirements.size)
        .memory_type_index(mem_type_index);

    log::info!("Allocating memory for image");
    let memory = unsafe { base.device.allocate_memory(&alloc_info, None)? };
    log::info!("Memory allocated successfully");

    unsafe { base.device.bind_image_memory(image, memory, 0)? };
    log::info!("Memory bound to image successfully");

    // --- 5. Transition Layout and Copy Buffer to Image ---
    log::info!("Recording and submitting command buffer for image transitions and copy");
    record_submit_commandbuffer(
        &base.device,
        base.setup_command_buffer,
        base.setup_commands_reuse_fence,
        base.present_queue,
        &[],
        &[],
        &[],
        |device, command_buffer| {
            log::info!("Recording command buffer: Transition UNDEFINED -> TRANSFER_DST_OPTIMAL");
            // Transition UNDEFINED -> TRANSFER_DST_OPTIMAL
            let barrier_to_transfer = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            unsafe {
                device.cmd_pipeline_barrier(
                    command_buffer,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier_to_transfer],
                );
            }
            log::info!("Recorded barrier for UNDEFINED -> TRANSFER_DST_OPTIMAL");

            log::info!("Recording buffer to image copy");
            // Copy Buffer to Image
            let buffer_image_copy = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(image_extent);

            unsafe {
                device.cmd_copy_buffer_to_image(
                    command_buffer,
                    staging_buffer.buffer,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[buffer_image_copy],
                );
            }
            log::info!("Recorded buffer to image copy");

            log::info!("Recording transition TRANSFER_DST_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL");
            // Transition TRANSFER_DST_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL
            let barrier_to_shader_read = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            unsafe {
                device.cmd_pipeline_barrier(
                    command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier_to_shader_read],
                );
            }
            log::info!("Recorded barrier for TRANSFER_DST_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL");
        },
    );
    log::info!("Command buffer submitted");

    unsafe {
        log::info!("Waiting for texture copy fence...");
        base.device
            .wait_for_fences(&[base.setup_commands_reuse_fence], true, u64::MAX)
            .map_err(|e| format!("Failed to wait for texture copy fence: {}", e))?;
        log::info!("Texture copy fence signaled.");
    }

    log::info!("Command buffer execution finished successfully");

    // --- 6. Clean up Staging Buffer ---
    log::info!("Destroying staging buffer");
    staging_buffer.destroy(&base.device);
    log::info!("Staging buffer destroyed");

    // --- 7. Create Image View ---
    log::info!("Creating image view");
    let image_view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .components(vk::ComponentMapping {
            r: vk::ComponentSwizzle::IDENTITY,
            g: vk::ComponentSwizzle::IDENTITY,
            b: vk::ComponentSwizzle::IDENTITY,
            a: vk::ComponentSwizzle::IDENTITY,
        })
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    let view = unsafe { base.device.create_image_view(&image_view_info, None)? };
    log::info!("Image view created successfully");

    // --- 8. Create Sampler ---
    log::info!("Creating sampler");
    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::REPEAT)
        .address_mode_w(vk::SamplerAddressMode::REPEAT)
        .anisotropy_enable(false)
        .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
        .unnormalized_coordinates(false)
        .compare_enable(false)
        .compare_op(vk::CompareOp::ALWAYS)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .mip_lod_bias(0.0)
        .min_lod(0.0)
        .max_lod(0.0);

    let sampler = unsafe { base.device.create_sampler(&sampler_info, None)? };
    log::info!("Sampler created successfully");

    log::info!("Texture loaded successfully: {:?}", path);

    Ok(TextureResource {
        image,
        memory,
        view,
        sampler,
        width,
        height,
    })
}
