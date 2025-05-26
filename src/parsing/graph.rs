// src/parsing/graph.rs
use std::io; // Keep for potential error types, though returning String for now

#[derive(Debug, Clone, Copy)]
pub enum ColorScheme {
    Default,
    // Alternative, // Can be removed if only one is used
}

pub struct GraphImageData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA data
}

pub fn generate_density_graph_rgba(
    measure_nps_vec: &[f64], // Expects f64 from simfile parsing stats
    max_nps: f64,
    // color_scheme: &ColorScheme, // Assuming default for now
) -> Result<GraphImageData, String> {
    // Dimensions for the generated texture. These can be adjusted.
    // Width can be fairly high for good horizontal resolution.
    // Height should ideally match the target display height of the graph box for 1:1 mapping.
    const IMAGE_WIDTH: u32 = 960;
    const IMAGE_HEIGHT: u32 = 376; // Matches LEFT_BOX_REF_HEIGHT, can be adjusted

    // Colors are defined in RGB, will be converted to RGBA
    let _bg_color_rgb = [30, 40, 47]; // This is the UI_BOX_DARK_COLOR, graph texture will be transparent over it.
    let bottom_color_rgb = [0, 184, 204];   // Cyan
    let top_color_rgb = [130, 0, 161];      // Purple

    // Pre-calculate the vertical color gradient.
    // color_gradient_rgb[0] will be the color for the very bottom of the graph area.
    // color_gradient_rgb[IMAGE_HEIGHT-1] will be for the very top.
    let color_gradient_rgb: Vec<[u8; 3]> = (0..IMAGE_HEIGHT)
        .map(|y_in_graph_area| { // y_in_graph_area: 0 is bottom of graph display, IMAGE_HEIGHT-1 is top
            let frac = y_in_graph_area as f64 / (IMAGE_HEIGHT.saturating_sub(1) as f64).max(1.0); // 0 at bottom, 1 at top
            let r = (bottom_color_rgb[0] as f64 * (1.0 - frac) + top_color_rgb[0] as f64 * frac).round() as u8;
            let g = (bottom_color_rgb[1] as f64 * (1.0 - frac) + top_color_rgb[1] as f64 * frac).round() as u8;
            let b = (bottom_color_rgb[2] as f64 * (1.0 - frac) + top_color_rgb[2] as f64 * frac).round() as u8;
            [r, g, b]
        })
        .collect();

    let mut img_buffer_rgba = vec![0u8; (IMAGE_WIDTH * IMAGE_HEIGHT * 4) as usize];

    // Fill with transparent background initially.
    // The graph area box in select_music::draw will provide the primary UI_BOX_DARK_COLOR background.
    let transparent_pixel = [0, 0, 0, 0];
    for pixel_chunk in img_buffer_rgba.chunks_exact_mut(4) {
        pixel_chunk.copy_from_slice(&transparent_pixel);
    }

    if !measure_nps_vec.is_empty() && max_nps > 0.0 && IMAGE_HEIGHT > 0 {
        let num_measures = measure_nps_vec.len();
        let measure_pixel_width = IMAGE_WIDTH as f64 / num_measures as f64;

        // Calculate the height of each measure bar in pixels (0 to IMAGE_HEIGHT)
        let bar_heights_px: Vec<f64> = measure_nps_vec
            .iter()
            .map(|&nps| (nps.max(0.0) / max_nps).min(1.0) * IMAGE_HEIGHT as f64) // Ensure nps is not negative
            .collect();

        for x_img in 0..IMAGE_WIDTH { // Iterate through each column of the output image
            let x_img_f = x_img as f64;
            // Determine which measure this pixel column corresponds to
            let measure_idx = (x_img_f / measure_pixel_width).floor() as usize;
            
            // Ensure measure_idx is within bounds, especially for the last pixel column
            let current_measure_idx = measure_idx.min(num_measures -1);


            // Interpolate height between the current measure's bar and the next one for smoother transitions
            let frac_within_measure_width = (x_img_f - (current_measure_idx as f64 * measure_pixel_width)) / measure_pixel_width;
            let h_start_px = bar_heights_px[current_measure_idx];
            let h_end_px = if current_measure_idx < num_measures - 1 {
                bar_heights_px[current_measure_idx + 1]
            } else {
                h_start_px // For the last measure, no next measure to interpolate to
            };

            let current_bar_height_at_x_px = h_start_px + frac_within_measure_width * (h_end_px - h_start_px);
            let current_bar_height_at_x_u32 = current_bar_height_at_x_px.round().max(0.0) as u32;

            if current_bar_height_at_x_u32 == 0 {
                continue; // No bar to draw for this pixel column
            }

            // y_img is 0 at top of image, IMAGE_HEIGHT-1 at bottom
            // We draw from the bottom of the image up to the bar_height
            // So, loop from (IMAGE_HEIGHT - bar_height) up to (IMAGE_HEIGHT - 1)
            let y_img_bar_top_inclusive = IMAGE_HEIGHT.saturating_sub(current_bar_height_at_x_u32);

            for y_img in y_img_bar_top_inclusive..IMAGE_HEIGHT {
                // y_img is the row in the image buffer (0=top, IMAGE_HEIGHT-1=bottom).
                // We need to get the color from `color_gradient_rgb`.
                // `color_gradient_rgb` is indexed by (y_in_graph_area), where 0 is bottom of graph, IMAGE_HEIGHT-1 is top.
                // So, y_in_graph_area = (IMAGE_HEIGHT - 1) - y_img;
                let y_for_gradient = (IMAGE_HEIGHT - 1).saturating_sub(y_img);
                
                let rgb_color = color_gradient_rgb[y_for_gradient as usize];
                let idx = (y_img * IMAGE_WIDTH + x_img) as usize * 4;

                img_buffer_rgba[idx] = rgb_color[0];     // R
                img_buffer_rgba[idx + 1] = rgb_color[1]; // G
                img_buffer_rgba[idx + 2] = rgb_color[2]; // B
                img_buffer_rgba[idx + 3] = 255;          // A (fully opaque)
            }
        }
    }

    Ok(GraphImageData {
        width: IMAGE_WIDTH,
        height: IMAGE_HEIGHT,
        data: img_buffer_rgba,
    })
}