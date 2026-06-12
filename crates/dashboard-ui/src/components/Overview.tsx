import { useState, useEffect } from 'react';

interface OverviewData {
  stats: {
    total_routes: number;
    tool_call_count: number;
    general_count: number;
    fallback_count: number;
    avg_latency_ms: number;
    p50_latency_ms: number;
    p95_latency_ms: number;
    avg_score: number;
    accuracy_pct: number;
  };
  backends: number;
  capabilities: number;
  embedding_model: string;
  threshold: number;
  fallback: string;
}

export default function Overview() {
  const [data, setData] = useState<OverviewData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch('/api/overview')
      .then(r => r.json())
      .then(setData)
      .catch(e => setError(e.message));
  }, []);

  if (error) return <ErrorState message={error} />;
  if (!data) return <LoadingState />;

  return (
    <div>
      <h1 style={{ fontSize: '24px', fontWeight: 700, marginBottom: '24px' }}>Overview</h1>

      {/* Stats Grid */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
        gap: '16px',
        marginBottom: '32px',
      }}>
        <StatCard label="Total Routes" value={data.stats.total_routes} />
        <StatCard label="Tool Calls" value={data.stats.tool_call_count} color="var(--green)" />
        <StatCard label="General" value={data.stats.general_count} color="var(--accent)" />
        <StatCard label="Fallbacks" value={data.stats.fallback_count} color="var(--yellow)" />
        <StatCard label="Avg Latency" value={`${data.stats.avg_latency_ms.toFixed(1)}ms`} />
        <StatCard label="P95 Latency" value={`${data.stats.p95_latency_ms.toFixed(1)}ms`} />
        <StatCard label="Accuracy" value={`${data.stats.accuracy_pct.toFixed(1)}%`} color="var(--green)" />
        <StatCard label="Avg Score" value={data.stats.avg_score.toFixed(4)} />
      </div>

      {/* Config Info */}
      <h2 style={{ fontSize: '16px', fontWeight: 600, marginBottom: '16px', color: '#a1a1aa' }}>Router Config</h2>
      <div style={{
        background: '#18181b',
        border: '1px solid #27272a',
        borderRadius: '12px',
        padding: '20px',
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
        gap: '16px',
      }}>
        <InfoItem label="Backends" value={String(data.backends)} />
        <InfoItem label="Capabilities" value={String(data.capabilities)} />
        <InfoItem label="Embedding Model" value={data.embedding_model} />
        <InfoItem label="Threshold" value={String(data.threshold)} />
        <InfoItem label="Fallback" value={data.fallback} />
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: string | number; color?: string }) {
  return (
    <div style={{
      background: '#18181b',
      border: '1px solid #27272a',
      borderRadius: '12px',
      padding: '20px',
    }}>
      <div style={{ fontSize: '13px', color: '#a1a1aa', marginBottom: '8px' }}>{label}</div>
      <div style={{ fontSize: '28px', fontWeight: 700, color: color || '#fafafa' }}>{value}</div>
    </div>
  );
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div style={{ fontSize: '12px', color: '#a1a1aa', marginBottom: '4px' }}>{label}</div>
      <div style={{ fontSize: '14px', color: '#fafafa' }}>{value}</div>
    </div>
  );
}

function LoadingState() {
  return (
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '200px', color: '#a1a1aa' }}>
      Loading...
    </div>
  );
}

function ErrorState({ message }: { message: string }) {
  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      justifyContent: 'center',
      alignItems: 'center',
      height: '200px',
      color: '#ef4444',
    }}>
      <div style={{ fontSize: '14px', marginBottom: '8px' }}>Failed to load overview</div>
      <div style={{ fontSize: '12px', color: '#a1a1aa' }}>{message}</div>
    </div>
  );
}
