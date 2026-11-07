import { useCallback, useEffect, useState } from "react";
import { Icon } from "../components/Icons";
import { Banner, Button, Card, EmptyState, Pill, Spinner, Stat } from "../components/ui";
import { api, isDesktop } from "../lib/api";
import { formatRelative, shortId, titleCase } from "../lib/format";
import type { Page } from "../lib/nav";
import type { Overview } from "../lib/types";

export function OverviewPage({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      setOverview(await api.overview());
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

  const configured = overview?.providers_configured ?? 0;
  const needsSetup = overview !== null && configured === 0;

  return (
    <div className="page">
      {error && <Banner tone="danger" title="Could not load the overview">{error}</Banner>}

      {needsSetup && (
        <Banner
          tone="info"
          title="Connect your first provider"
          action={
            <Button size="sm" variant="primary" onClick={() => onNavigate("settings")}>
              Open settings
            </Button>
          }
        >
          Add an API key for E2B, Boat, boxd, Daytona or Freestyle. The dashboard only shows numbers
          it reads from those APIs — there are no seeded or sample values.
        </Banner>
      )}

      <div className="stats">
        <Stat
          label="Active instances"
          value={overview ? overview.instances_running : "—"}
          hint={
            overview
              ? `${overview.instances_total} tracked across ${overview.providers_configured}/${overview.providers_total} providers`
              : "not loaded"
          }
          icon="server"
          tone={overview && overview.instances_running > 0 ? "ok" : "default"}
        />
        <Stat
          label="Android devices online"
          value={overview ? overview.devices_online : "—"}
          hint={
            overview
              ? `${overview.devices_total} attached over ADB`
              : "not loaded"
          }
          icon="phone"
          tone={overview && overview.devices_online > 0 ? "ok" : "default"}
        />
        <Stat
          label="Runs · 24 h"
          value={overview ? overview.runs_today : "—"}
          hint={
            overview
              ? overview.runs_failed_today > 0
                ? `${overview.runs_failed_today} failed`
                : "no failures"
              : "not loaded"
          }
          icon="workflow"
          tone={overview && overview.runs_failed_today > 0 ? "warn" : "default"}
        />
        <Stat
          label="Schedules active"
          value={overview ? overview.schedules_enabled : "—"}
          hint={overview ? `${overview.workflows} workflows defined` : "not loaded"}
          icon="clock"
        />
      </div>

      <Card
        title="Provider fleet"
        subtitle="Live counts from each provider API"
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load()} loading={loading}>
              Refresh
            </Button>
            <Button size="sm" variant="ghost" onClick={() => onNavigate("infrastructure")}>
              Manage
            </Button>
          </>
        }
      >
        {!overview ? (
          <Spinner label="Reading provider state…" />
        ) : (
          <div className="provider-grid">
            {overview.provider_breakdown.map((provider) => (
              <div key={provider.provider} className="provider-card">
                <div className="provider-head">
                  <span className="provider-name">
                    <Icon name="server" size={14} />
                    {provider.label}
                  </span>
                  {provider.configured ? (
                    <Pill tone={provider.error ? "warn" : "ok"} dot>
                      {provider.error ? "attention" : "connected"}
                    </Pill>
                  ) : (
                    <Pill tone="muted">not configured</Pill>
                  )}
                </div>
                <div className="provider-meta">
                  {provider.error
                    ? provider.error
                    : provider.configured
                      ? `${provider.running} running · ${provider.total} total`
                      : "Add an API key to start provisioning"}
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <div className="page-grid cols-2">
        <Card
          title="Recent runs"
          subtitle="Workflow executions, newest first"
          flush
          actions={
            <Button size="sm" variant="ghost" onClick={() => onNavigate("workflows")}>
              All workflows
            </Button>
          }
        >
          <div className="table-wrap">
            <table className="data">
              <thead>
                <tr>
                  <th>Workflow</th>
                  <th>Device</th>
                  <th>Status</th>
                  <th>Started</th>
                </tr>
              </thead>
              <tbody>
                {!overview || overview.recent_runs.length === 0 ? (
                  <tr>
                    <td colSpan={4}>
                      <div className="table-empty">
                        No runs yet. Build a workflow and run it against a device.
                      </div>
                    </td>
                  </tr>
                ) : (
                  overview.recent_runs.map((run) => (
                    <tr key={run.id}>
                      <td>
                        <div className="cell-main">
                          <span className="cell-title">{run.workflow_name}</span>
                        </div>
                        <div className="cell-sub">{shortId(run.id, 10)}</div>
                      </td>
                      <td className="mono">{run.device_id ?? "—"}</td>
                      <td>
                        <RunPill status={run.status} />
                      </td>
                      <td className="muted nowrap">{formatRelative(run.started_at)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </Card>

        <Card
          title="Activity"
          subtitle="Application log tail"
          flush
          actions={
            <Button size="sm" variant="ghost" onClick={() => onNavigate("logs")}>
              Open logs
            </Button>
          }
        >
          {!overview || overview.recent_logs.length === 0 ? (
            <EmptyState icon="list" title="Nothing logged yet" body="Activity appears here as providers, devices and workflows run." />
          ) : (
            <div>
              {overview.recent_logs.map((line) => (
                <div key={line.id} className="log-line">
                  <span className="log-time">{new Date(line.ts).toLocaleTimeString()}</span>
                  <span className={`log-level ${line.level}`}>{line.level}</span>
                  <span className="log-target">{titleCase(line.target)}</span>
                  <span className="log-message">{line.message}</span>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <Card title="Getting set up" subtitle="A short path from zero to an automated device farm">
        <div className="checklist">
          <SetupStep
            index="01"
            title="Connect a provider"
            body="Paste an API key in Settings and press Test connection. Only reachable APIs are used."
            done={configured > 0}
            action={
              <Button size="sm" variant="ghost" onClick={() => onNavigate("settings")}>
                Settings
              </Button>
            }
          />
          <SetupStep
            index="02"
            title="Attach Android devices"
            body="Install Android platform-tools and connect devices over USB or host:port ADB."
            done={(overview?.devices_total ?? 0) > 0}
            action={
              <Button size="sm" variant="ghost" onClick={() => onNavigate("devices")}>
                Devices
              </Button>
            }
          />
          <SetupStep
            index="03"
            title="Build a workflow"
            body="Chain ADB, WebDriver and HTTP steps with retries, then schedule it."
            done={(overview?.workflows ?? 0) > 0}
            action={
              <Button size="sm" variant="ghost" onClick={() => onNavigate("workflows")}>
                Workflows
              </Button>
            }
          />
        </div>
      </Card>
    </div>
  );
}

function SetupStep({
  index,
  title,
  body,
  done,
  action,
}: {
  index: string;
  title: string;
  body: string;
  done: boolean;
  action: React.ReactNode;
}) {
  return (
    <div className="setup-step">
      <span className={`setup-index ${done ? "done" : ""}`}>
        {done ? <Icon name="check" size={13} /> : index}
      </span>
      <div className="flex-1">
        <div className="setup-title">{title}</div>
        <div className="note">{body}</div>
      </div>
      {action}
    </div>
  );
}

export function RunPill({ status }: { status: string }) {
  const tone =
    status === "succeeded"
      ? "ok"
      : status === "failed"
        ? "danger"
        : status === "running"
          ? "info"
          : status === "cancelled"
            ? "warn"
            : "muted";
  return (
    <Pill tone={tone} dot={status === "running"}>
      {titleCase(status)}
    </Pill>
  );
}
