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
import type { AdbStatus, BatchOutcome, Device, DeviceHealth } from "../lib/types";

type DetailTab = "shell" | "logcat" | "apps" | "files";

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
  const [selected, setSelected] = useState<string[]>([]);
  const [pairAddress, setPairAddress] = useState("");
  const [pairCode, setPairCode] = useState("");
  const [pairing, setPairing] = useState(false);
  const [healthFor, setHealthFor] = useState<Device | null>(null);
  const [batch, setBatch] = useState<"shell" | "screenshot" | null>(null);

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

  const pair = async () => {
    if (!pairAddress.trim() || !pairCode.trim()) return;
    setPairing(true);
    setNotice(null);
    try {
      const message = await api.pairDevice(pairAddress.trim(), pairCode.trim());
      setNotice(`${message} — now connect to the device's debug port.`);
      setPairAddress("");
      setPairCode("");
      await load(true);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setPairing(false);
    }
  };

  const toggleSelected = (serial: string) => {
    setSelected((prev) =>
      prev.includes(serial) ? prev.filter((id) => id !== serial) : [...prev, serial],
    );
  };

  const onlineSerials = devices.filter((device) => device.state === "device").map((d) => d.serial);
  const allSelected = onlineSerials.length > 0 && onlineSerials.every((s) => selected.includes(s));

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

        <div className="form-row">
          <div className="flex-1">
            <Field
              label="Pair a device over Wi-Fi"
              hint="Android 11+ wireless debugging → “Pair device with pairing code”"
            >
              <input
                className="input mono"
                value={pairAddress}
                onChange={(event) => setPairAddress(event.target.value)}
                placeholder="192.168.1.50:41234"
              />
            </Field>
          </div>
          <div style={{ width: 150 }}>
            <Field label="Pairing code" hint="6 digits">
              <input
                className="input mono"
                value={pairCode}
                onChange={(event) => setPairCode(event.target.value)}
                placeholder="123456"
                inputMode="numeric"
              />
            </Field>
          </div>
          <Button icon="key" onClick={() => void pair()} loading={pairing}>
            Pair
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
                <th style={{ width: 34 }}>
                  <input
                    type="checkbox"
                    aria-label="Select every online device"
                    checked={allSelected}
                    disabled={onlineSerials.length === 0}
                    onChange={(event) =>
                      setSelected(event.target.checked ? onlineSerials : [])
                    }
                  />
                </th>
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
                  <td colSpan={7}>
                    <Spinner label="Enumerating adb devices…" />
                  </td>
                </tr>
              ) : devices.length === 0 ? (
                <tr>
                  <td colSpan={7}>
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
                      <input
                        type="checkbox"
                        aria-label={`Select ${device.label}`}
                        checked={selected.includes(device.serial)}
                        disabled={device.state !== "device"}
                        onChange={() => toggleSelected(device.serial)}
                      />
                    </td>
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
                        <IconButton
                          icon="chip"
                          label="Battery, storage, RAM and uptime"
                          disabled={device.state !== "device"}
                          onClick={() => setHealthFor(device)}
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

      {selected.length > 0 && (
        <div className="batch-bar">
          <span>
            {selected.length} device{selected.length === 1 ? "" : "s"} selected
          </span>
          <div className="row">
            <Button size="sm" icon="terminal" onClick={() => setBatch("shell")}>
              Run shell on all
            </Button>
            <Button size="sm" icon="camera" onClick={() => setBatch("screenshot")}>
              Screenshot all
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setSelected([])}>
              Clear
            </Button>
          </div>
        </div>
      )}

      {healthFor && <HealthModal device={healthFor} onClose={() => setHealthFor(null)} />}
      {batch && (
        <BatchModal
          mode={batch}
          serials={selected}
          onClose={() => setBatch(null)}
          onDone={(outcomes) => {
            const ok = outcomes.filter((outcome) => outcome.ok).length;
            if (batch === "screenshot" && ok > 0) {
              const first = outcomes.find((outcome) => outcome.ok);
              if (first) setNotice(`${ok} screenshot(s) saved, e.g. ${first.output}`);
            }
          }}
        />
      )}

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
        {(["shell", "logcat", "apps", "files"] as DetailTab[]).map((value) => (
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

      {tab === "files" && <FileTransfer serial={device.serial} />}

      <InlineError message={error} />
    </Modal>
  );
}

function FileTransfer({ serial }: { serial: string }) {
  const [local, setLocal] = useState("");
  const [remote, setRemote] = useState("/sdcard/Download/");
  const [pullRemote, setPullRemote] = useState("/sdcard/Download/");
  const [pullLocal, setPullLocal] = useState("");
  const [busy, setBusy] = useState<"push" | "pull" | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const push = async () => {
    if (!local.trim() || !remote.trim()) return;
    setBusy("push");
    setMessage(null);
    try {
      setMessage(await api.devicePush(serial, local.trim(), remote.trim()));
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const pull = async () => {
    if (!pullRemote.trim()) return;
    setBusy("pull");
    setMessage(null);
    try {
      let target = pullLocal.trim();
      if (!target) {
        const picked = await api.pickSaveFile("Save the pulled file here", "device-file");
        if (!picked) {
          setBusy(null);
          return;
        }
        target = picked;
        setPullLocal(picked);
      }
      setMessage(await api.devicePull(serial, pullRemote.trim(), target));
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="stack">
      <div className="form-row">
        <div className="flex-1">
          <Field label="Send a file to the device" hint="host path">
            <input
              className="input mono"
              value={local}
              onChange={(event) => setLocal(event.target.value)}
              placeholder="C:\\builds\\payload.zip"
            />
          </Field>
        </div>
        <div className="flex-1">
          <Field label="Device path">
            <input
              className="input mono"
              value={remote}
              onChange={(event) => setRemote(event.target.value)}
            />
          </Field>
        </div>
        <Button variant="ghost" onClick={() => void api.pickFile("Select a file to push").then((p) => p && setLocal(p))}>
          Browse
        </Button>
        <Button variant="primary" icon="up" loading={busy === "push"} onClick={() => void push()}>
          Push
        </Button>
      </div>

      <div className="form-row">
        <div className="flex-1">
          <Field label="Pull a file from the device" hint="separate adb pull command">
            <input
              className="input mono"
              value={pullRemote}
              onChange={(event) => setPullRemote(event.target.value)}
            />
          </Field>
        </div>
        <div className="flex-1">
          <Field label="Save as" hint="leave blank to choose with the save dialog">
            <input
              className="input mono"
              value={pullLocal}
              onChange={(event) => setPullLocal(event.target.value)}
              placeholder="C:\\captures\\trace.txt"
            />
          </Field>
        </div>
        <Button variant="secondary" icon="down" loading={busy === "pull"} onClick={() => void pull()}>
          Pull
        </Button>
      </div>

      {message && <InlineNote>{message}</InlineNote>}
      <InlineNote>
        Transfers run through the real <code>adb push</code> / <code>adb pull</code> of the attached
        device. Nothing leaves your machine except through the device connection.
      </InlineNote>
    </div>
  );
}

function HealthModal({ device, onClose }: { device: Device; onClose: () => void }) {
  const [health, setHealth] = useState<DeviceHealth | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      setHealth(await api.deviceHealth(device.serial));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [device.serial]);

  const gb = (kb: number | null) =>
    kb === null ? "not reported" : `${(kb / 1024 / 1024).toFixed(1)} GB`;

  return (
    <Modal
      open
      title="Device health"
      subtitle={`${device.label} · ${device.serial}`}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" icon="refresh" onClick={() => void load()} loading={loading}>
            Refresh
          </Button>
          <Button variant="secondary" onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      {loading ? (
        <Spinner label="Reading battery, storage and memory from the device…" />
      ) : error ? (
        <InlineError message={error} />
      ) : health ? (
        <div className="stack">
          <dl className="kv">
            <dt>Battery</dt>
            <dd>
              {health.battery_level !== null ? `${health.battery_level} %` : "not reported"}
              {health.battery_status ? ` · ${health.battery_status}` : ""}
              {health.battery_temp_c !== null
                ? ` · ${health.battery_temp_c.toFixed(1)} °C`
                : ""}
              {health.battery_health ? ` · ${health.battery_health}` : ""}
            </dd>
            <dt>Storage (/data)</dt>
            <dd>
              {health.storage_free_kb !== null ? `${gb(health.storage_free_kb)} free` : "not reported"}
              {health.storage_total_kb !== null ? ` of ${gb(health.storage_total_kb)}` : ""}
            </dd>
            <dt>Memory</dt>
            <dd>
              {health.ram_free_kb !== null ? `${gb(health.ram_free_kb)} available` : "not reported"}
              {health.ram_total_kb !== null ? ` of ${gb(health.ram_total_kb)}` : ""}
            </dd>
            <dt>Uptime</dt>
            <dd>
              {health.uptime_seconds !== null
                ? `${Math.floor(health.uptime_seconds / 3600)} h ${Math.floor(
                    (health.uptime_seconds % 3600) / 60,
                  )} m`
                : "not reported"}
            </dd>
            <dt>Screen</dt>
            <dd>{health.screen ?? "not reported"}</dd>
            <dt>SDK / ABI</dt>
            <dd>{`${health.sdk ?? "?"} · ${health.abi ?? "?"}`}</dd>
            <dt>Captured</dt>
            <dd>{formatRelative(health.fetched_at)}</dd>
          </dl>
          {health.errors.length > 0 && (
            <InlineError message={`Some probes failed: ${health.errors.join("; ")}`} />
          )}
          <InlineNote>
            Values come straight from <code>dumpsys battery</code>, <code>df -k /data</code>,
            <code> /proc/meminfo</code> and <code>/proc/uptime</code> on this device.
          </InlineNote>
        </div>
      ) : null}
    </Modal>
  );
}

function BatchModal({
  mode,
  serials,
  onClose,
  onDone,
}: {
  mode: "shell" | "screenshot";
  serials: string[];
  onClose: () => void;
  onDone: (outcomes: BatchOutcome[]) => void;
}) {
  const [command, setCommand] = useState("dumpsys battery | head -n 6");
  const [outcomes, setOutcomes] = useState<BatchOutcome[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      const result =
        mode === "shell"
          ? await api.deviceBatchShell(serials, command)
          : await api.deviceBatchScreenshot(serials);
      setOutcomes(result);
      onDone(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      wide
      title={mode === "shell" ? "Run a command on every selected device" : "Screenshot every selected device"}
      subtitle={`${serials.length} device(s)`}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button variant="primary" icon={mode === "shell" ? "terminal" : "camera"} loading={busy} onClick={() => void run()}>
            {mode === "shell" ? "Run" : "Capture all"}
          </Button>
        </>
      }
    >
      {mode === "shell" && (
        <Field label="adb shell command" hint="executed on each device in parallel">
          <input
            className="input mono"
            value={command}
            onChange={(event) => setCommand(event.target.value)}
          />
        </Field>
      )}
      <InlineError message={error} />
      {outcomes.length > 0 && (
        <div className="stack-sm">
          {outcomes.map((outcome) => (
            <div key={outcome.serial} className={`batch-result ${outcome.ok ? "ok" : "fail"}`}>
              <div className="batch-result-head">
                <span className="mono">{outcome.serial}</span>
                <Pill tone={outcome.ok ? "ok" : "danger"}>{outcome.ok ? "ok" : "failed"}</Pill>
              </div>
              <pre className="console">{outcome.output || "(no output)"}</pre>
            </div>
          ))}
        </div>
      )}
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
