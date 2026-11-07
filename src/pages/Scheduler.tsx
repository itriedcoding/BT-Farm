import { useCallback, useEffect, useState } from "react";
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
  Toggle,
} from "../components/ui";
import { api, isDesktop } from "../lib/api";
import { formatInterval } from "../lib/format";
import type { Device, Schedule, Workflow } from "../lib/types";

export function SchedulerPage() {
  const [schedules, setSchedules] = useState<Schedule[]>([]);
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<Schedule | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const [scheduleList, workflowList, deviceList] = await Promise.all([
        api.listSchedules(),
        api.listWorkflows(),
        api.devices(false).catch(() => [] as Device[]),
      ]);
      setSchedules(scheduleList);
      setWorkflows(workflowList);
      setDevices(deviceList);
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

  const toggle = async (schedule: Schedule, enabled: boolean) => {
    try {
      await api.saveSchedule({ ...schedule, enabled });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const remove = async (schedule: Schedule) => {
    try {
      await api.deleteSchedule(schedule.id);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="page">
      {error && <Banner tone="danger" title="Scheduler error">{error}</Banner>}

      <Card
        title="Scheduled workflows"
        subtitle="The engine ticks every 15 seconds and fires anything that is due"
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load()} loading={loading}>
              Refresh
            </Button>
            <Button size="sm" variant="primary" icon="plus" onClick={() => setCreating(true)}>
              New schedule
            </Button>
          </>
        }
        flush
      >
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Workflow</th>
                <th>Cadence</th>
                <th>Device</th>
                <th>Next run</th>
                <th>Last result</th>
                <th>Enabled</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {loading && schedules.length === 0 ? (
                <tr>
                  <td colSpan={7}>
                    <Spinner label="Loading schedules…" />
                  </td>
                </tr>
              ) : schedules.length === 0 ? (
                <tr>
                  <td colSpan={7}>
                    <EmptyState
                      icon="clock"
                      title="No schedules"
                      body="Schedule a workflow to run every N minutes or once a day."
                    />
                  </td>
                </tr>
              ) : (
                schedules.map((schedule) => (
                  <tr key={schedule.id}>
                    <td>{schedule.workflow_name}</td>
                    <td className="muted">{formatInterval(schedule.every_minutes, schedule.daily_at)}</td>
                    <td className="mono muted">{schedule.device_id ?? "workflow default"}</td>
                    <td className="muted nowrap">
                      {schedule.enabled ? formatRelativeFuture(schedule.next_run_at) : "paused"}
                    </td>
                    <td>
                      {schedule.last_status ? (
                        <Pill
                          tone={
                            schedule.last_status === "started"
                              ? "ok"
                              : schedule.last_status.includes("missing")
                                ? "warn"
                                : "danger"
                          }
                        >
                          {schedule.last_status.length > 30
                            ? `${schedule.last_status.slice(0, 29)}…`
                            : schedule.last_status}
                        </Pill>
                      ) : (
                        <span className="muted">never</span>
                      )}
                    </td>
                    <td>
                      <Toggle
                        checked={schedule.enabled}
                        onChange={(enabled) => void toggle(schedule, enabled)}
                        label=""
                      />
                    </td>
                    <td>
                      <div className="cell-actions">
                        <IconButton icon="settings" label="Edit" onClick={() => setEditing(schedule)} />
                        <IconButton
                          icon="trash"
                          label="Delete"
                          variant="danger"
                          onClick={() => void remove(schedule)}
                        />
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </Card>

      <InlineNote>
        Schedules run only while BT Farm is open — the desktop app is the scheduler host. Every fire
        is recorded as a workflow run with its own log.
      </InlineNote>

      {(creating || editing) && (
        <ScheduleModal
          schedule={
            editing ?? {
              id: "",
              workflow_id: workflows[0]?.id ?? "",
              workflow_name: workflows[0]?.name ?? "",
              device_id: null,
              every_minutes: 60,
              daily_at: null,
              enabled: true,
              next_run_at: null,
              last_run_at: null,
              last_status: null,
            }
          }
          workflows={workflows}
          devices={devices}
          onClose={() => {
            setCreating(false);
            setEditing(null);
          }}
          onSaved={async () => {
            setCreating(false);
            setEditing(null);
            await load();
          }}
        />
      )}
    </div>
  );
}

function formatRelativeFuture(iso: string | null): string {
  if (!iso) return "not scheduled";
  const diff = new Date(iso).getTime() - Date.now();
  if (Number.isNaN(diff)) return "not scheduled";
  if (diff <= 0) return "due now";
  const minutes = Math.round(diff / 60000);
  if (minutes < 60) return `in ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `in ${hours} h`;
  return `in ${Math.round(hours / 24)} d`;
}

function ScheduleModal({
  schedule,
  workflows,
  devices,
  onClose,
  onSaved,
}: {
  schedule: Schedule;
  workflows: Workflow[];
  devices: Device[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState<Schedule>(schedule);
  const [mode, setMode] = useState<"interval" | "daily">(
    schedule.daily_at ? "daily" : "interval",
  );
  const [interval, setInterval] = useState(String(schedule.every_minutes || 60));
  const [dailyAt, setDailyAt] = useState(schedule.daily_at ?? "09:00");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.saveSchedule({
        ...draft,
        every_minutes: mode === "interval" ? Math.max(1, Number(interval) || 60) : 0,
        daily_at: mode === "daily" ? dailyAt : null,
        device_id: draft.device_id || null,
      });
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
      title={schedule.id ? "Edit schedule" : "New schedule"}
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
      <Field label="Workflow">
        <select
          className="select"
          value={draft.workflow_id}
          onChange={(event) => setDraft({ ...draft, workflow_id: event.target.value })}
        >
          {workflows.length === 0 && <option value="">No workflows available</option>}
          {workflows.map((workflow) => (
            <option key={workflow.id} value={workflow.id}>
              {workflow.name}
            </option>
          ))}
        </select>
      </Field>
      <Field label="Device" hint="optional">
        <select
          className="select"
          value={draft.device_id ?? ""}
          onChange={(event) => setDraft({ ...draft, device_id: event.target.value || null })}
        >
          <option value="">Workflow default</option>
          {devices.map((device) => (
            <option key={device.serial} value={device.serial}>
              {device.label} ({device.state})
            </option>
          ))}
        </select>
      </Field>
      <div className="form-grid">
        <Field label="Cadence">
          <select
            className="select"
            value={mode}
            onChange={(event) => setMode(event.target.value as "interval" | "daily")}
          >
            <option value="interval">Every N minutes</option>
            <option value="daily">Once a day</option>
          </select>
        </Field>
        {mode === "interval" ? (
          <Field label="Minutes">
            <input
              className="input"
              type="number"
              min={1}
              value={interval}
              onChange={(event) => setInterval(event.target.value)}
            />
          </Field>
        ) : (
          <Field label="Time" hint="local time, 24h">
            <input
              className="input"
              type="time"
              value={dailyAt}
              onChange={(event) => setDailyAt(event.target.value)}
            />
          </Field>
        )}
      </div>
      <Toggle
        checked={draft.enabled}
        onChange={(enabled) => setDraft({ ...draft, enabled })}
        label="Enabled"
      />
      <InlineError message={error} />
    </Modal>
  );
}
