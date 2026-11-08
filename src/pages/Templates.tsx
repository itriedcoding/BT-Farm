import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Banner,
  Button,
  Card,
  EmptyState,
  Field,
  InlineError,
  InlineNote,
  Modal,
  Pill,
  Spinner,
} from "../components/ui";
import { api, isDesktop } from "../lib/api";
import type { Page } from "../lib/nav";
import type {
  CreateInstanceRequest,
  CustomTemplateDraft,
  InstanceTemplate,
  Provider,
  TemplateProvision,
} from "../lib/types";

const PROVIDERS: Provider[] = ["e2b", "boat", "boxd", "daytona", "freestyle"];

interface DraftState {
  id?: string;
  name: string;
  description: string;
  provider: Provider;
  workload: string;
  image: string;
  cpu: string;
  memory: string;
  disk: string;
  ttl: string;
  gpu: string;
  gpuType: string;
  setup: string;
}

const emptyDraft: DraftState = {
  name: "",
  description: "",
  provider: "daytona",
  workload: "",
  image: "",
  cpu: "",
  memory: "",
  disk: "",
  ttl: "",
  gpu: "",
  gpuType: "",
  setup: "",
};

function fromTemplate(template: InstanceTemplate): DraftState {
  return {
    id: template.builtin ? undefined : template.id,
    name: template.builtin ? `${template.name} (copy)` : template.name,
    description: template.description,
    provider: template.provider,
    workload: template.workload,
    image: template.image ?? "",
    cpu: template.cpu ? String(template.cpu) : "",
    memory: template.memory_gb ? String(template.memory_gb) : "",
    disk: template.disk_gb ? String(template.disk_gb) : "",
    ttl: template.ttl_seconds ? String(template.ttl_seconds) : "",
    gpu: template.gpu ? String(template.gpu) : "",
    gpuType: template.gpu_type ?? "",
    setup: template.setup ?? "",
  };
}

function toNumber(value: string): number | null {
  const parsed = Number(value);
  return value.trim() && Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function summary(template: InstanceTemplate): string {
  const parts: string[] = [];
  if (template.cpu) parts.push(`${template.cpu} vCPU`);
  if (template.memory_gb) parts.push(`${template.memory_gb} GB RAM`);
  if (template.disk_gb) parts.push(`${template.disk_gb} GB disk`);
  if (template.gpu) parts.push(`${template.gpu} GPU${template.gpu_type ? ` (${template.gpu_type})` : ""}`);
  if (template.size) parts.push(`class ${template.size}`);
  if (template.image) parts.push(template.image);
  if (template.ttl_seconds) parts.push(`TTL ${Math.round(template.ttl_seconds / 60)} min`);
  return parts.length ? parts.join(" · ") : "size fixed by the provider";
}

export function TemplatesPage({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const [templates, setTemplates] = useState<InstanceTemplate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [providerFilter, setProviderFilter] = useState<Provider | "all">("all");
  const [search, setSearch] = useState("");

  const [launching, setLaunching] = useState<InstanceTemplate | null>(null);
  const [launchName, setLaunchName] = useState("");
  const [launchBusy, setLaunchBusy] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [result, setResult] = useState<TemplateProvision | null>(null);

  const [draft, setDraft] = useState<DraftState | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    if (!isDesktop) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      setTemplates(await api.templates());
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

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return templates.filter((template) => {
      if (providerFilter !== "all" && template.provider !== providerFilter) return false;
      if (!needle) return true;
      return [template.name, template.description, template.workload, template.id]
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  }, [templates, providerFilter, search]);

  const customCount = templates.filter((template) => !template.builtin).length;

  const openLaunch = (template: InstanceTemplate) => {
    setLaunching(template);
    setLaunchName("");
    setLaunchError(null);
    setResult(null);
  };

  const provision = async () => {
    if (!launching) return;
    setLaunchBusy(true);
    setLaunchError(null);
    try {
      const provisioned = await api.createFromTemplate(
        launching.id,
        launchName.trim() || undefined,
      );
      setResult(provisioned);
      setNotice(
        `Provisioned ${provisioned.instance.name ?? provisioned.instance.id} from ${launching.name}.`,
      );
      await load();
    } catch (err) {
      setLaunchError(err instanceof Error ? err.message : String(err));
    } finally {
      setLaunchBusy(false);
    }
  };

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setError(null);
    try {
      const request: CreateInstanceRequest = {
        name: null,
        template: draft.image.trim() || null,
        size: null,
        cpu: toNumber(draft.cpu),
        memory_mb: toNumber(draft.memory) ? (toNumber(draft.memory) as number) * 1024 : null,
        disk_gb: toNumber(draft.disk),
        ttl_seconds: toNumber(draft.ttl),
        gpu: toNumber(draft.gpu),
        gpu_type: draft.gpuType.trim() || null,
        env: {},
      };
      const payload: CustomTemplateDraft = {
        id: draft.id,
        name: draft.name,
        description: draft.description,
        provider: draft.provider,
        workload: draft.workload,
        request,
        setup: draft.setup,
      };
      const saved = await api.saveCustomTemplate(payload);
      setNotice(`Saved template “${saved.name}”.`);
      setDraft(null);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (template: InstanceTemplate) => {
    if (!window.confirm(`Delete the template “${template.name}”? This does not touch any instance.`)) {
      return;
    }
    try {
      await api.deleteCustomTemplate(template.id);
      setNotice(`Deleted “${template.name}”.`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <>
      <div style={{ marginBottom: 16 }}>
        <Banner tone="info" title="Templates are real provisioning requests">
          A template is the create payload this app sends: provider image or snapshot identifier,
          CPU, RAM, disk and GPU — and, for install recipes, a real setup command that runs through
          the provider's exec API after boot. Setup output is the command's own output, and a
          provider without an exec API reports that the command was not run instead of pretending.
        </Banner>
      </div>

      {notice && (
        <div style={{ marginBottom: 16 }}>
          <Banner tone="ok" title={notice} />
        </div>
      )}
      <InlineError message={error} />

      <Card
        title="Catalog"
        subtitle={`${templates.length} templates · ${customCount} saved in this app · ${templates.filter((t) => t.setup).length} run a setup command`}
        actions={
          <Button icon="plus" variant="primary" onClick={() => setDraft({ ...emptyDraft })}>
            New template
          </Button>
        }
      >
        <div className="row" style={{ marginBottom: 12, gap: 8, flexWrap: "wrap" }}>
          <input
            className="input"
            style={{ maxWidth: 260 }}
            placeholder="Search name, workload or id…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
          <select
            className="select"
            style={{ maxWidth: 180 }}
            value={providerFilter}
            onChange={(event) => setProviderFilter(event.target.value as Provider | "all")}
          >
            <option value="all">All providers</option>
            {PROVIDERS.map((provider) => (
              <option key={provider} value={provider}>
                {provider}
              </option>
            ))}
          </select>
          <span className="muted">{filtered.length} shown</span>
        </div>

        {loading ? (
          <Spinner label="Loading templates…" />
        ) : filtered.length === 0 ? (
          <EmptyState icon="chip" title="No templates match" body="Clear the search or pick another provider." />
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))",
              gap: 12,
            }}
          >
            {filtered.map((template) => (
              <div
                key={template.id}
                style={{
                  border: "1px solid var(--border, #2a2f3a)",
                  borderRadius: 10,
                  padding: 12,
                  display: "flex",
                  flexDirection: "column",
                  gap: 8,
                }}
              >
                <div className="row" style={{ justifyContent: "space-between", gap: 8 }}>
                  <strong>{template.name}</strong>
                  <Pill tone={template.builtin ? "muted" : "accent"}>
                    {template.builtin ? template.provider : "custom"}
                  </Pill>
                </div>
                <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
                  <Pill tone="info">{template.workload}</Pill>
                  {template.setup && <Pill tone="warn">runs setup command</Pill>}
                  {template.gpu && <Pill tone="ok">GPU</Pill>}
                </div>
                <p className="muted" style={{ margin: 0 }}>
                  {template.description}
                </p>
                <div className="cell-sub mono">{summary(template)}</div>
                {template.setup && (
                  <details>
                    <summary className="muted">View setup command</summary>
                    <pre
                      className="mono"
                      style={{
                        maxHeight: 180,
                        overflow: "auto",
                        fontSize: 11,
                        whiteSpace: "pre-wrap",
                      }}
                    >
                      {template.setup}
                    </pre>
                  </details>
                )}
                <div className="cell-actions">
                  <Button size="sm" variant="primary" icon="play" onClick={() => openLaunch(template)}>
                    Launch
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setDraft(fromTemplate(template))}>
                    {template.builtin ? "Copy to custom" : "Edit"}
                  </Button>
                  {!template.builtin && (
                    <Button size="sm" variant="danger" icon="trash" onClick={() => void remove(template)}>
                      Delete
                    </Button>
                  )}
                </div>
                <div className="cell-sub mono">{template.source}</div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Modal
        open={launching !== null}
        title={launching ? `Launch “${launching.name}”` : "Launch"}
        subtitle={launching ? summary(launching) : undefined}
        onClose={() => {
          setLaunching(null);
          setResult(null);
        }}
        wide
        footer={
          <>
            <Button variant="ghost" onClick={() => setLaunching(null)}>
              Close
            </Button>
            <Button variant="primary" icon="play" loading={launchBusy} onClick={() => void provision()}>
              Provision now
            </Button>
          </>
        }
      >
        {launching && (
          <>
            <Field label="Instance name" hint="optional">
              <input
                className="input"
                value={launchName}
                onChange={(event) => setLaunchName(event.target.value)}
                placeholder="panel-1"
              />
            </Field>
            {launching.setup ? (
              <InlineNote>
                After the machine boots this setup command runs through {launching.provider}'s exec
                API. It can take several minutes and its real output is shown below.
              </InlineNote>
            ) : (
              <InlineNote>
                This template only provisions the machine; nothing is executed inside it.
              </InlineNote>
            )}
            <InlineError message={launchError} />
            {result && (
              <div style={{ marginTop: 12 }}>
                <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
                  <Pill tone="ok">{result.instance.state}</Pill>
                  <span className="mono">{result.instance.id}</span>
                  <Pill tone={result.setup_ran ? "info" : "warn"}>
                    {result.setup_ran
                      ? `setup exit ${result.setup_exit_code ?? "?"}`
                      : "setup not run"}
                  </Pill>
                </div>
                <p className="muted">{result.setup_note}</p>
                {result.setup_output && (
                  <pre
                    className="mono"
                    style={{
                      maxHeight: 260,
                      overflow: "auto",
                      fontSize: 11,
                      whiteSpace: "pre-wrap",
                    }}
                  >
                    {result.setup_output}
                  </pre>
                )}
                <Button size="sm" variant="secondary" onClick={() => onNavigate("infrastructure")}>
                  Open Infrastructure
                </Button>
              </div>
            )}
          </>
        )}
      </Modal>

      <Modal
        open={draft !== null}
        title={draft?.id ? "Edit template" : "New template"}
        subtitle="Saved locally in SQLite. The request is normalised for the provider when it is used."
        onClose={() => setDraft(null)}
        wide
        footer={
          <>
            <Button variant="ghost" onClick={() => setDraft(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              loading={saving}
              disabled={!draft?.name.trim()}
              onClick={() => void save()}
            >
              Save template
            </Button>
          </>
        }
      >
        {draft && (
          <>
            <Field label="Name">
              <input
                className="input"
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                placeholder="Pterodactyl panel"
              />
            </Field>
            <Field label="Provider">
              <select
                className="select"
                value={draft.provider}
                onChange={(event) =>
                  setDraft({ ...draft, provider: event.target.value as Provider })
                }
              >
                {PROVIDERS.map((provider) => (
                  <option key={provider} value={provider}>
                    {provider}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Workload" hint="grouping label">
              <input
                className="input"
                value={draft.workload}
                onChange={(event) => setDraft({ ...draft, workload: event.target.value })}
                placeholder="Game panel"
              />
            </Field>
            <Field label="Description">
              <textarea
                className="input"
                rows={2}
                value={draft.description}
                onChange={(event) => setDraft({ ...draft, description: event.target.value })}
              />
            </Field>
            <Field label="Image / template / snapshot" hint="sent verbatim where the provider takes one">
              <input
                className="input"
                value={draft.image}
                onChange={(event) => setDraft({ ...draft, image: event.target.value })}
                placeholder="freestyle/ubuntu-lg"
              />
            </Field>
            <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
              <Field label="vCPU">
                <input
                  className="input"
                  value={draft.cpu}
                  inputMode="numeric"
                  onChange={(event) => setDraft({ ...draft, cpu: event.target.value })}
                />
              </Field>
              <Field label="Memory (GB)">
                <input
                  className="input"
                  value={draft.memory}
                  inputMode="numeric"
                  onChange={(event) => setDraft({ ...draft, memory: event.target.value })}
                />
              </Field>
              <Field label="Disk (GB)">
                <input
                  className="input"
                  value={draft.disk}
                  inputMode="numeric"
                  onChange={(event) => setDraft({ ...draft, disk: event.target.value })}
                />
              </Field>
              <Field label="TTL (seconds)">
                <input
                  className="input"
                  value={draft.ttl}
                  inputMode="numeric"
                  onChange={(event) => setDraft({ ...draft, ttl: event.target.value })}
                />
              </Field>
              <Field label="GPU units">
                <input
                  className="input"
                  value={draft.gpu}
                  inputMode="numeric"
                  onChange={(event) => setDraft({ ...draft, gpu: event.target.value })}
                />
              </Field>
              <Field label="GPU type">
                <input
                  className="input"
                  value={draft.gpuType}
                  onChange={(event) => setDraft({ ...draft, gpuType: event.target.value })}
                  placeholder="H100"
                />
              </Field>
            </div>
            <Field
              label="Setup command"
              hint="optional shell script, executed after boot through the provider's exec API"
            >
              <textarea
                className="input mono"
                rows={8}
                value={draft.setup}
                onChange={(event) => setDraft({ ...draft, setup: event.target.value })}
                placeholder={"set -e\napt-get update -y\napt-get install -y curl"}
              />
            </Field>
            <InlineNote>
              Fields the chosen provider does not accept (memory or disk on Boat, GPU on E2B, and so
              on) are stripped before anything is sent and reported in the log.
            </InlineNote>
          </>
        )}
      </Modal>
    </>
  );
}
