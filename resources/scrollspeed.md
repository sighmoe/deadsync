# DeadSync Arrow Scroll Speed Analysis

This document provides a comprehensive analysis of how arrow scroll speed is determined and controlled in the DeadSync rhythm game engine.

## Overview

DeadSync uses a fixed scroll speed system that maintains consistent visual arrow movement across different screen resolutions. The system currently lacks user-configurable speed modifiers that are common in other rhythm games.

## Core Scroll Speed Configuration

### Base Speed Setting

**Location**: `src/config.rs:32`
```rust
pub const ARROW_SPEED: f32 = 1300.0;
```

- **Value**: 1300.0 pixels per second
- **Type**: Hardcoded constant
- **Scope**: Global, affects all gameplay
- **Limitation**: No user configuration options

### Reference Resolution Constants

**Location**: `src/config.rs:44-45`
```rust
pub const GAMEPLAY_REF_WIDTH: f32 = 1280.0;
pub const GAMEPLAY_REF_HEIGHT: f32 = 720.0;
```

These constants define the reference resolution for speed calculations, ensuring consistent visual speed across different screen sizes.

## Scroll Speed Calculation Formula

The effective scroll speed is calculated using resolution scaling to maintain consistent visual appearance:

### Main Formula

**Location**: `src/screens/gameplay.rs:611-612` and `src/screens/gameplay.rs:716-717`

```rust
let px_per_sec_scroll_speed = 
    config::ARROW_SPEED * (window_height / config::GAMEPLAY_REF_HEIGHT);
```

### Components

- **`config::ARROW_SPEED`**: Base speed (1300.0 px/sec)
- **`window_height`**: Current window height in pixels
- **`config::GAMEPLAY_REF_HEIGHT`**: Reference height (720.0 px)

### Examples

| Window Height | Calculation | Effective Speed |
|---------------|-------------|-----------------|
| 720p | 1300 × (720/720) | 1300 px/sec |
| 1080p | 1300 × (1080/720) | 1950 px/sec |
| 1440p | 1300 × (1440/720) | 2600 px/sec |

## Arrow Position Calculation

### Position Update Formula

**Location**: `src/screens/gameplay.rs:620-621`

```rust
arrow.y = target_receptor_y + time_difference_to_display_target_sec * px_per_sec_scroll_speed;
```

### Initial Spawn Position

**Location**: `src/screens/gameplay.rs:718-719`

```rust
let initial_y = target_receptor_y + time_difference_to_display_target_sec * px_per_sec_scroll_speed;
```

### Key Variables

- **`target_receptor_y`**: Y-coordinate of target receptors (hit line)
- **`time_difference_to_display_target_sec`**: Time until arrow should reach receptors
- **`px_per_sec_scroll_speed`**: Calculated effective speed

## BPM Changes and Timing Integration

### Timing Data Handling

**Location**: `src/screens/gameplay.rs:616-617`

```rust
let arrow_target_display_time_sec = game_state.timing_data.get_time_for_beat(arrow.target_beat);
let time_difference_to_display_target_sec = arrow_target_display_time_sec - current_display_time_sec;
```

### BPM Change Behavior

The system handles BPM changes through the `TimingData` structure:

1. **Timing Conversion**: `get_time_for_beat()` function (`src/screens/gameplay.rs:38-95`) converts beat positions to time positions
2. **Speed Independence**: BPM changes affect timing but NOT scroll speed
3. **Density Changes**: During BPM changes, arrow density changes but scroll speed remains constant
4. **Visual Consistency**: Arrows maintain constant visual speed regardless of chart BPM

### BPM Change Impact

| BPM Section | Arrow Density | Scroll Speed | Visual Effect |
|-------------|---------------|--------------|---------------|
| 120 BPM | Normal | 1300 px/sec | Standard spacing |
| 240 BPM | Double | 1300 px/sec | Closer together |
| 60 BPM | Half | 1300 px/sec | Further apart |

## Draw Range Configuration

**Location**: `src/config.rs:204-206`

```rust
pub const MAX_DRAW_BEATS_FORWARD: f32 = 12.0;
pub const MAX_DRAW_BEATS_BACK: f32 = 3.0;
```

These constants control how many beats ahead and behind the current position arrows are rendered, affecting performance and visual range.

## Current System Limitations

### Missing Speed Modifiers

The current implementation lacks common rhythm game speed modifiers:

1. **No X-mods**: No speed multipliers (1x, 2x, 3x, etc.)
2. **No C-mods**: No constant speed independent of BPM
3. **No M-mods**: No BPM-based speed adjustment
4. **No User Configuration**: No options menu settings for speed

### Options System Status

**Location**: `src/screens/options.rs`

The options screen is currently a placeholder with no speed configuration options implemented.

## Technical Implementation Details

### Resolution Scaling Logic

The system ensures visual consistency across different screen sizes:

```rust
effective_speed = base_speed × (current_height / reference_height)
```

This maintains the same visual travel time regardless of display resolution.

### Time-Based Positioning

- Arrows are positioned based on **time difference** to target, not beat difference
- This ensures proper synchronization with audio regardless of BPM changes
- Position updates occur every frame using delta time calculations

### Performance Considerations

- Fixed scroll speed reduces computational complexity
- Time-based calculations ensure smooth animation
- Draw range limits prevent unnecessary rendering

## Comparison with Other Rhythm Games

### StepMania/ITG Features Missing

| Feature | StepMania/ITG | DeadSync |
|---------|---------------|----------|
| Speed Multipliers | 1x-8x+ | Fixed |
| C-mods | Yes | No |
| M-mods | Yes | No |
| User Config | Yes | No |
| BPM Independence | Optional | Always |

### DeadSync Advantages

1. **Simplicity**: Fixed speed system reduces complexity
2. **Consistency**: Always maintains visual speed
3. **Resolution Independence**: Automatic scaling
4. **Performance**: Minimal computational overhead

## Future Enhancement Opportunities

### Speed Modifier System

1. **X-mods**: Implement speed multipliers (0.5x, 1x, 1.5x, 2x, etc.)
2. **C-mods**: Add constant speed option (e.g., C450, C600)
3. **M-mods**: Add BPM-based speed adjustment
4. **User Interface**: Add speed settings to options menu

### Configuration System

1. **Per-Song Settings**: Save speed preferences per song
2. **Global Defaults**: Set default speed modifier
3. **Quick Adjust**: In-game speed adjustment (common in rhythm games)

### Advanced Features

1. **Acceleration**: Gradual speed changes during gameplay
2. **Speed Gimmicks**: Chart-defined speed changes
3. **Visual Modifiers**: Additional scroll effects (reverse, split, etc.)

## Code Locations Summary

| Function | File | Lines | Purpose |
|----------|------|-------|---------|
| Base Speed | `config.rs` | 32 | Defines ARROW_SPEED constant |
| Speed Calculation | `gameplay.rs` | 611-612, 716-717 | Calculates effective speed |
| Position Update | `gameplay.rs` | 620-621 | Updates arrow positions |
| Timing Integration | `gameplay.rs` | 616-617 | Handles BPM changes |
| Draw Range | `config.rs` | 204-206 | Controls rendering range |

This analysis provides a complete understanding of DeadSync's current scroll speed system and identifies opportunities for future enhancement to match the flexibility of other rhythm games.