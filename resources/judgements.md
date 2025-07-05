# DeadSync Judgment System Analysis

This document provides a comprehensive analysis of the current judgment system in DeadSync and a detailed implementation plan for adding judgment fonts (text that appears when notes are hit).

## Current Judgment System

### Judgment Types and Timing Windows

**Location**: `src/state.rs`

DeadSync uses 6 judgment types with specific timing windows:

| Judgment | Type | Timing Window | Description |
|----------|------|---------------|-------------|
| W1 | Fantastic | ≤21.5ms | Best judgment |
| W2 | Perfect | ≤43.0ms | Excellent timing |
| W3 | Great | ≤102.0ms | Good timing |
| W4 | Decent | ≤135.0ms | Acceptable timing |
| W5 | Way Off | ≤180.0ms | Poor timing (MAX_HIT_WINDOW_MS) |
| Miss | Miss | >200.0ms | Complete miss (MISS_WINDOW_MS) |

### Current Explosion Effects System

**Location**: `src/screens/gameplay.rs:843-852`

When a note is hit, the game creates visual explosion effects:

```rust
let explosion_end_time = Instant::now() + config::EXPLOSION_DURATION;
state.active_explosions.insert(
    dir,
    ActiveExplosion {
        judgment,
        direction: dir,
        end_time: explosion_end_time,
    },
);
```

**Current Explosion System Features**:
- **Duration**: 80ms (`config::EXPLOSION_DURATION`)
- **Storage**: HashMap with one explosion per direction
- **Automatic Cleanup**: In update loop (lines 628-630)
- **Visual Assets**: Separate textures for W1-W5 judgments
- **No Miss Explosion**: Miss judgments don't create explosions

### Input Handling and Hit Detection

**Function**: `check_hits_on_press()` in `src/screens/gameplay.rs:767-858`

**Hit Detection Process**:
1. Maps `VirtualKeyCode` to `ArrowDirection`
2. Finds closest arrow within timing window
3. Calculates judgment based on timing difference
4. Updates judgment counts in game state
5. Creates explosion effect
6. Removes hit arrow from active arrows

### Font and Text Rendering System

**Location**: `src/graphics/`

DeadSync uses a robust MSDF (Multi-channel Signed Distance Field) font system:

**Available Fonts**:
- **Wendy**: Number-focused font
- **Miso**: Clean label font

**Text Rendering Features**:
- `draw_text()` function in `renderer.rs:467-584`
- Supports scaling, positioning, and color
- Font metrics for proper sizing
- Glyph positioning and UV mapping
- Text measurement functions
- Baseline and ascender/descender support

### Existing Text During Gameplay

**Current Text Rendering Locations**:
- Song title in duration bar
- Judgment count display (right side UI)
- Hold instructions ("Continue holding ESC...")
- Global offset feedback messages

### Judgment Count Display System

**Implementation**: `draw_judgment_line()` function (lines 890-984)

Features:
- Displays running totals for each judgment type
- Uses both Wendy font (numbers) and Miso font (labels)
- Color-coded by judgment type
- Positioned on the right side of screen

## Implementation Plan for Judgment Fonts

### 1. Extend the ActiveExplosion System

**Location**: `src/state.rs` (around line 261-266)

Extend the `ActiveExplosion` struct to include judgment text data:

```rust
pub struct ActiveExplosion {
    pub judgment: JudgmentType,
    pub direction: ArrowDirection,
    pub end_time: Instant,
    pub show_text: bool,           // New field
    pub text_fade_start: Instant,  // New field
}
```

### 2. Add Judgment Text Configuration

**Location**: `src/config.rs` (around line 82-89)

Add new constants for judgment text:

```rust
pub const JUDGMENT_TEXT_DURATION: Duration = Duration::from_millis(1000);
pub const JUDGMENT_TEXT_FADE_DURATION: Duration = Duration::from_millis(300);
pub const JUDGMENT_TEXT_SIZE: f32 = 48.0;
pub const JUDGMENT_TEXT_OFFSET_Y: f32 = -80.0; // Above receptors
```

### 3. Modify Hit Detection

**Location**: `src/screens/gameplay.rs` in `check_hits_on_press()` (around line 843-852)

Update the explosion creation to include judgment text:

```rust
let explosion_end_time = Instant::now() + config::EXPLOSION_DURATION;
let text_fade_start = Instant::now() + config::JUDGMENT_TEXT_DURATION - config::JUDGMENT_TEXT_FADE_DURATION;

state.active_explosions.insert(
    dir,
    ActiveExplosion {
        judgment,
        direction: dir,
        end_time: explosion_end_time,
        show_text: true,
        text_fade_start,
    },
);
```

### 4. Add Judgment Text Rendering Function

**Location**: `src/screens/gameplay.rs` (new function)

Add a new function to render judgment text:

```rust
fn draw_judgment_text(
    renderer: &mut Renderer,
    game_state: &GameplayState,
    window_size: (f32, f32),
) -> Result<(), Box<dyn Error>> {
    let current_time = Instant::now();
    
    for (direction, explosion) in &game_state.active_explosions {
        if !explosion.show_text {
            continue;
        }
        
        // Calculate text alpha based on time
        let time_alive = current_time.duration_since(explosion.end_time - config::JUDGMENT_TEXT_DURATION);
        let alpha = if current_time >= explosion.text_fade_start {
            let fade_progress = current_time.duration_since(explosion.text_fade_start).as_secs_f32() 
                / config::JUDGMENT_TEXT_FADE_DURATION.as_secs_f32();
            (1.0 - fade_progress).max(0.0)
        } else {
            1.0
        };
        
        if alpha <= 0.0 {
            continue;
        }
        
        // Get judgment text and color
        let (text, color) = match explosion.judgment {
            JudgmentType::W1 => ("FANTASTIC", [1.0, 1.0, 0.0, alpha]), // Yellow
            JudgmentType::W2 => ("PERFECT", [0.0, 1.0, 1.0, alpha]),   // Cyan
            JudgmentType::W3 => ("GREAT", [0.0, 1.0, 0.0, alpha]),     // Green
            JudgmentType::W4 => ("DECENT", [1.0, 0.5, 0.0, alpha]),    // Orange
            JudgmentType::W5 => ("WAY OFF", [1.0, 0.0, 0.0, alpha]),   // Red
            JudgmentType::Miss => ("MISS", [0.5, 0.5, 0.5, alpha]),    // Gray
        };
        
        // Get target position for this direction
        let target_pos = &game_state.targets[*direction as usize];
        let text_x = target_pos.x;
        let text_y = target_pos.y + config::JUDGMENT_TEXT_OFFSET_Y;
        
        // Draw the judgment text
        renderer.draw_text(
            text,
            text_x,
            text_y,
            config::JUDGMENT_TEXT_SIZE,
            color,
            TextAlignment::Center,
            FontId::Miso, // or FontId::Wendy depending on preference
        )?;
    }
    
    Ok(())
}
```

### 5. Update Explosion Cleanup

**Location**: `src/screens/gameplay.rs` in the update loop (around line 628-630)

Modify the cleanup logic to handle judgment text duration:

```rust
// Clean up expired explosions and judgment text
state.active_explosions.retain(|_, explosion| {
    let current_time = Instant::now();
    let explosion_alive = current_time < explosion.end_time;
    let text_alive = explosion.show_text && 
        current_time < explosion.end_time - config::EXPLOSION_DURATION + config::JUDGMENT_TEXT_DURATION;
    
    explosion_alive || text_alive
});
```

### 6. Add Text Alignment Support

**Location**: `src/graphics/renderer.rs` (if not already present)

```rust
pub enum TextAlignment {
    Left,
    Center,
    Right,
}
```

Update the `draw_text()` function to support text alignment if needed.

### 7. Integrate into Main Draw Loop

**Location**: `src/screens/gameplay.rs` in the main draw loop (after line 1443)

```rust
// Draw judgment text
draw_judgment_text(renderer, game_state, state.window_size)?;
```

## Implementation Details

### Font Selection

**Recommended Font**: `FontId::Miso`
- Clean, readable judgment text
- Consistent with existing UI elements
- Good visibility during gameplay

**Alternative**: `FontId::Wendy`
- More stylized appearance
- Number-focused design
- May be less readable at smaller sizes

### Positioning Strategy

**Text Positioning**:
- **X-coordinate**: Centered on target receptor (`target_pos.x`)
- **Y-coordinate**: Above receptor (`target_pos.y + JUDGMENT_TEXT_OFFSET_Y`)
- **Alignment**: Center-aligned for consistency
- **Reference**: Uses existing `game_state.targets` positions

### Animation Timing

**Timeline**:
1. **0ms**: Text appears immediately when note is hit
2. **0-700ms**: Text remains at full opacity
3. **700-1000ms**: Text fades out over 300ms
4. **1000ms**: Text disappears completely

**Synchronization**:
- Judgment text duration is independent of explosion duration
- Text can persist after explosion ends
- Both use the same timing system for consistency

### Color Scheme

**Judgment Colors** (following rhythm game conventions):
- **W1 (Fantastic)**: Yellow `[1.0, 1.0, 0.0, alpha]`
- **W2 (Perfect)**: Cyan `[0.0, 1.0, 1.0, alpha]`
- **W3 (Great)**: Green `[0.0, 1.0, 0.0, alpha]`
- **W4 (Decent)**: Orange `[1.0, 0.5, 0.0, alpha]`
- **W5 (Way Off)**: Red `[1.0, 0.0, 0.0, alpha]`
- **Miss**: Gray `[0.5, 0.5, 0.5, alpha]`

### Performance Considerations

**Optimizations**:
- Reuses existing text rendering system
- Only renders active judgment text
- Automatic cleanup prevents memory leaks
- Alpha testing skips invisible text
- Efficient HashMap storage for explosions

### Integration Benefits

**Advantages of This Approach**:
1. **Seamless Integration**: Builds on existing explosion system
2. **Consistent Architecture**: Uses established patterns
3. **Performance Efficient**: Minimal overhead
4. **Maintainable**: Clear separation of concerns
5. **Extensible**: Easy to add animations or effects

## Key Files Modified

| File | Purpose | Changes |
|------|---------|---------|
| `src/state.rs` | Data structures | Extend `ActiveExplosion` struct |
| `src/config.rs` | Configuration | Add judgment text constants |
| `src/screens/gameplay.rs` | Game logic | Add text rendering and cleanup |
| `src/graphics/renderer.rs` | Rendering | Add text alignment support |

## Testing Recommendations

1. **Timing Accuracy**: Verify judgment text appears immediately on hit
2. **Visual Clarity**: Test readability during intense gameplay
3. **Performance**: Monitor frame rate during busy sections
4. **Fade Animation**: Ensure smooth alpha transitions
5. **Multiple Hits**: Test overlapping judgments on different lanes

This implementation provides immediate visual feedback to players while maintaining the existing codebase architecture and performance characteristics.