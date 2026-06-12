import { useState, useEffect } from 'react';

interface RouteEvent {
  id: number;
  timestamp: string;
  prompt: string;
  category: string;
  score: number;
  is_fallback: boolean;
  backend: string;
  model: string;
  latency_ms: number | null;
  status: string;
  error: string | null;
}

export default function Routes() {
  const [routes, setRoutes] = useState<RouteEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [limit, setLimit] = useState(20);

  useEffect(() => {
    fetch(`/api/routes?limit=${limit}`)
      .then(r => r.json())
      .then(setRoutes)
      .catch(e => setError(e.message));
  }, [limit]);

  if (error) return (
    <div style={{ color: '#ef4444', padding: '20px' }}>Failed to load routes: {error}</div>
  );

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px' }}>
        <h1 style={{ fontSize: '24px', fontWeight: 700 }}>Routes</h1>
        <select
          value={limit}
          onChange={e => setLimit(Number(e.target.value))}
          style={{
            background: '#18181b',
            color: '#fafafa',
            border: '1px solid #27272a',
            borderRadius: '8px',
            padding: '8px 12px',
            fontSize: '14px',
          }}
        >
          <option value={10}>Last 10</option>
          <option value={20}>Last 20</option>
          <option value={50}>Last 50</option>
          <option value={100}>Last 100</option>
        </select>
      </div>

      {routes.length === 0 ? (
        <div style={{
          textAlign: 'center',
          padding: '60px 20px',
          color: '#a1a1aa',
          background: '#18181b',
          borderRadius: '12px',
          border: '1px solid #27272a',
        }}>
          No routes recorded yet
        </div>
      ) : (
        <div style={{
          background: '#18181b',
          borderRadius: '12px',
          border: '1px solid #27272a',
          overflow: 'hidden',
        }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '13px' }}>
            <thead>
              <tr style={{ borderBottom: '1px solid #27272a' }}>
                <Th>Time</Th>
                <Th>Prompt</Th>
                <Th>Category</Th>
                <Th>Score</Th>
                <Th>Backend</Th>
                <Th>Latency</Th>
                <Th>Status</Th>
              </tr>
            </thead>
            <tbody>
              {routes.map((r) => (
                <tr key={r.id} style={{ borderBottom: '1px solid #27272a' }}>
                  <Td>
                    {new Date(r.timestamp).toLocaleTimeString()}
                  </Td>
                  <Td style={{ maxWidth: '300px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {r.prompt.length > 60 ? r.prompt.slice(0, 57) + '...' : r.prompt}
                  </Td>
                  <Td>
                    <span style={{
                      color: r.is_fallback ? '#eab308' : r.category === 'tool_call' ? '#22c55e' : '#3b82f6',
                      fontWeight: 500,
                    }}>
                      {r.category}
                      {r.is_fallback && ' (fb)'}
                    </span>
                  </Td>
                  <Td style={{ color: '#a1a1aa' }}>{r.score.toFixed(4)}</Td>
                  <Td>{r.backend}</Td>
                  <Td style={{ color: '#a1a1aa' }}>
                    {r.latency_ms ? `${r.latency_ms.toFixed(0)}ms` : '-'}
                  </Td>
                  <Td>
                    <span style={{
                      color: r.status === 'ok' ? '#22c55e' : '#ef4444',
                      fontWeight: 500,
                    }}>
                      {r.status}
                    </span>
                  </Td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th style={{
      textAlign: 'left',
      padding: '12px 16px',
      color: '#a1a1aa',
      fontWeight: 500,
      fontSize: '12px',
      textTransform: 'uppercase',
      letterSpacing: '0.05em',
    }}>
      {children}
    </th>
  );
}

function Td({ children, style }: { children: React.ReactNode; style?: React.CSSProperties }) {
  return (
    <td style={{ padding: '12px 16px', color: '#fafafa', ...style }}>
      {children}
    </td>
  );
}
