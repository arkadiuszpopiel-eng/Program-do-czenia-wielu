//! Wątki RT WASAPI: render (mikser → bufor urządzenia) i przechwytywanie (bufor → kolejka SPSC).
//! W pętli: zero alokacji (bufory przygotowane przed startem), zero blokad, zero logowania.
//! Czas: `IAudioClock::GetPosition` (pozycja + QPC w jednostkach 100 ns) i znacznik QPC pakietu
//! przechwytywania — ten sam zegar dla wejścia i wyjścia (spójne znaczniki dla AEC).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use voice_audio_contract::{
    AudioError, AudioEvent, AudioFormat, CaptureReader, CaptureWriter, DeviceId, DeviceStatus,
    Ducking, Frame, InputStream, LoopLatency, MediaTime, MixerConfig, MixerOutput, MixerRender,
    OutputStream, PlaybackPosition, SourceId, StreamConfig, capture_ring, mixer::mixer,
};
use wasapi::{AudioClient, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

use super::{EVENT_WAIT, com_init, map_err};
use crate::convert::{f32_to_le_bytes, le_bytes_to_f32};

/// Rodzaj przechwytywania.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureKind {
    /// Mikrofon.
    Microphone,
    /// Pętla zwrotna urządzenia wyjściowego (referencja awaryjna AEC).
    Loopback,
}

/// Wątek urządzenia; zatrzymywany i dołączany przy `drop`.
struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn open_client(
    device: Option<&DeviceId>,
    dir: Direction,
    format: AudioFormat,
    stream_dir: Direction,
    exclusive: bool,
) -> Result<(AudioClient, i64), AudioError> {
    let name = device.map_or_else(|| "domyślne".to_owned(), ToString::to_string);
    let enumerator = DeviceEnumerator::new().map_err(|e| map_err(e, &name))?;
    let dev = match device {
        Some(id) => enumerator.get_device(id.as_str()),
        None => enumerator.get_default_device(&dir),
    }
    .map_err(|e| map_err(e, &name))?;
    let mut client = dev.get_iaudioclient().map_err(|e| map_err(e, &name))?;
    let fmt = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        format.sample_rate as usize,
        usize::from(format.channels),
        None,
    );
    let (default_period, _min) = client.get_device_period().map_err(|e| map_err(e, &name))?;
    let mode = if exclusive && stream_dir == dir {
        StreamMode::EventsExclusive {
            period_hns: default_period,
        }
    } else {
        StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: default_period * 2,
        }
    };
    client
        .initialize_client(&fmt, &stream_dir, &mode)
        .map_err(|e| map_err(e, &name))?;
    Ok((client, default_period))
}

fn wait_ready(rx: &mpsc::Receiver<Result<(), AudioError>>) -> Result<(), AudioError> {
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| AudioError::Backend("urządzenie nie odpowiedziało w 5 s".into()))?
}

/// Otwiera wyjście: wątek renderu z mikserem.
pub(crate) fn open_render(
    device: Option<DeviceId>,
    cfg: StreamConfig,
) -> Result<Box<dyn OutputStream>, AudioError> {
    let rate = cfg.format.sample_rate;
    let (control, render) = mixer(MixerConfig {
        voice_capacity_ms: cfg.queue_ms,
        ..MixerConfig::new(rate)
    });
    let status = Arc::new(DeviceStatus::default());
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (st, sp) = (Arc::clone(&status), Arc::clone(&stop));
    let thread = std::thread::Builder::new()
        .name("alfa-audio-render".into())
        .spawn(move || render_thread(device, cfg, render, &st, &sp, &ready_tx))
        .map_err(|e| AudioError::Backend(e.to_string()))?;
    let worker = Worker {
        stop,
        thread: Some(thread),
    };
    wait_ready(&ready_rx)?;
    Ok(Box::new(WasapiOutput {
        inner: MixerOutput::new(control, cfg.format, status),
        _worker: worker,
    }))
}

fn render_thread(
    device: Option<DeviceId>,
    cfg: StreamConfig,
    mut render: MixerRender,
    status: &DeviceStatus,
    stop: &AtomicBool,
    ready: &mpsc::SyncSender<Result<(), AudioError>>,
) {
    com_init();
    let setup = || -> Result<_, AudioError> {
        let (client, _) = open_client(
            device.as_ref(),
            Direction::Render,
            cfg.format,
            Direction::Render,
            cfg.exclusive,
        )?;
        let h = client
            .set_get_eventhandle()
            .map_err(|e| map_err(e, "render"))?;
        let rc = client
            .get_audiorenderclient()
            .map_err(|e| map_err(e, "render"))?;
        let frames = client.get_buffer_size().map_err(|e| map_err(e, "render"))? as usize;
        let clock = client.get_audioclock().ok();
        Ok((client, h, rc, frames, clock))
    };
    let (client, h, rc, buffer_frames, clock) = match setup() {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let ch = usize::from(cfg.format.channels);
    let rate = u64::from(cfg.format.sample_rate);
    let freq = clock
        .as_ref()
        .and_then(|c| c.get_frequency().ok())
        .unwrap_or(0);
    let mut buf = vec![0f32; buffer_frames * ch];
    let mut bytes = vec![0u8; buffer_frames * ch * 4];
    let mut written: u64 = 0;
    let mut started = false;
    let _ = ready.send(Ok(()));
    while !stop.load(Ordering::Relaxed) {
        if started && h.wait_for_event(EVENT_WAIT.as_millis() as u32).is_err() {
            continue;
        }
        let Ok(avail) = client.get_available_space_in_frames() else {
            status.closed.store(true, Ordering::Relaxed);
            break;
        };
        let avail = (avail as usize).min(buffer_frames);
        // Chwila odtworzenia pierwszej próbki bloku = QPC(pozycja) + to, co jeszcze w buforze.
        let (latency, ts) = match clock.as_ref().and_then(|c| c.get_position().ok()) {
            Some((pos, qpc)) if freq > 0 => {
                let played = u128::from(pos) * u128::from(rate) / u128::from(freq);
                let pending =
                    u64::try_from(u128::from(written).saturating_sub(played)).unwrap_or(0);
                let lat = MediaTime::from_samples(pending, cfg.format.sample_rate);
                (
                    lat,
                    MediaTime(qpc.saturating_mul(100)).plus(Duration::from_nanos(lat.0)),
                )
            }
            _ => (
                MediaTime::ZERO,
                MediaTime::from_samples(written, cfg.format.sample_rate),
            ),
        };
        let out = &mut buf[..avail * ch];
        render.render(out, cfg.format.channels, ts);
        let n = f32_to_le_bytes(out, &mut bytes);
        if rc.write_to_device(avail, &bytes[..n], None).is_err() {
            status.closed.store(true, Ordering::Relaxed);
            break;
        }
        written += avail as u64;
        status.set_output_latency(Duration::from_nanos(latency.0));
        if !started {
            if client.start_stream().is_err() {
                status.closed.store(true, Ordering::Relaxed);
                break;
            }
            started = true;
        }
    }
    let _ = client.stop_stream();
}

/// Otwiera przechwytywanie (mikrofon albo pętla zwrotna wyjścia).
pub(crate) fn open_capture(
    device: Option<DeviceId>,
    cfg: StreamConfig,
    kind: CaptureKind,
) -> Result<Box<dyn InputStream>, AudioError> {
    let (writer, reader) = capture_ring(cfg.format, cfg.frame_ms, cfg.queue_ms);
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let sp = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("alfa-audio-capture".into())
        .spawn(move || capture_thread(device, cfg, kind, writer, &sp, &ready_tx))
        .map_err(|e| AudioError::Backend(e.to_string()))?;
    let worker = Worker {
        stop,
        thread: Some(thread),
    };
    wait_ready(&ready_rx)?;
    Ok(Box::new(WasapiInput {
        reader,
        _worker: worker,
    }))
}

fn capture_thread(
    device: Option<DeviceId>,
    cfg: StreamConfig,
    kind: CaptureKind,
    mut writer: CaptureWriter,
    stop: &AtomicBool,
    ready: &mpsc::SyncSender<Result<(), AudioError>>,
) {
    com_init();
    let dev_dir = match kind {
        CaptureKind::Microphone => Direction::Capture,
        CaptureKind::Loopback => Direction::Render,
    };
    let setup = || -> Result<_, AudioError> {
        let (client, period) = open_client(
            device.as_ref(),
            dev_dir,
            cfg.format,
            Direction::Capture,
            cfg.exclusive,
        )?;
        let h = client
            .set_get_eventhandle()
            .map_err(|e| map_err(e, "capture"))?;
        let cc = client
            .get_audiocaptureclient()
            .map_err(|e| map_err(e, "capture"))?;
        let frames = client
            .get_buffer_size()
            .map_err(|e| map_err(e, "capture"))? as usize;
        client.start_stream().map_err(|e| map_err(e, "capture"))?;
        Ok((client, h, cc, frames, period))
    };
    let (client, h, cc, buffer_frames, period) = match setup() {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let ch = usize::from(cfg.format.channels);
    let mut bytes = vec![0u8; buffer_frames * ch * 4];
    let mut samples = vec![0f32; buffer_frames * ch];
    writer.set_latency(Duration::from_nanos(
        u64::try_from(period).unwrap_or(0).saturating_mul(100),
    ));
    let _ = ready.send(Ok(()));
    'outer: while !stop.load(Ordering::Relaxed) {
        if h.wait_for_event(EVENT_WAIT.as_millis() as u32).is_err() {
            continue;
        }
        loop {
            match cc.get_next_packet_size() {
                Ok(Some(n)) if n > 0 => {}
                Ok(_) => break,
                Err(_) => break 'outer,
            }
            let Ok((frames, info)) = cc.read_from_device(&mut bytes) else {
                break 'outer;
            };
            let n = le_bytes_to_f32(&bytes[..frames as usize * ch * 4], &mut samples);
            let ts = MediaTime(info.timestamp.saturating_mul(100));
            if info.flags.silent {
                samples[..n].fill(0.0);
            }
            writer.write(&samples[..n], ts);
        }
    }
    let _ = client.stop_stream();
}

/// Wyjście WASAPI: mikser + wątek renderu.
struct WasapiOutput {
    inner: MixerOutput,
    _worker: Worker,
}

impl OutputStream for WasapiOutput {
    fn format(&self) -> AudioFormat {
        self.inner.format()
    }
    fn play(&mut self, source: &SourceId, utterance: u64, chunk: &Frame) -> Result<(), AudioError> {
        self.inner.play(source, utterance, chunk)
    }
    fn end_utterance(&mut self, utterance: u64) -> Result<(), AudioError> {
        self.inner.end_utterance(utterance)
    }
    fn duck(&mut self, ducking: Ducking) -> Result<(), AudioError> {
        self.inner.duck(ducking)
    }
    fn unduck(&mut self, release: Duration) -> Result<(), AudioError> {
        self.inner.unduck(release)
    }
    fn stop_all(&mut self) -> Result<(), AudioError> {
        self.inner.stop_all()
    }
    fn position(&mut self, utterance: u64) -> Option<PlaybackPosition> {
        self.inner.position(utterance)
    }
    fn poll_events(&mut self) -> Vec<AudioEvent> {
        self.inner.poll_events()
    }
    fn drain_reference(&mut self) -> Vec<Frame> {
        self.inner.drain_reference()
    }
    fn latency(&self) -> LoopLatency {
        self.inner.latency()
    }
    fn set_calibrated_loop(&mut self, loop_latency: Duration) {
        self.inner.set_calibrated_loop(loop_latency);
    }
    fn duck_gain(&self) -> f32 {
        self.inner.duck_gain()
    }
}

/// Wejście WASAPI: kolejka przechwytywania + wątek urządzenia.
struct WasapiInput {
    reader: CaptureReader,
    _worker: Worker,
}

impl InputStream for WasapiInput {
    fn format(&self) -> AudioFormat {
        self.reader.format()
    }
    fn read(&mut self) -> Option<Frame> {
        self.reader.read()
    }
    fn overruns(&self) -> u64 {
        self.reader.overruns()
    }
    fn latency(&self) -> Duration {
        self.reader.latency()
    }
}
