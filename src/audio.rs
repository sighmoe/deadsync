use crate::assets::SoundId; // Use the SoundId enum from assets
use log::{error, info, warn};
use rodio::{source::Buffered, Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex}; // Use Mutex for internal sink management if needed

// Type alias for buffered sound effects
type SoundEffect = Buffered<Decoder<BufReader<File>>>;

pub struct AudioManager {
    // Keep the stream alive
    _stream: OutputStream,
    // Handle to the stream for creating sinks
    stream_handle: OutputStreamHandle,
    // Store loaded sound effects keyed by ID
    sfx_buffers: HashMap<SoundId, SoundEffect>,
    // Store the currently playing music sink (if any)
    // Use Arc<Mutex<>> if you need to control the sink from multiple threads/contexts safely
    // For simpler cases, Option<Sink> might suffice if managed carefully within App/AudioManager
    music_sink: Arc<Mutex<Option<Sink>>>,
}

impl AudioManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        info!("Initializing AudioManager...");
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to get default audio output stream: {}", e))?;
        info!("Audio output stream obtained.");
        Ok(AudioManager {
            _stream: stream, // Store the stream to keep it alive
            stream_handle,
            sfx_buffers: HashMap::new(),
            music_sink: Arc::new(Mutex::new(None)),
        })
    }

    /// Loads a sound effect and stores it for later playback.
    pub fn load_sfx(&mut self, id: SoundId, path: &Path) -> Result<(), Box<dyn Error>> {
        info!("Loading SFX '{:?}' from: {:?}", id, path);
        let file = File::open(path).map_err(|e| format!("Failed to open SFX {:?}: {}", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode SFX {:?}: {}", path, e))?;
        // Buffer the sound effect into memory for instant playback
        let buffered = source.buffered();
        self.sfx_buffers.insert(id, buffered);
        info!("SFX '{:?}' loaded and buffered.", id);
        Ok(())
    }

    /// Plays a loaded sound effect once.
    pub fn play_sfx(&self, id: SoundId) {
        if let Some(buffered_source) = self.sfx_buffers.get(&id) {
            match Sink::try_new(&self.stream_handle) {
                Ok(sink) => {
                    // Clone the buffered source (cheap operation)
                    sink.append(buffered_source.clone());
                    // Detach the sink to play in the background and clean itself up
                    sink.detach();
                    info!("Playing SFX '{:?}'", id);
                }
                Err(e) => {
                    error!("Failed to create temporary sink for SFX '{:?}': {}", id, e);
                }
            }
        } else {
            warn!("Attempted to play unloaded SFX: {:?}", id);
        }
    }

    /// Loads and starts playing music. Stops any previously playing music.
    pub fn play_music(&self, path: &Path, volume: f32) -> Result<(), Box<dyn Error>> {
        info!("Attempting to play music from: {:?}", path);
        // Stop previous music first
        self.stop_music();

        let file =
            File::open(path).map_err(|e| format!("Failed to open music file {:?}: {}", path, e))?;
        // Decode music on the fly (don't buffer large files)
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode music file {:?}: {}", path, e))?;

        // Create a new sink for the music
        match Sink::try_new(&self.stream_handle) {
            Ok(sink) => {
                sink.set_volume(volume.clamp(0.0, 2.0)); // Clamp volume
                sink.append(source); // Add the decoded source to the sink
                sink.play(); // Start playback

                // Store the new sink, replacing the old one
                let mut music_sink_guard = self
                    .music_sink
                    .lock()
                    .map_err(|_| "Failed to lock music sink mutex")?;
                *music_sink_guard = Some(sink);

                info!("Music playback started: {:?}", path);
                Ok(())
            }
            Err(e) => {
                error!("Failed to create sink for music: {}", e);
                Err(Box::new(e) as Box<dyn Error>)
            }
        }
    }

    /// Stops the currently playing music (if any).
    pub fn stop_music(&self) {
        info!("Stopping music...");
        // Lock the mutex to access the sink
        if let Ok(mut sink_guard) = self.music_sink.lock() {
            // take() removes the sink from the Option, returning it
            if let Some(sink) = sink_guard.take() {
                sink.stop(); // Stop playback
                info!("Music stopped.");
                // Sink is dropped here, releasing audio resources
            } else {
                info!("No music was playing.");
            }
        } else {
            error!("Failed to lock music sink mutex during stop.");
        }
    }

    // Optional: Add functions to pause, resume, set volume, etc.
    /*
    pub fn set_music_volume(&self, volume: f32) {
         if let Ok(sink_guard) = self.music_sink.lock() {
            if let Some(sink) = sink_guard.as_ref() {
                sink.set_volume(volume.clamp(0.0, 2.0));
                 info!("Set music volume to {}", volume);
            }
         } else {
              error!("Failed to lock music sink mutex during set_volume.");
         }
    } */
}

// Drop implementation ensures music stops if AudioManager is dropped
impl Drop for AudioManager {
    fn drop(&mut self) {
        info!("Dropping AudioManager, ensuring music is stopped.");
        self.stop_music();
    }
}
