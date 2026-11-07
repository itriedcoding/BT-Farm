import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent } from "react";
import { Icon } from "../components/Icons";
import {
  Banner,
  Button,
  Card,
  EmptyState,
  Field,
  IconButton,
  InlineError,
  Modal,
  Pill,
  Spinner,
} from "../components/ui";
import { onFarmEvent, api, isDesktop } from "../lib/api";
import { parseResolution, titleCase } from "../lib/format";
import type { CoreEvent, Device, RecordingFile, StreamStatus } from "../lib/types";

interface TileState {
  frame: string | null;
  status: StreamStatus | null;
  error: string | null;
}

export function ViewerPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [tiles, setTiles] = useState<Record<string, TileState>>({});
  const [selected, setSelected] = useState<string[]>([]);
  const [fps, setFps] = useState(4);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [focus, setFocus] = useState<string | null>(null);
  const [recordingPaths, setRecordingPaths] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState<string | null>(null);
  const [layout, setLayout] = useState<"comfortable" | "compact">(() => {
    const stored = window.localStorage.getItem("btfarm.viewerLayout");
    return stored === "compact" ? "compact" : "comfortable";
  });
  const [showRecordings, setShowRecordings] = useState(false);

  const changeLayout = (next: "comfortable" | "compact") => {
    setLayout(next);
    window.localStorage.setItem("btfarm.viewerLayout", next);
  };

  const selectedRef = useRef<string[]>([]);
  selectedRef.current = selected;

  const load = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const [list, statuses] = await Promise.all([api.devices(false), api.streamStatuses()]);
      setDevices(list);
      const next: Record<string, TileState> = {};
      for (const status of statuses) {
        next[status.device_id] = { frame: null, status, error: null };
        const frame = await api.streamFrame(status.device_id).catch(() => null);
        if (frame) next[status.device_id].frame = frame;
      }
      setTiles((prev) => {
        const merged: Record<string, TileState> = { ...prev };
        for (const [id, value] of Object.entries(next)) {
          merged[id] = { ...merged[id], ...value };
        }
        return merged;
      });
      setSelected((prev) => {
        const online = list.filter((d) => d.state === "device").map((d) => d.serial);
        const kept = prev.filter((id) => online.includes(id));
        return kept.length > 0 ? kept : online.slice(0, 1);
      });
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleEvent = useCallback((event: CoreEvent) => {
    if (event.kind === "stream_frame") {
      setTiles((prev) => ({
        ...prev,
        [event.device_id]: {
          frame: `data:image/png;base64,${event.image}`,
          status: prev[event.device_id]?.status ?? null,
          error: null,
        },
      }));
    } else if (event.kind === "stream_status") {
      const status = event.status;
      setTiles((prev) => ({
        ...prev,
        [status.device_id]: {
          frame: prev[status.device_id]?.frame ?? null,
          status,
          error: status.state === "error" ? status.message : null,
        },
      }));
    }
  }, []);

  useEffect(() => {
    return onFarmEvent(handleEvent);
  }, [handleEvent]);

  const onlineDevices = devices.filter((device) => device.state === "device");

  const startStreams = async (ids: string[]) => {
    setError(null);
    for (const id of ids) {
      try {
        const status = await api.startStream(id, fps);
        setTiles((prev) => ({
          ...prev,
          [id]: { frame: prev[id]?.frame ?? null, status, error: null },
        }));
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
  };

  const stopStreams = async (ids: string[]) => {
    for (const id of ids) {
      await api.stopStream(id).catch(() => false);
    }
  };

  const toggleSelected = (serial: string) => {
    setSelected((prev) =>
      prev.includes(serial) ? prev.filter((id) => id !== serial) : [...prev, serial],
    );
  };

  const capture = async (serial: string) => {
    try {
      const result = await api.deviceScreenshot(serial);
      setNotice(`Saved ${result.width ?? "?"}×${result.height ?? "?"} screenshot to ${result.path}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const toggleRecording = async (serial: string, recording: boolean) => {
    try {
      if (recording) {
        const dir = await api.setRecording(serial, true);
        setRecordingPaths((prev) => ({ ...prev, [serial]: dir }));
        setNotice(`Recording PNG frames into ${dir}`);
      } else {
        const result = await api.finishRecording(serial);
        setNotice(`${result.note} ${result.path}`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const focusDevice = useMemo(
    () => onlineDevices.find((device) => device.serial === focus) ?? null,
    [focus, onlineDevices],
  );

  return (
    <div className="page">
      {error && (
        <Banner tone="danger" title="Streaming problem">
          {error}
        </Banner>
      )}
      {notice && (
        <Banner tone="info" title="Capture">
          {notice}
        </Banner>
      )}

      <Card
        title="Live device screens"
        subtitle="Frames come from adb exec-out screencap -p on the attached device"
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load()} loading={loading}>
              Rescan
            </Button>
            <Button
              size="sm"
              variant="ghost"
              icon="grid"
              onClick={() => changeLayout(layout === "compact" ? "comfortable" : "compact")}
            >
              {layout === "compact" ? "Compact" : "Comfortable"}
            </Button>
            <Button size="sm" variant="ghost" icon="list" onClick={() => setShowRecordings(true)}>
              Recordings
            </Button>
            <Button
              size="sm"
              variant="primary"
              icon="play"
              disabled={selected.length === 0}
              onClick={() => void startStreams(selected)}
            >
              Start ({selected.length})
            </Button>
            <Button
              size="sm"
              variant="ghost"
              icon="stop"
              disabled={selected.length === 0}
              onClick={() => void stopStreams(selected)}
            >
              Stop
            </Button>
          </>
        }
      >
        {onlineDevices.length === 0 ? (
          <EmptyState
            icon="phone"
            title="No online devices"
            body="Attach a device in the Android devices screen, then return here to stream its screen."
          />
        ) : (
          <div className="stack">
            <div className="chip-row">
              {onlineDevices.map((device) => {
                const active = selected.includes(device.serial);
                return (
                  <button
                    key={device.serial}
                    type="button"
                    className={`btn btn-sm ${active ? "btn-primary" : "btn-ghost"}`}
                    onClick={() => toggleSelected(device.serial)}
                  >
                    {device.label}
                  </button>
                );
              })}
            </div>
            <div className="form-row">
              <div style={{ width: 160 }}>
                <Field label="Frame rate" hint="screencap polling rate">
                  <select
                    className="select"
                    value={fps}
                    onChange={(event) => setFps(Number(event.target.value))}
                  >
                    {[1, 2, 3, 4, 6, 8, 10, 15].map((value) => (
                      <option key={value} value={value}>
                        {value} fps
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
              <Button
                icon="play"
                onClick={() => void startStreams(onlineDevices.map((device) => device.serial))}
              >
                Stream every online device
              </Button>
            </div>
          </div>
        )}
      </Card>

      {selected.length > 0 && (
        <div className={layout === "compact" ? "viewer-grid compact" : "viewer-grid"}>
          {selected.map((serial) => {
            const device = devices.find((item) => item.serial === serial);
            const tile = tiles[serial];
            return (
              <div className="device-tile" key={serial}>
                <div className="device-tile-head">
                  <div className="device-tile-title">
                    <Icon name="phone" size={13} />
                    <span>{device?.label ?? serial}</span>
                  </div>
                  <div className="row" style={{ gap: 4 }}>
                    {tile?.status?.recording && <span className="recording-dot" />}
                    <Pill
                      tone={
                        tile?.status?.state === "streaming"
                          ? "ok"
                          : tile?.status?.state === "error"
                            ? "danger"
                            : "muted"
                      }
                      dot={tile?.status?.state === "streaming"}
                    >
                      {tile?.status ? titleCase(tile.status.state) : "idle"}
                    </Pill>
                  </div>
                </div>
                <div className="device-screen">
                  {tile?.frame ? (
                    <ScreenSurface
                      frame={tile.frame}
                      serial={serial}
                      onError={setError}
                    />
                  ) : (
                    <div className="screen-placeholder">
                      <Icon name="screen" size={22} />
                      <span>
                        {tile?.status?.state === "connecting"
                          ? "Connecting to device…"
                          : "No frames yet — press Start"}
                      </span>
                    </div>
                  )}
                </div>
                <div className="device-tile-foot">
                  <span>
                    {tile?.status
                      ? `${tile.status.fps.toFixed(1)} fps · ${tile.status.frames} frames${tile.status.message ? ` · ${tile.status.message}` : ""}`
                      : "not streaming"}
                  </span>
                  <div className="tile-actions">
                    <IconButton
                      icon="expand"
                      label="Fullscreen"
                      onClick={() => setFocus(serial)}
                    />
                    <IconButton icon="camera" label="Screenshot" onClick={() => void capture(serial)} />
                    <IconButton
                      icon="record"
                      label={tile?.status?.recording ? "Stop recording" : "Record"}
                      onClick={() => void toggleRecording(serial, !tile?.status?.recording)}
                    />
                    <IconButton
                      icon="stop"
                      label="Stop stream"
                      variant="danger"
                      onClick={() => void stopStreams([serial])}
                    />
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {onlineDevices.length > 0 && selected.length === 0 && (
        <Spinner label="Select at least one device to view its screen." />
      )}

      {focusDevice && (
        <Modal
          open
          xl
          title={focusDevice.label}
          subtitle={`${focusDevice.serial} · real-time ADB frames`}
          onClose={() => setFocus(null)}
          footer={
            <Button variant="ghost" onClick={() => setFocus(null)}>
              Close
            </Button>
          }
        >
          <DeviceFocus
            device={focusDevice}
            tile={tiles[focusDevice.serial]}
            onError={setError}
            onStart={() => void startStreams([focusDevice.serial])}
            onStop={() => void stopStreams([focusDevice.serial])}
            onCapture={() => void capture(focusDevice.serial)}
            onRecord={(recording) => void toggleRecording(focusDevice.serial, recording)}
            recordingPath={recordingPaths[focusDevice.serial]}
          />
        </Modal>
      )}

      {showRecordings && (
        <RecordingsModal
          onClose={() => setShowRecordings(false)}
          onNotice={(message) => setNotice(message)}
        />
      )}
    </div>
  );
}

function RecordingsModal({
  onClose,
  onNotice,
}: {
  onClose: () => void;
  onNotice: (message: string) => void;
}) {
  const [files, setFiles] = useState<RecordingFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      setFiles(await api.recordingFiles());
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  return (
    <Modal
      open
      wide
      title="Recordings"
      subtitle="Encoded MP4/WebM files produced by the in-app recorder"
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
        <Spinner label="Scanning the recordings folder…" />
      ) : error ? (
        <InlineError message={error} />
      ) : files.length === 0 ? (
        <EmptyState
          icon="record"
          title="No recordings yet"
          body="Start a stream, press Record on a tile, then stop it. Finished files appear here."
        />
      ) : (
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>File</th>
                <th>Size</th>
                <th>Modified</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {files.map((file) => (
                <tr key={file.path}>
                  <td className="mono">{file.name}</td>
                  <td className="muted">{(file.bytes / (1024 * 1024)).toFixed(1)} MB</td>
                  <td className="muted nowrap">{file.modified ? new Date(file.modified).toLocaleString() : "—"}</td>
                  <td>
                    <div className="cell-actions">
                      <IconButton
                        icon="external"
                        label="Reveal in Explorer"
                        onClick={() => {
                          void api
                            .openInExplorer(file.path)
                            .catch((err) =>
                              setError(err instanceof Error ? err.message : String(err)),
                            );
                          onNotice(`Revealed ${file.name}`);
                        }}
                      />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Modal>
  );
}

function ScreenSurface({
  frame,
  serial,
  onError,
}: {
  frame: string;
  serial: string;
  onError: (message: string) => void;
}) {
  const imgRef = useRef<HTMLImageElement | null>(null);
  const start = useRef<{ x: number; y: number } | null>(null);

  const toDeviceCoords = (event: PointerEvent<HTMLImageElement>) => {
    const img = imgRef.current;
    if (!img) return null;
    const rect = img.getBoundingClientRect();
    const width = img.naturalWidth || rect.width;
    const height = img.naturalHeight || rect.height;
    const x = Math.round(((event.clientX - rect.left) / rect.width) * width);
    const y = Math.round(((event.clientY - rect.top) / rect.height) * height);
    if (x < 0 || y < 0 || x > width || y > height) return null;
    return { x, y };
  };

  return (
    <img
      ref={imgRef}
      src={frame}
      alt={`Live screen of ${serial}`}
      draggable={false}
      onPointerDown={(event) => {
        start.current = toDeviceCoords(event);
      }}
      onPointerUp={(event) => {
        const from = start.current;
        const to = toDeviceCoords(event);
        start.current = null;
        if (!from || !to) return;
        const distance = Math.hypot(to.x - from.x, to.y - from.y);
        if (distance < 16) {
          void api
            .deviceInput(serial, "tap", { a: to.x, b: to.y })
            .catch((err) => onError(err instanceof Error ? err.message : String(err)));
        } else {
          void api
            .deviceInput(serial, "swipe", { a: from.x, b: from.y, c: to.x, d: to.y })
            .catch((err) => onError(err instanceof Error ? err.message : String(err)));
        }
      }}
    />
  );
}

function DeviceFocus({
  device,
  tile,
  onError,
  onStart,
  onStop,
  onCapture,
  onRecord,
  recordingPath,
}: {
  device: Device;
  tile: TileState | undefined;
  onError: (message: string) => void;
  onStart: () => void;
  onStop: () => void;
  onCapture: () => void;
  onRecord: (recording: boolean) => void;
  recordingPath?: string;
}) {
  const [text, setText] = useState("");
  const resolution = parseResolution(tile?.status?.message ?? null);

  const send = (kind: string, value?: string) => {
    void api
      .deviceInput(device.serial, kind, value ? { text: value } : {})
      .catch((err) => onError(err instanceof Error ? err.message : String(err)));
  };

  return (
    <div className="stack">
      <div className="row-between">
        <div className="row">
          <Pill
            tone={tile?.status?.state === "streaming" ? "ok" : tile?.status?.state === "error" ? "danger" : "muted"}
            dot={tile?.status?.state === "streaming"}
          >
            {tile?.status ? titleCase(tile.status.state) : "idle"}
          </Pill>
          <span className="muted">
            {resolution ? `${resolution.width}×${resolution.height}` : "resolution unknown"}
            {tile?.status ? ` · ${tile.status.fps.toFixed(1)} fps` : ""}
          </span>
          {tile?.status?.recording && <Pill tone="danger" dot>recording</Pill>}
        </div>
        <div className="row">
          <Button size="sm" icon="play" onClick={onStart}>
            Start
          </Button>
          <Button size="sm" variant="ghost" icon="stop" onClick={onStop}>
            Stop
          </Button>
          <Button size="sm" variant="secondary" icon="camera" onClick={onCapture}>
            Screenshot
          </Button>
          <Button
            size="sm"
            variant={tile?.status?.recording ? "danger" : "secondary"}
            icon="record"
            onClick={() => onRecord(!tile?.status?.recording)}
          >
            {tile?.status?.recording ? "Stop recording" : "Record"}
          </Button>
        </div>
      </div>

      <div className="fullscreen-screen">
        {tile?.frame ? (
          <ScreenSurface frame={tile.frame} serial={device.serial} onError={onError} />
        ) : (
          <div className="screen-placeholder">
            <Icon name="screen" size={24} />
            <span>Start the stream to see this device.</span>
          </div>
        )}
      </div>

      {recordingPath && <div className="note">Recording frames: {recordingPath}</div>}

      <div className="input-bar">
        <Button size="sm" variant="ghost" onClick={() => send("back")}>
          Back
        </Button>
        <Button size="sm" variant="ghost" onClick={() => send("home")}>
          Home
        </Button>
        <Button size="sm" variant="ghost" onClick={() => send("app_switch")}>
          Recents
        </Button>
        <Button size="sm" variant="ghost" onClick={() => send("enter")}>
          Enter
        </Button>
        <input
          className="input"
          placeholder="Type into the focused field, then Enter"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && text) {
              send("text", text);
              setText("");
            }
          }}
        />
        <Button
          size="sm"
          variant="secondary"
          icon="key"
          disabled={!text}
          onClick={() => {
            send("text", text);
            setText("");
          }}
        >
          Send
        </Button>
      </div>
      <div className="note">
        Click to tap, drag to swipe. Input is delivered with <code>adb shell input</code>; the screen
        itself is polled at the configured frame rate.
      </div>
    </div>
  );
}
