# DeadSync Judgment Graphics Implementation

This document provides a comprehensive analysis of the DeadSync texture system and detailed implementation plan for adding graphic-based judgments using the existing `assets/graphics/judgements/chromatic.png` sprite sheet.

## Judgment Graphics Asset Analysis

### Sprite Sheet Layout

**File**: `assets/graphics/judgements/chromatic.png`

The judgment graphics sprite sheet is organized as:
- **2 columns** (possibly for different states/animations)
- **6 rows** (one for each judgment type)
- **Row mapping**:
  - Row 0: FANTASTIC (cyan/blue glow)
  - Row 1: EXCELLENT (yellow/orange glow)
  - Row 2: GREAT (green glow)
  - Row 3: DECENT (purple/magenta glow)
  - Row 4: WAY OFF (orange/red glow)
  - Row 5: MISS (red glow)

### Visual Characteristics

- Each judgment has a distinctive color scheme and glow effect
- Text appears to have consistent typography across all judgments
- Left and right columns appear to be variations (possibly for animation)
- High contrast design suitable for gameplay visibility

## Current Texture Management System

### Texture Loading and Registration

**File**: `src/assets.rs`

**TextureId Enum** (lines 15-27):
- Central registry of all static textures
- Explosion textures already exist: `ExplosionW1` through `ExplosionW5`
- New texture IDs are added to this enum

**Loading Process**:
- `load_and_register_texture()` (lines 164-176) handles the workflow:
  1. Uses `load_texture()` from `graphics/texture.rs` to load from file
  2. Updates the descriptor set via `renderer.update_texture_descriptor()`
  3. Stores in the `textures` HashMap

### TextureResource Structure

**File**: `src/graphics/texture.rs`

**Core Structure** (lines 10-18):
```rust
pub struct TextureResource {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub width: u32,
    pub height: u32,
}
```

**Loading Functions**:
- `load_texture()` (line 476): Loads from file path using the `image` crate
- `create_texture_from_rgba_data()` (line 44): Creates from raw RGBA data
- Supports PNG, JPEG, and other common formats

### Descriptor Set System

**File**: `src/graphics/renderer.rs`

**DescriptorSetId Enum** (lines 14-33):
- Maps texture types to descriptor sets
- Each texture gets a unique descriptor set for binding to shaders
- Explosion textures have individual descriptor sets

**Descriptor Management**:
- `update_texture_descriptor()` (lines 342-376): Binds texture to descriptor set
- Descriptor sets are allocated in batches (line 126: `MAX_SETS: u32 = 16`)
- Each set contains UBO (binding 0) and texture sampler (binding 1)

### UV Coordinate and Sprite Sheet Handling

**UV System in PushConstantData** (`src/state.rs` lines 249-250):
```rust
pub uv_offset: [f32; 2],  // Starting UV coordinates
pub uv_scale: [f32; 2],   // UV region size
```

**Current Arrow Sprite Sheet Usage** (`src/screens/gameplay.rs` lines 1268-1316):
- 4-frame animation sprite sheet (horizontal layout)
- UV calculation: `uv_width = 1.0 / 4.0`, `uv_x_start = frame_index * uv_width`
- Demonstrates how to extract regions from a sprite sheet

### Drawing System

**Quad Rendering** (`src/graphics/renderer.rs` lines 410-464):
- `draw_quad()` is the primary drawing function
- Takes descriptor set ID, position, size, rotation, tint, UV offset, and UV scale
- All textures use the same quad geometry with different UV coordinates

## Implementation Plan for Graphic-Based Judgments

### 1. Add Judgment Texture to Assets System

**Location**: `src/assets.rs`

Add new texture ID around line 27:
```rust
pub enum TextureId {
    // ... existing entries
    JudgmentGraphics,
}
```

Add descriptor set ID in `src/graphics/renderer.rs` around line 33:
```rust
pub enum DescriptorSetId {
    // ... existing entries
    JudgmentGraphics,
}
```

### 2. Load the Judgment Texture

**Location**: `src/assets.rs` in the `load_assets()` function

Add this after the explosion texture loading (around line 145):
```rust
// Load judgment graphics
self.load_and_register_texture(
    base,
    renderer,
    TextureId::JudgmentGraphics,
    "assets/graphics/judgments/chromatic.png",
    DescriptorSetId::JudgmentGraphics,
)?;
```

### 3. Add Judgment Configuration

**Location**: `src/config.rs`

Add judgment display constants:
```rust
pub const JUDGMENT_GRAPHICS_DURATION: Duration = Duration::from_millis(1000);
pub const JUDGMENT_GRAPHICS_FADE_DURATION: Duration = Duration::from_millis(300);
pub const JUDGMENT_GRAPHICS_SIZE: f32 = 200.0; // Width/height in pixels
pub const JUDGMENT_GRAPHICS_OFFSET_Y: f32 = -80.0; // Above receptors
```

### 4. Update ActiveExplosion Structure

**Location**: `src/state.rs` around line 261-266

```rust
pub struct ActiveExplosion {
    pub judgment: JudgmentType,
    pub direction: ArrowDirection,
    pub end_time: Instant,
    pub show_judgment_graphic: bool,        // New field
    pub judgment_fade_start: Instant,       // New field
}
```

### 5. Update Hit Detection

**Location**: `src/screens/gameplay.rs` in `check_hits_on_press()` (around line 843-852)

```rust
let explosion_end_time = Instant::now() + config::EXPLOSION_DURATION;
let judgment_fade_start = Instant::now() + config::JUDGMENT_GRAPHICS_DURATION - config::JUDGMENT_GRAPHICS_FADE_DURATION;

state.active_explosions.insert(
    dir,
    ActiveExplosion {
        judgment,
        direction: dir,
        end_time: explosion_end_time,
        show_judgment_graphic: true,
        judgment_fade_start,
    },
);
```

### 6. Add Judgment Graphics Rendering Function

**Location**: `src/screens/gameplay.rs` (new function)

```rust
fn draw_judgment_graphics(
    renderer: &mut Renderer,
    game_state: &GameplayState,
    window_size: (f32, f32),
) -> Result<(), Box<dyn Error>> {
    let current_time = Instant::now();
    
    for (direction, explosion) in &game_state.active_explosions {
        if !explosion.show_judgment_graphic {
            continue;
        }
        
        // Calculate alpha based on timing
        let alpha = if current_time >= explosion.judgment_fade_start {
            let fade_progress = current_time.duration_since(explosion.judgment_fade_start).as_secs_f32() 
                / config::JUDGMENT_GRAPHICS_FADE_DURATION.as_secs_f32();
            (1.0 - fade_progress).max(0.0)
        } else {
            1.0
        };
        
        if alpha <= 0.0 {
            continue;
        }
        
        // Calculate UV coordinates for sprite sheet
        // Your sprite sheet is 2 columns x 6 rows
        let judgment_row = match explosion.judgment {
            JudgmentType::W1 => 0,    // FANTASTIC (top row)
            JudgmentType::W2 => 1,    // EXCELLENT 
            JudgmentType::W3 => 2,    // GREAT
            JudgmentType::W4 => 3,    // DECENT
            JudgmentType::W5 => 4,    // WAY OFF
            JudgmentType::Miss => 5,  // MISS (bottom row)
        };
        
        // Use column 0 for now (you can animate between columns later)
        let column = 0;
        
        // Calculate UV coordinates
        let uv_width = 1.0 / 2.0;   // 2 columns
        let uv_height = 1.0 / 6.0;  // 6 rows
        let uv_x_start = column as f32 * uv_width;
        let uv_y_start = judgment_row as f32 * uv_height;
        
        // Get target position for this direction
        let target_pos = &game_state.targets[*direction as usize];
        let graphics_x = target_pos.x;
        let graphics_y = target_pos.y + config::JUDGMENT_GRAPHICS_OFFSET_Y;
        
        // Draw the judgment graphic
        renderer.draw_quad(
            DescriptorSetId::JudgmentGraphics,
            graphics_x,
            graphics_y,
            config::JUDGMENT_GRAPHICS_SIZE,
            config::JUDGMENT_GRAPHICS_SIZE,
            Rad(0.0), // No rotation
            [1.0, 1.0, 1.0, alpha], // White tint with alpha
            [uv_x_start, uv_y_start], // UV offset
            [uv_width, uv_height],    // UV scale
        )?;
    }
    
    Ok(())
}
```

### 7. Update Explosion Cleanup

**Location**: `src/screens/gameplay.rs` in the update loop (around line 628-630)

```rust
// Clean up expired explosions and judgment graphics
state.active_explosions.retain(|_, explosion| {
    let current_time = Instant::now();
    let explosion_alive = current_time < explosion.end_time;
    let judgment_alive = explosion.show_judgment_graphic && 
        current_time < explosion.judgment_fade_start + config::JUDGMENT_GRAPHICS_FADE_DURATION;
    
    explosion_alive || judgment_alive
});
```

### 8. Call the Judgment Graphics Function

**Location**: `src/screens/gameplay.rs` in the main draw loop (after line 1443)

```rust
// Draw judgment graphics
draw_judgment_graphics(renderer, game_state, state.window_size)?;
```

## Implementation Details

### Sprite Sheet UV Coordinate Calculation

For the 2x6 sprite sheet layout:

```rust
// UV dimensions for each sprite
let uv_width = 1.0 / 2.0;   // Each sprite is 50% of texture width
let uv_height = 1.0 / 6.0;  // Each sprite is ~16.67% of texture height

// UV coordinates for specific judgment and column
let uv_x_start = column as f32 * uv_width;
let uv_y_start = judgment_row as f32 * uv_height;
```

### Judgment Row Mapping

| Judgment Type | Row Index | Description |
|---------------|-----------|-------------|
| W1 (Fantastic) | 0 | Top row, cyan/blue glow |
| W2 (Excellent) | 1 | Yellow/orange glow |
| W3 (Great) | 2 | Green glow |
| W4 (Decent) | 3 | Purple/magenta glow |
| W5 (Way Off) | 4 | Orange/red glow |
| Miss | 5 | Bottom row, red glow |

### Animation Possibilities

You can animate between columns for dynamic effects:

```rust
// Simple column animation (alternating every 200ms)
let column = if (current_time.elapsed().as_millis() / 200) % 2 == 0 { 0 } else { 1 };

// Smooth transition based on display time
let time_alive = current_time.duration_since(explosion.end_time - config::JUDGMENT_GRAPHICS_DURATION);
let column = if time_alive.as_millis() < 500 { 0 } else { 1 };
```

### Positioning Strategy

**Positioning Logic**:
- **X-coordinate**: Centered on target receptor (`target_pos.x`)
- **Y-coordinate**: Above receptor (`target_pos.y + JUDGMENT_GRAPHICS_OFFSET_Y`)
- **Size**: Square aspect ratio maintained (`JUDGMENT_GRAPHICS_SIZE x JUDGMENT_GRAPHICS_SIZE`)
- **Rotation**: No rotation applied (0.0 radians)

### Timing Configuration

**Animation Timeline**:
1. **0ms**: Judgment graphic appears immediately when note is hit
2. **0-700ms**: Graphic remains at full opacity
3. **700-1000ms**: Graphic fades out over 300ms
4. **1000ms**: Graphic disappears completely

**Synchronization**:
- Judgment graphics duration is independent of explosion duration
- Graphics can persist after explosion ends
- Both systems use the same timing framework

### Performance Considerations

**Optimizations**:
- Single texture load instead of multiple judgment textures
- Efficient UV coordinate system reduces draw calls
- Reuses existing quad rendering pipeline
- Alpha testing skips invisible graphics
- Automatic cleanup prevents memory leaks

**Memory Usage**:
- One texture holds all judgment graphics
- Efficient sprite sheet approach
- Minimal additional memory overhead

### Visual Quality

**Rendering Features**:
- High-quality glow effects preserved from original graphics
- Consistent typography across all judgments
- Distinctive color coding for each judgment type
- Smooth alpha blending for fade effects

## Integration Benefits

**Advantages of This Implementation**:

1. **Visual Consistency**: Uses custom-designed judgment graphics
2. **Performance Efficient**: Single texture with UV mapping
3. **Seamless Integration**: Builds on existing explosion system
4. **Extensible**: Easy to add animation frames or effects
5. **Maintainable**: Clear separation of concerns
6. **Memory Efficient**: Sprite sheet approach reduces texture memory

## Future Enhancement Opportunities

### Advanced Animation

1. **Multi-frame Animation**: Use both columns for frame-based animation
2. **Scale Animation**: Grow/shrink effect over time
3. **Rotation Effects**: Subtle rotation for dynamic feel
4. **Particle Integration**: Combine with particle effects

### Customization Options

1. **Noteskin Integration**: Different judgment graphics per noteskin
2. **Size Scaling**: User-configurable judgment graphic size
3. **Position Adjustment**: Customizable offset positions
4. **Duration Settings**: User-configurable display duration

### Visual Effects

1. **Glow Enhancement**: Additional glow effects
2. **Screen Shake**: Impact effects on perfect judgments
3. **Color Variations**: Judgment-specific color multipliers
4. **Combo Integration**: Special effects for combo milestones

## Code Files Modified

| File | Purpose | Changes |
|------|---------|---------|
| `src/assets.rs` | Asset loading | Add `TextureId::JudgmentGraphics` |
| `src/graphics/renderer.rs` | Rendering system | Add `DescriptorSetId::JudgmentGraphics` |
| `src/config.rs` | Configuration | Add judgment graphics constants |
| `src/state.rs` | Data structures | Extend `ActiveExplosion` struct |
| `src/screens/gameplay.rs` | Game logic | Add graphics rendering and cleanup |

## Testing Recommendations

1. **Visual Verification**: Ensure correct judgment graphics appear
2. **Timing Accuracy**: Verify graphics appear immediately on hit
3. **UV Mapping**: Test all judgment types display correctly
4. **Animation Smooth**: Check fade-out transitions
5. **Performance**: Monitor frame rate during intense gameplay
6. **Memory Usage**: Verify no texture memory leaks

This implementation provides immediate, high-quality visual feedback using custom judgment graphics while maintaining the existing codebase architecture and performance characteristics.