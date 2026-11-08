import type { IconName } from "../components/Icons";

export type Page =
  | "overview"
  | "infrastructure"
  | "templates"
  | "devices"
  | "viewer"
  | "workflows"
  | "scheduler"
  | "publishing"
  | "logs"
  | "settings";

export interface NavItem {
  page: Page;
  label: string;
  icon: IconName;
  description: string;
}

export interface NavGroup {
  label: string;
  items: NavItem[];
}

export const NAV: NavGroup[] = [
  {
    label: "Control",
    items: [
      {
        page: "overview",
        label: "Overview",
        icon: "grid",
        description: "Fleet health, recent runs and provider status",
      },
      {
        page: "infrastructure",
        label: "Infrastructure",
        icon: "server",
        description: "Provision and manage cloud instances",
      },
      {
        page: "templates",
        label: "Templates",
        icon: "chip",
        description: "Provisioning templates and install recipes",
      },
      {
        page: "devices",
        label: "Android devices",
        icon: "phone",
        description: "ADB-attached devices, apps and diagnostics",
      },
      {
        page: "viewer",
        label: "Live screens",
        icon: "screen",
        description: "Real-time device screens and input",
      },
    ],
  },
  {
    label: "Automation",
    items: [
      {
        page: "workflows",
        label: "Workflows",
        icon: "workflow",
        description: "Build and run multi-step automation",
      },
      {
        page: "scheduler",
        label: "Scheduler",
        icon: "clock",
        description: "Recurring workflow runs",
      },
      {
        page: "publishing",
        label: "Publishing",
        icon: "send",
        description: "Scheduled posts through official APIs",
      },
    ],
  },
  {
    label: "System",
    items: [
      {
        page: "logs",
        label: "Logs",
        icon: "list",
        description: "Application and provider diagnostics",
      },
      {
        page: "settings",
        label: "Settings",
        icon: "settings",
        description: "Tool paths, credentials and WebDriver",
      },
    ],
  },
];

export function navItem(page: Page): NavItem {
  for (const group of NAV) {
    const found = group.items.find((item) => item.page === page);
    if (found) return found;
  }
  return NAV[0].items[0];
}
