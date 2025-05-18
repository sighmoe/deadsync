use crate::assets::SoundId;
use log::{error, info, warn};
use rodio::{
    source::{Buffered, SamplesConverter, SkipDuration, TakeDuration},
    Decoder, OutputStream, OutputStreamHandle, Sink, Source,
};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type SoundEffect = Buffered<Decoder<BufReader<File>>>;

pub struct AudioManager {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sfx_buffers: HashMap<SoundId, SoundEffect>,
    music_sink: Arc<Mutex<Option<Sink>>>,
    preview_sink: Arc<Mutex<Option<Sink>>>,
}

impl AudioManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        info!("Initializing AudioManager...");
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to get default audio output stream: {}", e))?;
        info!("Audio output stream obtained.");
        Ok(AudioManager {
            _stream,
            stream_handle,
            sfx_buffers: HashMap::new(),
            music_sink: Arc::new(Mutex::new(None)),
            preview_sink: Arc::new(Mutex::new(None)),
        })
    }

    pub fn load_sfx(&mut self, id: SoundId, path: &Path) -> Result<(), Box<dyn Error>> {
        info!("Loading SFX '{:?}' from: {:?}", id, path);
        let file = File::open(path).map_err(|e| format!("Failed to open SFX {:?}: {}", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode SFX {:?}: {}", path, e))?;
        let buffered = source.buffered();
        self.sfx_buffers.insert(id, buffered);
        info!("SFX '{:?}' loaded and buffered.", id);
        Ok(())
    }

    pub fn play_sfx(&self, id: SoundId) {
        if let Some(buffered_source) = self.sfx_buffers.get(&id) {
            match Sink::try_new(&self.stream_handle) {
                Ok(sink) => {
                    sink.append(buffered_source.clone());
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

    pub fn play_music(&self, path: &Path, volume: f32) -> Result<(), Box<dyn Error>> {
        info!("Attempting to play music from: {:?}", path);
        self.stop_music();
        self.stop_preview();

        let file =
            File::open(path).map_err(|e| format!("Failed to open music file {:?}: {}", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode music file {:?}: {}", path, e))?;

        match Sink::try_new(&self.stream_handle) {
            Ok(sink) => {
                sink.set_volume(volume.clamp(0.0, 2.0));
                sink.append(source);
                sink.play();

                let mut music_sink_guard = self
                    .music_sink
                    .lock()
                    .map_err(|_| "Failed to lock music sink mutex for play_music")?;
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

    pub fn stop_music(&self) {
        info!("Stopping main music...");
        if let Ok(mut sink_guard) = self.music_sink.lock() {
            if let Some(sink) = sink_guard.take() {
                sink.stop();
                info!("Main music stopped.");
            }
        } else {
            error!("Failed to lock music sink mutex during stop_music.");
        }
    }

    pub fn play_preview(
        &self,
        path: &Path,
        volume: f32,
        start_sec: f32,
        duration_sec: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        info!(
            "Attempting to play preview from: {:?}, start: {:.2}s, duration: {:?}",
            path, start_sec, duration_sec
        );
        self.stop_preview();

        let file = File::open(path)
            .map_err(|e| format!("Failed to open preview file {:?}: {}", path, e))?;
        
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode preview file {:?}: {}", path, e))?;

        // Apply skip_duration if needed, then box it.
        let source_after_skip: Box<dyn Source<Item = i16> + Send> = if start_sec > 0.0 {
            Box::new(decoder.skip_duration(Duration::from_secs_f32(start_sec)))
        } else {
            Box::new(decoder.convert_samples()) // Convert to ensure type consistency for take_duration
        };
        
        // Apply take_duration if needed
        let final_source: Box<dyn Source<Item = i16> + Send> = if let Some(dur) = duration_sec {
            if dur > 0.0 {
                 Box::new(source_after_skip.take_duration(Duration::from_secs_f32(dur)))
            } else {
                warn!("Preview sample length is non-positive ({}s), playing from start without duration limit.", dur);
                source_after_skip // Return the source_after_skip directly
            }
        } else {
            source_after_skip // Return the source_after_skip directly
        };

        match Sink::try_new(&self.stream_handle) {
            Ok(sink) => {
                sink.set_volume(volume.clamp(0.0, 2.0));
                sink.append(final_source);
                sink.play();

                let mut preview_sink_guard = self
                    .preview_sink
                    .lock()
                    .map_err(|_| "Failed to lock preview_sink mutex for play_preview")?;
                *preview_sink_guard = Some(sink);

                info!("Preview playback started: {:?}", path);
                Ok(())
            }
            Err(e) => {
                error!("Failed to create sink for preview: {}", e);
                Err(Box::new(e) as Box<dyn Error>)
            }
        }
    }

    pub fn stop_preview(&self) {
        if let Ok(mut sink_guard) = self.preview_sink.lock() {
            if let Some(sink) = sink_guard.take() {
                sink.stop();
            }
        } else {
            error!("Failed to lock preview_sink mutex during stop_preview.");
        }
    }

    pub fn is_preview_playing(&self) -> bool {
        if let Ok(sink_guard) = self.preview_sink.lock() {
            if let Some(sink) = sink_guard.as_ref() {
                return !sink.empty() && sink.len() > 0; // Check len() > 0 as well for more robust "playing" status
            }
        }
        false
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        info!("Dropping AudioManager, ensuring all music is stopped.");
        self.stop_music();
        self.stop_preview();
    }
}