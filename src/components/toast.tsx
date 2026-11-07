import { useEffect, useState } from "react";
import { Icon } from "./Icons";

export type ToastTone = "info" | "ok" | "danger";

interface ToastItem {
  id: number;
  message: string;
  tone: ToastTone;
}

let items: ToastItem[] = [];
let seq = 0;
const listeners = new Set<(next: ToastItem[]) => void>();

function publish() {
  for (const listener of listeners) listener(items);
}

/** Shows a transient message. Used for background results and palette actions. */
export function toast(message: string, tone: ToastTone = "info") {
  const item: ToastItem = { id: ++seq, message, tone };
  items = [...items, item].slice(-4);
  publish();
  window.setTimeout(() => {
    items = items.filter((entry) => entry.id !== item.id);
    publish();
  }, 5200);
}

export function Toasts() {
  const [list, setList] = useState<ToastItem[]>(items);

  useEffect(() => {
    listeners.add(setList);
    return () => {
      listeners.delete(setList);
    };
  }, []);

  if (list.length === 0) return null;

  return (
    <div className="toast-stack" role="status" aria-live="polite">
      {list.map((item) => (
        <div key={item.id} className={`toast toast-${item.tone}`}>
          <Icon name={item.tone === "danger" ? "alert" : item.tone === "ok" ? "check" : "info"} size={14} />
          <span>{item.message}</span>
        </div>
      ))}
    </div>
  );
}
