use super::adb::{png_dimensions, Adb};
use crate::error::{CoreError, Result};
use crate::events::CoreEvent;
use crate::model::{StreamState, StreamStatus};
use base64::Engine;
use chrono::Utc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

struct Session {
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<StreamStatus>>,
    latest: Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    fps: u8,
}

/// Owns one screencap polling task per device. Frames come from
/// `adb exec-out screencap -p`, so this is a genuine device stream (at the frame
/// rate the device can produce), not a rendered mock.
pub struct StreamManager {
    sessions: Mutex<HashMap<String, Session>>,
    events: broadcast::Sender<CoreEvent>,
    recordings_root: PathBuf,
}

impl StreamManager {
    pub fn new(events: broadcast::Sender<CoreEvent>, recordings_root: PathBuf) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            events,
            recordings_root,
        }
    }

    pub fn is_streaming(&self, device_id: &str) -> bool {
        self.sessions.lock().contains_key(device_id)
    }

    pub fn statuses(&self) -> Vec<StreamStatus> {
        self.sessions
            .lock()
            .values()
            .map(|s| s.status.lock().clone())
            .collect()
    }

    pub fn status(&self, device_id: &str) -> Option<StreamStatus> {
        self.sessions
            .lock()
            .get(device_id)
            .map(|s| s.status.lock().clone())
    }

    pub fn latest_frame(&self, device_id: &str) -> Option<Arc<Vec<u8>>> {
        self.sessions
            .lock()
            .get(device_id)
            .and_then(|s| s.latest.lock().clone())
    }

    pub fn set_recording(&self, device_id: &str, recording: bool) -> Result<PathBuf> {
        let sessions = self.sessions.lock();
        let session = sessions
            .get(device_id)
            .ok_or_else(|| CoreError::Adb("start the stream before recording".into()))?;
        let mut status = session.status.lock();
        status.recording = recording;
        let dir = match &status.recording_dir {
            Some(dir) => PathBuf::from(dir),
            None => {
                let dir = self.recordings_root.join(format!(
                    "{}-{}",
                    sanitize(device_id),
                    Utc::now().format("%Y%m%d-%H%M%S")
                ));
                std::fs::create_dir_all(&dir)?;
                status.recording_dir = Some(dir.to_string_lossy().to_string());
                dir
            }
        };
        Ok(dir)
    }

    pub fn recording_dir(&self, device_id: &str) -> Option<PathBuf> {
        self.sessions
            .lock()
            .get(device_id)
            .and_then(|s| s.status.lock().recording_dir.clone())
            .map(PathBuf::from)
    }

    pub fn stop(&self, device_id: &str) -> bool {
        let mut sessions = self.sessions.lock();
        if let Some(session) = sessions.remove(device_id) {
            session.stop.store(true, Ordering::SeqCst);
            let mut status = session.status.lock();
            status.state = StreamState::Stopped;
            let _ = self.events.send(CoreEvent::StreamStatus { status: status.clone() });
            true
        } else {
            false
        }
    }

    pub fn stop_all(&self) {
        let ids: Vec<String> = self.sessions.lock().keys().cloned().collect();
        for id in ids {
            self.stop(&id);
        }
    }

    pub fn start(&self, device_id: &str, adb: Adb, fps: u8) -> Result<()> {
        if self.sessions.lock().contains_key(device_id) {
            return Ok(());
        }
        let fps = fps.clamp(1, 15);
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(StreamStatus {
            device_id: device_id.to_string(),
            state: StreamState::Connecting,
            fps: 0.0,
            frames: 0,
            message: None,
            recording: false,
            recording_dir: None,
        }));
        let latest: Arc<Mutex<Option<Arc<Vec<u8>>>>> = Arc::new(Mutex::new(None));
        self.sessions.lock().insert(
            device_id.to_string(),
            Session {
                stop: stop.clone(),
                status: status.clone(),
                latest: latest.clone(),
                fps,
            },
        );

        let serial = device_id.to_string();
        let events = self.events.clone();
        let session_status = status.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1000 / fps as u64));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut seq: u64 = 0;
            let mut errors: u32 = 0;
            let mut last_report = Instant::now();
            let mut frames_since_report: u64 = 0;
            let frame_paths = Arc::new(AtomicU64::new(0));

            while !stop.load(Ordering::SeqCst) {
                interval.tick().await;
                match adb.screencap(&serial).await {
                    Ok(bytes) => {
                        errors = 0;
                        seq += 1;
                        frames_since_report += 1;
                        let dims = png_dimensions(&bytes);
                        *latest.lock() = Some(Arc::new(bytes.clone()));

                        {
                            let mut st = session_status.lock();
                            st.state = StreamState::Streaming;
                            st.frames = seq;
                            st.message = dims.map(|(w, h)| format!("{w}×{h}"));
                            if last_report.elapsed() >= Duration::from_secs(1) {
                                st.fps = frames_since_report as f32 / last_report.elapsed().as_secs_f32();
                            }
                        }

                        let (recording, recording_dir) = {
                            let st = session_status.lock();
                            (st.recording, st.recording_dir.clone())
                        };
                        if recording {
                            if let Some(dir) = recording_dir {
                                let n = frame_paths.fetch_add(1, Ordering::SeqCst);
                                let path = PathBuf::from(dir).join(format!("frame-{n:06}.png"));
                                let frame = bytes.clone();
                                // Frame writing is blocking but small; keep it off the runtime.
                                let _ = tokio::task::spawn_blocking(move || {
                                    let _ = std::fs::write(path, frame);
                                })
                                .await;
                            }
                        }

                        if last_report.elapsed() >= Duration::from_secs(1) {
                            frames_since_report = 0;
                            last_report = Instant::now();
                            let status = session_status.lock().clone();
                            let _ = events.send(CoreEvent::StreamStatus { status });
                        }

                        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        let _ = events.send(CoreEvent::StreamFrame {
                            device_id: serial.clone(),
                            image: encoded,
                            seq,
                            bytes: bytes.len(),
                            ts: Utc::now(),
                        });
                    }
                    Err(err) => {
                        errors += 1;
                        {
                            let mut st = session_status.lock();
                            st.state = StreamState::Error;
                            st.fps = 0.0;
                            st.message = Some(err.to_string());
                        }
                        let status = session_status.lock().clone();
                        let _ = events.send(CoreEvent::StreamStatus { status });
                        if errors > 20 {
                            tracing::warn!(device = %serial, "stream stopped after repeated capture failures");
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            {
                let mut st = session_status.lock();
                st.state = StreamState::Stopped;
                st.fps = 0.0;
                let _ = events.send(CoreEvent::StreamStatus { status: st.clone() });
            }
        });

        let initial = status.lock().clone();
        let _ = self.events.send(CoreEvent::StreamStatus { status: initial });
        Ok(())
    }

    /// Encodes recorded PNG frames into an MP4 when ffmpeg is available. Returns
    /// the artefact path plus a human-readable note.
    pub async fn finish_recording(
        &self,
        device_id: &str,
        ffmpeg: Option<String>,
        fps: u8,
    ) -> Result<(PathBuf, String)> {
        let dir = self
            .recording_dir(device_id)
            .ok_or_else(|| CoreError::Adb("no recording is active for this device".into()))?;
        if let Some(session) = self.sessions.lock().get(device_id) {
            session.status.lock().recording = false;
        }
        let frames = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false))
            .count();
        if frames == 0 {
            return Err(CoreError::Adb(
                "the recording contains no frames; keep the stream running while recording".into(),
            ));
        }
        let Some(ffmpeg) = ffmpeg else {
            return Ok((
                dir.clone(),
                format!(
                    "{frames} PNG frames saved. Install ffmpeg (or set its path in Settings) to encode them into an MP4."
                ),
            ));
        };
        let mp4 = dir.join("recording.mp4");
        let args = vec![
            "-y".to_string(),
            "-framerate".to_string(),
            fps.clamp(1, 15).to_string(),
            "-i".to_string(),
            dir.join("frame-%06d.png").to_string_lossy().to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            mp4.to_string_lossy().to_string(),
        ];
        let output = crate::process::run(&ffmpeg, &args, &[], Duration::from_secs(300)).await?;
        if !output.success() {
            return Ok((
                dir.clone(),
                format!(
                    "{frames} PNG frames saved; ffmpeg encoding failed: {}",
                    output.combined().trim()
                ),
            ));
        }
        Ok((mp4, format!("Encoded {frames} frames into an MP4.")))
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}
