use crate::error::{CoreError, Result};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.code == 0 && !self.timed_out
    }

    pub fn combined(&self) -> String {
        let mut out = String::new();
        if !self.stdout.trim().is_empty() {
            out.push_str(self.stdout.trim_end());
        }
        if !self.stderr.trim().is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(self.stderr.trim_end());
        }
        out
    }
}

fn command_for(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        // Do not flash a console window for every adb/boxd invocation.
        // tokio::process::Command exposes creation_flags directly on Windows.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Runs a program, capturing UTF-8 output with a hard timeout.
pub async fn run(
    program: &str,
    args: &[String],
    envs: &[(String, String)],
    timeout: Duration,
) -> Result<ProcessOutput> {
    let output = run_bytes(program, args, envs, timeout).await?;
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        code: output.code,
        duration_ms: output.duration_ms,
        timed_out: output.timed_out,
    })
}

#[derive(Debug, Clone)]
pub struct RawOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub async fn run_bytes(
    program: &str,
    args: &[String],
    envs: &[(String, String)],
    timeout: Duration,
) -> Result<RawOutput> {
    let started = Instant::now();
    let mut cmd = command_for(program);
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CoreError::Adb(format!(
                "'{program}' was not found. Set its full path in Settings, or install it and make sure it is on PATH."
            ))
        } else {
            CoreError::Io(e)
        }
    })?;

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(RawOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            code: output.status.code().unwrap_or(-1),
            duration_ms: started.elapsed().as_millis() as u64,
            timed_out: false,
        }),
        Ok(Err(e)) => Err(CoreError::Io(e)),
        Err(_) => Ok(RawOutput {
            // `wait_with_output` consumed the child; kill_on_drop already fired.
            stdout: Vec::new(),
            stderr: format!("command timed out after {}s", timeout.as_secs()).into_bytes(),
            code: -1,
            duration_ms: started.elapsed().as_millis() as u64,
            timed_out: true,
        }),
    }
}

/// Locates an executable either from an explicit path or the PATH.
pub fn resolve_binary(explicit: Option<&str>, name: &str) -> Option<String> {
    if let Some(path) = explicit {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    which::which(name)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}
