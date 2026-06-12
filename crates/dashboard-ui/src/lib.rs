use leptos::prelude::*;
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

// ── Dashboard Shell ────────────────────────────────────────────────────

#[component]
fn Dashboard() -> impl IntoView {
    let (tab, set_tab) = signal(String::from("overview"));

    view! {
        <div class="app">
            <aside class="sidebar">
                <div class="logo">"Switchyard"</div>
                <div class="nav">
                    <button
                        class=move || if tab.get() == "overview" { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_tab.set(String::from("overview"))
                    >"Overview"</button>
                    <button
                        class=move || if tab.get() == "routes" { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_tab.set(String::from("routes"))
                    >"Routes"</button>
                    <button
                        class=move || if tab.get() == "config" { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_tab.set(String::from("config"))
                    >"Config"</button>
                </div>
            </aside>
            <main class="main">
                <div class:hidden=move || tab.get() != "overview"><OverviewTab/></div>
                <div class:hidden=move || tab.get() != "routes"><RoutesTab/></div>
                <div class:hidden=move || tab.get() != "config"><ConfigTab/></div>
            </main>
        </div>
    }
}

// ── Overview ───────────────────────────────────────────────────────────

#[component]
fn OverviewTab() -> impl IntoView {
    let (data, set_data) = signal(None::<Overview>);

    leptos::task::spawn_local(async move {
        set_data.set(fetch_overview().await);
    });
    leptos::task::spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(10000).await;
            set_data.set(fetch_overview().await);
        }
    });

    view! {
        <div class="page">
            <h1 class="page-title">"Overview"</h1>
            <div class="cards">
                {move || {
                    let d = data.get();
                    let s = d.as_ref().map(|o| &o.stats);
                    let rows = vec![
                        ("Total Routes", s.map(|s| s.total_routes.to_string()).unwrap_or_default(), "blue"),
                        ("Tool Calls", s.map(|s| s.tool_call_count.to_string()).unwrap_or_default(), "green"),
                        ("General", s.map(|s| s.general_count.to_string()).unwrap_or_default(), "green"),
                        ("Fallback", s.map(|s| s.fallback_count.to_string()).unwrap_or_default(), "amber"),
                        ("Avg Latency", s.map(|s| format!("{:.1}ms", s.avg_latency_ms)).unwrap_or_default(), "blue"),
                        ("P50 Latency", s.map(|s| format!("{:.1}ms", s.p50_latency_ms)).unwrap_or_default(), "blue"),
                        ("P95 Latency", s.map(|s| format!("{:.1}ms", s.p95_latency_ms)).unwrap_or_default(), "amber"),
                        ("Accuracy", s.map(|s| format!("{:.1}%", s.accuracy_pct)).unwrap_or_default(), "green"),
                    ];
                    rows.into_iter().map(|(label, value, color)| {
                        view! {
                            <div class="card">
                                <div class="card-label">{label}</div>
                                <div class=format!("card-value c-{}", color)>{value}</div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>

            <div class="info-cards">
                {move || {
                    let d = data.get();
                    let items = vec![
                        ("Backends", d.as_ref().map(|o| o.backends.to_string()).unwrap_or_default()),
                        ("Capabilities", d.as_ref().map(|o| o.capabilities.to_string()).unwrap_or_default()),
                        ("Embedding Model", d.as_ref().map(|o| o.embedding_model.clone()).unwrap_or_default()),
                        ("Threshold", d.as_ref().map(|o| format!("{:.2}", o.threshold)).unwrap_or_default()),
                        ("Fallback", d.as_ref().map(|o| o.fallback.clone()).unwrap_or_default()),
                    ];
                    items.into_iter().map(|(k, v)| {
                        view! {
                            <div class="info-card">
                                <div class="info-key">{k}</div>
                                <div class="info-val">{v}</div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}

// ── Routes ─────────────────────────────────────────────────────────────

#[component]
fn RoutesTab() -> impl IntoView {
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
        <div class="page">
            <h1 class="page-title">"Routes"</h1>
            <div class="table-wrap">
                <table class="tbl">
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
                            let t = r.timestamp.split('T').nth(1).unwrap_or(&r.timestamp).split('.').next().unwrap_or(&r.timestamp).to_string();
                            let p = if r.prompt.len() > 40 { format!("{}...", &r.prompt[..37]) } else { r.prompt.clone() };
                            let lat = r.latency_ms.map(|l| format!("{:.0}ms", l)).unwrap_or_else(|| "-".to_string());
                            let cat_cls = if r.is_fallback { "tag tag-warn" } else { "tag tag-info" };
                            let cat_lbl = if r.is_fallback { format!("{} (fb)", r.category) } else { r.category };
                            let st_cls = if r.status == "ok" { "tag tag-ok" } else { "tag tag-err" };
                            view! {
                                <tr>
                                    <td class="dim">{t}</td>
                                    <td class="prompt-cell" title=r.prompt>{p}</td>
                                    <td><span class=cat_cls>{cat_lbl}</span></td>
                                    <td class="mono">{format!("{:.4}", r.score)}</td>
                                    <td>{r.backend}</td>
                                    <td class="mono">{lat}</td>
                                    <td><span class=st_cls>{r.status}</span></td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

// ── Config ─────────────────────────────────────────────────────────────

#[component]
fn ConfigTab() -> impl IntoView {
    let (providers, set_providers) = signal(Vec::<Provider>::new());
    let (show_form, set_show_form) = signal(false);
    let (f_name, set_f_name) = signal(String::new());
    let (f_prov, set_f_prov) = signal(String::new());
    let (f_model, set_f_model) = signal(String::new());
    let (f_url, set_f_url) = signal(String::new());

    leptos::task::spawn_local(async move {
        set_providers.set(fetch_providers().await);
    });

    let reload = move || {
        leptos::task::spawn_local(async move {
            set_providers.set(fetch_providers().await);
        });
    };

    let submit = move |_: web_sys::MouseEvent| {
        let n = f_name.get();
        let p = f_prov.get();
        let m = f_model.get();
        let u = f_url.get();
        if n.is_empty() || p.is_empty() || m.is_empty() || u.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            let prov = Provider { name: n, provider: p, model: m, base_url: u };
            add_provider_api(&prov).await;
            set_providers.set(fetch_providers().await);
        });
        set_f_name.set(String::new());
        set_f_prov.set(String::new());
        set_f_model.set(String::new());
        set_f_url.set(String::new());
        set_show_form.set(false);
    };

    view! {
        <div class="page">
            <div class="page-head">
                <h1 class="page-title">"Providers"</h1>
                <button class="btn btn-add" on:click=move |_| set_show_form.set(true)>"Add Provider"</button>
            </div>

            <div class="form-card" class:visible=show_form>
                <div class="form-row">
                    <div class="form-field">
                        <label>"Name"</label>
                        <input type="text" placeholder="tool_call" prop:value=f_name on:input=move |e| set_f_name.set(event_target_value(&e))/>
                    </div>
                    <div class="form-field">
                        <label>"Provider"</label>
                        <input type="text" placeholder="openai" prop:value=f_prov on:input=move |e| set_f_prov.set(event_target_value(&e))/>
                    </div>
                    <div class="form-field">
                        <label>"Model"</label>
                        <input type="text" placeholder="gpt-4" prop:value=f_model on:input=move |e| set_f_model.set(event_target_value(&e))/>
                    </div>
                    <div class="form-field">
                        <label>"Base URL"</label>
                        <input type="text" placeholder="https://api.openai.com" prop:value=f_url on:input=move |e| set_f_url.set(event_target_value(&e))/>
                    </div>
                </div>
                <div class="form-btns">
                    <button class="btn btn-save" on:click=submit>"Save"</button>
                    <button class="btn btn-cancel" on:click=move |_| set_show_form.set(false)>"Cancel"</button>
                </div>
            </div>

            <div class="table-wrap">
                <table class="tbl">
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
                                    <td class="strong">{p.name}</td>
                                    <td><span class="tag tag-info">{p.provider}</span></td>
                                    <td class="mono">{p.model}</td>
                                    <td class="dim">{p.base_url}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn main() {
    leptos::mount::mount_to_body(Dashboard);
}

// ── CSS ────────────────────────────────────────────────────────────────


