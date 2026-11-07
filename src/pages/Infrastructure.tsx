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
  Capabilities,
  CreateInstanceRequest,
  InstancePreset,
  InstanceState,
  InstanceTemplate,
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
  const [selectedInstances, setSelectedInstances] = useState<string[]>([]);
  const [bulkBusy, setBulkBusy] = useState(false);

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

  const bulkAction = async (action: "start" | "stop" | "delete") => {
    const targets = instances
      .filter((instance) => selectedInstances.includes(`${instance.provider}:${instance.id}`))
      .map((instance) => ({ provider: instance.provider, id: instance.id }));
    if (targets.length === 0) return;
    setBulkBusy(true);
    setNotice(null);
    try {
      const outcomes = await api.instanceBulk(targets, action);
      const failed = outcomes.filter((outcome) => !outcome.ok);
      const summary = `${action} on ${outcomes.length - failed.length}/${outcomes.length} instance(s)`;
      setNotice(
        failed.length === 0
          ? summary
          : `${summary}. Failed: ${failed.map((f) => `${f.serial} — ${f.output}`).join("; ")}`,
      );
      setSelectedInstances([]);
      await loadInstances();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
    } finally {
      setBulkBusy(false);
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
                <th style={{ width: 34 }}>
                  <input
                    type="checkbox"
                    aria-label="Select every instance"
                    checked={instances.length > 0 && selectedInstances.length === instances.length}
                    onChange={(event) =>
                      setSelectedInstances(
                        event.target.checked
                          ? instances.map((instance) => `${instance.provider}:${instance.id}`)
                          : [],
                      )
                    }
                  />
                </th>
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
                  <td colSpan={7}>
                    <Spinner label="Querying provider APIs…" />
                  </td>
                </tr>
              ) : instances.length === 0 ? (
                <tr>
                  <td colSpan={7}>
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
                      <input
                        type="checkbox"
                        aria-label={`Select ${instance.id}`}
                        checked={selectedInstances.includes(`${instance.provider}:${instance.id}`)}
                        onChange={() => {
                          const key = `${instance.provider}:${instance.id}`;
                          setSelectedInstances((prev) =>
                            prev.includes(key) ? prev.filter((item) => item !== key) : [...prev, key],
                          );
                        }}
                      />
                    </td>
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

      {selectedInstances.length > 0 && (
        <div className="batch-bar">
          <span>{selectedInstances.length} instance(s) selected</span>
          <div className="row">
            <Button size="sm" icon="play" loading={bulkBusy} onClick={() => void bulkAction("start")}>
              Start all
            </Button>
            <Button size="sm" icon="stop" loading={bulkBusy} onClick={() => void bulkAction("stop")}>
              Stop all
            </Button>
            <Button
              size="sm"
              variant="danger"
              icon="trash"
              loading={bulkBusy}
              onClick={() => {
                if (window.confirm(`Delete ${selectedInstances.length} instance(s)? This cannot be undone.`)) {
                  void bulkAction("delete");
                }
              }}
            >
              Delete all
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setSelectedInstances([])}>
              Clear
            </Button>
          </div>
        </div>
      )}

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
  const [disk, setDisk] = useState("");
  const [gpu, setGpu] = useState("0");
  const [gpuType, setGpuType] = useState("");
  const [catalog, setCatalog] = useState<InstanceTemplate[]>([]);
  const [caps, setCaps] = useState<Capabilities | null>(null);

  const isBoxd = provider === "boxd";
  const sizeOptions = useMemo(() => ["small", "default", "large", "xlarge"], []);
  const cpuOptions = useMemo(() => [1, 2, 4, 8, 16], []);
  const memoryOptions = useMemo(() => [1024, 2048, 4096, 8192, 16384, 32768, 65536], []);
  const diskOptions = useMemo(() => [10, 20, 40, 80, 160], []);
  const gpuUnits = useMemo(() => [1, 2, 4, 8], []);
  const sizing = caps?.sizing ?? "fixed_by_template";
  const cpuSelectable = sizing !== "fixed_by_template";
  const memorySelectable = sizing === "selectable";
  const gpuSupported = caps?.gpu === "supported";

  useEffect(() => {
    void api
      .presets()
      .then((list) => setPresets(list.filter((preset) => preset.provider === provider)))
      .catch(() => undefined);
    void api
      .templates()
      .then((list) => setCatalog(list.filter((item) => item.provider === provider)))
      .catch(() => undefined);
    void api
      .providerStatuses()
      .then((list) =>
        setCaps(list.find((status) => status.provider === provider)?.capabilities ?? null),
      )
      .catch(() => undefined);
  }, [provider]);

  const applyTemplate = (id: string) => {
    const chosen = catalog.find((item) => item.id === id);
    if (!chosen) return;
    setTemplate(chosen.image ?? "");
    if (chosen.size) setSize(chosen.size);
    setCpu(chosen.cpu ? String(chosen.cpu) : "");
    setMemory(chosen.memory_gb ? String(chosen.memory_gb * 1024) : "");
    setDisk(chosen.disk_gb ? String(chosen.disk_gb) : "");
    setTtl(chosen.ttl_seconds ? String(chosen.ttl_seconds) : "7200");
    setGpu(chosen.gpu ? String(chosen.gpu) : "0");
    setGpuType(chosen.gpu_type ?? "");
    setPresetNotice(`Applied template “${chosen.name}” — ${chosen.description}`);
  };

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
      disk_gb: disk.trim() ? Number(disk) : null,
      gpu: Number(gpu) > 0 ? Number(gpu) : null,
      gpu_type: gpuType.trim() || null,
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
    setDisk(request.disk_gb ? String(request.disk_gb) : "");
    setGpu(request.gpu ? String(request.gpu) : "0");
    setGpuType(request.gpu_type ?? "");
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
      {caps && !memorySelectable && (
        <InlineNote>
          {caps.sizing_note} Anything set here for memory or disk is dropped before the request is
          sent, and the backend logs which fields were ignored.
        </InlineNote>
      )}
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
          label="Start from a template"
          hint={`${catalog.length} built-in for ${provider}`}
        >
          <select className="select" defaultValue="" onChange={(event) => applyTemplate(event.target.value)}>
            <option value="">Choose a template…</option>
            {catalog.map((item) => (
              <option key={item.id} value={item.id} title={item.description}>
                {item.name} · {item.workload}
              </option>
            ))}
          </select>
        </Field>
        <Field
          label={provider === "freestyle" ? "Snapshot / template" : "Template"}
          hint="the identifier sent to the provider — editable"
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
          hint={cpuSelectable ? (isBoxd ? "boxd rounds to 1 / 2 / 4 / 8" : "sent to the provider") : "not accepted by this provider"}
        >
          <select
            className="select"
            value={cpu}
            onChange={(event) => setCpu(event.target.value)}
            disabled={!cpuSelectable}
          >
            <option value="">Provider default</option>
            {cpuOptions.map((value) => (
              <option key={value} value={value}>
                {value} vCPU
              </option>
            ))}
          </select>
        </Field>
        <Field
          label="Memory"
          hint={memorySelectable ? "Daytona takes GiB" : "fixed by the template or machine class"}
        >
          <select
            className="select"
            value={memory}
            onChange={(event) => setMemory(event.target.value)}
            disabled={!memorySelectable}
          >
            <option value="">Provider default</option>
            {memoryOptions.map((value) => (
              <option key={value} value={value}>
                {value / 1024} GB
              </option>
            ))}
          </select>
        </Field>
        <Field label="Disk" hint={memorySelectable ? "GB on the create call" : "not accepted by this provider"}>
          <select
            className="select"
            value={disk}
            onChange={(event) => setDisk(event.target.value)}
            disabled={!memorySelectable}
          >
            <option value="">Provider default</option>
            {diskOptions.map((value) => (
              <option key={value} value={value}>
                {value} GB
              </option>
            ))}
          </select>
        </Field>
        <Field label="GPU" hint={caps?.gpu_note ?? "checking provider capabilities…"}>
          <select
            className="select"
            value={gpu}
            onChange={(event) => setGpu(event.target.value)}
            disabled={!gpuSupported}
          >
            <option value="0">No GPU</option>
            {gpuUnits.map((value) => (
              <option key={value} value={value}>
                {value} GPU{value === 1 ? "" : "s"}
              </option>
            ))}
          </select>
        </Field>
        {Number(gpu) > 0 && gpuSupported && (
          <Field label="GPU type" hint="documented identifiers">
            <select
              className="select"
              value={gpuType}
              onChange={(event) => setGpuType(event.target.value)}
            >
              <option value="">Provider default</option>
              {(caps?.gpu_types ?? []).map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </Field>
        )}
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
