use crate::model::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Every real-time signal the UI receives. The shell forwards these to Tauri
/// events; nothing is polled from the frontend for these streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreEvent {
    StreamFrame {
        device_id: String,
        /// base64-encoded PNG straight from `adb exec-out screencap -p`.
        image: String,
        seq: u64,
        bytes: usize,
        ts: DateTime<Utc>,
    },
    StreamStatus {
        status: StreamStatus,
    },
    RunUpdate {
        run_id: String,
        workflow_id: String,
        status: RunStatus,
        line: Option<String>,
        error: Option<String>,
    },
    DeviceListChanged,
    InstanceChanged {
        provider: Provider,
        instance_id: String,
        message: String,
    },
    PostUpdate {
        post_id: String,
        status: PostStatus,
        error: Option<String>,
    },
    Log {
        line: LogLine,
    },
}
