use leptos::prelude::*;
use leptos_meta::*;
use serde::Deserialize;
use wasm_bindgen::JsCast;

#[derive(Deserialize, Clone, Debug)]
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
}

#[derive(Deserialize, Clone, Debug)]
pub struct RouteEvent {
    pub id: String,
    pub timestamp: String,
    pub prompt: String,
    pub category: String,
    pub score: f32,
    pub is_fallback: bool,
    pub backend: String,
    pub model: String,
    pub latency_ms: Option<f64>,
    pub status: String,
    pub error: Option<String>,
}

async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> Option<T> {
    let window = web_sys::window()?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .ok()?;
    let resp: web_sys::Response = resp_value.dyn_into().ok()?;
    let json_value = wasm_bindgen_futures::JsFuture::from(resp.text().ok()?)
        .await
        .ok()?;
    let text: String = json_value.dyn_into::<js_sys::JsString>().ok()?.into();
    serde_json::from_str(&text).ok()
}

async fn fetch_stats() -> Option<RouteStats> {
    fetch_json("/api/stats").await
}

async fn fetch_routes() -> Vec<RouteEvent> {
    fetch_json("/api/routes?limit=50").await.unwrap_or_default()
}

#[component]
fn StatCard(label: &'static str, value: String, color: &'static str) -> impl IntoView {
    view! {
        <div class="stat-card">
            <div class="label">{label}</div>
            <div class=format!("value {}", color)>{value}</div>
        </div>
    }
}

#[component]
fn Dashboard() -> impl IntoView {
    let (stats, set_stats) = signal(None::<RouteStats>);
    let (routes, set_routes) = signal(Vec::<RouteEvent>::new());

    // Initial fetch
    leptos::task::spawn_local(async move {
        if let Some(s) = fetch_stats().await {
            set_stats.set(Some(s));
        }
        let r = fetch_routes().await;
        set_routes.set(r);
    });

    // Poll every 5 seconds
    leptos::task::spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            if let Some(s) = fetch_stats().await {
                set_stats.set(Some(s));
            }
            let r = fetch_routes().await;
            set_routes.set(r);
        }
    });

    view! {
        <Title text="Switchyard Dashboard"/>
        <Style>{DASHBOARD_CSS}</Style>

        <div class="container">
            <h1>"Switchyard Dashboard"</h1>

            <div class="stats">
                {move || {
                    let s = stats.get();
                    let total = s.as_ref().map(|s| s.total_routes.to_string()).unwrap_or_default();
                    let tc = s.as_ref().map(|s| s.tool_call_count.to_string()).unwrap_or_default();
                    let gen = s.as_ref().map(|s| s.general_count.to_string()).unwrap_or_default();
                    let fb = s.as_ref().map(|s| s.fallback_count.to_string()).unwrap_or_default();
                    let avg_lat = s.as_ref().map(|s| format!("{:.1}ms", s.avg_latency_ms)).unwrap_or_default();
                    let p50 = s.as_ref().map(|s| format!("{:.1}ms", s.p50_latency_ms)).unwrap_or_default();
                    let p95 = s.as_ref().map(|s| format!("{:.1}ms", s.p95_latency_ms)).unwrap_or_default();
                    let avg_sc = s.as_ref().map(|s| format!("{:.4}", s.avg_score)).unwrap_or_default();

                    view! {
                        <StatCard label="Total Routes" value=total color="blue"/>
                        <StatCard label="Tool Call" value=tc color="green"/>
                        <StatCard label="General" value=gen color="green"/>
                        <StatCard label="Fallback" value=fb color="yellow"/>
                        <StatCard label="Avg Latency" value=avg_lat color="blue"/>
                        <StatCard label="P50 Latency" value=p50 color="blue"/>
                        <StatCard label="P95 Latency" value=p95 color="yellow"/>
                        <StatCard label="Avg Score" value=avg_sc color="blue"/>
                    }
                }}
            </div>

            <table>
                <thead>
                    <tr>
                        <th>"Time"</th>
                        <th>"Prompt"</th>
                        <th>"Category"</th>
                        <th>"Score"</th>
                        <th>"Backend"</th>
                        <th>"Latency"</th>
                        <th>"Status"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || routes.get().into_iter().map(|r| {
                        let badge_class = if r.is_fallback {
                            "badge fallback".to_string()
                        } else {
                            format!("badge {}", r.category)
                        };
                        let category_label = if r.is_fallback {
                            format!("{} (fb)", r.category)
                        } else {
                            r.category.clone()
                        };
                        let status_class = if r.status == "ok" {
                            "badge general".to_string()
                        } else {
                            "badge error".to_string()
                        };
                        let latency = r.latency_ms.map(|l| format!("{:.0}ms", l)).unwrap_or_else(|| "-".to_string());
                        let prompt_short = if r.prompt.len() > 50 {
                            format!("{}...", &r.prompt[..47])
                        } else {
                            r.prompt.clone()
                        };
                        let time = r.timestamp.split('T').nth(1).unwrap_or(&r.timestamp).split('.').next().unwrap_or(&r.timestamp).to_string();
                        let prompt_for_title = r.prompt.clone();

                        view! {
                            <tr>
                                <td class="dimmed">{time}</td>
                                <td class="prompt" title=prompt_for_title>{prompt_short}</td>
                                <td><span class=badge_class>{category_label}</span></td>
                                <td class="score">{format!("{:.4}", r.score)}</td>
                                <td>{r.backend}</td>
                                <td>{latency}</td>
                                <td><span class=status_class>{r.status}</span></td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn main() {
    leptos::mount::mount_to_body(Dashboard);
}

const DASHBOARD_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f1117; color: #e1e4e8; }
.container { padding: 24px; max-width: 1400px; margin: 0 auto; }
h1 { font-size: 24px; margin-bottom: 24px; color: #58a6ff; }
.stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 16px; margin-bottom: 32px; }
.stat-card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; }
.stat-card .label { font-size: 12px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; }
.stat-card .value { font-size: 28px; font-weight: 600; margin-top: 4px; }
.value.green { color: #3fb950; }
.value.blue { color: #58a6ff; }
.value.yellow { color: #d29922; }
.value.red { color: #f85149; }
table { width: 100%; border-collapse: collapse; background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; }
th { background: #1c2128; text-align: left; padding: 12px 16px; font-size: 12px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; border-bottom: 1px solid #30363d; }
td { padding: 10px 16px; border-bottom: 1px solid #21262d; font-size: 14px; }
tr:hover { background: #1c2128; }
.badge { display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 12px; font-weight: 500; }
.badge.tool_call { background: #1f3a5f; color: #58a6ff; }
.badge.general { background: #1a3a1a; color: #3fb950; }
.badge.fallback { background: #3d2e00; color: #d29922; }
.badge.error { background: #3d1414; color: #f85149; }
.prompt { max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #8b949e; }
.score { font-family: monospace; }
.dimmed { color: #8b949e; }
"#;
