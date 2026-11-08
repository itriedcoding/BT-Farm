import { useCallback, useEffect, useState } from "react";
import { CommandPalette } from "./components/CommandPalette";
import { Icon } from "./components/Icons";
import { Toasts } from "./components/toast";
import { Banner, Button } from "./components/ui";
import { BRIDGE_ERROR, api, isDesktop } from "./lib/api";
import { NAV, navItem, type Page } from "./lib/nav";
import type { UpdateInfo } from "./lib/types";
import { InfrastructurePage } from "./pages/Infrastructure";
import { DevicesPage } from "./pages/Devices";
import { LogsPage } from "./pages/Logs";
import { OverviewPage } from "./pages/Overview";
import { PublishingPage } from "./pages/Publishing";
import { SchedulerPage } from "./pages/Scheduler";
import { SettingsPage } from "./pages/Settings";
import { TemplatesPage } from "./pages/Templates";
import { ViewerPage } from "./pages/Viewer";
import { WorkflowsPage } from "./pages/Workflows";

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const [bridgeReady, setBridgeReady] = useState<boolean | null>(isDesktop ? null : false);
  const [version, setVersion] = useState<string>("");
  const [deviceCount, setDeviceCount] = useState<number | null>(null);
  const [instanceCount, setInstanceCount] = useState<number | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

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

  // Startup update check, honouring the setting on the Settings screen.
  useEffect(() => {
    if (!isDesktop) return;
    let cancelled = false;
    (async () => {
      try {
        const settings = await api.settings();
        if (!settings.check_updates_on_start) return;
        const info = await api.checkUpdate();
        if (!cancelled && info.available) setUpdate(info);
      } catch {
        // Offline or GitHub rate-limited: never block the app on startup.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((value) => !value);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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
            <button
              type="button"
              className="palette-trigger"
              onClick={() => setPaletteOpen(true)}
              title="Command palette (Ctrl+K)"
            >
              <Icon name="search" size={13} />
              <span>Search</span>
              <kbd>Ctrl K</kbd>
            </button>
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

          {update && (
            <div style={{ marginBottom: 16 }}>
              <Banner
                tone="info"
                title={`BT Farm ${update.latest_version} is available`}
                action={
                  <Button size="sm" variant="primary" onClick={() => setPage("settings")}>
                    Review update
                  </Button>
                }
              >
                You are running {update.current_version}. The newer installer is published on this
                project's GitHub Releases page; Settings can download and verify it for you.
              </Banner>
            </div>
          )}

          {page === "overview" && <OverviewPage onNavigate={setPage} />}
          {page === "infrastructure" && <InfrastructurePage onNavigate={setPage} />}
          {page === "templates" && <TemplatesPage onNavigate={setPage} />}
          {page === "devices" && <DevicesPage onNavigate={setPage} />}
          {page === "viewer" && <ViewerPage />}
          {page === "workflows" && <WorkflowsPage />}
          {page === "scheduler" && <SchedulerPage />}
          {page === "publishing" && <PublishingPage />}
          {page === "logs" && <LogsPage />}
          {page === "settings" && <SettingsPage />}
        </div>
      </main>

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        onNavigate={setPage}
      />
      <Toasts />
    </div>
  );
}
