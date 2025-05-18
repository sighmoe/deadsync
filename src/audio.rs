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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type SoundEffect = Buffered<Decoder<BufReader<File>>>;

pub struct AudioManager {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sfx_buffers: HashMap<SoundId, SoundEffect>,
    music_sink: Arc<Mutex<Option<Sink>>>,
    preview_sink: Arc<Mutex<Option<Sink>>>,
    preloaded_preview_data: Arc<Mutex<Option<(PathBuf, SoundEffect)>>>, // Stores (path, data)
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
            preloaded_preview_data: Arc::new(Mutex::new(None)),
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
        self.stop_preview(); // Also stop preview and clear its preload

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

    pub fn preload_preview(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        info!("Preloading preview audio from: {:?}", path);
        let mut preloaded_data_guard = self.preloaded_preview_data.lock()
            .map_err(|_| "Failed to lock preloaded_preview_data mutex for preload")?;

        // If something is already preloaded, even if it's the same path, clear it to force re-evaluation.
        // Or, only reload if path is different. For simplicity, let's always try to load.
        *preloaded_data_guard = None; // Clear previous

        let file = File::open(path)
            .map_err(|e| format!("Failed to open preview file for preload {:?}: {}", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode preview file for preload {:?}: {}", path, e))?;
        
        let buffered = source.buffered(); // Buffer the entire source
        *preloaded_data_guard = Some((path.to_path_buf(), buffered));
        info!("Preview audio preloaded successfully: {:?}", path);
        Ok(())
    }

    pub fn clear_preloaded_preview(&self) {
        if let Ok(mut guard) = self.preloaded_preview_data.lock() {
            if guard.is_some() {
                info!("Clearing preloaded preview audio.");
                *guard = None;
            }
        } else {
            error!("Failed to lock preloaded_preview_data mutex for clear.");
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
        self.stop_preview(); // This will also clear any existing preloaded data that is NOT this path.
                             // If it IS this path, we want to use it.

        let source_to_play: Box<dyn Source<Item = i16> + Send>;

        // Try to use preloaded data if available and matches the path
        let mut preloaded_data_guard = self.preloaded_preview_data.lock()
            .map_err(|_| "Failed to lock preloaded_preview_data for play_preview check")?;
        
        if let Some((preloaded_path, preloaded_effect)) = preloaded_data_guard.as_ref() {
            if preloaded_path == path {
                info!("Using preloaded audio data for preview: {:?}", path);
                let cloned_effect = preloaded_effect.clone(); // Clone the buffered source

                let source_after_skip_temp: Box<dyn Source<Item = i16> + Send> = if start_sec > 0.0 {
                    Box::new(cloned_effect.skip_duration(Duration::from_secs_f32(start_sec)))
                } else {
                    //Rodio's Buffered<Decoder> is already SamplesConverter<Decoder, i16> if input is i16
                    //but to be safe and handle any decoder output:
                    Box::new(cloned_effect.convert_samples())
                };
                 source_to_play = if let Some(dur) = duration_sec {
                    if dur > 0.0 { Box::new(source_after_skip_temp.take_duration(Duration::from_secs_f32(dur))) } 
                    else { warn!("Preview sample length non-positive ({}s), playing preloaded from start without duration limit.", dur); source_after_skip_temp }
                } else { source_after_skip_temp };

            } else {
                // Preloaded data exists but for a different path, so clear it and load fresh.
                info!("Preloaded data for {:?} exists, but requesting {:?}. Clearing old and loading new.", preloaded_path, path);
                *preloaded_data_guard = None; // Clear the mismatched preload
                drop(preloaded_data_guard); // Release lock before new load

                let file = File::open(path).map_err(|e| format!("Failed to open preview file {:?}: {}", path, e))?;
                let decoder = Decoder::new(BufReader::new(file)).map_err(|e| format!("Failed to decode preview file {:?}: {}", path, e))?;
                let source_after_skip_temp: Box<dyn Source<Item = i16> + Send> = if start_sec > 0.0 {
                    Box::new(decoder.skip_duration(Duration::from_secs_f32(start_sec)))
                } else { Box::new(decoder.convert_samples()) };
                source_to_play = if let Some(dur) = duration_sec {
                    if dur > 0.0 { Box::new(source_after_skip_temp.take_duration(Duration::from_secs_f32(dur))) } 
                    else { warn!("Preview sample length non-positive ({}s), playing from start without duration limit.", dur); source_after_skip_temp }
                } else { source_after_skip_temp };
            }
        } else {
            // No preloaded data, load fresh
            drop(preloaded_data_guard); // Release lock

            let file = File::open(path).map_err(|e| format!("Failed to open preview file {:?}: {}", path, e))?;
            let decoder = Decoder::new(BufReader::new(file)).map_err(|e| format!("Failed to decode preview file {:?}: {}", path, e))?;
            let source_after_skip_temp: Box<dyn Source<Item = i16> + Send> = if start_sec > 0.0 {
                Box::new(decoder.skip_duration(Duration::from_secs_f32(start_sec)))
            } else { Box::new(decoder.convert_samples()) };
            source_to_play = if let Some(dur) = duration_sec {
                 if dur > 0.0 { Box::new(source_after_skip_temp.take_duration(Duration::from_secs_f32(dur))) }
                 else { warn!("Preview sample length non-positive ({}s), playing from start without duration limit.", dur); source_after_skip_temp }
            } else { source_after_skip_temp };
        }


        match Sink::try_new(&self.stream_handle) {
            Ok(sink) => {
                sink.set_volume(volume.clamp(0.0, 2.0));
                sink.append(source_to_play);
                sink.play();

                let mut preview_sink_guard = self.preview_sink.lock()
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
        // Also clear any preloaded preview data when stopping explicitly
        self.clear_preloaded_preview();
    }

    pub fn is_preview_playing(&self) -> bool {
        if let Ok(sink_guard) = self.preview_sink.lock() {
            if let Some(sink) = sink_guard.as_ref() {
                return !sink.empty() && sink.len() > 0; 
            }
        }
        false
    }

    pub fn is_preview_preloaded_for_path(&self, path: &Path) -> bool {
        if let Ok(guard) = self.preloaded_preview_data.lock() {
            if let Some((preloaded_path, _)) = guard.as_ref() {
                return preloaded_path == path;
            }
        }
        false
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        info!("Dropping AudioManager, ensuring all music is stopped.");
        self.stop_music();
        self.stop_preview(); // This will also clear preloaded_preview_data
    }
}