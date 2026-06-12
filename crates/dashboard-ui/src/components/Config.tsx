import { useState, useEffect } from 'react';

interface Provider {
  name: string;
  provider: string;
  model: string;
  base_url: string;
}

export default function Config() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState({ name: '', provider: 'openrouter', base_url: '', model: '' });
  const [saving, setSaving] = useState(false);

  const loadProviders = () => {
    fetch('/api/providers')
      .then(r => r.json())
      .then(setProviders)
      .catch(e => setError(e.message));
  };

  useEffect(() => { loadProviders(); }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const resp = await fetch('/api/providers', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...form, api_key: null }),
      });
      if (resp.ok) {
        setForm({ name: '', provider: 'openrouter', base_url: '', model: '' });
        setShowForm(false);
        loadProviders();
      }
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  };

  if (error) return (
    <div style={{ color: '#ef4444', padding: '20px' }}>Failed to load config: {error}</div>
  );

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px' }}>
        <h1 style={{ fontSize: '24px', fontWeight: 700 }}>Providers</h1>
        <button
          onClick={() => setShowForm(!showForm)}
          style={{
            background: '#3b82f6',
            color: '#fff',
            border: 'none',
            borderRadius: '8px',
            padding: '10px 16px',
            fontSize: '14px',
            fontWeight: 500,
            cursor: 'pointer',
          }}
        >
          {showForm ? 'Cancel' : '+ Add Provider'}
        </button>
      </div>

      {/* Add Provider Form */}
      {showForm && (
        <form onSubmit={handleSubmit} style={{
          background: '#18181b',
          border: '1px solid #27272a',
          borderRadius: '12px',
          padding: '20px',
          marginBottom: '24px',
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: '16px',
        }}>
          <FormField label="Name" value={form.name} onChange={v => setForm(f => ({ ...f, name: v }))} placeholder="e.g. my-provider" />
          <FormField label="Provider" value={form.provider} onChange={v => setForm(f => ({ ...f, provider: v }))} placeholder="e.g. openrouter" />
          <FormField label="Base URL" value={form.base_url} onChange={v => setForm(f => ({ ...f, base_url: v }))} placeholder="https://api.openrouter.ai/api" />
          <FormField label="Model" value={form.model} onChange={v => setForm(f => ({ ...f, model: v }))} placeholder="anthropic/claude-sonnet-4" />
          <div style={{ gridColumn: '1 / -1', display: 'flex', justifyContent: 'flex-end' }}>
            <button
              type="submit"
              disabled={saving || !form.name || !form.base_url || !form.model}
              style={{
                background: '#3b82f6',
                color: '#fff',
                border: 'none',
                borderRadius: '8px',
                padding: '10px 20px',
                fontSize: '14px',
                fontWeight: 500,
                cursor: saving ? 'wait' : 'pointer',
                opacity: saving || !form.name || !form.base_url || !form.model ? 0.5 : 1,
              }}
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </form>
      )}

      {/* Provider List */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
        {providers.map((p, i) => (
          <div key={i} style={{
            background: '#18181b',
            border: '1px solid #27272a',
            borderRadius: '12px',
            padding: '20px',
            display: 'grid',
            gridTemplateColumns: '1fr 1fr 1fr 1fr',
            gap: '16px',
            alignItems: 'center',
          }}>
            <div>
              <div style={{ fontSize: '12px', color: '#a1a1aa', marginBottom: '4px' }}>Name</div>
              <div style={{ fontSize: '14px', fontWeight: 500 }}>{p.name}</div>
            </div>
            <div>
              <div style={{ fontSize: '12px', color: '#a1a1aa', marginBottom: '4px' }}>Provider</div>
              <div style={{ fontSize: '14px', color: '#3b82f6' }}>{p.provider}</div>
            </div>
            <div>
              <div style={{ fontSize: '12px', color: '#a1a1aa', marginBottom: '4px' }}>Model</div>
              <div style={{ fontSize: '14px' }}>{p.model}</div>
            </div>
            <div>
              <div style={{ fontSize: '12px', color: '#a1a1aa', marginBottom: '4px' }}>Base URL</div>
              <div style={{ fontSize: '13px', color: '#a1a1aa', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.base_url}</div>
            </div>
          </div>
        ))}
        {providers.length === 0 && (
          <div style={{
            textAlign: 'center',
            padding: '60px 20px',
            color: '#a1a1aa',
            background: '#18181b',
            borderRadius: '12px',
            border: '1px solid #27272a',
          }}>
            No providers configured
          </div>
        )}
      </div>
    </div>
  );
}

function FormField({ label, value, onChange, placeholder }: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  return (
    <div>
      <label style={{ display: 'block', fontSize: '13px', color: '#a1a1aa', marginBottom: '6px' }}>{label}</label>
      <input
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        style={{
          width: '100%',
          background: '#09090b',
          color: '#fafafa',
          border: '1px solid #27272a',
          borderRadius: '8px',
          padding: '10px 12px',
          fontSize: '14px',
          outline: 'none',
        }}
      />
    </div>
  );
}
