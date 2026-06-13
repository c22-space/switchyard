use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// SQLite-backed event store for route events.
#[derive(Clone)]
pub struct EventStore {
    inner: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub prompt: String,
    pub category: String,
    pub score: f32,
    pub is_fallback: bool,
    pub backend: String,
    pub model: String,
    pub latency_ms: Option<f64>,
    pub status: String,
    pub error: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub estimated_cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteStats {
    pub total_routes: u64,
    pub tool_call_count: u64,
    pub general_count: u64,
    pub fallback_count: u64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub avg_score: f64,
    pub accuracy_pct: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_usd: f64,
}

impl EventStore {
    pub fn new(db_path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS route_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                prompt TEXT NOT NULL,
                category TEXT NOT NULL,
                score REAL NOT NULL,
                is_fallback INTEGER NOT NULL,
                backend TEXT NOT NULL,
                model TEXT NOT NULL,
                latency_ms REAL,
                status TEXT NOT NULL DEFAULT 'ok',
                error TEXT,
                input_tokens INTEGER,
                output_tokens INTEGER,
                estimated_cost REAL
            );
            CREATE INDEX IF NOT EXISTS idx_route_events_timestamp ON route_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_route_events_category ON route_events(category);
            ",
        )?;
        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn log_route(
        &self,
        prompt: &str,
        category: &str,
        score: f32,
        is_fallback: bool,
        backend: &str,
        model: &str,
        latency_ms: Option<f64>,
        status: &str,
        error: Option<&str>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        estimated_cost: Option<f64>,
    ) -> SqlResult<String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().to_rfc3339();
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO route_events (id, timestamp, prompt, category, score, is_fallback, backend, model, latency_ms, status, error, input_tokens, output_tokens, estimated_cost)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                id,
                timestamp,
                prompt,
                category,
                score,
                is_fallback as i32,
                backend,
                model,
                latency_ms,
                status,
                error,
                input_tokens,
                output_tokens,
                estimated_cost,
            ],
        )?;
        Ok(id)
    }

    pub fn recent_routes(&self, limit: u32) -> SqlResult<Vec<RouteEvent>> {
        let conn = self.inner.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, prompt, category, score, is_fallback, backend, model, latency_ms, status, error, input_tokens, output_tokens, estimated_cost
             FROM route_events ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(RouteEvent {
                id: row.get(0)?,
                timestamp: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                prompt: row.get(2)?,
                category: row.get(3)?,
                score: row.get(4)?,
                is_fallback: row.get::<_, i32>(5)? != 0,
                backend: row.get(6)?,
                model: row.get(7)?,
                latency_ms: row.get(8)?,
                status: row.get(9)?,
                error: row.get(10)?,
                input_tokens: row.get(11)?,
                output_tokens: row.get(12)?,
                estimated_cost: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    pub fn stats(&self) -> SqlResult<RouteStats> {
        let conn = self.inner.lock().unwrap();
        let total: u64 = conn.query_row("SELECT COUNT(*) FROM route_events", [], |r| r.get(0))?;
        let tool_call: u64 = conn.query_row(
            "SELECT COUNT(*) FROM route_events WHERE category = 'tool_call'",
            [],
            |r| r.get(0),
        )?;
        let general: u64 = conn.query_row(
            "SELECT COUNT(*) FROM route_events WHERE category = 'general'",
            [],
            |r| r.get(0),
        )?;
        let fallback: u64 = conn.query_row(
            "SELECT COUNT(*) FROM route_events WHERE is_fallback = 1",
            [],
            |r| r.get(0),
        )?;
        let avg_latency: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(latency_ms), 0) FROM route_events WHERE latency_ms IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);
        let avg_score: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(score), 0) FROM route_events",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        // p50 and p95 latency
        let p50: f64 = conn
            .query_row(
                "SELECT COALESCE(latency_ms, 0) FROM route_events WHERE latency_ms IS NOT NULL
                 ORDER BY latency_ms LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM route_events WHERE latency_ms IS NOT NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);
        let p95: f64 = conn
            .query_row(
                "SELECT COALESCE(latency_ms, 0) FROM route_events WHERE latency_ms IS NOT NULL
                 ORDER BY latency_ms LIMIT 1 OFFSET (SELECT CAST(COUNT(*)*0.95 AS INTEGER) FROM route_events WHERE latency_ms IS NOT NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        let non_fallback = total - fallback;
        let accuracy = if total > 0 {
            (non_fallback as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Token and cost aggregates
        let total_input_tokens: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(input_tokens), 0) FROM route_events WHERE input_tokens IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let total_output_tokens: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(output_tokens), 0) FROM route_events WHERE output_tokens IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let total_cost_usd: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(estimated_cost), 0) FROM route_events WHERE estimated_cost IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        Ok(RouteStats {
            total_routes: total,
            tool_call_count: tool_call,
            general_count: general,
            fallback_count: fallback,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            avg_score: avg_score,
            accuracy_pct: accuracy,
            total_input_tokens,
            total_output_tokens,
            total_cost_usd,
        })
    }
}
