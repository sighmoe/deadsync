# DeadSync Noteskin System Documentation

This document provides a comprehensive analysis of how noteskins are currently implemented and rendered in the DeadSync rhythm game engine.

## Current Noteskin System Overview

DeadSync uses a basic noteskin system that renders notes using a single texture with rotation-based direction handling. The system is functional but limited in scope and customization options.

### Architecture

**Single Texture Approach**: The system uses only one down arrow texture (`assets/noteskins/cel/down_arrow_cel.png`) for all arrow directions, applying rotation transformations as needed:
- Left: 90° rotation
- Down: 0° rotation (base)
- Up: 180° rotation
- Right: -90° rotation

**Configuration Location**: `src/config.rs:18-29`
```rust
pub const ARROW_TEXTURE_PATH: &str = "assets/noteskins/cel/down_arrow_cel.png";
```

### Note Types and Structure

**Supported Note Types** (`src/parsing/simfile.rs:16-26`):
```rust
pub enum NoteChar {
    Empty = b'0' as isize,
    Tap = b'1' as isize,
    HoldStart = b'2' as isize,
    HoldEnd = b'3' as isize,
    RollStart = b'4' as isize,
    Mine = b'M' as isize,
    Lift = b'L' as isize,
    Fake = b'F' as isize,
    Unsupported,
}
```

**Note Rendering Limitation**: Only Tap, HoldStart, and RollStart notes are currently rendered (`src/screens/gameplay.rs:730-732`).

**Arrow Data Structure** (`src/state.rs:205-211`):
```rust
pub struct Arrow {
    pub x: f32,
    pub y: f32,
    pub direction: ArrowDirection,
    pub note_char: NoteChar,
    pub target_beat: f32,
}
```

### Rendering Pipeline

The main rendering logic is located in `src/screens/gameplay.rs:1302-1397`.

#### 1. Texture Animation

The arrow texture uses a 4-frame sprite sheet with UV coordinates calculated based on beat timing:

```rust
let frame_index = (((-beat_diff_for_anim * 2.0).floor().abs() as i32 % 4) + 4) % 4;
```

#### 2. Arrow Direction Rotation

Rotation angles are applied based on arrow direction (`src/screens/gameplay.rs:1371-1376`):
```rust
let rotation_angle = match arrow.direction {
    ArrowDirection::Left => Rad(PI / 2.0),
    ArrowDirection::Down => Rad(0.0),
    ArrowDirection::Up => Rad(PI),
    ArrowDirection::Right => Rad(-PI / 2.0),
};
```

#### 3. Beat-based Color Tinting

Arrows are colored based on their beat subdivision (`src/screens/gameplay.rs:1318-1369`):
- **4th notes**: Red tint (`ARROW_TINT_4TH`)
- **8th notes**: Blue tint (`ARROW_TINT_8TH`)
- **16th notes**: Green tint (`ARROW_TINT_16TH`)
- **32nd notes**: Light green tint (`ARROW_TINT_32ND`)
- **Other subdivisions**: Specific color assignments

### Target Receptors

Target receptors (where notes are hit) use the same arrow texture as scrolling notes:
- **Location**: `src/screens/gameplay.rs:1268-1300`
- **Animation**: 4-frame cycle based on current beat
- **Appearance**: Semi-transparent tint `[0.7, 0.7, 0.7, 0.5]`
- **Rotation**: Same system as scrolling arrows

### Explosion Effects

Hit explosions use separate textures for each judgment window:

**Texture Paths** (`src/config.rs:25-29`):
```rust
pub const EXPLOSION_W1_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w1.png";
pub const EXPLOSION_W2_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w2.png";
pub const EXPLOSION_W3_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w3.png";
pub const EXPLOSION_W4_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w4.png";
pub const EXPLOSION_W5_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w5.png";
```

**Explosion Rendering** (`src/screens/gameplay.rs:1399-1443`):
- Size multiplier: `1.5x`
- Duration: `80ms`
- Different textures for each judgment (W1-W5)

### Graphics Infrastructure

**Rendering System** (`src/graphics/renderer.rs`):
- Vulkan-based rendering pipeline
- MSDF shaders for efficient quad rendering
- Descriptor sets for texture management
- Push constants for per-quad transformation and color data

**Asset Management** (`src/assets.rs`):
- TextureId enum for identifying different textures
- Automatic loading and descriptor set updates
- Dynamic banner system (separate from noteskins)

## Current File Structure

```
assets/noteskins/cel/
├── down_arrow_cel.png              # Main arrow texture (4-frame sprite sheet)
├── down_tap_explosion_dim_w1.png   # Perfect judgment explosion
├── down_tap_explosion_dim_w2.png   # Great judgment explosion
├── down_tap_explosion_dim_w3.png   # Good judgment explosion
├── down_tap_explosion_dim_w4.png   # Bad judgment explosion
└── down_tap_explosion_dim_w5.png   # Miss judgment explosion
```

## Key Limitations

1. **Single Direction Texture**: Only one down arrow texture is used, rotated for all directions
2. **Limited Note Type Support**: Only renders Tap, HoldStart, and RollStart notes
3. **No Noteskin Switching**: Hardcoded to "cel" noteskin
4. **Basic Animation**: Simple 4-frame UV cycling
5. **No Hold/Roll Bodies**: Only start notes are rendered, no hold note trails
6. **No Mine/Lift Rendering**: Special note types are not visually represented
7. **Static Explosion System**: Fixed explosion textures, no customization

## Technical Implementation Details

### Coordinate System
- Notes scroll vertically from top to bottom
- Y-coordinate represents the note's position relative to the target line
- X-coordinate is fixed based on arrow direction (lane position)

### Performance Considerations
- Texture caching and reuse
- Efficient batched rendering through Vulkan
- Pre-computed UV coordinates for animation frames
- Minimal state changes during rendering

### Timing and Synchronization
- Beat-based animation timing
- Precise note positioning using floating-point beat calculations
- Color tinting based on note subdivision analysis

## Future Enhancement Opportunities

1. **Multi-directional Textures**: Support for separate textures per arrow direction
2. **Noteskin Selection System**: Runtime switching between different noteskin sets
3. **Hold/Roll Body Rendering**: Complete hold note visualization
4. **Mine and Lift Support**: Visual representation of special note types
5. **Advanced Animation**: Multi-frame sequences, particle effects
6. **Customizable Explosions**: Per-noteskin explosion sets
7. **Skin Configuration**: JSON-based noteskin definition files

This documentation serves as a foundation for understanding the current implementation and planning future noteskin system enhancements.