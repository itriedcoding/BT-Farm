import { useCallback, useEffect, useState } from "react";
import { Banner, Button, Card, Field, InlineNote, Pill, Spinner } from "../components/ui";
import { UpdateCard, formatBytes } from "../components/updates";
import { api, isDesktop } from "../lib/api";
import type { AppSettings, CoreInfo, ProviderStatus, StorageReport } from "../lib/types";

function StorageTable({
  onNotice,
  onError,
}: {
  onNotice: (message: string) => void;
  onError: (message: string) => void;
}) {
  const [report, setReport] = useState<StorageReport | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const load = async () => {
    try {
      setReport(await api.storageReport());
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const clean = async (key: string, days: number) => {
    setBusyKey(key);
    try {
      const result = await api.cleanStorage(key, days);
      onNotice(
        `Removed ${result.deleted_files} file(s) from ${result.key}, freeing ${formatBytes(result.freed_bytes)}.`,
      );
      await load();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <div className="table-wrap">
      <table className="data">
        <thead>
          <tr>
            <th>Area</th>
            <th>Files</th>
            <th>Size</th>
            <th>Clean up</th>
          </tr>
        </thead>
        <tbody>
          {!report ? (
            <tr>
              <td colSpan={4}>
                <Spinner label="Measuring the app's data folders…" />
              </td>
            </tr>
          ) : (
            report.entries.map((entry) => (
              <tr key={entry.key}>
                <td>
                  <div className="cell-main">{entry.label}</div>
                  <div className="cell-sub mono">{entry.path}</div>
                </td>
                <td className="muted">{entry.files}</td>
                <td className="muted">{formatBytes(entry.bytes)}</td>
                <td>
                  <div className="cell-actions">
                    <Button
                      size="sm"
                      variant="ghost"
                      loading={busyKey === entry.key}
                      disabled={entry.files === 0}
                      onClick={() => void clean(entry.key, 7)}
                    >
                      Older than 7 days
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      loading={busyKey === entry.key}
                      disabled={entry.files === 0}
                      onClick={() => {
                        if (window.confirm(`Delete every file in ${entry.path}?`)) {
                          void clean(entry.key, 0);
                        }
                      }}
                    >
                      Delete all
                    </Button>
                  </div>
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

export function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<CoreInfo | null>(null);
  const [providers, setProviders] = useState<ProviderStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  const load = useCallback(async () => {
    if (!isDesktop) return;
    try {
      const [appSettings, coreInfo, statuses] = await Promise.all([
        api.settings(),
        api.coreInfo(),
        api.providerStatuses(),
      ]);
      setSettings(appSettings);
      setInfo(coreInfo);
      setProviders(statuses);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const save = async () => {
    if (!settings) return;
    setSaving(true);
    setNotice(null);
    try {
      const updated = await api.saveSettings(settings);
      setSettings(updated);
      setNotice("Settings saved");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const testWebdriver = async () => {
    setTesting(true);
    setNotice(null);
    try {
      setNotice(await api.testWebdriver());
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setTesting(false);
    }
  };

  const pick = async (field: "adb_path" | "boxd_path" | "ffmpeg_path") => {
    const picked = await api.pickFile("Select executable", ["exe"]).catch(() => null);
    if (picked && settings) {
      setSettings({ ...settings, [field]: picked });
    }
  };

  if (!settings || !info) {
    return (
      <div className="page">
        {error ? (
          <Banner tone="danger" title="Settings unavailable">{error}</Banner>
        ) : (
          <Spinner label="Loading settings…" />
        )}
      </div>
    );
  }

  return (
    <div className="page">
      {error && <Banner tone="danger" title="Settings error">{error}</Banner>}
      {notice && <Banner tone="info" title="Settings">{notice}</Banner>}

      <Card
        title="Toolchain"
        subtitle="Paths to the native tools BT Farm drives"
        actions={
          <Button size="sm" variant="primary" icon="check" onClick={() => void save()} loading={saving}>
            Save
          </Button>
        }
      >
        <div className="form-grid">
          <div className="span-2">
            <Field
              label="adb executable"
              hint={info.adb_path ? `detected: ${info.adb_path}` : "not detected on PATH"}
            >
              <div className="form-row">
                <input
                  className="input mono"
                  value={settings.adb_path ?? ""}
                  onChange={(event) =>
                    setSettings({ ...settings, adb_path: event.target.value || null })
                  }
                  placeholder="C:\\Users\\you\\AppData\\Local\\Android\\Sdk\\platform-tools\\adb.exe"
                />
                <Button variant="ghost" onClick={() => void pick("adb_path")}>
                  Browse
                </Button>
              </div>
            </Field>
          </div>
          <div className="span-2">
            <Field
              label="boxd CLI"
              hint="required only for the boxd provider — it is driven through its official CLI"
            >
              <div className="form-row">
                <input
                  className="input mono"
                  value={settings.boxd_path ?? ""}
                  onChange={(event) =>
                    setSettings({ ...settings, boxd_path: event.target.value || null })
                  }
                  placeholder="C:\\Users\\you\\bin\\boxd.exe"
                />
                <Button variant="ghost" onClick={() => void pick("boxd_path")}>
                  Browse
                </Button>
              </div>
            </Field>
          </div>
          <div className="span-2">
            <Field
              label="Selenium / WebDriver endpoint"
              hint="W3C WebDriver base URL; can point at a Selenium container inside a provisioned VM"
            >
              <div className="form-row">
                <input
                  className="input mono"
                  value={settings.webdriver_url}
                  onChange={(event) => setSettings({ ...settings, webdriver_url: event.target.value })}
                  placeholder="http://127.0.0.1:4444"
                />
                <Button variant="secondary" onClick={() => void testWebdriver()} loading={testing}>
                  Test
                </Button>
              </div>
            </Field>
          </div>
          <Field label="Stream frame rate" hint="1–15 fps; adb screencap polling rate">
            <input
              className="input"
              type="number"
              min={1}
              max={15}
              value={settings.stream_fps}
              onChange={(event) =>
                setSettings({ ...settings, stream_fps: Number(event.target.value) || 4 })
              }
            />
          </Field>
          <Field label="ffmpeg executable" hint="optional — encodes recordings to MP4">
            <div className="form-row">
              <input
                className="input mono"
                value={settings.ffmpeg_path ?? ""}
                onChange={(event) =>
                  setSettings({ ...settings, ffmpeg_path: event.target.value || null })
                }
                placeholder="C:\\ffmpeg\\bin\\ffmpeg.exe"
              />
              <Button variant="ghost" onClick={() => void pick("ffmpeg_path")}>
                Browse
              </Button>
            </div>
          </Field>
        </div>
        <InlineNote>
          Recordings are captured as PNG frames next to the app data. With ffmpeg available the
          recording is encoded into an MP4 automatically.
        </InlineNote>
      </Card>

      <Card
        title="Providers"
        subtitle="Credentials are entered on the Infrastructure screen"
        flush
      >
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Provider</th>
                <th>Workload</th>
                <th>Terminal</th>
                <th>Transport</th>
                <th>Credential</th>
                <th>Android virtualisation</th>
                <th>Docs</th>
              </tr>
            </thead>
            <tbody>
              {providers.map((provider) => (
                <tr key={provider.provider}>
                  <td>{provider.label}</td>
                  <td>
                    <Pill tone="muted" title="What this provider actually runs">
                      {provider.capabilities.runtime}
                    </Pill>
                  </td>
                  <td className="muted small">{provider.capabilities.terminal}</td>
                  <td>
                    <Pill tone="muted">
                      {provider.capabilities.transport === "cli" ? "Official CLI" : "REST API"}
                    </Pill>
                  </td>
                  <td>
                    <Pill tone={provider.configured ? "ok" : "muted"}>
                      {provider.configured ? "configured" : "missing"}
                    </Pill>
                  </td>
                  <td>
                    <Pill tone={provider.capabilities.android ? "ok" : "warn"}>
                      {provider.capabilities.android ? "supported" : "not supported"}
                    </Pill>
                  </td>
                  <td>
                    <button
                      type="button"
                      className="btn btn-sm btn-ghost"
                      onClick={() => void api.openUrl(provider.docs_url)}
                    >
                      Open
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>

      <UpdateCard />

      <Card
        title="Local storage"
        subtitle="Screenshots, recordings, logs, downloaded updates and diagnostics written by this app"
        flush
      >
        <StorageTable onNotice={setNotice} onError={setError} />
      </Card>

      <Card title="Runtime" subtitle="Where the app keeps its data and how secrets are protected">
        <dl className="kv">
          <dt>Version</dt>
          <dd>{info.version}</dd>
          <dt>Platform</dt>
          <dd>{info.platform}</dd>
          <dt>Data directory</dt>
          <dd className="mono">{info.data_dir}</dd>
          <dt>Logs folder</dt>
          <dd className="mono">{info.log_dir}</dd>
          <dt>Credential storage</dt>
          <dd>
            {info.secret_backend === "dpapi" ? (
              <Pill tone="ok">Windows DPAPI (per-user)</Pill>
            ) : (
              <Pill tone="warn">file (0600) — non-Windows build</Pill>
            )}
          </dd>
        </dl>
        <div className="divider" />
        <div className="row">
          <Button variant="secondary" icon="external" onClick={() => void api.openInExplorer(info.data_dir)}>
            Open data folder
          </Button>
          <Button
            variant="ghost"
            icon="external"
            onClick={() => void api.openInExplorer(info.log_dir)}
          >
            Open logs
          </Button>
          <Button
            variant="secondary"
            icon="copy"
            onClick={async () => {
              try {
                const path = await api.diagnosticsReport();
                setNotice(`Diagnostics report written to ${path}`);
              } catch (err) {
                setError(err instanceof Error ? err.message : String(err));
              }
            }}
          >
            Write diagnostics report
          </Button>
        </div>
        <div style={{ marginTop: 12 }}>
          <InlineNote>
            API keys are sealed with DPAPI under your Windows account. They are never written to the
            log, never sent to the frontend, and never included in an error message — provider
            responses are redacted before they reach the UI.
          </InlineNote>
        </div>
      </Card>
    </div>
  );
}
