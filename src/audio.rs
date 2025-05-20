// src/audio.rs
use crate::assets::SoundId;
use log::{error, info, warn};
use rodio::{
    buffer::SamplesBuffer,
    // SkipDuration and TakeDuration are still used for non-OGG previews
    source::{SamplesConverter, SkipDuration, TakeDuration}, 
    Decoder, OutputStream, OutputStreamHandle, Sink, Source,
};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom}; // Added Seek, SeekFrom for LewtonOggSource
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lewton::inside_ogg::OggStreamReader;
use lewton::VorbisError;

// Custom Source for Lewton OGG streaming
struct LewtonOggSource {
    reader: OggStreamReader<File>,
    channels: u16,
    sample_rate: u32,
    current_packet: Option<Vec<i16>>,
    current_packet_offset: usize,
}

impl LewtonOggSource {
    fn new(file_path: &Path) -> Result<Self, Box<dyn Error>> {
        let file = File::open(file_path)?;
        let ogg_reader = OggStreamReader::new(file) // No mut needed here initially
            .map_err(|e: VorbisError| format!("Failed to create OGG reader for {:?}: {}", file_path, e))?;
        
        let channels = ogg_reader.ident_hdr.audio_channels;
        let sample_rate = ogg_reader.ident_hdr.audio_sample_rate;

        if channels == 0 || sample_rate == 0 {
            return Err(format!("OGG file {:?} has invalid headers (channels: {}, sample_rate: {}).", file_path.file_name().unwrap_or_default(), channels, sample_rate).into());
        }

        Ok(LewtonOggSource {
            reader: ogg_reader, // ogg_reader is moved here
            channels: channels as u16, // Corrected type
            sample_rate,
            current_packet: None,
            current_packet_offset: 0,
        })
    }
}

impl Iterator for LewtonOggSource {
    type Item = i16;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref packet) = self.current_packet {
                if self.current_packet_offset < packet.len() {
                    let sample = packet[self.current_packet_offset];
                    self.current_packet_offset += 1;
                    return Some(sample);
                }
            }
            // Current packet exhausted or no packet, get a new one
            self.current_packet_offset = 0;
            match self.reader.read_dec_packet_itl() {
                Ok(Some(packet_data)) => {
                    if packet_data.is_empty() {
                        self.current_packet = None; // Mark as exhausted if empty
                        continue; // Lewton might return empty packets
                    }
                    self.current_packet = Some(packet_data);
                    // Fall through to re-check current_packet in the next loop iteration
                }
                Ok(None) => { // End of stream
                    self.current_packet = None;
                    return None;
                }
                Err(e) => {
                    error!("Error decoding OGG packet in LewtonOggSource: {}", e);
                    self.current_packet = None;
                    return None; // Stop playback on error
                }
            }
        }
    }
}

impl Source for LewtonOggSource {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        // Return remaining samples in the current packet
        self.current_packet.as_ref().map(|p| p.len().saturating_sub(self.current_packet_offset))
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.channels
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        // Lewton doesn't provide total duration easily without scanning.
        None
    }
}


type SoundEffect = rodio::source::Buffered<Decoder<BufReader<File>>>;

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

        let new_sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create sink for music: {}", e))?;
        new_sink.set_volume(volume.clamp(0.0, 2.0));

        if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("ogg")) {
            info!("Playing music using LewtonOggSource for OGG: {:?}", path.file_name().unwrap_or_default());
            match LewtonOggSource::new(path) {
                Ok(source) => {
                    new_sink.append(source);
                }
                Err(e) => {
                    error!("Failed to create LewtonOggSource for {:?}: {}", path.file_name().unwrap_or_default(), e);
                    // Fallback or return error. For now, let's return the error.
                    return Err(e);
                }
            }
        } else {
            info!("Playing music using Rodio Decoder for: {:?}", path.file_name().unwrap_or_default());
            let file = File::open(path).map_err(|e| format!("Failed to open music file {:?}: {}", path, e))?;
            let source = Decoder::new(BufReader::new(file))
                .map_err(|e| format!("Failed to decode music file {:?}: {}", path, e))?;
            new_sink.append(source);
        }
        
        new_sink.play();
        let mut music_sink_guard = self.music_sink.lock()
            .map_err(|_| "Failed to lock music sink mutex for play_music")?;
        *music_sink_guard = Some(new_sink);
        info!("Music playback started for: {:?}", path.file_name().unwrap_or_default());
        Ok(())
    }

    pub fn stop_music(&self) {
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
            "Playing preview from: {:?}, start: {:.2}s, duration: {:?}", 
            path.file_name().unwrap_or_default(), start_sec, duration_sec
        );
        self.stop_preview(); 

        let new_sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create sink for preview: {}", e))?;
        new_sink.set_volume(volume.clamp(0.0, 2.0));

        if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("ogg")) {
            info!("Using Lewton for OGG preview: {:?}", path.file_name().unwrap_or_default());
            let file = File::open(path).map_err(|e| format!("Failed to open OGG preview file {:?}: {}", path, e))?;
            let mut ogg_reader = OggStreamReader::new(file)
                .map_err(|e: VorbisError| format!("Failed to create OGG reader for preview file {:?}: {}", path, e))?;

            let channels = ogg_reader.ident_hdr.audio_channels;
            let sample_rate = ogg_reader.ident_hdr.audio_sample_rate;

            if channels == 0 || sample_rate == 0 {
                return Err(format!("OGG preview file {:?} has invalid headers (channels: {}, sample_rate: {}).", path.file_name().unwrap_or_default(), channels, sample_rate).into());
            }

            if start_sec > 0.0 {
                let start_sample_frame = (start_sec * sample_rate as f32) as u64;
                match ogg_reader.seek_absgp_pg(start_sample_frame) {
                    Ok(()) => info!("OGG preview: Seek to {:.2}s (frame {}) successful.", start_sec, start_sample_frame),
                    Err(e) => warn!("OGG preview: Seek to {:.2}s (frame {}) failed: {}. Playback may start from beginning or be incorrect.", start_sec, start_sample_frame, e),
                }
            }

            let mut collected_samples: Vec<i16> = Vec::new();
            if let Some(dur) = duration_sec {
                if dur > 0.0 {
                    let max_frames_to_read = (dur * sample_rate as f32) as u64;
                    // Lewton returns interleaved samples, so max_samples is frames * channels
                    let max_samples_to_collect_interleaved = (max_frames_to_read * channels as u64) as usize; 
                    collected_samples.reserve(max_samples_to_collect_interleaved);

                    while collected_samples.len() < max_samples_to_collect_interleaved {
                        match ogg_reader.read_dec_packet_itl() {
                            Ok(Some(mut packet)) => {
                                if packet.is_empty() { continue; }
                                let remaining_to_collect = max_samples_to_collect_interleaved - collected_samples.len();
                                if packet.len() > remaining_to_collect {
                                    packet.truncate(remaining_to_collect);
                                }
                                collected_samples.extend_from_slice(&packet);
                            }
                            Ok(None) => { 
                                info!("OGG preview: EOF reached while collecting samples for {:?}.", path.file_name().unwrap_or_default());
                                break;
                            }
                            Err(e) => {
                                error!("OGG preview: Error decoding packet for {:?}: {}", path.file_name().unwrap_or_default(), e);
                                break;
                            }
                        }
                    }
                } else { 
                    warn!("OGG preview: Duration {:.2}s is non-positive. Playing empty segment.", dur);
                }
            } else { 
                warn!("OGG preview: No duration specified. Reading up to ~10s. Please provide duration_sec.");
                let fallback_duration_s = 10.0;
                let max_frames_to_read = (fallback_duration_s * sample_rate as f32) as u64;
                let max_samples_to_collect_interleaved = (max_frames_to_read * channels as u64) as usize;
                 collected_samples.reserve(max_samples_to_collect_interleaved);
                 while collected_samples.len() < max_samples_to_collect_interleaved {
                     match ogg_reader.read_dec_packet_itl() {
                        Ok(Some(mut packet)) => {
                            if packet.is_empty() { continue; }
                            let remaining_to_collect = max_samples_to_collect_interleaved - collected_samples.len();
                            if packet.len() > remaining_to_collect { packet.truncate(remaining_to_collect); }
                            collected_samples.extend_from_slice(&packet);
                        }
                        Ok(None) => break,
                        Err(e) => { error!("OGG preview (fallback duration): Error decoding packet for {:?}: {}", path.file_name().unwrap_or_default(), e); break; }
                     }
                 }
            }
            
            if !collected_samples.is_empty() {
                let buffer = SamplesBuffer::new(channels as u16, sample_rate, collected_samples); // Corrected: channels as u16
                new_sink.append(buffer);
                info!("OGG preview: Appended {} sample frames to sink for {:?}.", new_sink.len(), path.file_name().unwrap_or_default());
            } else {
                info!("OGG preview: No samples collected for {:?}. Sink remains empty.", path.file_name().unwrap_or_default());
            }

        } else { 
            info!("Using Rodio Decoder for preview: {:?}", path.file_name().unwrap_or_default());
            let file = File::open(path).map_err(|e| format!("Failed to open preview file {:?}: {}", path, e))?;
            let decoder = Decoder::new(BufReader::new(file))
                .map_err(|e| format!("Failed to decode preview file {:?}: {}", path, e))?;
            
            let mut source: Box<dyn Source<Item = i16> + Send> = Box::new(decoder.convert_samples());

            if start_sec > 0.0 {
                source = Box::new(source.skip_duration(Duration::from_secs_f32(start_sec)));
            }
            if let Some(dur) = duration_sec {
                if dur > 0.0 {
                    source = Box::new(source.take_duration(Duration::from_secs_f32(dur)));
                } else {
                     warn!("Rodio preview: Duration {:.2}s is non-positive. Playing empty segment.", dur);
                     source = Box::new(rodio::source::Empty::new());
                }
            }
            new_sink.append(source);
        }

        if new_sink.empty() {
             warn!("Preview sink is empty for {:?}, playback might not occur.", path.file_name().unwrap_or_default());
        }
        new_sink.play();
        let mut preview_sink_guard = self.preview_sink.lock()
            .map_err(|_| "Failed to lock preview_sink mutex for play_preview")?;
        *preview_sink_guard = Some(new_sink);
        info!("Preview playback initiated for: {:?}", path.file_name().unwrap_or_default());
        Ok(())
    }

    pub fn stop_preview(&self) {
        if let Ok(mut sink_guard) = self.preview_sink.lock() {
            if sink_guard.is_some() { 
                let sink = sink_guard.take().unwrap(); 
                sink.stop();
                info!("Preview stopped.");
            }
        } else {
            error!("Failed to lock preview_sink mutex during stop_preview.");
        }
    }

    pub fn is_preview_playing(&self) -> bool {
        if let Ok(sink_guard) = self.preview_sink.lock() {
            if let Some(sink) = sink_guard.as_ref() {
                return !sink.empty(); 
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