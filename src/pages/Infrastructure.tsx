import { useCallback, useEffect, useMemo, useState } from "react";
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
  SectionTitle,
  Spinner,
  Toggle,
} from "../components/ui";
import { api, isDesktop } from "../lib/api";
import { formatMb, formatRelative, shortId, textToEnv, titleCase } from "../lib/format";
import type { Page } from "../lib/nav";
import type {
  CreateInstanceRequest,
  InstancePreset,
  InstanceState,
  Provider,
  ProviderStatus,
  RemoteInstance,
} from "../lib/types";

interface ConsoleState {
  provider: Provider;
  id: string;
  output: string;
  running: boolean;
  error: string | null;
}

export function InfrastructurePage({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const [statuses, setStatuses] = useState<ProviderStatus[]>([]);
  const [instances, setInstances] = useState<RemoteInstance[]>([]);
  const [errors, setErrors] = useState<{ provider: Provider; message: string }[]>([]);
  const [refreshedAt, setRefreshedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [pageError, setPageError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [configuring, setConfiguring] = useState<ProviderStatus | null>(null);
  const [creating, setCreating] = useState<Provider | null>(null);
  const [consoleState, setConsoleState] = useState<ConsoleState | null>(null);
  const [metricsFor, setMetricsFor] = useState<RemoteInstance | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<RemoteInstance | null>(null);
  const [busyInstance, setBusyInstance] = useState<string | null>(null);
  const [testingProvider, setTestingProvider] = useState<Provider | null>(null);

  const loadStatuses = useCallback(async () => {
    if (!isDesktop) return;
    try {
      setStatuses(await api.providerStatuses());
      setPageError(null);
    } catch (err) {
      setPageError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const loadInstances = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const snapshot = await api.instances(true);
      setInstances(snapshot.instances);
      setErrors(snapshot.errors);
      setRefreshedAt(snapshot.refreshed_at);
      setPageError(null);
    } catch (err) {
      setPageError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
      await loadStatuses();
    }
  }, [loadStatuses]);

  useEffect(() => {
    void loadStatuses();
    void loadInstances();
  }, [loadStatuses, loadInstances]);

  const configured = statuses.filter((status) => status.configured);
  const enabled = statuses.filter((status) => status.enabled);

  const totalRunning = instances.filter((instance) => instance.state === "running").length;

  const testConnection = async (provider: Provider) => {
    setTestingProvider(provider);
    setNotice(null);
    try {
      const info = await api.testProvider(provider);
      setNotice(`${providerLabel(statuses, provider)} verified as ${info.account}`);
      await loadStatuses();
      await loadInstances();
    } catch (err) {
      setNotice(`${providerLabel(statuses, provider)}: ${err instanceof Error ? err.message : err}`);
      await loadStatuses();
    } finally {
      setTestingProvider(null);
    }
  };

  const runAction = async (
    instance: RemoteInstance,
    action: "start" | "stop" | "delete",
  ) => {
    setBusyInstance(instance.id);
    setNotice(null);
    try {
      await api.instanceAction(instance.provider, instance.id, action);
      setNotice(`${action} requested for ${instance.name ?? instance.id}`);
      await loadInstances();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyInstance(null);
      setConfirmDelete(null);
    }
  };

  return (
    <div className="page">
      {pageError && (
        <Banner tone="danger" title="Infrastructure unavailable">
          {pageError}
        </Banner>
      )}
      {notice && (
        <Banner tone="info" title="Provider response">
          {notice}
        </Banner>
      )}

      {isDesktop && configured.length === 0 && (
        <Banner
          tone="info"
          title="No provider credentials yet"
          action={
            <Button size="sm" variant="primary" onClick={() => onNavigate("settings")}>
              Add a key
            </Button>
          }
        >
          Instances are created through real provider APIs. Nothing is listed until a provider is
          configured and enabled.
        </Banner>
      )}

      <div>
        <SectionTitle
          aside={
            <div className="row">
              <span className="muted" style={{ fontSize: 11.5 }}>
                {refreshedAt ? `synced ${formatRelative(refreshedAt)}` : "never synced"}
              </span>
              <Button size="sm" icon="refresh" onClick={() => void loadInstances()} loading={loading}>
                Sync all
              </Button>
            </div>
          }
        >
          Providers
        </SectionTitle>
        <div className="provider-grid">
          {statuses.map((status) => (
            <div key={status.provider} className="provider-card">
              <div className="provider-head">
                <span className="provider-name">
                  <Icon name={status.capabilities.transport === "cli" ? "terminal" : "link"} size={14} />
                  {status.label}
                </span>
                {status.configured ? (
                  status.enabled ? (
                    <Pill tone="ok" dot>
                      enabled
                    </Pill>
                  ) : (
                    <Pill tone="warn" dot>
                      disabled
                    </Pill>
                  )
                ) : (
                  <Pill tone="muted">no key</Pill>
                )}
              </div>
              <div className="provider-meta">
                {status.account
                  ? `Account: ${status.account}`
                  : status.configured
                    ? "Credentials stored"
                    : status.credential_hint}
              </div>
              <div className="badge-row">
                <Pill tone="muted" title="What the provider actually runs">
                  {status.capabilities.runtime}
                </Pill>
                <Pill
                  tone={status.capabilities.exec ? "info" : "warn"}
                  title="How you get a shell for these machines"
                >
                  {status.capabilities.terminal}
                </Pill>
              </div>
              {status.last_error && (
                <div className="note" style={{ color: "#ffb3bd" }}>
                  {status.last_error}
                </div>
              )}
              <div className="caps" title={status.capabilities.note}>
                <Cap on={status.capabilities.provision} label="provision" />
                <Cap on={status.capabilities.lifecycle} label="lifecycle" />
                <Cap on={status.capabilities.exec} label="exec" />
                <Cap on={status.capabilities.metrics} label="metrics" />
                <Cap on={status.capabilities.android} label="android" />
              </div>
              <div className="row">
                <Button
                  size="sm"
                  variant="secondary"
                  icon="check"
                  loading={testingProvider === status.provider}
                  disabled={!status.configured}
                  onClick={() => void testConnection(status.provider)}
                >
                  Test
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setConfiguring(status)}>
                  Configure
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  icon="plus"
                  disabled={!status.configured || !status.capabilities.provision}
                  onClick={() => setCreating(status.provider)}
                >
                  New
                </Button>
                <button
                  type="button"
                  className="icon-btn"
                  title="Open provider documentation"
                  onClick={() => void api.openUrl(status.docs_url)}
                >
                  <Icon name="external" size={14} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {errors.length > 0 && (
        <Card title="Provider errors" subtitle="Requests that failed during the last sync">
          <div className="stack-sm">
            {errors.map((error) => (
              <InlineError key={error.provider} message={`${titleCase(error.provider)}: ${error.message}`} />
            ))}
          </div>
        </Card>
      )}

      <Card
        title="Instances"
        subtitle={`${instances.length} visible · ${totalRunning} running`}
        flush
        actions={
          enabled.length > 1 ? (
            <Pill tone="muted">{enabled.length} providers enabled</Pill>
          ) : undefined
        }
      >
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Instance</th>
                <th>Provider</th>
                <th>State</th>
                <th>Resources</th>
                <th>Created</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {loading && instances.length === 0 ? (
                <tr>
                  <td colSpan={6}>
                    <Spinner label="Querying provider APIs…" />
                  </td>
                </tr>
              ) : instances.length === 0 ? (
                <tr>
                  <td colSpan={6}>
                    <EmptyState
                      icon="server"
                      title="No instances"
                      body="Provision a workspace on any configured provider. Lists are fetched live, never cached from a fixture."
                    />
                  </td>
                </tr>
              ) : (
                instances.map((instance) => (
                  <tr key={`${instance.provider}-${instance.id}`}>
                    <td>
                      <div className="cell-main">
                        <StateDot state={instance.state} />
                        <span className="cell-title">{instance.name ?? shortId(instance.id, 14)}</span>
                      </div>
                      <div className="cell-sub">{instance.id}</div>
                    </td>
                    <td>
                      <Pill tone="muted">{providerLabel(statuses, instance.provider)}</Pill>
                      <div className="cell-sub">{runtimeLabel(statuses, instance.provider)}</div>
                    </td>
                    <td>
                      <StatePill state={instance.state} raw={instance.raw_state} />
                    </td>
                    <td className="muted nowrap">
                      {instance.cpu ? `${instance.cpu} vCPU` : "—"}
                      {instance.memory_mb ? ` · ${formatMb(instance.memory_mb)}` : ""}
                      {instance.disk_gb ? ` · ${instance.disk_gb} GB disk` : ""}
                    </td>
                    <td className="muted nowrap">{formatRelative(instance.created_at)}</td>
                    <td>
                      <div className="cell-actions">
                        <IconButton
                          icon="terminal"
                          label="Run a command"
                          disabled={!supports(statuses, instance.provider, "exec")}
                          onClick={() =>
                            setConsoleState({
                              provider: instance.provider,
                              id: instance.id,
                              output: "",
                              running: false,
                              error: null,
                            })
                          }
                        />
                        <IconButton
                          icon="chip"
                          label="Live metrics"
                          disabled={!supports(statuses, instance.provider, "metrics")}
                          onClick={() => setMetricsFor(instance)}
                        />
                        <IconButton
                          icon="play"
                          label="Start"
                          disabled={
                            busyInstance === instance.id ||
                            instance.state === "running" ||
                            !supports(statuses, instance.provider, "lifecycle")
                          }
                          onClick={() => void runAction(instance, "start")}
                        />
                        <IconButton
                          icon="stop"
                          label="Stop"
                          disabled={
                            busyInstance === instance.id ||
                            instance.state === "stopped" ||
                            !supports(statuses, instance.provider, "lifecycle")
                          }
                          onClick={() => void runAction(instance, "stop")}
                        />
                        <IconButton
                          icon="trash"
                          label="Delete"
                          variant="danger"
                          disabled={busyInstance === instance.id}
                          onClick={() => setConfirmDelete(instance)}
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

      {configuring && (
        <ProviderConfigModal
          status={configuring}
          onClose={() => setConfiguring(null)}
          onSaved={async () => {
            setConfiguring(null);
            await loadStatuses();
            await loadInstances();
          }}
        />
      )}

      {creating && (
        <CreateInstanceModal
          provider={creating}
          onClose={() => setCreating(null)}
          onCreated={async (instance) => {
            setCreating(null);
            setNotice(`Created ${instance.id} on ${providerLabel(statuses, instance.provider)}`);
            await loadInstances();
          }}
        />
      )}

      {consoleState && (
        <ConsoleModal
          state={consoleState}
          providerLabel={providerLabel(statuses, consoleState.provider)}
          onChange={setConsoleState}
          onClose={() => setConsoleState(null)}
        />
      )}

      {metricsFor && (
        <MetricsModal
          instance={metricsFor}
          onClose={() => setMetricsFor(null)}
        />
      )}

      <Modal
        open={confirmDelete !== null}
        title="Delete instance"
        subtitle="This calls the provider's delete API and cannot be undone."
        onClose={() => setConfirmDelete(null)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirmDelete(null)}>
              Cancel
            </Button>
            <Button
              variant="danger"
              icon="trash"
              loading={busyInstance === confirmDelete?.id}
              onClick={() => confirmDelete && void runAction(confirmDelete, "delete")}
            >
              Delete permanently
            </Button>
          </>
        }
      >
        {confirmDelete && (
          <div className="stack">
            <p className="soft">
              Delete <code>{confirmDelete.id}</code> on{" "}
              {providerLabel(statuses, confirmDelete.provider)}?
            </p>
            <InlineNote>
              Any data on the instance is discarded by the provider. Local run history is kept.
            </InlineNote>
          </div>
        )}
      </Modal>
    </div>
  );
}

function Cap({ on, label }: { on: boolean; label: string }) {
  return <span className={`cap ${on ? "yes" : "no"}`}>{label}</span>;
}

function StateDot({ state }: { state: InstanceState }) {
  const color =
    state === "running"
      ? "var(--ok)"
      : state === "error"
        ? "var(--danger)"
        : state === "starting" || state === "creating"
          ? "var(--warn)"
          : "#5c6678";
  return (
    <span
      style={{
        width: 7,
        height: 7,
        borderRadius: 999,
        background: color,
        flex: "0 0 auto",
        boxShadow: state === "running" ? "0 0 0 3px rgba(57,211,160,.12)" : undefined,
      }}
    />
  );
}

function StatePill({ state, raw }: { state: InstanceState; raw: string }) {
  const tone =
    state === "running"
      ? "ok"
      : state === "error"
        ? "danger"
        : state === "starting" || state === "creating"
          ? "warn"
          : state === "paused"
            ? "info"
            : "muted";
  return (
    <Pill tone={tone} dot={state === "running" || state === "starting"} title={`provider state: ${raw}`}>
      {titleCase(state)}
    </Pill>
  );
}

function providerLabel(statuses: ProviderStatus[], provider: Provider): string {
  return statuses.find((status) => status.provider === provider)?.label ?? titleCase(provider);
}

function runtimeLabel(statuses: ProviderStatus[], provider: Provider): string {
  const capabilities = statuses.find((status) => status.provider === provider)?.capabilities;
  if (!capabilities) return "";
  return capabilities.exec
    ? `${capabilities.runtime} · terminal available`
    : `${capabilities.runtime} · no shell`;
}

function supports(
  statuses: ProviderStatus[],
  provider: Provider,
  capability: "exec" | "metrics" | "lifecycle" | "provision",
): boolean {
  return statuses.find((status) => status.provider === provider)?.capabilities[capability] ?? false;
}

function ProviderConfigModal({
  status,
  onClose,
  onSaved,
}: {
  status: ProviderStatus;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState(status.base_url ?? "");
  const [enabled, setEnabled] = useState(status.enabled);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.saveProviderSettings(
        status.provider,
        apiKey.trim() === "" ? null : apiKey.trim(),
        baseUrl.trim() === "" ? null : baseUrl.trim(),
        enabled,
      );
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
      title={`${status.label} credentials`}
      subtitle={status.credential_hint}
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
      {status.capabilities.transport === "cli" && (
        <InlineNote>
          {status.label} is driven through its official CLI. Install it and set the CLI path in
          Settings; the API key below is passed to the CLI as <code>BOXD_TOKEN</code>.
        </InlineNote>
      )}
      <Field
        label="API key"
        hint={status.configured ? "stored — leave blank to keep it" : "required"}
      >
        <input
          className="input"
          type="password"
          autoComplete="off"
          placeholder={status.configured ? "•••••••• (unchanged)" : status.credential_hint}
          value={apiKey}
          onChange={(event) => setApiKey(event.target.value)}
        />
      </Field>
      <Field label="Base URL" hint={`default: ${status.base_url ?? ""}`}>
        <input
          className="input"
          value={baseUrl}
          onChange={(event) => setBaseUrl(event.target.value)}
          placeholder="leave blank for the documented default"
        />
      </Field>
      <Toggle
        checked={enabled}
        onChange={setEnabled}
        label="Include this provider in fleet syncs"
      />
      <InlineNote>{status.capabilities.note}</InlineNote>
      <InlineError message={error} />
    </Modal>
  );
}

function CreateInstanceModal({
  provider,
  onClose,
  onCreated,
}: {
  provider: Provider;
  onClose: () => void;
  onCreated: (instance: RemoteInstance) => void;
}) {
  const [name, setName] = useState("");
  const [template, setTemplate] = useState("");
  const [size, setSize] = useState("default");
  const [cpu, setCpu] = useState("");
  const [memory, setMemory] = useState("");
  const [ttl, setTtl] = useState("7200");
  const [envText, setEnvText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [presets, setPresets] = useState<InstancePreset[]>([]);
  const [presetName, setPresetName] = useState("");
  const [savingPreset, setSavingPreset] = useState(false);
  const [presetNotice, setPresetNotice] = useState<string | null>(null);

  const isBoxd = provider === "boxd";
  const sizeOptions = useMemo(() => ["small", "default", "large", "xlarge"], []);

  useEffect(() => {
    void api
      .presets()
      .then((list) => setPresets(list.filter((preset) => preset.provider === provider)))
      .catch(() => undefined);
  }, [provider]);

  const buildRequest = (): CreateInstanceRequest => {
    let env: Record<string, string> = {};
    if (envText.trim()) {
      env = textToEnv(envText);
    }
    return {
      name: name.trim() || null,
      template: template.trim() || null,
      size: provider === "boat" ? size : null,
      cpu: cpu.trim() ? Number(cpu) : null,
      memory_mb: memory.trim() ? Number(memory) : null,
      ttl_seconds: ttl.trim() ? Number(ttl) : null,
      env,
    };
  };

  const applyPreset = (preset: InstancePreset) => {
    const request = preset.request;
    setName(request.name ?? "");
    setTemplate(request.template ?? "");
    if (request.size) setSize(request.size);
    setCpu(request.cpu ? String(request.cpu) : "");
    setMemory(request.memory_mb ? String(request.memory_mb) : "");
    setTtl(request.ttl_seconds ? String(request.ttl_seconds) : "");
    setEnvText(
      Object.entries(request.env)
        .map(([key, value]) => `${key}=${value}`)
        .join("\n"),
    );
    setPresetNotice(`Applied preset “${preset.name}”`);
  };

  const savePreset = async () => {
    if (!presetName.trim()) {
      setError("Give the preset a name first.");
      return;
    }
    setSavingPreset(true);
    setError(null);
    try {
      let request: CreateInstanceRequest;
      try {
        request = buildRequest();
      } catch (err) {
        throw new Error(err instanceof Error ? err.message : String(err));
      }
      const saved = await api.savePreset({
        id: "",
        name: presetName.trim(),
        provider,
        request: { ...request, name: null },
        created_at: new Date().toISOString(),
      });
      setPresets((prev) => [saved, ...prev]);
      setPresetName("");
      setPresetNotice(`Preset “${saved.name}” saved`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingPreset(false);
    }
  };

  const submit = async () => {
    setSaving(true);
    setError(null);
    try {
      const request = buildRequest();
      const instance = await api.createInstance(provider, request);
      onCreated(instance);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open
      wide
      title={`New ${provider} instance`}
      subtitle="The request goes straight to the provider API"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="secondary" icon="copy" onClick={() => void savePreset()} loading={savingPreset}>
            Save as preset
          </Button>
          <Button variant="primary" icon="plus" onClick={() => void submit()} loading={saving}>
            Provision
          </Button>
        </>
      }
    >
      <div className="form-row">
        <div className="flex-1">
          <Field label="Saved preset" hint="fills the form with a stored recipe">
            <select
              className="select"
              defaultValue=""
              onChange={(event) => {
                const chosen = presets.find((preset) => preset.id === event.target.value);
                if (chosen) applyPreset(chosen);
              }}
            >
              <option value="">Choose a preset…</option>
              {presets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.name}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <div className="flex-1">
          <Field label="Preset name" hint="stores the values below for reuse">
            <input
              className="input"
              value={presetName}
              onChange={(event) => setPresetName(event.target.value)}
              placeholder="4 vCPU worker"
            />
          </Field>
        </div>
      </div>
      {presetNotice && <InlineNote>{presetNotice}</InlineNote>}
      <div className="form-grid">
        <Field label="Name" hint="optional, sanitised">
          <input
            className="input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="worker-01"
          />
        </Field>
        <Field
          label={provider === "freestyle" ? "Snapshot / template" : "Template"}
          hint="optional"
        >
          <input
            className="input"
            value={template}
            onChange={(event) => setTemplate(event.target.value)}
            placeholder={provider === "e2b" ? "base" : "provider default"}
          />
        </Field>
        {provider === "boat" && (
          <Field label="Machine size" hint="maps to Boat's type field">
            <select className="select" value={size} onChange={(event) => setSize(event.target.value)}>
              {sizeOptions.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </Field>
        )}
        <Field
          label="vCPU"
          hint={
            isBoxd
              ? "rounded to 1 / 2 / 4 / 8 for the boxd CLI"
              : provider === "e2b"
                ? "not configurable on E2B"
                : "provider default when blank"
          }
        >
          <input
            className="input"
            value={cpu}
            onChange={(event) => setCpu(event.target.value)}
            placeholder="4"
            inputMode="numeric"
            disabled={provider === "e2b"}
          />
        </Field>
        <Field label="Memory (MB)" hint="Daytona expects GiB and converts">
          <input
            className="input"
            value={memory}
            onChange={(event) => setMemory(event.target.value)}
            placeholder="8192"
            inputMode="numeric"
            disabled={provider === "e2b" || provider === "boat" || isBoxd}
          />
        </Field>
        <Field label="TTL (seconds)" hint="provider default when blank">
          <input
            className="input"
            value={ttl}
            onChange={(event) => setTtl(event.target.value)}
            placeholder="7200"
            inputMode="numeric"
            disabled={provider === "boxd"}
          />
        </Field>
        <div className="span-2">
          <Field label="Environment" hint="KEY=value per line; sent as env to the provider">
            <textarea
              className="textarea"
              value={envText}
              onChange={(event) => setEnvText(event.target.value)}
              placeholder={"WORKER_ID=1\nDEBUG=0"}
              spellCheck={false}
            />
          </Field>
        </div>
      </div>
      <InlineError message={error} />
    </Modal>
  );
}

function ConsoleModal({
  state,
  providerLabel: label,
  onChange,
  onClose,
}: {
  state: ConsoleState;
  providerLabel: string;
  onChange: (next: ConsoleState) => void;
  onClose: () => void;
}) {
  const [command, setCommand] = useState("uname -a");

  const run = async () => {
    if (!command.trim()) return;
    onChange({ ...state, running: true, error: null });
    try {
      const result = await api.instanceExec(state.provider, state.id, command, 120);
      const output = [
        state.output,
        `$ ${command}`,
        result.stdout.trimEnd(),
        result.stderr.trimEnd(),
        `[exit ${result.exit_code}]`,
      ]
        .filter((chunk) => chunk !== "")
        .join("\n");
      onChange({ ...state, running: false, output });
    } catch (err) {
      onChange({
        ...state,
        running: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  };

  return (
    <Modal
      open
      wide
      title={`Command · ${label}`}
      subtitle={state.id}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button
            variant="primary"
            icon="terminal"
            loading={state.running}
            onClick={() => void run()}
          >
            Run
          </Button>
        </>
      }
    >
      <div className="form-row">
        <div className="flex-1">
          <Field label="Command">
            <input
              className="input mono"
              value={command}
              onChange={(event) => setCommand(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void run();
              }}
              spellCheck={false}
            />
          </Field>
        </div>
      </div>
      <div className="console">{state.output || "No output yet."}</div>
      <InlineError message={state.error} />
    </Modal>
  );
}

function MetricsModal({ instance, onClose }: { instance: RemoteInstance; onClose: () => void }) {
  const [loading, setLoading] = useState(true);
  const [metrics, setMetrics] = useState<Awaited<ReturnType<typeof api.instanceMetrics>>>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await api.instanceMetrics(instance.provider, instance.id);
        if (!cancelled) setMetrics(result);
      } catch (err) {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [instance]);

  return (
    <Modal open title="Instance metrics" subtitle={instance.id} onClose={onClose}>
      {loading ? (
        <Spinner label="Reading provider metrics…" />
      ) : error ? (
        <InlineError message={error} />
      ) : !metrics ? (
        <EmptyState
          icon="chip"
          title="No samples"
          body="The provider returned no metrics for this instance right now."
        />
      ) : (
        <dl className="kv">
          <dt>CPU</dt>
          <dd>{metrics.cpu_percent !== null ? `${metrics.cpu_percent.toFixed(1)} %` : "not reported"}</dd>
          <dt>Memory used</dt>
          <dd>
            {metrics.memory_used_mb !== null ? `${Math.round(metrics.memory_used_mb)} MB` : "not reported"}
            {metrics.memory_total_mb !== null ? ` of ${Math.round(metrics.memory_total_mb)} MB` : ""}
          </dd>
          <dt>Disk used</dt>
          <dd>
            {metrics.disk_used_mb !== null ? `${Math.round(metrics.disk_used_mb)} MB` : "not reported"}
            {metrics.disk_total_mb !== null ? ` of ${Math.round(metrics.disk_total_mb)} MB` : ""}
          </dd>
          <dt>Sampled</dt>
          <dd>{formatRelative(metrics.sampled_at)}</dd>
          {metrics.note && (
            <>
              <dt>Note</dt>
              <dd className="muted">{metrics.note}</dd>
            </>
          )}
        </dl>
      )}
    </Modal>
  );
}
