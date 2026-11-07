use crate::error::Result;
use crate::model::*;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Store {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct ProviderMeta {
    pub provider: Provider,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub account: Option<String>,
    pub last_error: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub instance_count: usize,
    pub active_count: usize,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn parse_dt(value: Option<String>) -> Option<DateTime<Utc>> {
    value.and_then(|v| DateTime::parse_from_rfc3339(&v).ok().map(|d| d.with_timezone(&Utc)))
}

fn dt_string(value: Option<DateTime<Utc>>) -> Option<String> {
    value.map(|v| v.to_rfc3339())
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self { conn: Mutex::new(conn) };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn: Mutex::new(conn) };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS providers (
                provider TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL DEFAULT 0,
                base_url TEXT,
                account TEXT,
                last_error TEXT,
                verified_at TEXT
            );
            CREATE TABLE IF NOT EXISTS device_meta (
                serial TEXT PRIMARY KEY,
                label TEXT,
                notes TEXT
            );
            CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                target_device TEXT,
                steps TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                workflow_name TEXT NOT NULL,
                device_id TEXT,
                status TEXT NOT NULL,
                trigger TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_runs_started ON runs(started_at DESC);
            CREATE TABLE IF NOT EXISTS run_lines (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                ts TEXT NOT NULL,
                line TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_run_lines_run ON run_lines(run_id, id);
            CREATE TABLE IF NOT EXISTS schedules (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                device_id TEXT,
                every_minutes INTEGER NOT NULL DEFAULT 0,
                daily_at TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                next_run_at TEXT,
                last_run_at TEXT,
                last_status TEXT
            );
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                platform TEXT NOT NULL,
                label TEXT NOT NULL,
                kind TEXT NOT NULL,
                config TEXT NOT NULL,
                verified_at TEXT,
                verified_as TEXT,
                last_error TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS posts (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                body TEXT NOT NULL,
                scheduled_at TEXT,
                status TEXT NOT NULL,
                remote_id TEXT,
                published_at TEXT,
                error TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_logs_ts ON logs(ts DESC);
            CREATE TABLE IF NOT EXISTS instance_presets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                request TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    // -- settings ----------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let value = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_json_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get_setting(key)? {
            Some(raw) => Ok(serde_json::from_str(&raw).ok()),
            None => Ok(None),
        }
    }

    pub fn set_json_setting<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set_setting(key, &serde_json::to_string(value)?)
    }

    // -- providers ---------------------------------------------------------

    pub fn set_provider_meta(&self, provider: Provider, enabled: bool, base_url: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO providers(provider, enabled, base_url) VALUES(?1, ?2, ?3)
             ON CONFLICT(provider) DO UPDATE SET enabled = excluded.enabled, base_url = excluded.base_url",
            params![provider.slug(), enabled as i32, base_url],
        )?;
        Ok(())
    }

    pub fn set_provider_check(&self, provider: Provider, account: Option<&str>, error: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE providers SET account = ?2, last_error = ?3, verified_at = ?4 WHERE provider = ?1",
            params![provider.slug(), account, error, now()],
        )?;
        Ok(())
    }

    pub fn provider_meta(&self) -> Result<Vec<ProviderMeta>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT provider, enabled, base_url, account, last_error, verified_at FROM providers",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (slug, enabled, base_url, account, last_error, verified_at) = row?;
            let Some(provider) = Provider::from_slug(&slug) else { continue };
            out.push(ProviderMeta {
                provider,
                enabled: enabled != 0,
                base_url,
                account,
                last_error,
                verified_at: parse_dt(verified_at),
                instance_count: 0,
                active_count: 0,
            });
        }
        Ok(out)
    }

    // -- device labels -----------------------------------------------------

    pub fn set_device_meta(&self, serial: &str, label: &str, notes: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO device_meta(serial, label, notes) VALUES(?1, ?2, ?3)
             ON CONFLICT(serial) DO UPDATE SET label = excluded.label, notes = excluded.notes",
            params![serial, label, notes],
        )?;
        Ok(())
    }

    pub fn device_meta(&self) -> Result<Vec<(String, String, Option<String>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT serial, label, notes FROM device_meta")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // -- workflows ---------------------------------------------------------

    pub fn list_workflows(&self) -> Result<Vec<Workflow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, target_device, steps, created_at, updated_at
             FROM workflows ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, description, target_device, steps, created_at, updated_at) = row?;
            out.push(Workflow {
                id,
                name,
                description,
                target_device,
                steps: serde_json::from_str(&steps).unwrap_or_default(),
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            });
        }
        Ok(out)
    }

    pub fn get_workflow(&self, id: &str) -> Result<Option<Workflow>> {
        Ok(self.list_workflows()?.into_iter().find(|w| w.id == id))
    }

    pub fn save_workflow(&self, workflow: &Workflow) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO workflows(id, name, description, target_device, steps, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
                 target_device = excluded.target_device, steps = excluded.steps, updated_at = excluded.updated_at",
            params![
                workflow.id,
                workflow.name,
                workflow.description,
                workflow.target_device,
                serde_json::to_string(&workflow.steps)?,
                workflow.created_at.to_rfc3339(),
                workflow.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_workflow(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM workflows WHERE id = ?1", params![id])?;
        conn.execute("DELETE FROM schedules WHERE workflow_id = ?1", params![id])?;
        Ok(())
    }

    // -- runs --------------------------------------------------------------

    pub fn insert_run(&self, run: &WorkflowRun) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO runs(id, workflow_id, workflow_name, device_id, status, trigger, started_at, finished_at, error)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id,
                run.workflow_id,
                run.workflow_name,
                run.device_id,
                status_key(run.status),
                run.trigger,
                run.started_at.to_rfc3339(),
                dt_string(run.finished_at),
                run.error,
            ],
        )?;
        Ok(())
    }

    pub fn update_run(&self, id: &str, status: RunStatus, error: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        let finished = if status.is_finished() { Some(now()) } else { None };
        conn.execute(
            "UPDATE runs SET status = ?2, error = ?3, finished_at = COALESCE(?4, finished_at) WHERE id = ?1",
            params![id, status_key(status), error, finished],
        )?;
        Ok(())
    }

    pub fn append_run_line(&self, run_id: &str, line: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO run_lines(run_id, ts, line) VALUES(?1, ?2, ?3)",
            params![run_id, now(), line],
        )?;
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> Result<Option<WorkflowRun>> {
        let run = {
            let conn = self.conn.lock();
            conn.query_row(
                "SELECT id, workflow_id, workflow_name, device_id, status, trigger, started_at, finished_at, error
                 FROM runs WHERE id = ?1",
                params![id],
                |row| {
                    Ok(WorkflowRun {
                        id: row.get(0)?,
                        workflow_id: row.get(1)?,
                        workflow_name: row.get(2)?,
                        device_id: row.get(3)?,
                        status: parse_status(&row.get::<_, String>(4)?),
                        trigger: row.get(5)?,
                        started_at: parse_dt(Some(row.get::<_, String>(6)?)).unwrap_or_else(Utc::now),
                        finished_at: parse_dt(row.get(7)?),
                        error: row.get(8)?,
                        lines: Vec::new(),
                    })
                },
            )
            .optional()?
        };
        let Some(mut run) = run else { return Ok(None) };
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT line FROM run_lines WHERE run_id = ?1 ORDER BY id ASC")?;
        let rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
        for row in rows {
            run.lines.push(row?);
        }
        Ok(Some(run))
    }

    pub fn recent_runs(&self, limit: usize) -> Result<Vec<WorkflowRun>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare("SELECT id FROM runs ORDER BY started_at DESC LIMIT ?1")?;
            let rows = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };
        let mut runs = Vec::new();
        for id in ids {
            if let Some(run) = self.get_run(&id)? {
                runs.push(run);
            }
        }
        Ok(runs)
    }

    /// Runs filtered by status and/or workflow, newest first.
    pub fn query_runs(
        &self,
        limit: usize,
        status: Option<&str>,
        workflow_id: Option<&str>,
    ) -> Result<Vec<WorkflowRun>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(
                "SELECT id FROM runs
                 WHERE (?1 IS NULL OR status = ?1)
                   AND (?2 IS NULL OR workflow_id = ?2)
                 ORDER BY started_at DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![status, workflow_id, limit as i64], |row| {
                row.get::<_, String>(0)
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };
        let mut runs = Vec::new();
        for id in ids {
            if let Some(run) = self.get_run(&id)? {
                runs.push(run);
            }
        }
        Ok(runs)
    }

    // -- instance presets --------------------------------------------------

    pub fn list_presets(&self) -> Result<Vec<InstancePreset>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, request, created_at FROM instance_presets
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, provider, request, created_at) = row?;
            let Some(provider) = Provider::from_slug(&provider) else {
                continue;
            };
            let Ok(request) = serde_json::from_str(&request) else {
                continue;
            };
            out.push(InstancePreset {
                id,
                name,
                provider,
                request,
                created_at: parse_dt(Some(created_at)).unwrap_or_else(Utc::now),
            });
        }
        Ok(out)
    }

    pub fn save_preset(&self, preset: &InstancePreset) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO instance_presets(id, name, provider, request, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, provider = excluded.provider,
                 request = excluded.request",
            params![
                preset.id,
                preset.name,
                preset.provider.slug(),
                serde_json::to_string(&preset.request)?,
                preset.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_preset(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM instance_presets WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn count_runs_since(&self, since: DateTime<Utc>, only_failed: bool) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = if only_failed {
            conn.query_row(
                "SELECT COUNT(*) FROM runs WHERE started_at >= ?1 AND status = 'failed'",
                params![since.to_rfc3339()],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM runs WHERE started_at >= ?1",
                params![since.to_rfc3339()],
                |row| row.get(0),
            )?
        };
        Ok(count as usize)
    }

    pub fn running_runs(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id FROM runs WHERE status IN ('queued','running')")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn fail_stale_runs(&self, message: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE runs SET status = 'failed', error = ?1, finished_at = ?2 WHERE status IN ('queued','running')",
            params![message, now()],
        )?;
        Ok(())
    }

    // -- schedules ---------------------------------------------------------

    pub fn list_schedules(&self) -> Result<Vec<Schedule>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.workflow_id, COALESCE(w.name, 'deleted workflow'), s.device_id,
                    s.every_minutes, s.daily_at, s.enabled, s.next_run_at, s.last_run_at, s.last_status
             FROM schedules s LEFT JOIN workflows w ON w.id = s.workflow_id
             ORDER BY s.enabled DESC, s.next_run_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Schedule {
                id: row.get(0)?,
                workflow_id: row.get(1)?,
                workflow_name: row.get(2)?,
                device_id: row.get(3)?,
                every_minutes: row.get::<_, i64>(4)?.max(0) as u32,
                daily_at: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                next_run_at: parse_dt(row.get(7)?),
                last_run_at: parse_dt(row.get(8)?),
                last_status: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn save_schedule(&self, schedule: &Schedule) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO schedules(id, workflow_id, device_id, every_minutes, daily_at, enabled, next_run_at, last_run_at, last_status)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET device_id = excluded.device_id, every_minutes = excluded.every_minutes,
                 daily_at = excluded.daily_at, enabled = excluded.enabled, next_run_at = excluded.next_run_at,
                 last_run_at = excluded.last_run_at, last_status = excluded.last_status",
            params![
                schedule.id,
                schedule.workflow_id,
                schedule.device_id,
                schedule.every_minutes as i64,
                schedule.daily_at,
                schedule.enabled as i64,
                dt_string(schedule.next_run_at),
                dt_string(schedule.last_run_at),
                schedule.last_status,
            ],
        )?;
        Ok(())
    }

    pub fn delete_schedule(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM schedules WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn due_schedules(&self, at: DateTime<Utc>) -> Result<Vec<Schedule>> {
        Ok(self
            .list_schedules()?
            .into_iter()
            .filter(|s| s.enabled)
            .filter(|s| match s.next_run_at {
                Some(next) => next <= at,
                None => true,
            })
            .collect())
    }

    // -- publishing accounts ----------------------------------------------

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, platform, label, kind, config, verified_at, verified_as, last_error, created_at FROM accounts ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                platform: row.get(1)?,
                label: row.get(2)?,
                kind: parse_publisher(&row.get::<_, String>(3)?),
                config: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or(serde_json::Value::Null),
                secret_set: false,
                verified_at: parse_dt(row.get(5)?),
                verified_as: row.get(6)?,
                last_error: row.get(7)?,
                created_at: parse_dt(Some(row.get::<_, String>(8)?)).unwrap_or_else(Utc::now),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_account(&self, id: &str) -> Result<Option<Account>> {
        Ok(self.list_accounts()?.into_iter().find(|a| a.id == id))
    }

    pub fn save_account(&self, account: &Account) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO accounts(id, platform, label, kind, config, verified_at, verified_as, last_error, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET platform = excluded.platform, label = excluded.label, kind = excluded.kind,
                 config = excluded.config, verified_at = excluded.verified_at, verified_as = excluded.verified_as,
                 last_error = excluded.last_error",
            params![
                account.id,
                account.platform,
                account.label,
                publisher_key(account.kind),
                serde_json::to_string(&account.config)?,
                dt_string(account.verified_at),
                account.verified_as,
                account.last_error,
                account.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        conn.execute("DELETE FROM posts WHERE account_id = ?1", params![id])?;
        Ok(())
    }

    // -- posts -------------------------------------------------------------

    pub fn list_posts(&self, limit: usize) -> Result<Vec<Post>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.account_id, COALESCE(a.label, 'removed account'), p.body, p.scheduled_at,
                    p.status, p.remote_id, p.published_at, p.error, p.created_at
             FROM posts p LEFT JOIN accounts a ON a.id = p.account_id
             ORDER BY COALESCE(p.scheduled_at, p.created_at) DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(Post {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_label: row.get(2)?,
                body: row.get(3)?,
                scheduled_at: parse_dt(row.get(4)?),
                status: parse_post_status(&row.get::<_, String>(5)?),
                remote_id: row.get(6)?,
                published_at: parse_dt(row.get(7)?),
                error: row.get(8)?,
                created_at: parse_dt(Some(row.get::<_, String>(9)?)).unwrap_or_else(Utc::now),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_post(&self, id: &str) -> Result<Option<Post>> {
        Ok(self.list_posts(500)?.into_iter().find(|p| p.id == id))
    }

    pub fn save_post(&self, post: &Post) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO posts(id, account_id, body, scheduled_at, status, remote_id, published_at, error, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET body = excluded.body, scheduled_at = excluded.scheduled_at,
                 status = excluded.status, remote_id = excluded.remote_id, published_at = excluded.published_at,
                 error = excluded.error",
            params![
                post.id,
                post.account_id,
                post.body,
                dt_string(post.scheduled_at),
                post_status_key(post.status),
                post.remote_id,
                dt_string(post.published_at),
                post.error,
                post.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_post(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM posts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn due_posts(&self, at: DateTime<Utc>) -> Result<Vec<Post>> {
        Ok(self
            .list_posts(500)?
            .into_iter()
            .filter(|p| p.status == PostStatus::Scheduled)
            .filter(|p| p.scheduled_at.map(|s| s <= at).unwrap_or(false))
            .collect())
    }

    // -- logs --------------------------------------------------------------

    pub fn log(&self, level: &str, target: &str, message: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO logs(ts, level, target, message) VALUES(?1, ?2, ?3, ?4)",
            params![now(), level, target, message],
        )?;
        // Keep the local log bounded.
        conn.execute(
            "DELETE FROM logs WHERE id <= (SELECT MAX(id) FROM logs) - 5000",
            [],
        )?;
        Ok(())
    }

    pub fn list_logs(&self, limit: usize, level: Option<&str>, search: Option<&str>) -> Result<Vec<LogLine>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, ts, level, target, message FROM logs
             WHERE (?1 IS NULL OR level = ?1)
               AND (?2 IS NULL OR message LIKE ?3 OR target LIKE ?3)
             ORDER BY id DESC LIMIT ?4",
        )?;
        let like = search.map(|s| format!("%{s}%"));
        let rows = stmt.query_map(params![level, search, like, limit as i64], |row| {
            Ok(LogLine {
                id: row.get(0)?,
                ts: parse_dt(Some(row.get::<_, String>(1)?)).unwrap_or_else(Utc::now),
                level: row.get(2)?,
                target: row.get(3)?,
                message: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

fn status_key(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::Succeeded => "succeeded",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
    }
}

fn parse_status(value: &str) -> RunStatus {
    match value {
        "queued" => RunStatus::Queued,
        "running" => RunStatus::Running,
        "succeeded" => RunStatus::Succeeded,
        "cancelled" => RunStatus::Cancelled,
        _ => RunStatus::Failed,
    }
}

fn publisher_key(kind: PublisherKind) -> &'static str {
    match kind {
        PublisherKind::Mastodon => "mastodon",
        PublisherKind::Webhook => "webhook",
    }
}

fn parse_publisher(value: &str) -> PublisherKind {
    match value {
        "mastodon" => PublisherKind::Mastodon,
        _ => PublisherKind::Webhook,
    }
}

fn post_status_key(status: PostStatus) -> &'static str {
    match status {
        PostStatus::Draft => "draft",
        PostStatus::Scheduled => "scheduled",
        PostStatus::Publishing => "publishing",
        PostStatus::Published => "published",
        PostStatus::Failed => "failed",
    }
}

fn parse_post_status(value: &str) -> PostStatus {
    match value {
        "draft" => PostStatus::Draft,
        "scheduled" => PostStatus::Scheduled,
        "publishing" => PostStatus::Publishing,
        "published" => PostStatus::Published,
        _ => PostStatus::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflow(name: &str) -> Workflow {
        Workflow {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: "test".into(),
            target_device: None,
            steps: vec![Step {
                id: "s1".into(),
                name: "tap".into(),
                action: StepAction::Tap { x: 10, y: 20 },
                retries: 0,
                timeout_ms: None,
                continue_on_error: false,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn workflows_round_trip() {
        let store = Store::open_in_memory().unwrap();
        let wf = workflow("Daily check");
        store.save_workflow(&wf).unwrap();
        let loaded = store.get_workflow(&wf.id).unwrap().unwrap();
        assert_eq!(loaded.name, "Daily check");
        assert_eq!(loaded.steps.len(), 1);
        assert!(matches!(loaded.steps[0].action, StepAction::Tap { x: 10, .. }));

        let mut updated = loaded.clone();
        updated.name = "Renamed".into();
        store.save_workflow(&updated).unwrap();
        assert_eq!(store.get_workflow(&wf.id).unwrap().unwrap().name, "Renamed");
        store.delete_workflow(&wf.id).unwrap();
        assert!(store.get_workflow(&wf.id).unwrap().is_none());
    }

    #[test]
    fn runs_and_lines_round_trip() {
        let store = Store::open_in_memory().unwrap();
        let run = WorkflowRun {
            id: "run-1".into(),
            workflow_id: "wf-1".into(),
            workflow_name: "Checkout".into(),
            device_id: Some("emulator-5554".into()),
            status: RunStatus::Running,
            trigger: "manual".into(),
            started_at: Utc::now(),
            finished_at: None,
            error: None,
            lines: Vec::new(),
        };
        store.insert_run(&run).unwrap();
        store.append_run_line(&run.id, "step 1 ok").unwrap();
        store.append_run_line(&run.id, "step 2 ok").unwrap();
        let loaded = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(loaded.lines.len(), 2);
        assert_eq!(loaded.status, RunStatus::Running);

        store.update_run(&run.id, RunStatus::Failed, Some("boom")).unwrap();
        let loaded = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(loaded.status, RunStatus::Failed);
        assert_eq!(loaded.error.as_deref(), Some("boom"));
        assert!(loaded.finished_at.is_some());
        assert!(store.running_runs().unwrap().is_empty());
    }

    #[test]
    fn settings_and_logs_work() {
        let store = Store::open_in_memory().unwrap();
        store.set_setting("adb_path", "C:/platform-tools/adb.exe").unwrap();
        assert_eq!(store.get_setting("adb_path").unwrap().as_deref(), Some("C:/platform-tools/adb.exe"));

        let settings = AppSettings::default();
        store.set_json_setting("app_settings", &settings).unwrap();
        let loaded: AppSettings = store.get_json_setting("app_settings").unwrap().unwrap();
        assert_eq!(loaded.webdriver_url, "http://127.0.0.1:4444");

        store.log("info", "test", "hello").unwrap();
        store.log("error", "test", "failed").unwrap();
        assert_eq!(store.list_logs(10, None, None).unwrap().len(), 2);
        assert_eq!(store.list_logs(10, Some("error"), None).unwrap().len(), 1);
        assert_eq!(store.list_logs(10, None, Some("hello")).unwrap().len(), 1);
    }

    #[test]
    fn schedules_are_due_when_next_run_passed() {
        let store = Store::open_in_memory().unwrap();
        let wf = workflow("Nightly");
        store.save_workflow(&wf).unwrap();
        let now = Utc::now();
        let mut schedule = Schedule {
            id: "sch-1".into(),
            workflow_id: wf.id.clone(),
            workflow_name: wf.name.clone(),
            device_id: None,
            every_minutes: 30,
            daily_at: None,
            enabled: true,
            next_run_at: Some(now - chrono::Duration::minutes(1)),
            last_run_at: None,
            last_status: None,
        };
        store.save_schedule(&schedule).unwrap();
        assert_eq!(store.due_schedules(now).unwrap().len(), 1);

        // A schedule whose next_run_at is still in the future is not due yet.
        schedule.next_run_at = Some(now + chrono::Duration::minutes(5));
        store.save_schedule(&schedule).unwrap();
        assert!(store.due_schedules(now).unwrap().is_empty());
        assert_eq!(
            store
                .due_schedules(now + chrono::Duration::minutes(6))
                .unwrap()
                .len(),
            1
        );

        // Disabled schedules never fire, even when their timestamp has passed.
        schedule.enabled = false;
        schedule.next_run_at = Some(now - chrono::Duration::minutes(1));
        store.save_schedule(&schedule).unwrap();
        assert!(store.due_schedules(now).unwrap().is_empty());
    }
}
