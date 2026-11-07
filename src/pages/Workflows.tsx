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
  Spinner,
} from "../components/ui";
import { RunPill } from "./Overview";
import { api, isDesktop, onFarmEvent } from "../lib/api";
import { formatRelative, shortId, titleCase } from "../lib/format";
import type { Device, RunStatus, Step, StepAction, Workflow, WorkflowRun } from "../lib/types";

const STEP_KINDS: { kind: StepAction["kind"]; label: string; group: string }[] = [
  { kind: "tap", label: "Tap", group: "Android" },
  { kind: "swipe", label: "Swipe", group: "Android" },
  { kind: "text", label: "Type text", group: "Android" },
  { kind: "key", label: "Key event", group: "Android" },
  { kind: "launch_app", label: "Launch app", group: "Android" },
  { kind: "stop_app", label: "Stop app", group: "Android" },
  { kind: "install_apk", label: "Install APK", group: "Android" },
  { kind: "shell", label: "ADB shell", group: "Android" },
  { kind: "wait_for_activity", label: "Wait for activity", group: "Android" },
  { kind: "screenshot", label: "Device screenshot", group: "Android" },
  { kind: "wd_navigate", label: "Browser: navigate", group: "WebDriver" },
  { kind: "wd_click", label: "Browser: click", group: "WebDriver" },
  { kind: "wd_type", label: "Browser: type", group: "WebDriver" },
  { kind: "wd_wait_for", label: "Browser: wait for", group: "WebDriver" },
  { kind: "wd_assert_text", label: "Browser: assert text", group: "WebDriver" },
  { kind: "wd_screenshot", label: "Browser: screenshot", group: "WebDriver" },
  { kind: "http_request", label: "HTTP request", group: "Integration" },
  { kind: "wait", label: "Wait", group: "Flow" },
  { kind: "notify", label: "Note", group: "Flow" },
];

export function defaultAction(kind: StepAction["kind"]): StepAction {
  switch (kind) {
    case "tap":
      return { kind, x: 540, y: 1200 };
    case "swipe":
      return { kind, x1: 540, y1: 1600, x2: 540, y2: 600, duration_ms: 300 };
    case "text":
      return { kind, text: "" };
    case "key":
      return { kind, code: "KEYCODE_HOME" };
    case "launch_app":
      return { kind, package: "com.android.settings" };
    case "stop_app":
      return { kind, package: "com.android.settings" };
    case "install_apk":
      return { kind, path: "" };
    case "shell":
      return { kind, command: "dumpsys battery" };
    case "wait_for_activity":
      return { kind, package: "com.android.settings", timeout_ms: 20000 };
    case "screenshot":
      return { kind, label: "device" };
    case "wd_navigate":
      return { kind, url: "https://example.com" };
    case "wd_click":
      return { kind, selector: "#submit" };
    case "wd_type":
      return { kind, selector: "input[name=q]", text: "" };
    case "wd_wait_for":
      return { kind, selector: "h1", timeout_ms: 15000 };
    case "wd_assert_text":
      return { kind, selector: "h1", contains: "Example" };
    case "wd_screenshot":
      return { kind, label: "browser" };
    case "http_request":
      return {
        kind,
        method: "POST",
        url: "https://example.com/webhook",
        headers: { "Content-Type": "application/json" },
        body: '{"event":"workflow"}',
        expect_status: 200,
      };
    case "wait":
      return { kind, ms: 1000 };
    case "notify":
      return { kind, message: "checkpoint reached" };
  }
}

function emptyWorkflow(): Workflow {
  const now = new Date().toISOString();
  return {
    id: "",
    name: "New workflow",
    description: "",
    target_device: null,
    steps: [],
    created_at: now,
    updated_at: now,
  };
}

export function WorkflowsPage() {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [runs, setRuns] = useState<WorkflowRun[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [editing, setEditing] = useState<Workflow | null>(null);
  const [runTarget, setRunTarget] = useState<Workflow | null>(null);
  const [activeRun, setActiveRun] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [runFilter, setRunFilter] = useState<string>("");

  const load = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const [list, recent, deviceList] = await Promise.all([
        api.listWorkflows(),
        api.runs(30, runFilter || null),
        api.devices(false).catch(() => [] as Device[]),
      ]);
      setWorkflows(list);
      setRuns(recent);
      setDevices(deviceList);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [runFilter]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    return onFarmEvent((event) => {
      if (event.kind === "run_update" && event.status !== "running") {
        void api.runs(30, runFilter || null).then(setRuns).catch(() => undefined);
      }
    });
  }, [runFilter]);

  const duplicate = async (workflow: Workflow) => {
    try {
      const copy = await api.duplicateWorkflow(workflow.id);
      setNotice(`Duplicated as "${copy.name}"`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const exportOne = async (workflow: Workflow) => {
    try {
      const path = await api.pickSaveFile(
        "Export workflow as JSON",
        `${workflow.name.replace(/[^a-z0-9-_]+/gi, "-").toLowerCase()}.json`,
        ["json"],
      );
      if (!path) return;
      await api.exportWorkflow(workflow.id, path);
      setNotice(`Exported to ${path}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const importOne = async () => {
    try {
      const picked = await api.pickFile("Import a workflow JSON file", ["json"]);
      if (!picked) return;
      const imported = await api.importWorkflow(picked);
      setNotice(`Imported "${imported.name}" with ${imported.steps.length} step(s)`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const remove = async (workflow: Workflow) => {
    if (!window.confirm(`Delete "${workflow.name}"? Its schedules are removed too.`)) return;
    try {
      await api.deleteWorkflow(workflow.id);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="page">
      {error && <Banner tone="danger" title="Workflow error">{error}</Banner>}
      {notice && <Banner tone="info" title="Run queued">{notice}</Banner>}

      <Card
        title="Workflows"
        subtitle="Reusable step sequences executed against a device and/or a WebDriver endpoint"
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load()} loading={loading}>
              Refresh
            </Button>
            <Button size="sm" variant="ghost" icon="down" onClick={() => void importOne()}>
              Import
            </Button>
            <Button size="sm" variant="primary" icon="plus" onClick={() => setEditing(emptyWorkflow())}>
              New workflow
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
                <th>Steps</th>
                <th>Target device</th>
                <th>Updated</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {workflows.length === 0 ? (
                <tr>
                  <td colSpan={5}>
                    <EmptyState
                      icon="workflow"
                      title="No workflows yet"
                      body="Chain ADB taps, app launches, WebDriver checks and HTTP calls into one runnable sequence."
                    />
                  </td>
                </tr>
              ) : (
                workflows.map((workflow) => (
                  <tr key={workflow.id}>
                    <td>
                      <div className="cell-main">
                        <Icon name="workflow" size={14} />
                        <span className="cell-title">{workflow.name}</span>
                      </div>
                      <div className="cell-sub">{workflow.description || shortId(workflow.id, 12)}</div>
                    </td>
                    <td className="muted">{workflow.steps.length}</td>
                    <td className="mono muted">{workflow.target_device ?? "choose at run time"}</td>
                    <td className="muted nowrap">{formatRelative(workflow.updated_at)}</td>
                    <td>
                      <div className="cell-actions">
                        <IconButton icon="play" label="Run" onClick={() => setRunTarget(workflow)} />
                        <IconButton icon="settings" label="Edit" onClick={() => setEditing(workflow)} />
                        <IconButton
                          icon="copy"
                          label="Duplicate"
                          onClick={() => void duplicate(workflow)}
                        />
                        <IconButton
                          icon="external"
                          label="Export as JSON"
                          onClick={() => void exportOne(workflow)}
                        />
                        <IconButton
                          icon="trash"
                          label="Delete"
                          variant="danger"
                          onClick={() => void remove(workflow)}
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

      <Card
        title="Recent runs"
        subtitle="Live status is pushed from the automation engine"
        flush
        actions={
          <select
            className="select select-sm"
            value={runFilter}
            onChange={(event) => setRunFilter(event.target.value)}
            aria-label="Filter runs by status"
          >
            <option value="">All statuses</option>
            <option value="running">Running</option>
            <option value="succeeded">Succeeded</option>
            <option value="failed">Failed</option>
            <option value="cancelled">Cancelled</option>
            <option value="queued">Queued</option>
          </select>
        }
      >
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Run</th>
                <th>Workflow</th>
                <th>Device</th>
                <th>Status</th>
                <th>Trigger</th>
                <th>Started</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {runs.length === 0 ? (
                <tr>
                  <td colSpan={7}>
                    <div className="table-empty">
                      {runFilter ? `No runs with status “${runFilter}”.` : "No runs recorded yet."}
                    </div>
                  </td>
                </tr>
              ) : (
                runs.map((run) => (
                  <tr key={run.id}>
                    <td className="cell-sub">{shortId(run.id, 10)}</td>
                    <td>{run.workflow_name}</td>
                    <td className="mono muted">{run.device_id ?? "—"}</td>
                    <td>
                      <RunPill status={run.status} />
                    </td>
                    <td className="muted">{run.trigger}</td>
                    <td className="muted nowrap">{formatRelative(run.started_at)}</td>
                    <td>
                      <div className="cell-actions">
                        <IconButton icon="list" label="Open log" onClick={() => setActiveRun(run.id)} />
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </Card>

      {editing && (
        <WorkflowEditor
          workflow={editing}
          devices={devices}
          onClose={() => setEditing(null)}
          onSaved={async (saved) => {
            setEditing(null);
            setNotice(`Saved "${saved.name}"`);
            await load();
          }}
        />
      )}

      {runTarget && (
        <RunModal
          workflow={runTarget}
          devices={devices}
          onClose={() => setRunTarget(null)}
          onStarted={async (runId) => {
            setRunTarget(null);
            setActiveRun(runId);
            setNotice(`Run ${shortId(runId, 8)} queued`);
            await load();
          }}
        />
      )}

      {activeRun && (
        <RunLogModal
          runId={activeRun}
          onClose={() => setActiveRun(null)}
          onStarted={(id) => setActiveRun(id)}
        />
      )}
    </div>
  );
}

function WorkflowEditor({
  workflow,
  devices,
  onClose,
  onSaved,
}: {
  workflow: Workflow;
  devices: Device[];
  onClose: () => void;
  onSaved: (workflow: Workflow) => void;
}) {
  const [draft, setDraft] = useState<Workflow>(workflow);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [picker, setPicker] = useState(false);

  const updateStep = (index: number, patch: Partial<Step>) => {
    setDraft((prev) => {
      const steps = [...prev.steps];
      steps[index] = { ...steps[index], ...patch };
      return { ...prev, steps };
    });
  };

  const updateAction = (index: number, action: StepAction) => {
    updateStep(index, { action });
  };

  const move = (index: number, delta: number) => {
    setDraft((prev) => {
      const target = index + delta;
      if (target < 0 || target >= prev.steps.length) return prev;
      const steps = [...prev.steps];
      const [item] = steps.splice(index, 1);
      steps.splice(target, 0, item);
      return { ...prev, steps };
    });
  };

  const addStep = (kind: StepAction["kind"]) => {
    const action = defaultAction(kind);
    const label = STEP_KINDS.find((item) => item.kind === kind)?.label ?? kind;
    setDraft((prev) => ({
      ...prev,
      steps: [
        ...prev.steps,
        {
          id: crypto.randomUUID(),
          name: label,
          action,
          retries: 0,
          timeout_ms: null,
          continue_on_error: false,
        },
      ],
    }));
    setPicker(false);
  };

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const saved = await api.saveWorkflow(draft);
      onSaved(saved);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open
      xl
      title={draft.id ? "Edit workflow" : "New workflow"}
      subtitle="Every step is validated before it can be saved"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" icon="check" onClick={() => void save()} loading={saving}>
            Save workflow
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <Field label="Name">
          <input
            className="input"
            value={draft.name}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          />
        </Field>
        <Field label="Default device" hint="optional — can be chosen per run">
          <select
            className="select"
            value={draft.target_device ?? ""}
            onChange={(event) =>
              setDraft({ ...draft, target_device: event.target.value || null })
            }
          >
            <option value="">Choose at run time</option>
            {devices.map((device) => (
              <option key={device.serial} value={device.serial}>
                {device.label} ({device.state})
              </option>
            ))}
          </select>
        </Field>
        <div className="span-2">
          <Field label="Description" hint="optional">
            <input
              className="input"
              value={draft.description}
              onChange={(event) => setDraft({ ...draft, description: event.target.value })}
              placeholder="Smoke test the checkout flow"
            />
          </Field>
        </div>
      </div>

      <div className="row-between">
        <span className="muted">{draft.steps.length} steps</span>
        <div className="row">
          <Button size="sm" icon="plus" onClick={() => setPicker(true)}>
            Add step
          </Button>
        </div>
      </div>

      {draft.steps.length === 0 ? (
        <EmptyState
          icon="workflow"
          title="No steps"
          body="Add a step to start building the automation."
        />
      ) : (
        <div className="step-list">
          {draft.steps.map((step, index) => (
            <div className="step-card" key={step.id || index}>
              <div className="step-head">
                <span className="step-index">{index + 1}</span>
                <input
                  className="input"
                  style={{ maxWidth: 280 }}
                  value={step.name}
                  onChange={(event) => updateStep(index, { name: event.target.value })}
                  aria-label="Step name"
                />
                <span className="step-kind">{titleCase(step.action.kind)}</span>
                <div className="step-toolbar">
                  <IconButton icon="up" label="Move up" onClick={() => move(index, -1)} />
                  <IconButton icon="down" label="Move down" onClick={() => move(index, 1)} />
                  <IconButton
                    icon="trash"
                    label="Remove step"
                    variant="danger"
                    onClick={() =>
                      setDraft((prev) => ({
                        ...prev,
                        steps: prev.steps.filter((_, i) => i !== index),
                      }))
                    }
                  />
                </div>
              </div>
              <StepFields action={step.action} onChange={(action) => updateAction(index, action)} />
              <div className="step-grid">
                <Field label="Retries" hint="on failure">
                  <input
                    className="input"
                    type="number"
                    min={0}
                    max={10}
                    value={step.retries}
                    onChange={(event) =>
                      updateStep(index, { retries: Math.max(0, Number(event.target.value) || 0) })
                    }
                  />
                </Field>
                <Field label="Timeout (ms)" hint="blank uses the step default">
                  <input
                    className="input"
                    type="number"
                    min={500}
                    value={step.timeout_ms ?? ""}
                    onChange={(event) =>
                      updateStep(index, {
                        timeout_ms: event.target.value ? Number(event.target.value) : null,
                      })
                    }
                  />
                </Field>
                <div className="span-2">
                  <label className="checkline">
                    <input
                      type="checkbox"
                      checked={step.continue_on_error}
                      onChange={(event) =>
                        updateStep(index, { continue_on_error: event.target.checked })
                      }
                    />
                    Continue the run if this step fails
                  </label>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      <InlineError message={error} />

      <Modal open={picker} title="Add a step" onClose={() => setPicker(false)} wide>
        <div className="stack">
          {["Android", "WebDriver", "Integration", "Flow"].map((group) => (
            <div key={group}>
              <div className="section-title">
                <h3>{group}</h3>
              </div>
              <div className="chip-row">
                {STEP_KINDS.filter((item) => item.group === group).map((item) => (
                  <button
                    key={item.kind}
                    type="button"
                    className="btn btn-sm btn-secondary"
                    onClick={() => addStep(item.kind)}
                  >
                    {item.label}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      </Modal>
    </Modal>
  );
}

function StepFields({
  action,
  onChange,
}: {
  action: StepAction;
  onChange: (action: StepAction) => void;
}) {
  const numberField = (
    label: string,
    value: number,
    set: (value: number) => void,
    hint?: string,
  ) => (
    <Field label={label} hint={hint}>
      <input
        className="input"
        type="number"
        value={Number.isFinite(value) ? value : 0}
        onChange={(event) => set(Number(event.target.value) || 0)}
      />
    </Field>
  );

  switch (action.kind) {
    case "tap":
      return (
        <div className="step-grid">
          {numberField("X", action.x, (x) => onChange({ ...action, x }))}
          {numberField("Y", action.y, (y) => onChange({ ...action, y }))}
        </div>
      );
    case "swipe":
      return (
        <div className="step-grid">
          {numberField("From X", action.x1, (x1) => onChange({ ...action, x1 }))}
          {numberField("From Y", action.y1, (y1) => onChange({ ...action, y1 }))}
          {numberField("To X", action.x2, (x2) => onChange({ ...action, x2 }))}
          {numberField("To Y", action.y2, (y2) => onChange({ ...action, y2 }))}
          {numberField("Duration (ms)", action.duration_ms, (duration_ms) =>
            onChange({ ...action, duration_ms }),
          )}
        </div>
      );
    case "text":
      return (
        <Field label="Text" hint="newlines are not supported by adb input">
          <input
            className="input"
            value={action.text}
            onChange={(event) => onChange({ ...action, text: event.target.value })}
          />
        </Field>
      );
    case "key":
      return (
        <Field label="Keycode" hint="e.g. KEYCODE_ENTER, KEYCODE_HOME">
          <input
            className="input mono"
            value={action.code}
            onChange={(event) => onChange({ ...action, code: event.target.value })}
          />
        </Field>
      );
    case "launch_app":
    case "stop_app":
      return (
        <Field label="Package">
          <input
            className="input mono"
            value={action.package}
            onChange={(event) => onChange({ ...action, package: event.target.value })}
          />
        </Field>
      );
    case "install_apk":
      return (
        <div className="form-row">
          <div className="flex-1">
            <Field label="APK path">
              <input
                className="input mono"
                value={action.path}
                onChange={(event) => onChange({ ...action, path: event.target.value })}
              />
            </Field>
          </div>
          <Button
            variant="ghost"
            onClick={async () => {
              const picked = await api.pickFile("Select an APK", ["apk"]).catch(() => null);
              if (picked) onChange({ ...action, path: picked });
            }}
          >
            Browse
          </Button>
        </div>
      );
    case "shell":
      return (
        <Field label="Shell command" hint="runs through adb shell">
          <input
            className="input mono"
            value={action.command}
            onChange={(event) => onChange({ ...action, command: event.target.value })}
          />
        </Field>
      );
    case "wait_for_activity":
      return (
        <div className="step-grid">
          <Field label="Package">
            <input
              className="input mono"
              value={action.package}
              onChange={(event) => onChange({ ...action, package: event.target.value })}
            />
          </Field>
          {numberField("Timeout (ms)", action.timeout_ms, (timeout_ms) =>
            onChange({ ...action, timeout_ms }),
          )}
        </div>
      );
    case "screenshot":
    case "wd_screenshot":
      return (
        <Field label="Label" hint="used in the artefact file name">
          <input
            className="input"
            value={action.label}
            onChange={(event) => onChange({ ...action, label: event.target.value })}
          />
        </Field>
      );
    case "wd_navigate":
      return (
        <Field label="URL">
          <input
            className="input mono"
            value={action.url}
            onChange={(event) => onChange({ ...action, url: event.target.value })}
          />
        </Field>
      );
    case "wd_click":
      return (
        <Field label="CSS selector">
          <input
            className="input mono"
            value={action.selector}
            onChange={(event) => onChange({ ...action, selector: event.target.value })}
          />
        </Field>
      );
    case "wd_type":
      return (
        <div className="step-grid">
          <Field label="CSS selector">
            <input
              className="input mono"
              value={action.selector}
              onChange={(event) => onChange({ ...action, selector: event.target.value })}
            />
          </Field>
          <Field label="Text">
            <input
              className="input"
              value={action.text}
              onChange={(event) => onChange({ ...action, text: event.target.value })}
            />
          </Field>
        </div>
      );
    case "wd_wait_for":
      return (
        <div className="step-grid">
          <Field label="CSS selector">
            <input
              className="input mono"
              value={action.selector}
              onChange={(event) => onChange({ ...action, selector: event.target.value })}
            />
          </Field>
          {numberField("Timeout (ms)", action.timeout_ms, (timeout_ms) =>
            onChange({ ...action, timeout_ms }),
          )}
        </div>
      );
    case "wd_assert_text":
      return (
        <div className="step-grid">
          <Field label="CSS selector">
            <input
              className="input mono"
              value={action.selector}
              onChange={(event) => onChange({ ...action, selector: event.target.value })}
            />
          </Field>
          <Field label="Must contain">
            <input
              className="input"
              value={action.contains}
              onChange={(event) => onChange({ ...action, contains: event.target.value })}
            />
          </Field>
        </div>
      );
    case "http_request":
      return (
        <div className="step-grid">
          <Field label="Method">
            <select
              className="select"
              value={action.method}
              onChange={(event) => onChange({ ...action, method: event.target.value })}
            >
              {["GET", "POST", "PUT", "PATCH", "DELETE"].map((method) => (
                <option key={method} value={method}>
                  {method}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Expected status" hint="blank accepts any 2xx/3xx">
            <input
              className="input"
              type="number"
              value={action.expect_status ?? ""}
              onChange={(event) =>
                onChange({
                  ...action,
                  expect_status: event.target.value ? Number(event.target.value) : null,
                })
              }
            />
          </Field>
          <div className="span-2">
            <Field label="URL">
              <input
                className="input mono"
                value={action.url}
                onChange={(event) => onChange({ ...action, url: event.target.value })}
              />
            </Field>
          </div>
          <div className="span-2">
            <Field label="Headers" hint="JSON object">
              <textarea
                className="textarea mono"
                value={JSON.stringify(action.headers, null, 2)}
                onChange={(event) => {
                  try {
                    const parsed = JSON.parse(event.target.value) as Record<string, string>;
                    onChange({ ...action, headers: parsed });
                  } catch {
                    /* keep typing; invalid JSON is not committed */
                  }
                }}
              />
            </Field>
          </div>
          <div className="span-2">
            <Field label="Body">
              <textarea
                className="textarea mono"
                value={action.body ?? ""}
                onChange={(event) =>
                  onChange({ ...action, body: event.target.value || null })
                }
              />
            </Field>
          </div>
        </div>
      );
    case "wait":
      return (
        <div className="step-grid">
          {numberField("Duration (ms)", action.ms, (ms) => onChange({ ...action, ms }))}
        </div>
      );
    case "notify":
      return (
        <Field label="Message">
          <input
            className="input"
            value={action.message}
            onChange={(event) => onChange({ ...action, message: event.target.value })}
          />
        </Field>
      );
  }
}

function RunModal({
  workflow,
  devices,
  onClose,
  onStarted,
}: {
  workflow: Workflow;
  devices: Device[];
  onClose: () => void;
  onStarted: (runId: string) => void;
}) {
  const [deviceId, setDeviceId] = useState(workflow.target_device ?? devices[0]?.serial ?? "");
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const needsDevice = workflow.steps.some((step) =>
    [
      "tap",
      "swipe",
      "text",
      "key",
      "shell",
      "launch_app",
      "stop_app",
      "install_apk",
      "wait_for_activity",
      "screenshot",
    ].includes(step.action.kind),
  );

  const start = async () => {
    setStarting(true);
    setError(null);
    try {
      const runId = await api.runWorkflow(workflow.id, deviceId || null);
      onStarted(runId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setStarting(false);
    }
  };

  return (
    <Modal
      open
      title={`Run "${workflow.name}"`}
      subtitle={`${workflow.steps.length} steps`}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" icon="play" onClick={() => void start()} loading={starting}>
            Start run
          </Button>
        </>
      }
    >
      <Field label="Device" hint={needsDevice ? "required by this workflow" : "optional"}>
        <select className="select" value={deviceId} onChange={(event) => setDeviceId(event.target.value)}>
          <option value="">No device</option>
          {devices.map((device) => (
            <option key={device.serial} value={device.serial}>
              {device.label} ({device.state})
            </option>
          ))}
        </select>
      </Field>
      {needsDevice && devices.length === 0 && (
        <InlineNote>
          No Android devices are attached. Connect one on the Android devices screen first.
        </InlineNote>
      )}
      <InlineError message={error} />
    </Modal>
  );
}

function RunLogModal({
  runId,
  onClose,
  onStarted,
}: {
  runId: string;
  onClose: () => void;
  onStarted: (runId: string) => void;
}) {
  const [run, setRun] = useState<WorkflowRun | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [status, setStatus] = useState<RunStatus>("queued");

  useEffect(() => {
    let cancelled = false;
    void api
      .getRun(runId)
      .then((result) => {
        if (cancelled || !result) return;
        setRun(result);
        setLines(result.lines);
        setStatus(result.status);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [runId]);

  useEffect(() => {
    return onFarmEvent((event) => {
      if (event.kind !== "run_update" || event.run_id !== runId) return;
      setStatus(event.status);
      if (event.line) {
        setLines((prev) => [...prev, event.line!]);
      }
      if (event.error) {
        setLines((prev) => [...prev, `error: ${event.error}`]);
      }
    });
  }, [runId]);

  const cancel = async () => {
    await api.cancelRun(runId).catch(() => false);
  };

  const finished = status === "succeeded" || status === "failed" || status === "cancelled";

  const rerun = async () => {
    if (!run) return;
    try {
      const next = await api.runWorkflow(run.workflow_id, run.device_id);
      onStarted(next);
    } catch {
      // The parent already surfaces workflow errors; keep this quiet.
    }
  };

  return (
    <Modal
      open
      wide
      title={`Run ${shortId(runId, 10)}`}
      subtitle={run ? `${run.workflow_name} · ${run.device_id ?? "no device"}` : undefined}
      onClose={onClose}
      footer={
        <>
          {!finished && (
            <Button variant="danger" icon="stop" onClick={() => void cancel()}>
              Cancel run
            </Button>
          )}
          {finished && run && (
            <Button variant="secondary" icon="refresh" onClick={() => void rerun()}>
              Run again
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </>
      }
    >
      <div className="run-status">
        <RunPill status={status} />
        <span className="muted">
          {finished ? "finished" : "streaming log lines from the engine"}
        </span>
      </div>
      <div className="console" style={{ maxHeight: 420 }}>
        {lines.length === 0 ? <Spinner label="Waiting for the first log line…" /> : lines.join("\n")}
      </div>
    </Modal>
  );
}

export { STEP_KINDS };
