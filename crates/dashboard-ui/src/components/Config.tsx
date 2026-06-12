import { useState, useEffect } from 'react';

interface Provider {
  name: string;
  provider: string;
  model: string;
  base_url: string;
  api_key_masked: string;
}

export default function Config() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [editingIdx, setEditingIdx] = useState<number | null>(null);
  const [form, setForm] = useState({ name: '', provider: 'openrouter', base_url: '', model: '', api_key: '' });
  const [saving, setSaving] = useState(false);

  const loadProviders = () => {
    fetch('/api/providers')
      .then(r => r.json())
      .then(setProviders)
      .catch(e => setError(e.message));
  };

  useEffect(() => { loadProviders(); }, []);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const body: Record<string, string> = { name: form.name, provider: form.provider, base_url: form.base_url, model: form.model };
      if (form.api_key) body.api_key = form.api_key;
      const resp = await fetch('/api/providers', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      if (resp.ok) {
        resetForm();
        loadProviders();
      }
    } catch (e) { setError(String(e)); }
    setSaving(false);
  };

  const handleUpdate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (editingIdx === null) return;
    setSaving(true);
    try {
      const body: Record<string, string | number> = {
        index: editingIdx, name: form.name, provider: form.provider,
        base_url: form.base_url, model: form.model,
      };
      if (form.api_key) body.api_key = form.api_key;
      const resp = await fetch('/api/providers', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      if (resp.ok) { setEditingIdx(null); resetForm(); loadProviders(); }
    } catch (e) { setError(String(e)); }
    setSaving(false);
  };

  const handleDelete = async (idx: number) => {
    if (!confirm(`Delete provider "${providers[idx].name}"?`)) return;
    try {
      const resp = await fetch('/api/providers', {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ index: idx }),
      });
      if (resp.ok) loadProviders();
    } catch (e) { setError(String(e)); }
  };

  const startEdit = (idx: number) => {
    const p = providers[idx];
    setForm({ name: p.name, provider: p.provider, base_url: p.base_url, model: p.model, api_key: '' });
    setEditingIdx(idx);
    setShowForm(false);
  };

  const resetForm = () => {
    setForm({ name: '', provider: 'openrouter', base_url: '', model: '', api_key: '' });
    setShowForm(false);
    setEditingIdx(null);
  };

  if (error) return <div style={{ color: '#ef4444', padding: '20px' }}>Failed to load config: {error}</div>;

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px' }}>
        <h1 style={{ fontSize: '24px', fontWeight: 700 }}>Providers</h1>
        <button onClick={() => { setShowForm(!showForm); setEditingIdx(null); resetForm(); }}
          style={{ background: '#3b82f6', color: '#fff', border: 'none', borderRadius: '8px', padding: '10px 16px', fontSize: '14px', fontWeight: 500, cursor: 'pointer' }}>
          {showForm ? 'Cancel' : '+ Add Provider'}
        </button>
      </div>

      {showForm && (
        <form onSubmit={handleAdd} style={formStyle}>
          <FormField label="Name" value={form.name} onChange={v => setForm(f => ({ ...f, name: v }))} placeholder="e.g. my-provider" />
          <FormField label="Provider" value={form.provider} onChange={v => setForm(f => ({ ...f, provider: v }))} placeholder="e.g. openrouter" />
          <FormField label="Base URL" value={form.base_url} onChange={v => setForm(f => ({ ...f, base_url: v }))} placeholder="https://api.openrouter.ai/api" />
          <FormField label="Model" value={form.model} onChange={v => setForm(f => ({ ...f, model: v }))} placeholder="anthropic/claude-sonnet-4" />
          <FormField label="API Key" value={form.api_key} onChange={v => setForm(f => ({ ...f, api_key: v }))} placeholder="sk-..." type="password" />
          <div style={{ gridColumn: '1 / -1', display: 'flex', justifyContent: 'flex-end' }}>
            <button type="submit" disabled={saving || !form.name || !form.base_url || !form.model} style={submitBtn(saving, form)}>
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </form>
      )}

      {editingIdx !== null && (
        <form onSubmit={handleUpdate} style={formStyle}>
          <FormField label="Name" value={form.name} onChange={v => setForm(f => ({ ...f, name: v }))} placeholder="e.g. my-provider" />
          <FormField label="Provider" value={form.provider} onChange={v => setForm(f => ({ ...f, provider: v }))} placeholder="e.g. openrouter" />
          <FormField label="Base URL" value={form.base_url} onChange={v => setForm(f => ({ ...f, base_url: v }))} placeholder="https://api.openrouter.ai/api" />
          <FormField label="Model" value={form.model} onChange={v => setForm(f => ({ ...f, model: v }))} placeholder="anthropic/claude-sonnet-4" />
          <FormField label="Replace API Key (leave blank to keep current)" value={form.api_key} onChange={v => setForm(f => ({ ...f, api_key: v }))} placeholder="New key or leave blank" type="password" />
          <div style={{ gridColumn: '1 / -1', display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
            <button type="button" onClick={resetForm} style={cancelBtn}>Cancel</button>
            <button type="submit" disabled={saving || !form.name || !form.base_url || !form.model} style={submitBtn(saving, form)}>
              {saving ? 'Updating...' : 'Update'}
            </button>
          </div>
        </form>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
        {providers.map((p, i) => (
          <div key={i} style={cardStyle}>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr auto', gap: '16px', alignItems: 'center', width: '100%' }}>
              <div>
                <div style={labelStyle}>Name</div>
                <div style={{ fontSize: '14px', fontWeight: 500 }}>{p.name}</div>
              </div>
              <div>
                <div style={labelStyle}>Provider</div>
                <div style={{ fontSize: '14px', color: '#3b82f6' }}>{p.provider}</div>
              </div>
              <div>
                <div style={labelStyle}>Model</div>
                <div style={{ fontSize: '14px' }}>{p.model}</div>
              </div>
              <div>
                <div style={labelStyle}>API Key</div>
                <div style={{ fontSize: '13px', color: '#a1a1aa', fontFamily: 'monospace' }}>{p.api_key_masked}</div>
              </div>
              <div style={{ display: 'flex', gap: '6px' }}>
                <button onClick={() => startEdit(i)} style={iconBtn} title="Edit">✏️</button>
                <button onClick={() => handleDelete(i)} style={{ ...iconBtn, color: '#ef4444' }} title="Delete">🗑️</button>
              </div>
            </div>
          </div>
        ))}
        {providers.length === 0 && (
          <div style={emptyState}>No providers configured</div>
        )}
      </div>
    </div>
  );
}

function FormField({ label, value, onChange, placeholder, type = 'text' }: {
  label: string; value: string; onChange: (v: string) => void; placeholder: string; type?: string;
}) {
  return (
    <div>
      <label style={{ display: 'block', fontSize: '13px', color: '#a1a1aa', marginBottom: '6px' }}>{label}</label>
      <input type={type} value={value} onChange={e => onChange(e.target.value)} placeholder={placeholder}
        style={{ width: '100%', background: '#09090b', color: '#fafafa', border: '1px solid #27272a', borderRadius: '8px', padding: '10px 12px', fontSize: '14px', outline: 'none' }} />
    </div>
  );
}

const formStyle: React.CSSProperties = { background: '#18181b', border: '1px solid #27272a', borderRadius: '12px', padding: '20px', marginBottom: '24px', display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' };
const cardStyle: React.CSSProperties = { background: '#18181b', border: '1px solid #27272a', borderRadius: '12px', padding: '20px' };
const labelStyle: React.CSSProperties = { fontSize: '12px', color: '#a1a1aa', marginBottom: '4px' };
const iconBtn: React.CSSProperties = { background: 'transparent', border: '1px solid #27272a', borderRadius: '6px', padding: '6px 8px', cursor: 'pointer', fontSize: '14px' };
const cancelBtn: React.CSSProperties = { background: 'transparent', color: '#a1a1aa', border: '1px solid #27272a', borderRadius: '8px', padding: '10px 20px', fontSize: '14px', fontWeight: 500, cursor: 'pointer' };
const emptyState: React.CSSProperties = { textAlign: 'center', padding: '60px 20px', color: '#a1a1aa', background: '#18181b', borderRadius: '12px', border: '1px solid #27272a' };

function submitBtn(saving: boolean, form: { name: string; base_url: string; model: string }): React.CSSProperties {
  return { background: '#3b82f6', color: '#fff', border: 'none', borderRadius: '8px', padding: '10px 20px', fontSize: '14px', fontWeight: 500, cursor: saving ? 'wait' : 'pointer', opacity: saving || !form.name || !form.base_url || !form.model ? 0.5 : 1 };
}
