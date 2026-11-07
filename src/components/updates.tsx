import { useCallback, useEffect, useState } from "react";
import { Banner, Button, Card, InlineError, InlineNote, Pill, Spinner, Toggle } from "./ui";
import { api, isDesktop } from "../lib/api";
import { formatRelative } from "../lib/format";
import type { AppSettings, UpdateDownload, UpdateInfo } from "../lib/types";
import { toast } from "./toast";

const RELEASES_PAGE = "https://github.com/itriedcoding/BT-Farm/releases";

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} kB`;
  return `${bytes} B`;
}

/**
 * Real updater: asks the GitHub Releases API for the newest release of this
 * project, downloads the published installer, verifies its SHA-256 digest when
 * GitHub provides one, and only then launches it.
 */
export function UpdateCard() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [downloaded, setDownloaded] = useState<UpdateDownload | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [busy, setBusy] = useState<"check" | "download" | "install" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [savedToggle, setSavedToggle] = useState(false);

  useEffect(() => {
    if (!isDesktop) return;
    void api
      .settings()
      .then(setSettings)
      .catch(() => undefined);
  }, []);

  const check = useCallback(async () => {
    setBusy("check");
    setError(null);
    setDownloaded(null);
    try {
      const result = await api.checkUpdate();
      setInfo(result);
      if (result.available) {
        toast(`Version ${result.latest_version} is available`, "ok");
      } else {
        toast(
          result.note ?? `You are running the newest release (${result.current_version})`,
          "ok",
        );
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }, []);

  const download = async () => {
    setBusy("download");
    setError(null);
    try {
      const result = await api.downloadUpdate();
      setDownloaded(result);
      toast(result.note, result.verified ? "ok" : "info");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const install = async () => {
    setBusy("install");
    setError(null);
    try {
      const path = await api.installUpdate();
      toast(`Installer started: ${path}. BT Farm will close to let it run.`, "ok");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const toggleStartupCheck = async (value: boolean) => {
    if (!settings) return;
    const next = { ...settings, check_updates_on_start: value };
    setSettings(next);
    try {
      const saved = await api.saveSettings(next);
      setSettings(saved);
      setSavedToggle(true);
      window.setTimeout(() => setSavedToggle(false), 1800);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <Card
      title="Application updates"
      subtitle="Checked against this project's GitHub Releases page"
      actions={
        <Button size="sm" icon="refresh" onClick={() => void check()} loading={busy === "check"}>
          Check now
        </Button>
      }
    >
      {!isDesktop ? (
        <InlineNote>
          Update checks run inside the desktop shell. The browser preview has no backend to call.
        </InlineNote>
      ) : (
        <div className="stack">
          {settings && (
            <Toggle
              checked={settings.check_updates_on_start}
              onChange={(value) => void toggleStartupCheck(value)}
              label={
                savedToggle
                  ? "Check for updates on launch (saved)"
                  : "Check for updates on launch"
              }
            />
          )}

          {!info && !busy && (
            <InlineNote>
              Nothing checked yet in this session. The check downloads no code — it only reads the
              release metadata.
            </InlineNote>
          )}

          {busy === "check" && <Spinner label="Asking GitHub for the newest release…" />}

          {info && (
            <div className="stack-sm">
              <div className="row">
                <Pill tone="muted" title="Version installed on this machine">
                  installed {info.current_version}
                </Pill>
                <Pill tone={info.available ? "ok" : "muted"} title="Newest published release">
                  latest {info.latest_version}
                </Pill>
                {info.published_at && (
                  <span className="muted" style={{ fontSize: 11.5 }}>
                    published {formatRelative(info.published_at)}
                  </span>
                )}
                <Pill tone={info.available ? "warn" : "ok"}>
                  {info.available ? "update available" : "up to date"}
                </Pill>
              </div>

              {info.note && <InlineNote>{info.note}</InlineNote>}

              {info.release_name && (
                <div className="soft" style={{ fontSize: 12 }}>
                  {info.release_name}
                  {info.asset_name ? ` · asset ${info.asset_name}` : ""}
                  {info.asset_size ? ` (${formatBytes(info.asset_size)})` : ""}
                </div>
              )}

              {info.notes && <pre className="console release-notes">{info.notes}</pre>}

              <div className="row">
                <Button
                  variant="primary"
                  icon="down"
                  disabled={!info.available}
                  loading={busy === "download"}
                  onClick={() => void download()}
                >
                  Download installer
                </Button>
                <Button
                  variant="secondary"
                  icon="check"
                  disabled={!downloaded}
                  loading={busy === "install"}
                  onClick={() => void install()}
                >
                  Install and restart
                </Button>
                <Button
                  variant="ghost"
                  icon="external"
                  onClick={() => void api.openUrl(info.html_url || RELEASES_PAGE)}
                >
                  Open release page
                </Button>
              </div>

              {downloaded && (
                <Banner
                  tone={downloaded.verified ? "ok" : "warn"}
                  title={downloaded.verified ? "Installer verified" : "Installer downloaded"}
                >
                  {downloaded.note} Saved to <code>{downloaded.path}</code> (
                  {formatBytes(downloaded.bytes)}).
                </Banner>
              )}
            </div>
          )}

          <InlineError message={error} />
          <InlineNote>
            The updater only ever runs the installer it downloaded into the app's own{" "}
            <code>updates</code> folder, and it refuses any file whose SHA-256 digest does not match
            the release metadata.
          </InlineNote>
        </div>
      )}
    </Card>
  );
}
