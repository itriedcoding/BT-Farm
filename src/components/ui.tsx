import type { ButtonHTMLAttributes, ReactNode } from "react";
import { useEffect } from "react";
import { Icon, type IconName } from "./Icons";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

export function Button({
  children,
  variant = "secondary",
  size = "md",
  icon,
  loading = false,
  className = "",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: "sm" | "md";
  icon?: IconName;
  loading?: boolean;
}) {
  return (
    <button
      {...rest}
      className={`btn btn-${variant} btn-${size} ${className}`.trim()}
      disabled={rest.disabled || loading}
    >
      {loading ? <span className="spinner spinner-sm" /> : icon ? <Icon name={icon} size={16} /> : null}
      {children}
    </button>
  );
}

export function IconButton({
  icon,
  label,
  variant = "ghost",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string; variant?: ButtonVariant }) {
  return (
    <button
      {...rest}
      className={`icon-btn icon-btn-${variant}`}
      title={label}
      aria-label={label}
    >
      <Icon name={icon} size={16} />
    </button>
  );
}

export function Card({
  title,
  subtitle,
  actions,
  children,
  className = "",
  flush = false,
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  flush?: boolean;
}) {
  return (
    <section className={`card ${className}`.trim()}>
      {(title || actions) && (
        <header className="card-head">
          <div>
            {title && <h2 className="card-title">{title}</h2>}
            {subtitle && <p className="card-subtitle">{subtitle}</p>}
          </div>
          {actions && <div className="card-actions">{actions}</div>}
        </header>
      )}
      <div className={flush ? "card-body card-body-flush" : "card-body"}>{children}</div>
    </section>
  );
}

export function Stat({
  label,
  value,
  hint,
  icon,
  tone = "default",
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  icon: IconName;
  tone?: "default" | "ok" | "warn" | "danger";
}) {
  return (
    <div className={`stat stat-${tone}`}>
      <div className="stat-top">
        <span className="stat-label">{label}</span>
        <span className="stat-icon">
          <Icon name={icon} size={15} />
        </span>
      </div>
      <div className="stat-value">{value}</div>
      {hint && <div className="stat-hint">{hint}</div>}
    </div>
  );
}

type Tone = "ok" | "warn" | "danger" | "info" | "muted" | "accent";

export function Pill({
  children,
  tone = "muted",
  dot = false,
  title,
}: {
  children: ReactNode;
  tone?: Tone;
  dot?: boolean;
  title?: string;
}) {
  return (
    <span className={`pill pill-${tone}`} title={title}>
      {dot && <span className="pill-dot" />}
      {children}
    </span>
  );
}

export function Field({
  label,
  hint,
  error,
  children,
  inline = false,
}: {
  label: string;
  hint?: ReactNode;
  error?: string | null;
  children: ReactNode;
  inline?: boolean;
}) {
  return (
    <label className={inline ? "field field-inline" : "field"}>
      <span className="field-label">
        {label}
        {hint && <span className="field-hint">{hint}</span>}
      </span>
      {children}
      {error && <span className="field-error">{error}</span>}
    </label>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <label className={`toggle ${disabled ? "toggle-disabled" : ""}`}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="toggle-track">
        <span className="toggle-thumb" />
      </span>
      <span className="toggle-label">{label}</span>
    </label>
  );
}

export function Banner({
  tone = "info",
  title,
  children,
  action,
}: {
  tone?: Tone;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className={`banner banner-${tone}`}>
      <div className="banner-icon">
        <Icon name={tone === "danger" || tone === "warn" ? "alert" : "info"} size={16} />
      </div>
      <div className="banner-copy">
        <strong>{title}</strong>
        {children && <div className="banner-body">{children}</div>}
      </div>
      {action && <div className="banner-action">{action}</div>}
    </div>
  );
}

export function EmptyState({
  icon = "info",
  title,
  body,
  action,
}: {
  icon?: IconName;
  title: string;
  body?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <span className="empty-icon">
        <Icon name={icon} size={22} />
      </span>
      <h3>{title}</h3>
      {body && <p>{body}</p>}
      {action && <div className="empty-action">{action}</div>}
    </div>
  );
}

export function Modal({
  open,
  title,
  subtitle,
  onClose,
  children,
  footer,
  wide = false,
  xl = false,
}: {
  open: boolean;
  title: string;
  subtitle?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  xl?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className={`modal ${xl ? "modal-xl" : wide ? "modal-wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modal-head">
          <div>
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          <IconButton icon="close" label="Close" onClick={onClose} />
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-foot">{footer}</footer>}
      </div>
    </div>
  );
}

export function Spinner({ label }: { label?: string }) {
  return (
    <div className="loading">
      <span className="spinner" />
      {label && <span>{label}</span>}
    </div>
  );
}

export function InlineError({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <div className="inline-error">
      <Icon name="alert" size={14} />
      <span>{message}</span>
    </div>
  );
}

export function InlineNote({ children }: { children: ReactNode }) {
  return (
    <div className="inline-note">
      <Icon name="info" size={14} />
      <div>{children}</div>
    </div>
  );
}

export function SectionTitle({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="section-title">
      <h3>{children}</h3>
      {aside}
    </div>
  );
}
