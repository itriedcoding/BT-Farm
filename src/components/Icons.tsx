export type IconName =
  | "grid"
  | "server"
  | "phone"
  | "screen"
  | "workflow"
  | "clock"
  | "send"
  | "list"
  | "settings"
  | "refresh"
  | "play"
  | "stop"
  | "trash"
  | "terminal"
  | "plus"
  | "close"
  | "check"
  | "alert"
  | "chevron"
  | "external"
  | "search"
  | "camera"
  | "record"
  | "up"
  | "down"
  | "copy"
  | "key"
  | "bolt"
  | "chip"
  | "link"
  | "expand"
  | "info";

const PATHS: Record<IconName, string> = {
  grid: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
  server: "M4 5h16v5H4zM4 14h16v5H4zM7 7.5h.01M7 16.5h.01",
  phone: "M8 3h8a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM11 18.5h2",
  screen: "M3 5h18v11H3zM8 20h8M12 16v4",
  workflow: "M5 4h5v5H5zM14 15h5v5h-5zM10 6.5h4a2 2 0 0 1 2 2v6.5",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7.5V12l3 2",
  send: "M21 3 10.5 13.5M21 3l-6.5 18-4-8-8-4z",
  list: "M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01",
  settings:
    "M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4zM19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-2.87 1.2V21a2 2 0 1 1-4 0v-.1A1.7 1.7 0 0 0 7 19.4a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 3 14.1H3a2 2 0 1 1 0-4h.1A1.7 1.7 0 0 0 4.6 7a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 3.6h.1A2 2 0 1 1 13 3.6v.1a1.7 1.7 0 0 0 2.87 1.2l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 20.4 9v.1a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1.9z",
  refresh: "M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6",
  play: "M8 5.5v13l11-6.5z",
  stop: "M7 7h10v10H7z",
  trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6",
  terminal: "M4 5h16v14H4zM8 10l2.5 2.5L8 15M13 15h4",
  plus: "M12 5v14M5 12h14",
  close: "M6 6l12 12M18 6 6 18",
  check: "M4.5 12.5 9 17l10.5-10.5",
  alert: "M12 4 2.5 20h19zM12 10v4M12 17.2h.01",
  chevron: "M9 6l6 6-6 6",
  external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
  search: "M10.5 18a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15zM16 16l5 5",
  camera: "M4 8h3l1.5-2h7L17 8h3v11H4zM12 16.5a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4z",
  record: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
  up: "M12 19V5M5 12l7-7 7 7",
  down: "M12 5v14M19 12l-7 7-7-7",
  copy: "M9 9h11v11H9zM4 15V4h11",
  key: "M15 4a5 5 0 1 1-4.6 6.9L4 17.3V20h2.7l1-1v-2h2v-2h2l1.4-1.4A5 5 0 0 1 15 4z",
  bolt: "M13 2 4 14h6l-1 8 9-12h-6z",
  chip: "M8 8h8v8H8zM4 9h4M4 15h4M16 9h4M16 15h4M9 4v4M15 4v4M9 16v4M15 16v4",
  link: "M10 13a5 5 0 0 0 7 0l2-2a5 5 0 0 0-7-7l-1 1M14 11a5 5 0 0 0-7 0l-2 2a5 5 0 0 0 7 7l1-1",
  expand: "M4 9V4h5M20 15v5h-5M20 9V4h-5M4 15v5h5",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v5M12 7.6h.01",
};

export function Icon({
  name,
  size = 18,
  className,
  strokeWidth = 1.6,
}: {
  name: IconName;
  size?: number;
  className?: string;
  strokeWidth?: number;
}) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
