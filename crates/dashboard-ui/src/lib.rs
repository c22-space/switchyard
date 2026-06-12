use leptos::prelude::*;
use leptos_meta::*;
use serde::Deserialize;
use wasm_bindgen::JsCast;

const API_BASE: &str = "";

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

#[derive(Deserialize, Clone, Debug)]
pub struct Overview {
    pub stats: RouteStats,
    pub backends: u64,
    pub capabilities: u64,
    pub embedding_model: String,
    pub threshold: f32,
    pub fallback: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Provider {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub base_url: String,
}

impl serde::Serialize for Provider {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Provider", 4)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("provider", &self.provider)?;
        state.serialize_field("model", &self.model)?;
        state.serialize_field("base_url", &self.base_url)?;
        state.end()
    }
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

async fn post_json<T: serde::de::DeserializeOwned>(url: &str, body: &str) -> Option<T> {
    let window = web_sys::window()?;
    let mut opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&wasm_bindgen::JsValue::from_str(body));
    let headers = web_sys::Headers::new().ok()?;
    headers.set("Content-Type", "application/json").ok()?;
    opts.set_headers(&headers);
    let req = web_sys::Request::new_with_str_and_init(url, &opts).ok()?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
        .await
        .ok()?;
    let resp: web_sys::Response = resp_value.dyn_into().ok()?;
    let json_value = wasm_bindgen_futures::JsFuture::from(resp.text().ok()?)
        .await
        .ok()?;
    let text: String = json_value.dyn_into::<js_sys::JsString>().ok()?.into();
    serde_json::from_str(&text).ok()
}

async fn fetch_overview() -> Option<Overview> {
    fetch_json(&format!("{}/api/overview", API_BASE)).await
}

async fn fetch_stats() -> Option<RouteStats> {
    fetch_json(&format!("{}/api/stats", API_BASE)).await
}

async fn fetch_routes() -> Vec<RouteEvent> {
    fetch_json::<Vec<RouteEvent>>(&format!("{}/api/routes?limit=50", API_BASE))
        .await
        .unwrap_or_default()
}

async fn fetch_providers() -> Vec<Provider> {
    fetch_json::<Vec<Provider>>(&format!("{}/api/providers", API_BASE))
        .await
        .unwrap_or_default()
}

async fn add_provider_api(provider: &Provider) -> Option<serde_json::Value> {
    let body = serde_json::to_string(provider).ok()?;
    post_json(&format!("{}/api/providers", API_BASE), &body).await
}

// ── Pages ──────────────────────────────────────────────────────────────

#[component]
fn OverviewPage() -> impl IntoView {
    let (overview, set_overview) = signal(None::<Overview>);

    leptos::task::spawn_local(async move {
        if let Some(o) = fetch_overview().await {
            set_overview.set(Some(o));
        }
    });

    leptos::task::spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(10000).await;
            if let Some(o) = fetch_overview().await {
                set_overview.set(Some(o));
            }
        }
    });

    view! {
        <div class="page-overview">
            <div class="metric-grid">
                {move || {
                    let o = overview.get();
                    let s = o.as_ref().map(|o| &o.stats);
                    view! {
                        <MetricCard label="Total Routes" value=s.map(|s| s.total_routes.to_string()).unwrap_or_default() accent="blue"/>
                        <MetricCard label="Tool Calls" value=s.map(|s| s.tool_call_count.to_string()).unwrap_or_default() accent="green"/>
                        <MetricCard label="General" value=s.map(|s| s.general_count.to_string()).unwrap_or_default() accent="green"/>
                        <MetricCard label="Fallback" value=s.map(|s| s.fallback_count.to_string()).unwrap_or_default() accent="amber"/>
                        <MetricCard label="Avg Latency" value=s.map(|s| format!("{:.1}ms", s.avg_latency_ms)).unwrap_or_default() accent="blue"/>
                        <MetricCard label="P50 Latency" value=s.map(|s| format!("{:.1}ms", s.p50_latency_ms)).unwrap_or_default() accent="blue"/>
                        <MetricCard label="P95 Latency" value=s.map(|s| format!("{:.1}ms", s.p95_latency_ms)).unwrap_or_default() accent="amber"/>
                        <MetricCard label="Accuracy" value=s.map(|s| format!("{:.1}%", s.accuracy_pct)).unwrap_or_default() accent="green"/>
                    }
                }}
            </div>

            <div class="info-row">
                {move || {
                    let o = overview.get();
                    view! {
                        <InfoTile label="Backends" value=o.as_ref().map(|o| o.backends.to_string()).unwrap_or_default()/>
                        <InfoTile label="Capabilities" value=o.as_ref().map(|o| o.capabilities.to_string()).unwrap_or_default()/>
                        <InfoTile label="Embedding Model" value=o.as_ref().map(|o| o.embedding_model.clone()).unwrap_or_default()/>
                        <InfoTile label="Threshold" value=o.as_ref().map(|o| format!("{:.2}", o.threshold)).unwrap_or_default()/>
                        <InfoTile label="Fallback" value=o.as_ref().map(|o| o.fallback.clone()).unwrap_or_default()/>
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn MetricCard(label: &'static str, value: String, accent: &'static str) -> impl IntoView {
    view! {
        <div class="metric-card">
            <span class="metric-label">{label}</span>
            <span class=format!("metric-value accent-{}", accent)>{value}</span>
        </div>
    }
}

#[component]
fn InfoTile(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="info-tile">
            <span class="info-label">{label}</span>
            <span class="info-value">{value}</span>
        </div>
    }
}

#[component]
fn RoutesPage() -> impl IntoView {
    let (routes, set_routes) = signal(Vec::<RouteEvent>::new());

    leptos::task::spawn_local(async move {
        set_routes.set(fetch_routes().await);
    });

    leptos::task::spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            set_routes.set(fetch_routes().await);
        }
    });

    view! {
        <div class="page-routes">
            <table class="data-table">
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
                        let time = r.timestamp.split('T').nth(1).unwrap_or(&r.timestamp).split('.').next().unwrap_or(&r.timestamp).to_string();
                        let prompt_short = if r.prompt.len() > 50 { format!("{}...", &r.prompt[..47]) } else { r.prompt.clone() };
                        let latency = r.latency_ms.map(|l| format!("{:.0}ms", l)).unwrap_or_else(|| "-".to_string());
                        let cat_class = if r.is_fallback { "tag tag-amber".to_string() } else { format!("tag tag-{}", r.category) };
                        let cat_label = if r.is_fallback { format!("{} (fb)", r.category) } else { r.category.clone() };
                        let status_class = if r.status == "ok" { "tag tag-green" } else { "tag tag-red" };

                        view! {
                            <tr>
                                <td class="cell-dim">{time}</td>
                                <td class="cell-prompt" title=r.prompt.clone()>{prompt_short}</td>
                                <td><span class=cat_class>{cat_label}</span></td>
                                <td class="cell-mono">{format!("{:.4}", r.score)}</td>
                                <td>{r.backend}</td>
                                <td class="cell-mono">{latency}</td>
                                <td><span class=status_class>{r.status}</span></td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn ConfigPage() -> impl IntoView {
    let (providers, set_providers) = signal(Vec::<Provider>::new());
    let (show_form, set_show_form) = signal(false);
    let (form_name, set_form_name) = signal(String::new());
    let (form_provider, set_form_provider) = signal(String::new());
    let (form_model, set_form_model) = signal(String::new());
    let (form_url, set_form_url) = signal(String::new());

    let load = move || {
        leptos::task::spawn_local(async move {
            set_providers.set(fetch_providers().await);
        });
    };

    load();

    let submit = move |_: web_sys::MouseEvent| {
        let n = form_name.get();
        let p = form_provider.get();
        let m = form_model.get();
        let u = form_url.get();
        if n.is_empty() || p.is_empty() || m.is_empty() || u.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            let provider = Provider { name: n, provider: p, model: m, base_url: u };
            add_provider_api(&provider).await;
            set_providers.set(fetch_providers().await);
        });
        set_form_name.set(String::new());
        set_form_provider.set(String::new());
        set_form_model.set(String::new());
        set_form_url.set(String::new());
        set_show_form.set(false);
    };

    view! {
        <div class="page-config">
            <div class="page-header">
                <h2>"Providers"</h2>
                <button class="btn btn-primary" on:click=move |_| set_show_form.set(true)>"Add Provider"</button>
            </div>

            <div class="form-panel" style=move || if show_form.get() { "display: block" } else { "display: none" }>
                <div class="form-grid">
                    <div class="form-field">
                        <label>"Name"</label>
                        <input type="text" placeholder="e.g. tool_call" prop:value=form_name on:input=move |e| set_form_name.set(event_target_value(&e))/>
                    </div>
                    <div class="form-field">
                        <label>"Provider"</label>
                        <input type="text" placeholder="e.g. openai" prop:value=form_provider on:input=move |e| set_form_provider.set(event_target_value(&e))/>
                    </div>
                    <div class="form-field">
                        <label>"Model"</label>
                        <input type="text" placeholder="e.g. gpt-4" prop:value=form_model on:input=move |e| set_form_model.set(event_target_value(&e))/>
                    </div>
                    <div class="form-field">
                        <label>"Base URL"</label>
                        <input type="text" placeholder="https://api.openai.com" prop:value=form_url on:input=move |e| set_form_url.set(event_target_value(&e))/>
                    </div>
                </div>
                <div class="form-actions">
                    <button class="btn btn-primary" on:click=submit>"Save"</button>
                    <button class="btn btn-ghost" on:click=move |_| set_show_form.set(false)>"Cancel"</button>
                </div>
            </div>

            <table class="data-table">
                <thead>
                    <tr>
                        <th>"Name"</th>
                        <th>"Provider"</th>
                        <th>"Model"</th>
                        <th>"Base URL"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || providers.get().into_iter().map(|p| {
                        view! {
                            <tr>
                                <td class="cell-strong">{p.name}</td>
                                <td><span class="tag tag-blue">{p.provider}</span></td>
                                <td class="cell-mono">{p.model}</td>
                                <td class="cell-dim">{p.base_url}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

// ── Shell ──────────────────────────────────────────────────────────────

#[component]
fn Dashboard() -> impl IntoView {
    let (tab, set_tab) = signal("overview".to_string());

    view! {
        <Title text="Switchyard Dashboard"/>
        <Style>{DASHBOARD_CSS}</Style>

        <div class="shell">
            <nav class="sidebar">
                <div class="sidebar-brand">"Switchyard"</div>
                <button class=move || format!("nav-item{}", if tab.get() == "overview" { " active" } else { "" })
                    on:click=move |_| set_tab.set("overview".to_string())>
                    "Overview"
                </button>
                <button class=move || format!("nav-item{}", if tab.get() == "routes" { " active" } else { "" })
                    on:click=move |_| set_tab.set("routes".to_string())>
                    "Routes"
                </button>
                <button class=move || format!("nav-item{}", if tab.get() == "config" { " active" } else { "" })
                    on:click=move |_| set_tab.set("config".to_string())>
                    "Config"
                </button>
            </nav>
            <main class="content">
                <div style=move || if tab.get() == "overview" { "display: block" } else { "display: none" }><OverviewPage/></div>
                <div style=move || if tab.get() == "routes" { "display: block" } else { "display: none" }><RoutesPage/></div>
                <div style=move || if tab.get() == "config" { "display: block" } else { "display: none" }><ConfigPage/></div>
            </main>
        </div>
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn main() {
    leptos::mount::mount_to_body(Dashboard);
}

// ── Styles ─────────────────────────────────────────────────────────────

const DASHBOARD_CSS: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
:root {
    --bg: #0c0d0f;
    --surface: #151619;
    --surface-2: #1c1d21;
    --border: #2a2b30;
    --border-hover: #3a3b40;
    --text: #e1e4e8;
    --text-dim: #8b949e;
    --text-muted: #555b63;
    --blue: #58a6ff;
    --green: #3fb950;
    --amber: #d29922;
    --red: #f85149;
    --radius: 8px;
}

body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: var(--bg); color: var(--text); }

/* Layout */
.shell { display: flex; height: 100vh; }
.sidebar { width: 200px; background: var(--surface); border-right: 1px solid var(--border); padding: 20px 0; flex-shrink: 0; display: flex; flex-direction: column; gap: 2px; }
.sidebar-brand { padding: 0 20px 20px; font-size: 15px; font-weight: 600; color: var(--blue); letter-spacing: 0.3px; border-bottom: 1px solid var(--border); margin-bottom: 12px; }
.nav-item { display: block; width: 100%; padding: 10px 20px; background: none; border: none; color: var(--text-dim); font-size: 14px; text-align: left; cursor: pointer; transition: all 0.15s; }
.nav-item:hover { color: var(--text); background: var(--surface-2); }
.nav-item.active { color: var(--text); background: var(--surface-2); border-right: 2px solid var(--blue); }
.content { flex: 1; overflow-y: auto; padding: 32px; }

/* Metric cards */
.metric-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; margin-bottom: 24px; }
.metric-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 20px; display: flex; flex-direction: column; gap: 8px; }
.metric-label { font-size: 12px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; }
.metric-value { font-size: 28px; font-weight: 600; font-variant-numeric: tabular-nums; }
.accent-blue { color: var(--blue); }
.accent-green { color: var(--green); }
.accent-amber { color: var(--amber); }
.accent-red { color: var(--red); }

/* Info row */
.info-row { display: grid; grid-template-columns: repeat(5, 1fr); gap: 12px; }
.info-tile { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px; }
.info-label { display: block; font-size: 11px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 6px; }
.info-value { font-size: 15px; font-weight: 500; }

/* Tables */
.data-table { width: 100%; border-collapse: collapse; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }
.data-table th { background: var(--surface-2); text-align: left; padding: 12px 16px; font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; border-bottom: 1px solid var(--border); font-weight: 500; }
.data-table td { padding: 10px 16px; border-bottom: 1px solid var(--border); font-size: 13px; }
.data-table tr:last-child td { border-bottom: none; }
.data-table tr:hover td { background: var(--surface-2); }
.cell-dim { color: var(--text-dim); }
.cell-strong { font-weight: 500; }
.cell-mono { font-family: 'SF Mono', Menlo, monospace; font-size: 12px; }
.cell-prompt { max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-dim); }

/* Tags */
.tag { display: inline-block; padding: 2px 8px; border-radius: 10px; font-size: 11px; font-weight: 500; }
.tag-blue { background: rgba(88,166,255,0.15); color: var(--blue); }
.tag-green { background: rgba(63,185,80,0.15); color: var(--green); }
.tag-amber { background: rgba(210,153,34,0.15); color: var(--amber); }
.tag-red { background: rgba(248,81,73,0.15); color: var(--red); }
.tag-tool_call { background: rgba(88,166,255,0.15); color: var(--blue); }
.tag-general { background: rgba(63,185,80,0.15); color: var(--green); }

/* Buttons */
.btn { padding: 8px 16px; border-radius: 6px; font-size: 13px; font-weight: 500; cursor: pointer; border: 1px solid transparent; transition: all 0.15s; }
.btn-primary { background: #238636; color: #fff; }
.btn-primary:hover { background: #2ea043; }
.btn-ghost { background: transparent; color: var(--text-dim); border-color: var(--border); }
.btn-ghost:hover { background: var(--surface-2); color: var(--text); }

/* Page header */
.page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 20px; }
.page-header h2 { font-size: 16px; font-weight: 600; }

/* Config form */
.form-panel { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); padding: 20px; margin-bottom: 20px; }
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 16px; }
.form-field label { display: block; font-size: 12px; color: var(--text-dim); margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.3px; }
.form-field input { width: 100%; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 8px 12px; color: var(--text); font-size: 13px; }
.form-field input::placeholder { color: var(--text-muted); }
.form-field input:focus { outline: none; border-color: var(--blue); }
.form-actions { display: flex; gap: 8px; }
"#;
