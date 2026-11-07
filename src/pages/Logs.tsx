import { useCallback, useEffect, useState } from "react";
import { Banner, Button, Card, EmptyState, Field, Pill, Spinner } from "../components/ui";
import { api, isDesktop, onFarmEvent } from "../lib/api";
import { formatDateTime, titleCase } from "../lib/format";
import type { LogLine } from "../lib/types";

export function LogsPage() {
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [level, setLevel] = useState<string>("");
  const [search, setSearch] = useState("");
  const [limit, setLimit] = useState(200);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [follow, setFollow] = useState(true);

  const load = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const lines = await api.logs(limit, level || null, search.trim() || null);
      setLogs(lines);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [limit, level, search]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!follow) return;
    return onFarmEvent((event) => {
      if (event.kind !== "log") return;
      const line = event.line;
      if (level && line.level !== level) return;
      if (search.trim() && !`${line.message} ${line.target}`.toLowerCase().includes(search.trim().toLowerCase())) {
        return;
      }
      setLogs((prev) => [line, ...prev].slice(0, limit));
    });
  }, [follow, level, search, limit]);

  return (
    <div className="page">
      {error && <Banner tone="danger" title="Could not read logs">{error}</Banner>}

      <Card
        title="Application log"
        subtitle="Every provider call, device command and workflow event is recorded here"
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load()} loading={loading}>
              Refresh
            </Button>
            <Button
              size="sm"
              variant="ghost"
              icon="external"
              onClick={() => void api.logFilePath().then((path) => api.openInExplorer(path))}
            >
              Open logs folder
            </Button>
          </>
        }
      >
        <div className="form-row">
          <div style={{ width: 150 }}>
            <Field label="Level">
              <select className="select" value={level} onChange={(event) => setLevel(event.target.value)}>
                <option value="">All levels</option>
                <option value="info">info</option>
                <option value="warn">warn</option>
                <option value="error">error</option>
              </select>
            </Field>
          </div>
          <div className="flex-1">
            <Field label="Search">
              <input
                className="input"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="provider, device serial, message…"
              />
            </Field>
          </div>
          <div style={{ width: 130 }}>
            <Field label="Rows">
              <select
                className="select"
                value={limit}
                onChange={(event) => setLimit(Number(event.target.value))}
              >
                {[100, 200, 500, 1000].map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </Field>
          </div>
          <Button variant={follow ? "primary" : "secondary"} onClick={() => setFollow(!follow)}>
            {follow ? "Following" : "Paused"}
          </Button>
        </div>
      </Card>

      <Card
        title="Entries"
        subtitle={`${logs.length} shown`}
        flush
        actions={<Pill tone={follow ? "ok" : "muted"} dot={follow}>{follow ? "live" : "paused"}</Pill>}
      >
        {loading && logs.length === 0 ? (
          <Spinner label="Reading the log…" />
        ) : logs.length === 0 ? (
          <EmptyState icon="list" title="No matching entries" body="Adjust the filters or run something." />
        ) : (
          <div>
            {logs.map((line) => (
              <div key={line.id} className="log-line" title={formatDateTime(line.ts)}>
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
  );
}
