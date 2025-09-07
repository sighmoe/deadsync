// FILE: src/core/audio.rs
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig};
use lewton::inside_ogg::OggStreamReader;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

// --- Public API Structs ---
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    pub start_sec: f64,
    pub length_sec: f64,
}

impl Default for Cut {
    fn default() -> Self {
        Self { start_sec: 0.0, length_sec: f64::INFINITY }
    }
}

// --- Engine Commands ---
enum AudioCommand {
    PlaySfx(Arc<Vec<i16>>),
    PlayMusic(PathBuf, Cut),
    StopMusic,
}

// --- Global State ---
static mut STREAM: Option<Stream> = None;
static ENGINE: Lazy<AudioEngine> = Lazy::new(init_engine_and_thread);

struct AudioEngine {
    command_sender: Sender<AudioCommand>,
    sfx_cache: Mutex<HashMap<String, Arc<Vec<i16>>>>,
    device_sample_rate: u32,
    device_channels: usize,
}

/// A handle to a streaming music track.
struct MusicStream {
    thread: thread::JoinHandle<()>,
    stop_signal: Arc<std::sync::atomic::AtomicBool>,
}

// --- Public API ---

/// Initializes the audio engine. Must be called once at startup.
pub fn init() -> Result<(), String> {
    Lazy::force(&ENGINE);
    Ok(())
}

/// Plays a sound effect from the given path.
pub fn play_sfx(path: &str) {
    let sound_data = {
        let mut cache = ENGINE.sfx_cache.lock().unwrap();
        if let Some(data) = cache.get(path) {
            data.clone()
        } else {
            match load_and_resample_sfx(path) {
                Ok(data) => {
                    cache.insert(path.to_string(), data.clone());
                    info!("Cached SFX: {}", path);
                    data
                }
                Err(e) => {
                    warn!("Failed to load SFX '{}': {}", path, e);
                    return;
                }
            }
        }
    };
    let _ = ENGINE.command_sender.send(AudioCommand::PlaySfx(sound_data));
}

/// Plays a music track from a file path.
#[allow(dead_code)]
pub fn play_music(path: PathBuf, cut: Cut) {
    let _ = ENGINE.command_sender.send(AudioCommand::PlayMusic(path, cut));
}

/// Stops the currently playing music track.
#[allow(dead_code)]
pub fn stop_music() {
    let _ = ENGINE.command_sender.send(AudioCommand::StopMusic);
}

// --- Engine Implementation ---

fn init_engine_and_thread() -> AudioEngine {
    let (command_sender, command_receiver) = channel();

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no audio output device");
    let config = device.default_output_config().expect("no default audio config");
    let stream_config: StreamConfig = config.clone().into();

    let device_sample_rate = stream_config.sample_rate.0;
    let device_channels = stream_config.channels as usize;

    thread::spawn(move || {
        audio_manager_thread(command_receiver);
    });

    info!("Audio engine initialized ({} Hz, {} ch).", device_sample_rate, device_channels);
    AudioEngine {
        command_sender,
        sfx_cache: Mutex::new(HashMap::new()),
        device_sample_rate,
        device_channels,
    }
}

/// Manages audio state, commands, and the CPAL stream from a dedicated thread.
fn audio_manager_thread(command_receiver: Receiver<AudioCommand>) {
    let mut music_stream: Option<MusicStream> = None;
    let music_ring = internal::ring_new(internal::RING_CAP_SAMPLES);
    let (sfx_sender, sfx_receiver) = channel();

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no audio output device");
    let config = device.default_output_config().expect("no default audio config");
    let stream_config: StreamConfig = config.clone().into();
    
    let mut active_sfx_for_callback = Vec::new();
    let music_ring_for_callback = music_ring.clone();

    let stream = device.build_output_stream(
        &stream_config,
        move |data: &mut [f32], _| {
            // This is the real-time audio callback.
            for new_sfx in sfx_receiver.try_iter() {
                active_sfx_for_callback.push((new_sfx, 0));
            }
            audio_callback_mixer(data, &music_ring_for_callback, &mut active_sfx_for_callback);
        },
        |err| error!("Audio stream error: {}", err),
        None,
    ).expect("Failed to build audio stream");
    stream.play().expect("Failed to play audio stream");
    
    // This thread now manages state and keeps the stream alive.
    loop {
        match command_receiver.recv() {
            Ok(AudioCommand::PlaySfx(data)) => { let _ = sfx_sender.send(data); },
            Ok(AudioCommand::PlayMusic(path, cut)) => {
                if let Some(old) = music_stream.take() {
                    old.stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = old.thread.join();
                }
                music_stream = Some(spawn_music_decoder_thread(path, cut, music_ring.clone()));
            }
            Ok(AudioCommand::StopMusic) => {
                if let Some(old) = music_stream.take() {
                    old.stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = old.thread.join();
                }
            }
            Err(_) => break, // Main thread disconnected, so we exit.
        }
    }
}

/// Mixes all audio sources directly in the audio callback for low latency.
fn audio_callback_mixer(
    out_buffer: &mut [f32],
    music_ring: &Arc<internal::SpscRingI16>,
    active_sfx: &mut Vec<(Arc<Vec<i16>>, usize)>,
) {
    let mut mix_buffer_i16 = vec![0i16; out_buffer.len()];
    internal::callback_fill_from_ring_i16(music_ring, &mut mix_buffer_i16);

    active_sfx.retain_mut(|(data, cursor)| {
        let samples_to_mix = (data.len() - *cursor).min(mix_buffer_i16.len());
        for i in 0..samples_to_mix {
            mix_buffer_i16[i] = mix_buffer_i16[i].saturating_add(data[*cursor + i]);
        }
        *cursor += samples_to_mix;
        *cursor < data.len()
    });

    for (out_sample, mix_sample) in out_buffer.iter_mut().zip(mix_buffer_i16) {
        *out_sample = mix_sample.to_sample::<f32>();
    }
}


/// Spawns a thread to decode and resample a single music file.
fn spawn_music_decoder_thread(path: PathBuf, cut: Cut, ring: Arc<internal::SpscRingI16>) -> MusicStream {
    let stop_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_signal_clone = stop_signal.clone();

    let thread = thread::spawn(move || {
        if let Err(e) = music_decoder_thread_loop(path, cut, ring, stop_signal_clone) {
            error!("Music decoder thread failed: {}", e);
        }
    });

    MusicStream { thread, stop_signal }
}

/// The logic for a music decoder thread.
fn music_decoder_thread_loop(
    path: PathBuf, cut: Cut, ring: Arc<internal::SpscRingI16>, stop: Arc<std::sync::atomic::AtomicBool>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file = File::open(&path)?;
    let mut ogg = OggStreamReader::new(BufReader::new(file))?;
    let in_ch = ogg.ident_hdr.audio_channels as usize;
    let in_hz = ogg.ident_hdr.audio_sample_rate;

    let out_ch = ENGINE.device_channels;
    let out_hz = ENGINE.device_sample_rate;

    let mut st = internal::poly_init(in_hz, out_hz, in_ch, out_ch, internal::BASE_TAPS, internal::BETA);
    
    let start_frame_f = (cut.start_sec * in_hz as f64).max(0.0);
    let start_floor = start_frame_f.floor() as u64;
    let mut to_drop_in = if start_floor > 0 {
        if ogg.seek_absgp_pg(start_floor).is_err() { start_floor } else { 0 }
    } else { 0 };

    let mut frames_left_out: Option<u64> = if cut.length_sec.is_finite() {
        Some((cut.length_sec * out_hz as f64).round().max(0.0) as u64)
    } else { None };

    let mut out_tmp = Vec::with_capacity(1 << 15);

    while let Ok(pkt_opt) = ogg.read_dec_packet_itl() {
        if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
        
        let p = match pkt_opt { Some(p) if !p.is_empty() => p, _ => continue };
        let mut slice = &p[..];

        if to_drop_in > 0 {
            let pkt_frames = (p.len() / in_ch) as u64;
            if to_drop_in >= pkt_frames { to_drop_in -= pkt_frames; continue; }
            slice = &p[(to_drop_in as usize * in_ch)..];
            to_drop_in = 0;
        }

        out_tmp.clear();
        internal::poly_push_produce(&mut st, slice, &mut out_tmp);

        if let Some(left) = &mut frames_left_out {
            let frames_produced = (out_tmp.len() / out_ch) as u64;
            if *left == 0 { break; }
            if frames_produced > *left {
                out_tmp.truncate(*left as usize * out_ch);
                *left = 0;
            } else { *left -= frames_produced; }
        }

        if !out_tmp.is_empty() {
            let mut written = 0;
            while written < out_tmp.len() {
                if stop.load(std::sync::atomic::Ordering::Relaxed) { return Ok(()); }
                let pushed = internal::ring_push(&ring, &out_tmp[written..]);
                if pushed == 0 { thread::sleep(std::time::Duration::from_micros(300)); }
                written += pushed;
            }
        }
        if frames_left_out == Some(0) { break; }
    }
    Ok(())
}

/// Loads an Ogg file fully into memory and resamples it to the output device's rate.
fn load_and_resample_sfx(path: &str) -> Result<Arc<Vec<i16>>, Box<dyn std::error::Error>> {
    let file = File::open(Path::new(path))?;
    let mut ogg = OggStreamReader::new(BufReader::new(file))?;
    let in_ch = ogg.ident_hdr.audio_channels as usize;
    let in_hz = ogg.ident_hdr.audio_sample_rate;
    
    let mut st = internal::poly_init(in_hz, ENGINE.device_sample_rate, in_ch, ENGINE.device_channels, internal::BASE_TAPS, internal::BETA);
    let mut resampled_data = Vec::new();
    
    while let Some(pkt) = ogg.read_dec_packet_itl()? {
        internal::poly_push_produce(&mut st, &pkt, &mut resampled_data);
    }
    internal::poly_push_produce(&mut st, &[], &mut resampled_data); // flush

    Ok(Arc::new(resampled_data))
}

// --- Internal Implementation Details (from your oggplay example) ---
mod internal {
    use super::*;
    use std::cell::UnsafeCell;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub const BASE_TAPS: usize = 8;
    pub const BETA: f64 = 8.0;
    pub const RING_CAP_SAMPLES: usize = 1 << 18;

    pub struct SpscRingI16 { buf: UnsafeCell<Box<[i16]>>, mask: usize, head: AtomicUsize, tail: AtomicUsize }
    unsafe impl Send for SpscRingI16 {}
    unsafe impl Sync for SpscRingI16 {}

    pub fn ring_new(cap_pow2: usize) -> Arc<SpscRingI16> {
        Arc::new(SpscRingI16 {
            buf: UnsafeCell::new(vec![0i16; cap_pow2].into_boxed_slice()),
            mask: cap_pow2 - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        })
    }
    
    pub fn ring_push(r: &SpscRingI16, data: &[i16]) -> usize {
        let cap = unsafe { (&*r.buf.get()).len() }; let mask = r.mask;
        let h = r.head.load(Ordering::Relaxed); let t = r.tail.load(Ordering::Acquire);
        let free = cap - h.wrapping_sub(t);
        let n = data.len().min(free); if n == 0 { return 0; }
        let idx = h & mask;
        unsafe {
            let buf = &mut *r.buf.get();
            let first = (cap - idx).min(n);
            buf[idx..idx + first].copy_from_slice(&data[..first]);
            if n > first { buf[0..(n - first)].copy_from_slice(&data[first..n]); }
        }
        r.head.store(h.wrapping_add(n), Ordering::Release); n
    }

    pub fn ring_pop(r: &SpscRingI16, out: &mut [i16]) -> usize {
        let cap = unsafe { (&*r.buf.get()).len() }; let mask = r.mask;
        let h = r.head.load(Ordering::Acquire); let t = r.tail.load(Ordering::Relaxed);
        let avail = h.wrapping_sub(t);
        let n = out.len().min(avail); if n == 0 { return 0; }
        let idx = t & mask;
        unsafe {
            let buf = &*r.buf.get();
            let first = (cap - idx).min(n);
            out[..first].copy_from_slice(&buf[idx..idx + first]);
            if n > first { out[first..n].copy_from_slice(&buf[0..(n - first)]); }
        }
        r.tail.store(t.wrapping_add(n), Ordering::Release); n
    }

    pub fn callback_fill_from_ring_i16(ring: &SpscRingI16, dst: &mut [i16]) {
        let mut filled = 0;
        while filled < dst.len() {
            let got = ring_pop(ring, &mut dst[filled..]);
            if got == 0 { dst[filled..].iter_mut().for_each(|s| *s = 0); break; }
            filled += got;
        }
    }

    #[inline(always)] fn gcd(mut a: u32, mut b: u32) -> u32 { while b != 0 { let r = a % b; a = b; b = r; } a }
    #[inline(always)] fn reduce_ratio(out_hz: u32, in_hz: u32) -> (u32, u32) { let g = gcd(out_hz, in_hz); (out_hz / g, in_hz / g) }
    
    #[inline(always)] fn i0(mut x: f64) -> f64 { x*=0.5; let (mut t, mut s)=(1.0,1.0); for k in 1..=10 { t *= (x*x)/((k as f64)*(k as f64)); s+=t; } s }

    fn design_kaiser_sinc(n: usize, fc: f64, beta: f64) -> Vec<f64> {
        let m = (n - 1) as f64 / 2.0; let denom = i0(beta); let two_fc = 2.0 * fc;
        (0..n).map(|u| {
            let x = u as f64 - m;
            let sinc = if x == 0.0 { 1.0 } else { (std::f64::consts::PI * two_fc * x).sin() / (std::f64::consts::PI * x) };
            let w = i0(beta * (1.0 - (x / m).powi(2)).max(0.0).sqrt()) / denom;
            two_fc * sinc * w
        }).collect()
    }

    fn build_polyphase(l: usize, m: usize, base_taps: usize, beta: f64) -> (Vec<f32>, usize) {
        let n = base_taps * l;
        let mut h = design_kaiser_sinc(n, 0.5f64 / (l.max(m) as f64), beta);
        let scale = (l as f64) / h.iter().sum::<f64>();
        h.iter_mut().for_each(|v| *v *= scale);
        let tpp = n / l;
        let mut phases = vec![0.0f32; n];
        for p in 0..l {
            let mut t: Vec<f32> = (0..tpp).map(|k| h[p + k * l] as f32).collect();
            t.reverse();
            phases[p * tpp .. (p+1)*tpp].copy_from_slice(&t);
        }
        (phases, tpp)
    }
    
    #[inline(always)] fn dot8(a: &[f32;8], b: &[f32;8]) -> f32 { (0..8).map(|i| a[i]*b[i]).sum() }

    pub struct PolyState { l: usize, m: usize, in_ch: usize, out_ch: usize, tpp: usize, phase: usize, phases: Vec<f32>, delay8: Option<Vec<[f32;8]>>, delay: Vec<f32>, inbuf: VecDeque<f32>, mapped: Vec<i16>, acc_frame: Vec<f32> }
    
    pub fn poly_init(in_hz: u32, out_hz: u32, in_ch: usize, out_ch: usize, base_taps: usize, beta: f64) -> PolyState {
        let (l, m) = reduce_ratio(out_hz, in_hz);
        let (phases, tpp) = build_polyphase(l as usize, m as usize, base_taps, beta);
        PolyState {
            l: l as usize, m: m as usize, in_ch, out_ch, tpp, phase: 0, phases,
            delay8: if tpp == 8 { Some(vec![[0.0;8]; in_ch]) } else { None },
            delay:  if tpp != 8 { vec![0.0; in_ch * tpp] } else { Vec::new() },
            inbuf: VecDeque::new(), mapped: vec![0; out_ch], acc_frame: vec![0.0; in_ch],
        }
    }

    #[inline(always)] fn poly_need_input(st: &PolyState) -> bool { st.phase >= st.l }

    fn poly_shift_in(st: &mut PolyState) -> bool {
        if st.inbuf.len() < st.in_ch { return false; }
        if let Some(d8) = &mut st.delay8 {
            (0..st.in_ch).for_each(|c| { d8[c].rotate_right(1); d8[c][0] = st.inbuf.pop_front().unwrap(); });
        } else {
            (0..st.in_ch).for_each(|c| { st.delay[c*st.tpp..].rotate_right(1); st.delay[c*st.tpp] = st.inbuf.pop_front().unwrap(); });
        }
        true
    }

    pub fn poly_push_produce(st: &mut PolyState, input: &[i16], out_tmp: &mut Vec<i16>) {
        st.inbuf.extend(input.iter().map(|&s| s as f32 / 32768.0));
        loop {
            while poly_need_input(st) {
                st.phase -= st.l; if !poly_shift_in(st) { return; }
            }
            if st.tpp == 8 {
                let d8 = st.delay8.as_ref().unwrap(); let p = st.phase;
                let coeffs: &[f32;8] = st.phases[p*8..(p+1)*8].try_into().unwrap();
                (0..st.in_ch).for_each(|c| st.acc_frame[c] = dot8(coeffs, &d8[c]));
            } else {
                let p = st.phase; let tpp = st.tpp;
                let coeffs = &st.phases[p*tpp..(p+1)*tpp];
                (0..st.in_ch).for_each(|c| st.acc_frame[c] = (0..tpp).map(|k| coeffs[k] * st.delay[c*tpp + k]).sum());
            }

            (0..st.out_ch).for_each(|c| st.mapped[c] = (st.acc_frame[c % st.in_ch] * 32767.0).clamp(-32768.0, 32767.0) as i16);
            out_tmp.extend_from_slice(&st.mapped);

            st.phase += st.m;
            if poly_need_input(st) && st.inbuf.len() < st.in_ch { return; }
        }
    }
}
