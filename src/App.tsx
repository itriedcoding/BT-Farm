import { useCallback, useEffect, useState } from "react";
import { Icon } from "./components/Icons";
import { Banner } from "./components/ui";
import { BRIDGE_ERROR, api, isDesktop } from "./lib/api";
import { NAV, navItem, type Page } from "./lib/nav";
import { InfrastructurePage } from "./pages/Infrastructure";
import { DevicesPage } from "./pages/Devices";
import { LogsPage } from "./pages/Logs";
import { OverviewPage } from "./pages/Overview";
import { PublishingPage } from "./pages/Publishing";
import { SchedulerPage } from "./pages/Scheduler";
import { SettingsPage } from "./pages/Settings";
import { ViewerPage } from "./pages/Viewer";
import { WorkflowsPage } from "./pages/Workflows";

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const [bridgeReady, setBridgeReady] = useState<boolean | null>(isDesktop ? null : false);
  const [version, setVersion] = useState<string>("");
  const [deviceCount, setDeviceCount] = useState<number | null>(null);
  const [instanceCount, setInstanceCount] = useState<number | null>(null);

  const bootstrap = useCallback(async () => {
    if (!isDesktop) {
      setBridgeReady(false);
      return;
    }
    try {
      const [info, snapshot, devices] = await Promise.all([
        api.coreInfo(),
        api.instances(false),
        api.devices(false),
      ]);
      setVersion(info.version);
      setInstanceCount(snapshot.instances.filter((i) => i.state === "running").length);
      setDeviceCount(devices.filter((d) => d.state === "device").length);
      setBridgeReady(true);
    } catch {
      setBridgeReady(false);
    }
  }, []);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    if (page !== "overview") return;
    // Refresh the sidebar counters whenever the operator lands on Overview.
    const timer = window.setInterval(() => {
      void bootstrap();
    }, 30_000);
    return () => window.clearInterval(timer);
  }, [page, bootstrap]);

  const current = navItem(page);

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">
            <Icon name="chip" size={17} />
          </span>
          <div>
            <div className="brand-name">BT Farm</div>
            <div className="brand-sub">Control plane</div>
          </div>
        </div>

        <nav className="nav" aria-label="Main navigation">
          {NAV.map((group) => (
            <div key={group.label}>
              <div className="nav-group-label">{group.label}</div>
              {group.items.map((item) => {
                const badge =
                  item.page === "devices"
                    ? deviceCount
                    : item.page === "infrastructure"
                      ? instanceCount
                      : null;
                return (
                  <button
                    key={item.page}
                    type="button"
                    className={`nav-item ${page === item.page ? "active" : ""}`}
                    onClick={() => setPage(item.page)}
                    aria-current={page === item.page ? "page" : undefined}
                  >
                    <Icon name={item.icon} size={16} />
                    <span>{item.label}</span>
                    {badge !== null && badge > 0 && <span className="nav-count">{badge}</span>}
                  </button>
                );
              })}
            </div>
          ))}
        </nav>

        <div className="sidebar-foot">
          <div className="bridge">
            <span className={`bridge-dot ${bridgeReady ? "on" : "off"}`} />
            <span>{bridgeReady ? "Desktop bridge online" : "Bridge offline"}</span>
          </div>
          <div>
            {version ? `v${version}` : "v1.0.0"} · Windows desktop
          </div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="topbar-title">
            <h1>{current.label}</h1>
            <p>{current.description}</p>
          </div>
          <div className="topbar-actions">
            <span className="pill pill-muted" title="This build targets Windows 10/11 x64">
              <Icon name="bolt" size={12} />
              Local-first
            </span>
          </div>
        </header>

        <div className="content">
          {!isDesktop && (
            <div style={{ marginBottom: 16 }}>
              <Banner tone="warn" title="Running outside the desktop shell">
                {BRIDGE_ERROR} This browser preview renders the interface only — every data view is
                intentionally empty because there is no backend to talk to.
              </Banner>
            </div>
          )}

          {page === "overview" && <OverviewPage onNavigate={setPage} />}
          {page === "infrastructure" && <InfrastructurePage onNavigate={setPage} />}
          {page === "devices" && <DevicesPage onNavigate={setPage} />}
          {page === "viewer" && <ViewerPage />}
          {page === "workflows" && <WorkflowsPage />}
          {page === "scheduler" && <SchedulerPage />}
          {page === "publishing" && <PublishingPage />}
          {page === "logs" && <LogsPage />}
          {page === "settings" && <SettingsPage />}
        </div>
      </main>
    </div>
  );
}
