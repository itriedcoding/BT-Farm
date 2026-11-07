use crate::error::{CoreError, Result};
use crate::process::{self, ProcessOutput};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// One entry from `adb devices -l`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdbDevice {
    pub serial: String,
    pub state: String,
    pub product: Option<String>,
    pub model: Option<String>,
    pub device: Option<String>,
    pub transport_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceProps {
    pub model: Option<String>,
    pub android: Option<String>,
    pub sdk: Option<String>,
}

/// Thin wrapper over the real adb binary. Every call shells out to adb; nothing
/// is simulated.
#[derive(Debug, Clone)]
pub struct Adb {
    path: String,
}

impl Adb {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    async fn run(&self, args: &[&str], timeout: Duration) -> Result<ProcessOutput> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        process::run(&self.path, &owned, &[], timeout).await
    }

    async fn run_checked(&self, args: &[&str], timeout: Duration) -> Result<ProcessOutput> {
        let output = self.run(args, timeout).await?;
        if output.timed_out {
            return Err(CoreError::Adb(format!(
                "adb {} timed out after {}s",
                args.join(" "),
                timeout.as_secs()
            )));
        }
        if output.code != 0 {
            return Err(CoreError::Adb(format!(
                "adb {} failed ({}): {}",
                args.join(" "),
                output.code,
                output.combined().trim()
            )));
        }
        Ok(output)
    }

    pub async fn version(&self) -> Result<String> {
        let output = self.run_checked(&["version"], Duration::from_secs(15)).await?;
        Ok(output.stdout.lines().next().unwrap_or_default().trim().to_string())
    }

    pub async fn start_server(&self) -> Result<()> {
        self.run_checked(&["start-server"], Duration::from_secs(30)).await?;
        Ok(())
    }

    pub async fn devices(&self) -> Result<Vec<AdbDevice>> {
        let output = self.run_checked(&["devices", "-l"], Duration::from_secs(25)).await?;
        Ok(parse_devices(&output.stdout))
    }

    pub async fn connect(&self, address: &str) -> Result<String> {
        let output = self
            .run_checked(&["connect", address], Duration::from_secs(30))
            .await?;
        let text = output.combined();
        let lowered = text.to_ascii_lowercase();
        if lowered.contains("cannot") || lowered.contains("failed") || lowered.contains("unable") {
            return Err(CoreError::Adb(text.trim().to_string()));
        }
        Ok(text.trim().to_string())
    }

    pub async fn disconnect(&self, address: &str) -> Result<String> {
        let output = self
            .run_checked(&["disconnect", address], Duration::from_secs(20))
            .await?;
        Ok(output.combined().trim().to_string())
    }

    pub async fn shell(&self, serial: &str, command: &str) -> Result<ProcessOutput> {
        self.run(&["-s", serial, "shell", command], Duration::from_secs(45))
            .await
    }

    pub async fn shell_checked(&self, serial: &str, command: &str) -> Result<String> {
        let output = self
            .run(&["-s", serial, "shell", command], Duration::from_secs(45))
            .await?;
        if output.timed_out {
            return Err(CoreError::Adb(format!("`{command}` timed out on {serial}")));
        }
        if output.code != 0 {
            return Err(CoreError::Adb(format!(
                "`{command}` failed on {serial} ({}): {}",
                output.code,
                output.combined().trim()
            )));
        }
        Ok(output.stdout)
    }

    pub async fn props(&self, serial: &str) -> Result<DeviceProps> {
        let output = self
            .run(
                &[
                    "-s",
                    serial,
                    "shell",
                    "getprop ro.product.model; getprop ro.build.version.release; getprop ro.build.version.sdk",
                ],
                Duration::from_secs(20),
            )
            .await?;
        let mut lines = output.stdout.lines().map(|l| l.trim().to_string());
        let model = lines.next().filter(|l| !l.is_empty());
        let android = lines.next().filter(|l| !l.is_empty());
        let sdk = lines.next().filter(|l| !l.is_empty());
        Ok(DeviceProps { model, android, sdk })
    }

    /// Captures a PNG frame using `exec-out screencap -p`.
    pub async fn screencap(&self, serial: &str) -> Result<Vec<u8>> {
        let args = ["-s", serial, "exec-out", "screencap", "-p"];
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let output = process::run_bytes(&self.path, &owned, &[], Duration::from_secs(20)).await?;
        if output.timed_out {
            return Err(CoreError::Adb("screencap timed out".into()));
        }
        if output.code != 0 {
            return Err(CoreError::Adb(format!(
                "screencap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        if !output.stdout.starts_with(&[0x89, b'P', b'N', b'G']) {
            return Err(CoreError::Adb(
                "screencap did not return PNG data (is the device authorised and awake?)".into(),
            ));
        }
        Ok(output.stdout)
    }

    pub async fn tap(&self, serial: &str, x: u32, y: u32) -> Result<()> {
        self.shell_checked(serial, &format!("input tap {x} {y}")).await.map(|_| ())
    }

    pub async fn swipe(&self, serial: &str, x1: u32, y1: u32, x2: u32, y2: u32, duration_ms: u32) -> Result<()> {
        self.shell_checked(
            serial,
            &format!("input swipe {x1} {y1} {x2} {y2} {duration_ms}"),
        )
        .await
        .map(|_| ())
    }

    pub async fn input_text(&self, serial: &str, text: &str) -> Result<()> {
        // `input text` treats spaces specially.
        let escaped = text.replace(' ', "%s");
        self.shell_checked(serial, &format!("input text {}", shell_quote(&escaped)))
            .await
            .map(|_| ())
    }

    pub async fn keyevent(&self, serial: &str, code: &str) -> Result<()> {
        self.shell_checked(serial, &format!("input keyevent {code}"))
            .await
            .map(|_| ())
    }

    pub async fn install(&self, serial: &str, apk_path: &str) -> Result<String> {
        let output = self
            .run(
                &["-s", serial, "install", "-r", "-g", apk_path],
                Duration::from_secs(300),
            )
            .await?;
        let text = output.combined();
        if output.timed_out {
            return Err(CoreError::Adb("apk install timed out".into()));
        }
        if !text.to_ascii_lowercase().contains("success") {
            return Err(CoreError::Adb(format!("install failed: {}", text.trim())));
        }
        Ok(text.trim().to_string())
    }

    pub async fn uninstall(&self, serial: &str, package: &str) -> Result<()> {
        self.run_checked(&["-s", serial, "uninstall", package], Duration::from_secs(90))
            .await
            .map(|_| ())
    }

    pub async fn start_app(&self, serial: &str, package: &str) -> Result<()> {
        self.shell_checked(
            serial,
            &format!("monkey -p {package} -c android.intent.category.LAUNCHER 1"),
        )
        .await
        .map(|_| ())
    }

    pub async fn stop_app(&self, serial: &str, package: &str) -> Result<()> {
        self.shell_checked(serial, &format!("am force-stop {package}"))
            .await
            .map(|_| ())
    }

    pub async fn current_focus(&self, serial: &str) -> Result<String> {
        let out = self
            .shell_checked(serial, "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'")
            .await?;
        Ok(out.trim().to_string())
    }

    pub async fn logcat(&self, serial: &str, lines: u32) -> Result<String> {
        let output = self
            .run(
                &["-s", serial, "logcat", "-d", "-t", &lines.to_string()],
                Duration::from_secs(30),
            )
            .await?;
        Ok(output.stdout)
    }

    pub async fn list_packages(&self, serial: &str) -> Result<Vec<String>> {
        let out = self.shell_checked(serial, "pm list packages -3").await?;
        Ok(out
            .lines()
            .filter_map(|l| l.trim().strip_prefix("package:"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    pub async fn push(&self, serial: &str, local: &str, remote: &str) -> Result<()> {
        self.run_checked(&["-s", serial, "push", local, remote], Duration::from_secs(180))
            .await
            .map(|_| ())
    }

    pub async fn pull(&self, serial: &str, remote: &str, local: &str) -> Result<()> {
        self.run_checked(&["-s", serial, "pull", remote, local], Duration::from_secs(180))
            .await
            .map(|_| ())
    }

    /// `adb pair host:port CODE` — the wireless-debugging pairing flow that pairs
    /// without needing the mDNS service to be visible to us.
    pub async fn pair(&self, address: &str, code: &str) -> Result<String> {
        let output = self
            .run_checked(&["pair", address, code], Duration::from_secs(60))
            .await?;
        let text = output.combined();
        if !text.to_ascii_lowercase().contains("successfully paired") {
            return Err(CoreError::Adb(text.trim().to_string()));
        }
        Ok(text.trim().to_string())
    }

    /// Runs a shell command and returns exit code plus combined output instead of
    /// failing, so batch operations can report per-device results.
    pub async fn shell_result(&self, serial: &str, command: &str) -> Result<ProcessOutput> {
        self.run(&["-s", serial, "shell", command], Duration::from_secs(60))
            .await
    }
}

fn shell_quote(value: &str) -> String {
    // adb shell runs the string through the device's sh, so single-quote it and
    // escape embedded quotes.
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn parse_devices(stdout: &str) -> Vec<AdbDevice> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(serial) = parts.next() else { continue };
        let Some(state) = parts.next() else { continue };
        let mut device = AdbDevice {
            serial: serial.to_string(),
            state: state.to_string(),
            product: None,
            model: None,
            device: None,
            transport_id: None,
        };
        for field in parts {
            if let Some((key, value)) = field.split_once(':') {
                match key {
                    "product" => device.product = Some(value.to_string()),
                    "model" => device.model = Some(value.to_string()),
                    "device" => device.device = Some(value.to_string()),
                    "transport_id" => device.transport_id = Some(value.to_string()),
                    _ => {}
                }
            }
        }
        out.push(device);
    }
    out
}

/// Width and height from a PNG IHDR chunk.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adb_devices_output() {
        let stdout = "List of devices attached\n\
             emulator-5554 device product:sdk_gphone64_x86_64 model:sdk_gphone64_x86_64 device:emu64xa transport_id:1\n\
             R58M123ABC unauthorized usb:1-1 transport_id:2\n\
             192.168.1.50:5555 offline transport_id:3\n";
        let devices = parse_devices(stdout);
        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, "device");
        assert_eq!(devices[0].model.as_deref(), Some("sdk_gphone64_x86_64"));
        assert_eq!(devices[1].state, "unauthorized");
        assert_eq!(devices[2].state, "offline");
    }

    #[test]
    fn png_dimensions_are_read_from_ihdr() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend_from_slice(&[0, 0, 0, 13]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1080u32.to_be_bytes());
        png.extend_from_slice(&2400u32.to_be_bytes());
        assert_eq!(png_dimensions(&png), Some((1080, 2400)));
        assert_eq!(png_dimensions(b"not a png"), None);
    }

    #[test]
    fn quotes_spaces_for_input_text() {
        assert_eq!(shell_quote("hello%s world"), "'hello%s world'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
