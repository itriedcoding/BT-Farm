import { useCallback, useEffect, useState } from "react";
import { Icon } from "../components/Icons";
import {
  Banner,
  Button,
  Card,
  EmptyState,
  Field,
  IconButton,
  InlineError,
  InlineNote,
  Modal,
  Pill,
  Spinner,
} from "../components/ui";
import { api, isDesktop } from "../lib/api";
import { formatRelative, titleCase } from "../lib/format";
import type { Page } from "../lib/nav";
import type { AdbStatus, Device } from "../lib/types";

type DetailTab = "shell" | "logcat" | "apps";

export function DevicesPage({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const [adb, setAdb] = useState<AdbStatus | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [address, setAddress] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [detail, setDetail] = useState<Device | null>(null);
  const [renaming, setRenaming] = useState<Device | null>(null);

  const load = useCallback(async (refresh = true) => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const [status, list] = await Promise.all([api.adbStatus(), api.devices(refresh)]);
      setAdb(status);
      setDevices(list);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setAdb(await api.adbStatus().catch(() => null));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(false);
  }, [load]);

  const connect = async () => {
    if (!address.trim()) return;
    setConnecting(true);
    setNotice(null);
    try {
      const message = await api.connectDevice(address.trim());
      setNotice(message);
      setAddress("");
      await load(true);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setConnecting(false);
    }
  };

  const disconnect = async (device: Device) => {
    try {
      const target = device.address ?? device.serial;
      setNotice(await api.disconnectDevice(target));
      await load(true);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    }
  };

  const screenshot = async (device: Device) => {
    try {
      const result = await api.deviceScreenshot(device.serial);
      setNotice(`Screenshot saved to ${result.path}`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="page">
      {error && <Banner tone="danger" title="ADB is not ready">{error}</Banner>}
      {notice && <Banner tone="info" title="ADB response">{notice}</Banner>}

      <Card
        title="ADB connection"
        subtitle={
          adb?.available
            ? `${adb.version ?? "adb"} · ${adb.adb_path ?? "PATH"}`
            : "Android platform-tools are required to talk to devices"
        }
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load(true)} loading={loading}>
              Rescan
            </Button>
            <Button size="sm" variant="ghost" onClick={() => onNavigate("settings")}>
              Locate adb
            </Button>
          </>
        }
      >
        {adb && !adb.available && (
          <Banner tone="warn" title="adb binary not found">
            {adb.message} Install Android platform-tools, then point BT Farm at{" "}
            <code>adb.exe</code> in Settings.
          </Banner>
        )}

        <div className="form-row" style={{ marginTop: adb && !adb.available ? 12 : 0 }}>
          <div className="flex-1">
            <Field
              label="Connect a network device"
              hint="host:port — cloud phone endpoints, emulators or devices over TCP/IP"
            >
              <input
                className="input mono"
                value={address}
                onChange={(event) => setAddress(event.target.value)}
                placeholder="192.168.1.50:5555"
                onKeyDown={(event) => {
                  if (event.key === "Enter") void connect();
                }}
              />
            </Field>
          </div>
          <Button icon="link" onClick={() => void connect()} loading={connecting}>
            Connect
          </Button>
        </div>

        <InlineNote>
          <strong>Cloud phones:</strong> this app drives real Android devices over ADB. Attach a
          device-host endpoint (Genymotion SaaS, a Redroid/Waydroid host, an Android x86 VM, or a
          USB device) — Linux sandbox providers cannot boot an Android emulator, so they are never
          presented as phones.
        </InlineNote>
      </Card>

      <Card title="Devices" subtitle={`${devices.filter((d) => d.state === "device").length} online of ${devices.length}`} flush>
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Device</th>
                <th>Android</th>
                <th>Transport</th>
                <th>State</th>
                <th>Seen</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {loading && devices.length === 0 ? (
                <tr>
                  <td colSpan={6}>
                    <Spinner label="Enumerating adb devices…" />
                  </td>
                </tr>
              ) : devices.length === 0 ? (
                <tr>
                  <td colSpan={6}>
                    <EmptyState
                      icon="phone"
                      title="No devices attached"
                      body="Enable USB debugging and plug a device in, or connect an ADB endpoint above."
                    />
                  </td>
                </tr>
              ) : (
                devices.map((device) => (
                  <tr key={device.id}>
                    <td>
                      <div className="cell-main">
                        <Icon name="phone" size={14} />
                        <span className="cell-title">{device.label}</span>
                      </div>
                      <div className="cell-sub">{device.serial}</div>
                    </td>
                    <td className="muted nowrap">
                      {device.android ? `Android ${device.android}` : "—"}
                      {device.model ? ` · ${device.model}` : ""}
                    </td>
                    <td>
                      <Pill tone="muted">{device.transport === "tcp" ? "Network" : "USB"}</Pill>
                    </td>
                    <td>
                      <Pill
                        tone={
                          device.state === "device"
                            ? "ok"
                            : device.state === "unauthorized"
                              ? "warn"
                              : "danger"
                        }
                        dot={device.state === "device"}
                      >
                        {device.state}
                      </Pill>
                    </td>
                    <td className="muted nowrap">{formatRelative(device.last_seen)}</td>
                    <td>
                      <div className="cell-actions">
                        <IconButton
                          icon="screen"
                          label="Open live screen"
                          disabled={device.state !== "device"}
                          onClick={() => onNavigate("viewer")}
                        />
                        <IconButton
                          icon="terminal"
                          label="Console, logcat and apps"
                          disabled={device.state !== "device"}
                          onClick={() => setDetail(device)}
                        />
                        <IconButton
                          icon="camera"
                          label="Capture screenshot"
                          disabled={device.state !== "device"}
                          onClick={() => void screenshot(device)}
                        />
                        <IconButton icon="settings" label="Rename" onClick={() => setRenaming(device)} />
                        {device.transport === "tcp" && (
                          <IconButton
                            icon="close"
                            label="Disconnect"
                            variant="danger"
                            onClick={() => void disconnect(device)}
                          />
                        )}
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </Card>

      {detail && <DeviceDetailModal device={detail} onClose={() => setDetail(null)} />}
      {renaming && (
        <RenameModal
          device={renaming}
          onClose={() => setRenaming(null)}
          onSaved={async () => {
            setRenaming(null);
            await load(false);
          }}
        />
      )}
    </div>
  );
}

function DeviceDetailModal({ device, onClose }: { device: Device; onClose: () => void }) {
  const [tab, setTab] = useState<DetailTab>("shell");
  const [command, setCommand] = useState("getprop ro.product.model");
  const [output, setOutput] = useState("");
  const [packages, setPackages] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [apkPath, setApkPath] = useState("");

  const runShell = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await api.deviceShell(device.serial, command);
      setOutput((prev) => `${prev}${prev ? "\n" : ""}$ ${command}\n${result}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const loadLogcat = async () => {
    setBusy(true);
    setError(null);
    try {
      setOutput(await api.deviceLogcat(device.serial, 400));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const loadPackages = async () => {
    setBusy(true);
    setError(null);
    try {
      setPackages(await api.devicePackages(device.serial));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const install = async () => {
    setBusy(true);
    setError(null);
    try {
      let path = apkPath;
      if (!path) {
        const picked = await api.pickFile("Select an APK", ["apk"]);
        if (!picked) {
          setBusy(false);
          return;
        }
        path = picked;
        setApkPath(picked);
      }
      const message = await api.deviceInstall(device.serial, path);
      setOutput((prev) => `${prev}${prev ? "\n" : ""}${message}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void loadPackages();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [device.serial]);

  return (
    <Modal
      open
      xl
      title={device.label}
      subtitle={`${device.serial}${device.android ? ` · Android ${device.android}` : ""}`}
      onClose={onClose}
      footer={
        <Button variant="ghost" onClick={onClose}>
          Close
        </Button>
      }
    >
      <div className="chip-row">
        {(["shell", "logcat", "apps"] as DetailTab[]).map((value) => (
          <button
            key={value}
            type="button"
            className={`btn btn-sm ${tab === value ? "btn-primary" : "btn-ghost"}`}
            onClick={() => setTab(value)}
          >
            {titleCase(value)}
          </button>
        ))}
      </div>

      {tab === "shell" && (
        <>
          <div className="form-row">
            <div className="flex-1">
              <Field label="adb shell command">
                <input
                  className="input mono"
                  value={command}
                  onChange={(event) => setCommand(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void runShell();
                  }}
                  spellCheck={false}
                />
              </Field>
            </div>
            <Button icon="terminal" onClick={() => void runShell()} loading={busy}>
              Run
            </Button>
          </div>
          <div className="console">{output || "Output appears here."}</div>
        </>
      )}

      {tab === "logcat" && (
        <>
          <div className="row">
            <Button icon="refresh" onClick={() => void loadLogcat()} loading={busy}>
              Capture logcat
            </Button>
            <Button variant="ghost" onClick={() => setOutput("")}>
              Clear
            </Button>
          </div>
          <div className="console">{output || "Capture the device log buffer to inspect crashes."}</div>
        </>
      )}

      {tab === "apps" && (
        <>
          <div className="form-row">
            <div className="flex-1">
              <Field label="APK path" hint="pick a file or paste a path">
                <input
                  className="input mono"
                  value={apkPath}
                  onChange={(event) => setApkPath(event.target.value)}
                  placeholder="C:\\builds\\app-release.apk"
                />
              </Field>
            </div>
            <Button variant="secondary" onClick={() => void install()} loading={busy}>
              Install
            </Button>
            <Button
              variant="ghost"
              onClick={async () => {
                const picked = await api.pickFile("Select an APK", ["apk"]).catch(() => null);
                if (picked) setApkPath(picked);
              }}
            >
              Browse
            </Button>
          </div>
          <div className="table-wrap" style={{ maxHeight: 320, overflowY: "auto" }}>
            <table className="data">
              <thead>
                <tr>
                  <th>Installed package</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {packages.length === 0 ? (
                  <tr>
                    <td colSpan={2}>
                      <div className="table-empty">No third-party packages reported.</div>
                    </td>
                  </tr>
                ) : (
                  packages.map((pkg) => (
                    <tr key={pkg}>
                      <td className="mono">{pkg}</td>
                      <td>
                        <div className="cell-actions">
                          <IconButton
                            icon="play"
                            label="Launch"
                            onClick={() => void api.deviceLaunch(device.serial, pkg)}
                          />
                          <IconButton
                            icon="stop"
                            label="Force stop"
                            onClick={() => void api.deviceStopApp(device.serial, pkg)}
                          />
                          <IconButton
                            icon="trash"
                            label="Uninstall"
                            variant="danger"
                            onClick={async () => {
                              try {
                                await api.deviceUninstall(device.serial, pkg);
                                await loadPackages();
                              } catch (err) {
                                setError(err instanceof Error ? err.message : String(err));
                              }
                            }}
                          />
                        </div>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </>
      )}

      <InlineError message={error} />
    </Modal>
  );
}

function RenameModal({
  device,
  onClose,
  onSaved,
}: {
  device: Device;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [label, setLabel] = useState(device.label);
  const [notes, setNotes] = useState(device.notes ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.setDeviceLabel(device.serial, label, notes.trim() || null);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open
      title="Device label"
      subtitle={device.serial}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => void save()} loading={saving}>
            Save
          </Button>
        </>
      }
    >
      <Field label="Label">
        <input className="input" value={label} onChange={(event) => setLabel(event.target.value)} />
      </Field>
      <Field label="Notes" hint="optional">
        <textarea
          className="textarea"
          value={notes}
          onChange={(event) => setNotes(event.target.value)}
          placeholder="Which account or test suite uses this device"
        />
      </Field>
      <InlineError message={error} />
    </Modal>
  );
}
