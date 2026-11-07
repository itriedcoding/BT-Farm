import { useCallback, useEffect, useState } from "react";
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
  Spinner,
} from "../components/ui";
import { api, isDesktop } from "../lib/api";
import { formatDateTime, localInputToRfc3339, titleCase } from "../lib/format";
import type { Account, Post, PublisherKind } from "../lib/types";

export function PublishingPage() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [posts, setPosts] = useState<Post[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editingAccount, setEditingAccount] = useState<Account | "new" | null>(null);
  const [composerOpen, setComposerOpen] = useState(false);

  const load = useCallback(async () => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const [accountList, postList] = await Promise.all([api.listAccounts(), api.listPosts(100)]);
      setAccounts(accountList);
      setPosts(postList);
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

  const verify = async (account: Account) => {
    setNotice(null);
    try {
      const updated = await api.verifyAccount(account.id);
      setNotice(`${updated.label} verified as ${updated.verified_as ?? "unknown"}`);
      await load();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
      await load();
    }
  };

  const removeAccount = async (account: Account) => {
    if (!window.confirm(`Remove ${account.label} and its queued posts?`)) return;
    try {
      await api.deleteAccount(account.id);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const publishNow = async (post: Post) => {
    setNotice(null);
    try {
      const updated = await api.publishPost(post.id);
      setNotice(updated.remote_id ? `Published (remote id ${updated.remote_id})` : "Published");
      await load();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err));
      await load();
    }
  };

  return (
    <div className="page">
      {error && <Banner tone="danger" title="Publishing error">{error}</Banner>}
      {notice && <Banner tone="info" title="Publisher">{notice}</Banner>}

      <Banner tone="warn" title="Authorised publishing only">
        Posts go through official platform APIs with credentials you supply. BT Farm never creates
        accounts, bypasses CAPTCHAs, evades rate limits or scrapes private endpoints. Anything a
        platform does not permit through its API is out of scope.
      </Banner>

      <Card
        title="Accounts"
        subtitle="Tokens are sealed with Windows DPAPI and never shown again"
        actions={
          <>
            <Button size="sm" icon="refresh" onClick={() => void load()} loading={loading}>
              Refresh
            </Button>
            <Button size="sm" variant="primary" icon="plus" onClick={() => setEditingAccount("new")}>
              Add account
            </Button>
          </>
        }
        flush
      >
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Account</th>
                <th>Integration</th>
                <th>Credential</th>
                <th>Verified</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {accounts.length === 0 ? (
                <tr>
                  <td colSpan={5}>
                    <EmptyState
                      icon="send"
                      title="No publishing accounts"
                      body="Connect a Mastodon account with an access token, or point a templated request at any official HTTP API you already use."
                    />
                  </td>
                </tr>
              ) : (
                accounts.map((account) => (
                  <tr key={account.id}>
                    <td>
                      <div className="cell-title">{account.label}</div>
                      <div className="cell-sub">{account.platform || account.kind}</div>
                    </td>
                    <td>
                      <Pill tone="muted">
                        {account.kind === "mastodon" ? "Mastodon API" : "HTTP API"}
                      </Pill>
                    </td>
                    <td>
                      <Pill tone={account.secret_set ? "ok" : "warn"}>
                        {account.secret_set ? "stored" : "missing"}
                      </Pill>
                    </td>
                    <td className="muted">
                      {account.verified_at ? (
                        <span title={formatDateTime(account.verified_at)}>
                          {account.verified_as ?? "ok"}
                        </span>
                      ) : account.last_error ? (
                        <span style={{ color: "#ffb3bd" }}>{account.last_error}</span>
                      ) : (
                        "not verified"
                      )}
                    </td>
                    <td>
                      <div className="cell-actions">
                        <IconButton
                          icon="check"
                          label="Test credentials"
                          onClick={() => void verify(account)}
                        />
                        <IconButton
                          icon="settings"
                          label="Edit"
                          onClick={() => setEditingAccount(account)}
                        />
                        <IconButton
                          icon="trash"
                          label="Remove"
                          variant="danger"
                          onClick={() => void removeAccount(account)}
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

      <Card
        title="Posts"
        subtitle="Drafts, queued posts and published results"
        actions={
          <Button
            size="sm"
            variant="primary"
            icon="send"
            disabled={accounts.length === 0}
            onClick={() => setComposerOpen(true)}
          >
            Compose
          </Button>
        }
        flush
      >
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Content</th>
                <th>Account</th>
                <th>Status</th>
                <th>Scheduled</th>
                <th>Published</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {posts.length === 0 ? (
                <tr>
                  <td colSpan={6}>
                    <div className="table-empty">Nothing composed yet.</div>
                  </td>
                </tr>
              ) : (
                posts.map((post) => (
                  <tr key={post.id}>
                    <td>
                      <div className="cell-title" style={{ maxWidth: 340, whiteSpace: "normal" }}>
                        {post.body}
                      </div>
                      {post.error && (
                        <div className="cell-sub" style={{ color: "#ffb3bd", fontFamily: "inherit" }}>
                          {post.error}
                        </div>
                      )}
                    </td>
                    <td className="muted">{post.account_label}</td>
                    <td>
                      <Pill
                        tone={
                          post.status === "published"
                            ? "ok"
                            : post.status === "failed"
                              ? "danger"
                              : post.status === "scheduled"
                                ? "info"
                                : "muted"
                        }
                        dot={post.status === "publishing"}
                      >
                        {titleCase(post.status)}
                      </Pill>
                    </td>
                    <td className="muted nowrap">{formatDateTime(post.scheduled_at)}</td>
                    <td className="muted nowrap">{formatDateTime(post.published_at)}</td>
                    <td>
                      <div className="cell-actions">
                        {post.status !== "published" && (
                          <IconButton
                            icon="send"
                            label="Publish now"
                            onClick={() => void publishNow(post)}
                          />
                        )}
                        <IconButton
                          icon="trash"
                          label="Delete"
                          variant="danger"
                          onClick={async () => {
                            await api.deletePost(post.id).catch(() => undefined);
                            await load();
                          }}
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

      <InlineNote>
        Queued posts are dispatched by a background loop while the app is running. If a publish
        fails, the provider's error message is stored on the post so you can fix the credential or
        endpoint and retry.
      </InlineNote>

      {loading && posts.length === 0 && <Spinner label="Loading publishing state…" />}

      {editingAccount && (
        <AccountModal
          account={editingAccount === "new" ? null : editingAccount}
          onClose={() => setEditingAccount(null)}
          onSaved={async () => {
            setEditingAccount(null);
            await load();
          }}
        />
      )}

      {composerOpen && (
        <ComposerModal
          accounts={accounts}
          onClose={() => setComposerOpen(false)}
          onSaved={async () => {
            setComposerOpen(false);
            await load();
          }}
        />
      )}
    </div>
  );
}

function AccountModal({
  account,
  onClose,
  onSaved,
}: {
  account: Account | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [kind, setKind] = useState<PublisherKind>(account?.kind ?? "mastodon");
  const [label, setLabel] = useState(account?.label ?? "");
  const [platform, setPlatform] = useState(account?.platform ?? "");
  const [instanceUrl, setInstanceUrl] = useState(
    (account?.config?.instance_url as string | undefined) ?? "",
  );
  const [url, setUrl] = useState((account?.config?.url as string | undefined) ?? "");
  const [method, setMethod] = useState((account?.config?.method as string | undefined) ?? "POST");
  const [headers, setHeaders] = useState(
    JSON.stringify(account?.config?.headers ?? { "Content-Type": "application/json" }, null, 2),
  );
  const [bodyTemplate, setBodyTemplate] = useState(
    (account?.config?.body_template as string | undefined) ?? '{"content":"{{content}}"}',
  );
  const [token, setToken] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      let parsedHeaders: Record<string, string> = {};
      try {
        parsedHeaders = JSON.parse(headers) as Record<string, string>;
      } catch {
        throw new Error("headers must be a JSON object of string values");
      }
      const config =
        kind === "mastodon"
          ? { instance_url: instanceUrl.trim() }
          : {
              url: url.trim(),
              method: method.toUpperCase(),
              headers: parsedHeaders,
              body_template: bodyTemplate,
            };
      await api.saveAccount({
        id: account?.id ?? null,
        platform: platform.trim() || (kind === "mastodon" ? "Mastodon" : "Custom"),
        label,
        kind,
        config,
        token: token.trim() ? token.trim() : null,
      });
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
      wide
      title={account ? "Edit account" : "Add publishing account"}
      subtitle="Only official API endpoints are supported"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => void save()} loading={saving}>
            Save account
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <Field label="Integration">
          <select
            className="select"
            value={kind}
            onChange={(event) => setKind(event.target.value as PublisherKind)}
          >
            <option value="mastodon">Mastodon (official API)</option>
            <option value="webhook">Official HTTP API (templated)</option>
          </select>
        </Field>
        <Field label="Label">
          <input
            className="input"
            value={label}
            onChange={(event) => setLabel(event.target.value)}
            placeholder="Company Mastodon"
          />
        </Field>
        <Field label="Platform name" hint="shown in the list">
          <input
            className="input"
            value={platform}
            onChange={(event) => setPlatform(event.target.value)}
            placeholder={kind === "mastodon" ? "Mastodon" : "Buffer / X / …"}
          />
        </Field>
        <Field
          label={kind === "mastodon" ? "Instance URL" : "Endpoint URL"}
          hint="https://"
        >
          <input
            className="input mono"
            value={kind === "mastodon" ? instanceUrl : url}
            onChange={(event) =>
              kind === "mastodon" ? setInstanceUrl(event.target.value) : setUrl(event.target.value)
            }
            placeholder={
              kind === "mastodon" ? "https://mastodon.social" : "https://api.example.com/v1/post"
            }
          />
        </Field>
        <div className="span-2">
          <Field
            label={kind === "mastodon" ? "Access token" : "Bearer token (optional)"}
            hint={account?.secret_set ? "stored — leave blank to keep it" : undefined}
          >
            <input
              className="input"
              type="password"
              autoComplete="off"
              value={token}
              onChange={(event) => setToken(event.target.value)}
              placeholder={kind === "mastodon" ? "your instance access token" : "optional"}
            />
          </Field>
        </div>
        {kind === "webhook" && (
          <>
            <Field label="HTTP method">
              <select
                className="select"
                value={method}
                onChange={(event) => setMethod(event.target.value)}
              >
                {["POST", "PUT", "PATCH"].map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Body template" hint="{{content}} is replaced by the post text">
              <input
                className="input mono"
                value={bodyTemplate}
                onChange={(event) => setBodyTemplate(event.target.value)}
              />
            </Field>
            <div className="span-2">
              <Field label="Headers" hint="JSON object">
                <textarea
                  className="textarea mono"
                  value={headers}
                  onChange={(event) => setHeaders(event.target.value)}
                />
              </Field>
            </div>
          </>
        )}
      </div>
      <InlineNote>
        For platforms that require OAuth (X, Instagram, TikTok, LinkedIn), create an app and use the
        templated HTTP integration with an access token you obtained through the official flow.
      </InlineNote>
      <InlineError message={error} />
    </Modal>
  );
}

function ComposerModal({
  accounts,
  onClose,
  onSaved,
}: {
  accounts: Account[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [body, setBody] = useState("");
  const [scheduleAt, setScheduleAt] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (publishNow: boolean) => {
    setSaving(true);
    setError(null);
    try {
      await api.savePost({
        accountId,
        body,
        scheduledAt: publishNow ? null : localInputToRfc3339(scheduleAt),
        publishNow,
      });
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
      wide
      title="Compose"
      subtitle="Queued posts are dispatched by the background publisher"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="secondary" onClick={() => void submit(false)} loading={saving}>
            {scheduleAt ? "Schedule" : "Save draft"}
          </Button>
          <Button variant="primary" icon="send" onClick={() => void submit(true)} loading={saving}>
            Publish now
          </Button>
        </>
      }
    >
      <Field label="Account">
        <select
          className="select"
          value={accountId}
          onChange={(event) => setAccountId(event.target.value)}
        >
          {accounts.map((account) => (
            <option key={account.id} value={account.id}>
              {account.label}
            </option>
          ))}
        </select>
      </Field>
      <Field label="Content" hint={`${body.length} characters`}>
        <textarea
          className="textarea"
          style={{ minHeight: 140 }}
          value={body}
          onChange={(event) => setBody(event.target.value)}
          placeholder="What do you want to publish?"
        />
      </Field>
      <Field
        label="Schedule for"
        hint="leave blank to keep it as a draft"
      >
        <input
          className="input"
          type="datetime-local"
          value={scheduleAt}
          onChange={(event) => setScheduleAt(event.target.value)}
        />
      </Field>
      <InlineError message={error} />
    </Modal>
  );
}
