use crate::assets::SoundId;
use log::{error, info, trace, warn};
use rodio::{
    buffer::SamplesBuffer,
    Decoder, OutputStream, OutputStreamHandle, Sink, Source,
};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader};
use std::path::{Path};
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
        let ogg_reader = OggStreamReader::new(file).map_err(|e: VorbisError| {
            format!(
                "Failed to create OGG reader for {}: {}",
                file_path.display(),
                e
            )
        })?;

        let channels = ogg_reader.ident_hdr.audio_channels;
        let sample_rate = ogg_reader.ident_hdr.audio_sample_rate;

        if channels == 0 || sample_rate == 0 {
            return Err(format!(
                "OGG file {} has invalid headers (channels: {}, sample_rate: {}).",
                file_path.display(),
                channels,
                sample_rate
            )
            .into());
        }

        Ok(LewtonOggSource {
            reader: ogg_reader,
            channels: channels as u16,
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
                    // Lewton might return empty packets, which should not be treated as EOF immediately.
                    if packet_data.is_empty() {
                        self.current_packet = None; // Mark as exhausted if empty and try to read next
                        continue;
                    }
                    self.current_packet = Some(packet_data);
                }
                Ok(None) => {
                    // End of stream
                    self.current_packet = None;
                    return None;
                }
                Err(e) => {
                    error!("Error decoding OGG packet in LewtonOggSource for unknown file (path not stored): {}", e);
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
        self.current_packet
            .as_ref()
            .map(|p| p.len().saturating_sub(self.current_packet_offset))
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
        None
    } // Lewton doesn't provide this easily
}

type SoundEffect = rodio::source::Buffered<Decoder<BufReader<File>>>;

pub struct AudioManager {
    _stream: OutputStream, // Keep stream alive
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
        info!("Loading SFX '{:?}' from: {}", id, path.display());
        let file = File::open(path)
            .map_err(|e| format!("Failed to open SFX {}: {}", path.display(), e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode SFX {}: {}", path.display(), e))?;
        self.sfx_buffers.insert(id, source.buffered());
        info!("SFX '{:?}' loaded and buffered.", id);
        Ok(())
    }

    pub fn play_sfx(&self, id: SoundId) {
        if let Some(buffered_source) = self.sfx_buffers.get(&id) {
            // sink.detach() means Rodio will manage its lifetime.
            if let Ok(sink) = Sink::try_new(&self.stream_handle) {
                sink.append(buffered_source.clone());
                sink.detach();
                trace!("Playing SFX '{:?}'", id); // Changed to trace for less log spam
            } else {
                error!(
                    "Failed to create temporary sink for SFX '{:?}': unknown error",
                    id
                );
            }
        } else {
            warn!("Attempted to play unloaded SFX: {:?}", id);
        }
    }

    fn create_source_from_path(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Source<Item = i16> + Send>, Box<dyn Error>> {
        if path
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("ogg"))
        {
            info!("Creating LewtonOggSource for: {}", path.display());
            LewtonOggSource::new(path).map(|s| Box::new(s) as Box<dyn Source<Item = i16> + Send>)
        } else {
            info!("Creating Rodio Decoder for: {}", path.display());
            let file = File::open(path)
                .map_err(|e| format!("Failed to open audio file {}: {}", path.display(), e))?;
            Decoder::new(BufReader::new(file))
                .map(|s| Box::new(s.convert_samples()) as Box<dyn Source<Item = i16> + Send>)
                .map_err(|e| {
                    format!("Failed to decode audio file {}: {}", path.display(), e).into()
                })
        }
    }

    fn play_to_sink(
        sink_arc_mutex: &Arc<Mutex<Option<Sink>>>,
        stream_handle: &OutputStreamHandle,
        source: Box<dyn Source<Item = i16> + Send>,
        volume: f32,
        path_for_log: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let new_sink = Sink::try_new(stream_handle).map_err(|e| {
            format!(
                "Failed to create sink for {}: {}",
                path_for_log.display(),
                e
            )
        })?;
        new_sink.set_volume(volume.clamp(0.0, 2.0));
        new_sink.append(source);
        new_sink.play();

        let mut sink_guard = sink_arc_mutex
            .lock()
            .map_err(|_| "Failed to lock sink mutex".to_string())?;
        if let Some(old_sink) = sink_guard.take() {
            old_sink.stop(); // Stop any previous playback on this sink
        }
        *sink_guard = Some(new_sink);
        info!("Playback started for: {}", path_for_log.display());
        Ok(())
    }

    fn stop_sink(sink_arc_mutex: &Arc<Mutex<Option<Sink>>>, sink_name: &str) {
        if let Ok(mut sink_guard) = sink_arc_mutex.lock() {
            if let Some(sink) = sink_guard.take() {
                // take() removes it
                sink.stop();
                info!("{} sink stopped.", sink_name);
            }
        } else {
            error!(
                "Failed to lock {} sink mutex during stop operation.",
                sink_name
            );
        }
    }

    pub fn play_music(&self, path: &Path, volume: f32) -> Result<(), Box<dyn Error>> {
        info!("Attempting to play music from: {}", path.display());
        Self::stop_sink(&self.music_sink, "Main music"); // Stop previous music
        Self::stop_sink(&self.preview_sink, "Preview music"); // Stop preview if it was playing

        let source = self.create_source_from_path(path)?;
        Self::play_to_sink(&self.music_sink, &self.stream_handle, source, volume, path)
    }

    pub fn stop_music(&self) {
        Self::stop_sink(&self.music_sink, "Main music");
    }

    pub fn play_preview(
        &self,
        path: &Path,
        volume: f32,
        start_sec: f32,
        duration_sec: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        info!(
            "Playing preview from: {}, start: {:.2}s, duration: {:?}",
            path.display(),
            start_sec,
            duration_sec
        );
        Self::stop_sink(&self.preview_sink, "Preview music"); // Stop previous preview

        if path
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("ogg"))
        {
            self.play_ogg_preview(path, volume, start_sec, duration_sec)
        } else {
            self.play_generic_preview(path, volume, start_sec, duration_sec)
        }
    }

    fn play_ogg_preview(
        &self,
        path: &Path,
        volume: f32,
        start_sec: f32,
        duration_sec: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        let file = File::open(path)
            .map_err(|e| format!("Failed to open OGG preview file {}: {}", path.display(), e))?;
        let mut ogg_reader = OggStreamReader::new(file).map_err(|e: VorbisError| {
            format!(
                "Failed to create OGG reader for preview file {}: {}",
                path.display(),
                e
            )
        })?;

        let channels = ogg_reader.ident_hdr.audio_channels;
        let sample_rate = ogg_reader.ident_hdr.audio_sample_rate;
        if channels == 0 || sample_rate == 0 {
            return Err(format!(
                "OGG preview file {} has invalid headers (channels: {}, sample_rate: {}).",
                path.display(),
                channels,
                sample_rate
            )
            .into());
        }

        if start_sec > 0.0 {
            let start_sample_frame = (start_sec * sample_rate as f32) as u64;
            if ogg_reader.seek_absgp_pg(start_sample_frame).is_err() {
                warn!("OGG preview: Seek to {:.2}s (frame {}) failed for {}. Playback may start from beginning or be incorrect.", start_sec, start_sample_frame, path.display());
            } else {
                info!(
                    "OGG preview: Seek to {:.2}s (frame {}) successful for {}.",
                    start_sec,
                    start_sample_frame,
                    path.display()
                );
            }
        }

        let mut collected_samples: Vec<i16> = Vec::new();
        let max_samples_to_collect_interleaved = if let Some(dur) = duration_sec {
            if dur <= 0.0 {
                warn!(
                    "OGG preview: Duration {:.2}s is non-positive for {}. Playing empty segment.",
                    dur,
                    path.display()
                );
                0
            } else {
                ((dur * sample_rate as f32) as u64 * channels as u64) as usize
            }
        } else {
            warn!(
                "OGG preview: No duration specified for {}. Reading up to ~10s.",
                path.display()
            );
            ((10.0 * sample_rate as f32) as u64 * channels as u64) as usize
        };

        if max_samples_to_collect_interleaved > 0 {
            collected_samples.reserve(max_samples_to_collect_interleaved);
            while collected_samples.len() < max_samples_to_collect_interleaved {
                match ogg_reader.read_dec_packet_itl() {
                    Ok(Some(mut packet_data)) => {
                        if packet_data.is_empty() {
                            continue;
                        }
                        let remaining_to_collect =
                            max_samples_to_collect_interleaved - collected_samples.len();
                        if packet_data.len() > remaining_to_collect {
                            packet_data.truncate(remaining_to_collect);
                        }
                        collected_samples.extend_from_slice(&packet_data);
                    }
                    Ok(None) => {
                        info!("OGG preview: EOF reached for {}.", path.display());
                        break;
                    }
                    Err(e) => {
                        error!(
                            "OGG preview: Error decoding packet for {}: {}",
                            path.display(),
                            e
                        );
                        break;
                    }
                }
            }
        }

        if collected_samples.is_empty() {
            info!(
                "OGG preview: No samples collected for {}. Sink will be empty.",
                path.display()
            );
            // Still "succeed" by playing nothing, consistent with rodio's empty source
        }

        let buffer_source = SamplesBuffer::new(channels as u16, sample_rate, collected_samples);
        Self::play_to_sink(
            &self.preview_sink,
            &self.stream_handle,
            Box::new(buffer_source),
            volume,
            path,
        )
    }

    fn play_generic_preview(
        &self,
        path: &Path,
        volume: f32,
        start_sec: f32,
        duration_sec: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        let file = File::open(path)
            .map_err(|e| format!("Failed to open preview file {}: {}", path.display(), e))?;
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode preview file {}: {}", path.display(), e))?;

        let mut source: Box<dyn Source<Item = i16> + Send> = Box::new(decoder.convert_samples());

        if start_sec > 0.0 {
            source = Box::new(source.skip_duration(Duration::from_secs_f32(start_sec)));
        }
        if let Some(dur) = duration_sec {
            if dur > 0.0 {
                source = Box::new(source.take_duration(Duration::from_secs_f32(dur)));
            } else {
                warn!(
                    "Rodio preview: Duration {:.2}s is non-positive for {}. Playing empty segment.",
                    dur,
                    path.display()
                );
                source = Box::new(rodio::source::Empty::new());
            }
        }
        Self::play_to_sink(
            &self.preview_sink,
            &self.stream_handle,
            source,
            volume,
            path,
        )
    }

    pub fn stop_preview(&self) {
        Self::stop_sink(&self.preview_sink, "Preview music");
    }

    pub fn is_preview_playing(&self) -> bool {
        self.preview_sink.lock().map_or(false, |guard| {
            guard.as_ref().map_or(false, |sink| !sink.empty())
        })
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        info!("Dropping AudioManager, ensuring all music is stopped.");
        self.stop_music();
        self.stop_preview();
    }
}