use crate::config; // For color constants

pub struct GraphImageData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA data
}

fn calculate_gradient_color(y_in_graph_area: u32, image_height: u32, bottom_rgb: [u8; 3], top_rgb: [u8; 3]) -> [u8; 3] {
    let frac = if image_height <= 1 { 0.0 } else { y_in_graph_area as f64 / (image_height - 1) as f64 }; // 0 at bottom, 1 at top
    let r = (bottom_rgb[0] as f64 * (1.0 - frac) + top_rgb[0] as f64 * frac).round() as u8;
    let g = (bottom_rgb[1] as f64 * (1.0 - frac) + top_rgb[1] as f64 * frac).round() as u8;
    let b = (bottom_rgb[2] as f64 * (1.0 - frac) + top_rgb[2] as f64 * frac).round() as u8;
    [r, g, b]
}

fn precompute_color_gradient(image_height: u32) -> Vec<[u8; 3]> {
    // Using colors from config.rs
    let bottom_color_rgb = [
        (config::GRAPH_BOTTOM_COLOR[0] * 255.0) as u8,
        (config::GRAPH_BOTTOM_COLOR[1] * 255.0) as u8,
        (config::GRAPH_BOTTOM_COLOR[2] * 255.0) as u8,
    ];
    let top_color_rgb = [
        (config::GRAPH_TOP_COLOR[0] * 255.0) as u8,
        (config::GRAPH_TOP_COLOR[1] * 255.0) as u8,
        (config::GRAPH_TOP_COLOR[2] * 255.0) as u8,
    ];

    (0..image_height)
        .map(|y| calculate_gradient_color(y, image_height, bottom_color_rgb, top_color_rgb))
        .collect()
}

fn calculate_bar_heights_pixels(measure_nps_vec: &[f64], max_nps: f64, image_height: u32) -> Vec<f64> {
    if max_nps <= 0.0 || image_height == 0 { // Prevent division by zero or no height
        return vec![0.0; measure_nps_vec.len()];
    }
    measure_nps_vec
        .iter()
        .map(|&nps| (nps.max(0.0) / max_nps).min(1.0) * image_height as f64)
        .collect()
}

pub fn generate_density_graph_rgba(
    measure_nps_vec: &[f64],
    max_nps: f64,
) -> Result<GraphImageData, String> {
    const IMAGE_WIDTH: u32 = 960; 
    const IMAGE_HEIGHT: u32 = 376;

    if IMAGE_HEIGHT == 0 { return Err("Image height cannot be zero.".to_string()); }

    let color_gradient_rgb = precompute_color_gradient(IMAGE_HEIGHT);
    let mut img_buffer_rgba = vec![0u8; (IMAGE_WIDTH * IMAGE_HEIGHT * 4) as usize]; // Initialize to transparent black

    if measure_nps_vec.is_empty() || max_nps <= 0.0 {
        // Return a fully transparent image if no data or no max_nps
        return Ok(GraphImageData { width: IMAGE_WIDTH, height: IMAGE_HEIGHT, data: img_buffer_rgba });
    }
    
    let num_measures = measure_nps_vec.len();
    let measure_pixel_width = if num_measures > 0 { IMAGE_WIDTH as f64 / num_measures as f64 } else { IMAGE_WIDTH as f64 };
    let bar_heights_px = calculate_bar_heights_pixels(measure_nps_vec, max_nps, IMAGE_HEIGHT);

    for x_img in 0..IMAGE_WIDTH {
        let current_measure_idx = if measure_pixel_width > 0.0 {
            (x_img as f64 / measure_pixel_width).floor() as usize
        } else {
            0 // Should only happen if num_measures is 0, handled above
        }.min(num_measures.saturating_sub(1)); // Ensure in bounds

        let frac_within_measure_width = if measure_pixel_width > 0.0 {
            (x_img as f64 - (current_measure_idx as f64 * measure_pixel_width)) / measure_pixel_width
        } else {
            0.0 // Avoid division by zero if measure_pixel_width is somehow zero
        };

        let h_start_px = bar_heights_px[current_measure_idx];
        let h_end_px = if current_measure_idx < num_measures - 1 { bar_heights_px[current_measure_idx + 1] } else { h_start_px };
        
        let current_bar_height_at_x_px = h_start_px + frac_within_measure_width * (h_end_px - h_start_px);
        let current_bar_height_at_x_u32 = current_bar_height_at_x_px.round().max(0.0) as u32;

        if current_bar_height_at_x_u32 == 0 { continue; }

        let y_img_bar_top_inclusive = IMAGE_HEIGHT.saturating_sub(current_bar_height_at_x_u32);
        for y_img in y_img_bar_top_inclusive..IMAGE_HEIGHT {
            let y_for_gradient = (IMAGE_HEIGHT - 1).saturating_sub(y_img); // 0 for bottom row of bar, IMAGE_HEIGHT-1 for top row of image
            let rgb_color = color_gradient_rgb[y_for_gradient.min(IMAGE_HEIGHT -1) as usize]; // Ensure index is safe
            let idx = (y_img * IMAGE_WIDTH + x_img) as usize * 4;
            img_buffer_rgba[idx..idx+4].copy_from_slice(&[rgb_color[0], rgb_color[1], rgb_color[2], 255]);
        }
    }

    Ok(GraphImageData { width: IMAGE_WIDTH, height: IMAGE_HEIGHT, data: img_buffer_rgba })
}