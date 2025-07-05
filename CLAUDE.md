# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## About DeadSync

DeadSync is a Rust-based rhythm game engine inspired by ITG (In The Groove) and StepMania. It uses Vulkan for graphics rendering and focuses on accurate sync and competitive performance. The engine handles .sm/.ssc simfiles and provides a complete gameplay experience with music selection, gameplay, and scoring.

## Build Commands

- `cargo build` - Build the project
- `cargo run` - Build and run the game
- `cargo check` - Check for compilation errors without building
- `cargo clippy` - Run linting checks
- `cargo fmt` - Format the code according to Rust standards
- `cargo test` - Run tests (if any exist)

## Core Architecture

### Main Application Structure

The application follows a state machine pattern with these main states:
- **Menu** - Main menu navigation
- **SelectMusic** - Song selection with music wheel and difficulty selection
- **Gameplay** - Active rhythm game play
- **Options** - Settings configuration
- **ScoreScreen** - Post-game results
- **Exiting** - Application shutdown

### Key Modules

- **`app.rs`** - Main application loop, event handling, and state management
- **`state.rs`** - All state structures and enums for the application
- **`graphics/`** - Vulkan-based rendering system
  - `vulkan_base.rs` - Low-level Vulkan initialization and management
  - `renderer.rs` - High-level rendering abstractions
  - `texture.rs` - Texture loading and management
  - `font.rs` - Text rendering with MSDF fonts
- **`parsing/`** - Simfile parsing and chart processing
  - `simfile.rs` - .sm/.ssc file parsing
  - `parse.rs` - Chart data processing
  - `bpm.rs` - BPM change handling
  - `graph.rs` - NPS (Notes Per Second) graph generation
  - `stats.rs` - Chart statistics calculation
- **`screens/`** - Individual screen implementations
  - `menu.rs` - Main menu screen
  - `select_music.rs` - Song selection screen
  - `gameplay.rs` - Rhythm game logic
  - `options.rs` - Settings screen
  - `score.rs` - Score display screen
- **`audio.rs`** - Audio management with Rodio
- **`assets.rs`** - Asset loading and management
- **`config.rs`** - Configuration constants

### Input System

The game uses a custom `VirtualKeyCode` enum for input abstraction:
- Arrow keys or DFJK for gameplay arrows
- Enter/Escape for menu navigation
- Modifier keys supported for advanced gameplay features

### Graphics Pipeline

Uses Vulkan with:
- MSDF font rendering for text
- Texture atlas management
- Dynamic banner loading for songs
- Real-time NPS graph generation
- Efficient batched rendering

### Audio System

Built on Rodio with:
- Music playback with seeking
- Preview audio for song selection
- .ogg file support via Lewton
- Precise timing for rhythm gameplay

## Development Notes

### File Organization

- Songs are stored in `songs/` directory with pack structure
- Assets (fonts, graphics, sounds) in `assets/` directory
- Shaders in `shaders/` directory (pre-compiled SPIR-V)

### State Management

The application uses a centralized state machine where:
- `current_app_state` tracks the active screen
- `next_app_state` queues state transitions
- Each state has its own struct in `state.rs`
- State transitions handle resource cleanup and initialization

### Performance Considerations

- Vulkan rendering for GPU efficiency
- Texture caching and reuse
- Debounced window resizing
- Efficient chart processing with pre-computed data

### Testing

Standard Rust testing with `cargo test`. The codebase focuses on gameplay accuracy and performance rather than extensive unit testing.