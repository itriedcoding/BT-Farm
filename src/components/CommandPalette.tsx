import { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "./Icons";
import { api } from "../lib/api";
import { NAV, type Page } from "../lib/nav";
import { toast } from "./toast";

interface Command {
  id: string;
  label: string;
  hint: string;
  group: string;
  run: () => void;
}

/** Ctrl/Cmd+K palette: real navigation and real backend actions, no stubs. */
export function CommandPalette({
  open,
  onClose,
  onNavigate,
}: {
  open: boolean;
  onClose: () => void;
  onNavigate: (page: Page) => void;
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const commands = useMemo<Command[]>(() => {
    const pages: Command[] = NAV.flatMap((group) =>
      group.items.map((item) => ({
        id: `page-${item.page}`,
        label: item.label,
        hint: item.description,
        group: "Go to",
        run: () => onNavigate(item.page),
      })),
    );
    const actions: Command[] = [
      {
        id: "check-update",
        label: "Check for updates",
        hint: "Query the GitHub releases page",
        group: "Actions",
        run: () => {
          onNavigate("settings");
          void api
            .checkUpdate()
            .then((info) =>
              toast(
                info.available
                  ? `Version ${info.latest_version} is available`
                  : info.note ?? "You are on the newest release",
                "ok",
              ),
            )
            .catch((err) => toast(err instanceof Error ? err.message : String(err), "danger"));
        },
      },
      {
        id: "diagnostics",
        label: "Write diagnostics report",
        hint: "Collects adb, provider and settings state — no secrets",
        group: "Actions",
        run: () => {
          void api
            .diagnosticsReport()
            .then((path) => toast(`Diagnostics written to ${path}`, "ok"))
            .catch((err) => toast(err instanceof Error ? err.message : String(err), "danger"));
        },
      },
      {
        id: "logs-folder",
        label: "Open the logs folder",
        hint: "Rolling tracing log files",
        group: "Actions",
        run: () => {
          void api
            .logFilePath()
            .then((path) => api.openInExplorer(path))
            .catch((err) => toast(err instanceof Error ? err.message : String(err), "danger"));
        },
      },
      {
        id: "releases",
        label: "Open the release page",
        hint: "github.com/itriedcoding/BT-Farm/releases",
        group: "Actions",
        run: () => {
          void api.openUrl("https://github.com/itriedcoding/BT-Farm/releases");
        },
      },
    ];
    return [...pages, ...actions];
  }, [onNavigate]);

  const filtered = useMemo(() => {
    const term = query.trim().toLowerCase();
    if (!term) return commands;
    return commands.filter((command) =>
      `${command.label} ${command.hint} ${command.group}`.toLowerCase().includes(term),
    );
  }, [commands, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setIndex(0);
    const timer = window.setTimeout(() => inputRef.current?.focus(), 30);
    return () => window.clearTimeout(timer);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        setIndex((prev) => (filtered.length === 0 ? 0 : (prev + 1) % filtered.length));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setIndex((prev) =>
          filtered.length === 0 ? 0 : (prev - 1 + filtered.length) % filtered.length,
        );
      } else if (event.key === "Enter") {
        const chosen = filtered[index];
        if (chosen) {
          event.preventDefault();
          onClose();
          chosen.run();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, index, onClose]);

  if (!open) return null;

  return (
    <div className="palette-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="palette-input">
          <Icon name="search" size={15} />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setIndex(0);
            }}
            placeholder="Jump to a screen or run an action…"
            aria-label="Search commands"
          />
          <span className="palette-hint">esc</span>
        </div>
        <div className="palette-list">
          {filtered.length === 0 ? (
            <div className="palette-empty">Nothing matches “{query}”.</div>
          ) : (
            filtered.map((command, position) => (
              <button
                key={command.id}
                type="button"
                className={`palette-item ${position === index ? "active" : ""}`}
                onMouseEnter={() => setIndex(position)}
                onClick={() => {
                  onClose();
                  command.run();
                }}
              >
                <span className="palette-label">{command.label}</span>
                <span className="palette-sub">{command.hint}</span>
                <span className="palette-group">{command.group}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
