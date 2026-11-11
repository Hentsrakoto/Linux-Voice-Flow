//! CPAL streams never cross threads: the dedicated owner handles lifecycle and timeout.
use super::*;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SizedSample,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

enum Command {
    List(mpsc::Sender<Result<Vec<Microphone>, AudioError>>),
    Start(String, mpsc::Sender<Result<(), AudioError>>),
    Stop(mpsc::Sender<Result<AudioBuffer, AudioError>>),
    Discard(mpsc::Sender<()>),
    Quit,
}
pub struct NativeAudioRecorder {
    tx: mpsc::Sender<Command>,
    worker: Option<thread::JoinHandle<()>>,
    recording: Arc<AtomicBool>,
    error: Arc<AtomicU8>,
}
fn error_for(code: u8) -> AudioError {
    match code {
        1 => AudioError::new("audio_limit", "Recording reached sixty seconds"),
        _ => AudioError::new(
            "audio_capture",
            "Microphone disconnected or capture incomplete",
        ),
    }
}
impl NativeAudioRecorder {
    pub fn new() -> Result<Self, AudioError> {
        let (tx, rx) = mpsc::channel();
        let recording = Arc::new(AtomicBool::new(false));
        let error = Arc::new(AtomicU8::new(0));
        let (r, e) = (recording.clone(), error.clone());
        let worker = thread::Builder::new()
            .name("microphone-owner".into())
            .spawn(move || worker(rx, r, e))
            .map_err(|_| AudioError::new("audio_capture", "Cannot start microphone owner"))?;
        Ok(Self {
            tx,
            worker: Some(worker),
            recording,
            error,
        })
    }
    fn request<T>(
        &self,
        command: impl FnOnce(mpsc::Sender<T>) -> Command,
    ) -> Result<T, AudioError> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(command(tx))
            .map_err(|_| AudioError::new("audio_capture", "Microphone owner unavailable"))?;
        rx.recv()
            .map_err(|_| AudioError::new("audio_capture", "Microphone owner unavailable"))
    }
    pub fn list_devices(&self) -> Result<Vec<Microphone>, AudioError> {
        self.request(Command::List)?
    }
    pub fn start(&mut self, device_id: &str) -> Result<(), AudioError> {
        self.request(|tx| Command::Start(device_id.into(), tx))?
    }
    pub fn stop(&mut self) -> Result<AudioBuffer, AudioError> {
        self.request(Command::Stop)?
    }
    pub fn discard(&mut self) {
        let _ = self.request(Command::Discard);
    }
    pub fn status(&self) -> bool {
        self.recording.load(Ordering::Acquire)
    }
    pub fn poll_error(&self) -> Option<AudioError> {
        let code = self.error.load(Ordering::Acquire);
        (code != 0).then(|| error_for(code))
    }
}
impl Drop for NativeAudioRecorder {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
struct Session {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<CaptureBuffer>>,
    rate: u32,
    deadline: Instant,
}
fn devices() -> Result<Vec<Microphone>, AudioError> {
    let host = cpal::default_host();
    let mut result = Vec::new();
    if host.default_input_device().is_some() {
        result.push(Microphone {
            id: "default".into(),
            name: "Microphone par défaut du système".into(),
            is_default: true,
        });
    }
    for device in host
        .input_devices()
        .map_err(|_| AudioError::new("audio_device", "Cannot enumerate microphones"))?
    {
        let name = device
            .name()
            .map_err(|_| AudioError::new("audio_device", "Cannot read microphone name"))?;
        // Name IDs survive reordered enumeration. Duplicates remain visible but cannot be selected ambiguously.
        result.push(Microphone {
            id: format!("name:{name}"),
            name,
            is_default: false,
        });
    }
    Ok(result)
}
fn select_device(id: &str) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();
    if id == "default" {
        return host
            .default_input_device()
            .ok_or_else(|| AudioError::new("audio_device", "No default microphone"));
    }
    let mut matched = host
        .input_devices()
        .map_err(|_| AudioError::new("audio_device", "Cannot enumerate microphones"))?
        .filter(|d| d.name().is_ok_and(|name| format!("name:{name}") == id));
    let device = matched
        .next()
        .ok_or_else(|| AudioError::new("audio_device", "Selected microphone unavailable"))?;
    if matched.next().is_some() {
        return Err(AudioError::new(
            "audio_device",
            "Ambiguous microphone name; use system default",
        ));
    }
    Ok(device)
}
fn stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<CaptureBuffer>>,
    error: Arc<AtomicU8>,
    deadline: Instant,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + Copy,
    f32: cpal::FromSample<T>,
{
    let callback_error = error.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if Instant::now() >= deadline {
                    callback_error
                        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                        .ok();
                    return;
                }
                if callback_error.load(Ordering::Relaxed) != 0 {
                    return;
                }
                match buffer.try_lock() {
                    Ok(mut buffer) => {
                        buffer.push(data, |s| s.to_sample::<f32>());
                        if buffer.failed {
                            callback_error.store(2, Ordering::Release);
                        }
                    }
                    Err(_) => {
                        callback_error.store(2, Ordering::Release);
                    }
                }
            },
            move |_| {
                error.store(2, Ordering::Release);
            },
            None,
        )
        .map_err(|_| AudioError::new("audio_capture", "Cannot open microphone stream"))
}
fn begin(id: &str, error: Arc<AtomicU8>) -> Result<Session, AudioError> {
    let device = select_device(id)?;
    let supported = device
        .default_input_config()
        .map_err(|_| AudioError::new("audio_device", "No supported microphone configuration"))?;
    let config: cpal::StreamConfig = supported.clone().into();
    let buffer = Arc::new(Mutex::new(CaptureBuffer::new(
        config.sample_rate.0,
        config.channels as usize,
    )?));
    let deadline = Instant::now() + Duration::from_secs(MAX_SECONDS as u64);
    macro_rules! build {
        ($t:ty) => {
            stream::<$t>(&device, &config, buffer.clone(), error.clone(), deadline)?
        };
    }
    let stream = match supported.sample_format() {
        cpal::SampleFormat::I8 => build!(i8),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::I32 => build!(i32),
        cpal::SampleFormat::I64 => build!(i64),
        cpal::SampleFormat::U8 => build!(u8),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::U32 => build!(u32),
        cpal::SampleFormat::U64 => build!(u64),
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::F64 => build!(f64),
        _ => {
            return Err(AudioError::new(
                "audio_format",
                "Unsupported microphone sample format",
            ))
        }
    };
    stream
        .play()
        .map_err(|_| AudioError::new("audio_capture", "Cannot start microphone stream"))?;
    Ok(Session {
        stream: Some(stream),
        buffer,
        rate: config.sample_rate.0,
        deadline,
    })
}
fn enforce_deadline(s: &mut Session, recording: &AtomicBool, error: &AtomicU8, now: Instant) {
    if now >= s.deadline {
        error
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .ok();
    }
    if error.load(Ordering::Acquire) != 0 {
        s.stream.take();
        recording.store(false, Ordering::Release);
        // Release captured voice immediately on timeout or capture error.
        if let Ok(mut b) = s.buffer.lock() {
            b.samples.zeroize();
            b.samples.clear();
            b.failed = true;
        }
    }
}
fn worker(rx: mpsc::Receiver<Command>, recording: Arc<AtomicBool>, error: Arc<AtomicU8>) {
    let mut session: Option<Session> = None;
    loop {
        if let Some(s) = session.as_mut() {
            enforce_deadline(s, &recording, &error, Instant::now());
        }
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(Command::List(tx)) => {
                let result = if session.is_some() {
                    Err(AudioError::new(
                        "audio_busy",
                        "Stop recording before enumerating microphones",
                    ))
                } else {
                    devices()
                };
                let _ = tx.send(result);
            }
            Ok(Command::Start(id, tx)) => {
                let result = if session.is_some() {
                    Err(AudioError::new(
                        "audio_busy",
                        "Microphone session already active",
                    ))
                } else {
                    error.store(0, Ordering::Release);
                    begin(&id, error.clone()).map(|s| {
                        session = Some(s);
                        recording.store(true, Ordering::Release);
                    })
                };
                let _ = tx.send(result);
            }
            Ok(Command::Stop(tx)) => {
                recording.store(false, Ordering::Release);
                let result = if let Some(mut s) = session.take() {
                    s.stream.take();
                    let code = error.load(Ordering::Acquire);
                    if Instant::now() >= s.deadline {
                        error.store(1, Ordering::Release);
                        Err(error_for(1))
                    } else if code != 0 {
                        Err(error_for(code))
                    } else {
                        s.buffer
                            .lock()
                            .map_err(|_| {
                                AudioError::new("audio_capture", "Capture buffer unavailable")
                            })
                            .and_then(|mut b| b.finish(s.rate))
                    }
                } else {
                    Err(AudioError::new(
                        "audio_idle",
                        "No active microphone session",
                    ))
                };
                let _ = tx.send(result);
            }
            Ok(Command::Discard(tx)) => {
                session.take();
                recording.store(false, Ordering::Release);
                error.store(0, Ordering::Release);
                let _ = tx.send(());
            }
            Ok(Command::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    drop(session);
    recording.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorder_is_send_and_stays_idle_without_a_user_start() {
        fn assert_send<T: Send>() {}
        assert_send::<NativeAudioRecorder>();
        let mut recorder = NativeAudioRecorder::new().unwrap();
        assert!(!recorder.status());
        assert!(recorder.poll_error().is_none());
        assert_eq!(recorder.stop().unwrap_err().code, "audio_idle");
        recorder.discard();
        assert!(!recorder.status());
    }
    #[test]
    fn deadline_and_disconnect_discard_capture_without_controller_stop() {
        for cause in [0, 2] {
            let mut buffer = CaptureBuffer::new(16000, 1).unwrap();
            buffer.push(&[0.5f32], |s| s);
            let mut session = Session {
                stream: None,
                buffer: Arc::new(Mutex::new(buffer)),
                rate: 16000,
                deadline: Instant::now(),
            };
            let recording = AtomicBool::new(true);
            let error = AtomicU8::new(cause);
            enforce_deadline(&mut session, &recording, &error, Instant::now());
            assert!(!recording.load(Ordering::Acquire));
            assert_eq!(
                error.load(Ordering::Acquire),
                if cause == 0 { 1 } else { cause }
            );
            assert!(session.buffer.lock().unwrap().finish(16000).is_err());
            assert!(session.buffer.lock().unwrap().samples.is_empty());
        }
    }
}
