import { useState } from 'react';
import Overview from './Overview';
import Routes from './Routes';
import Config from './Config';

type Tab = 'overview' | 'routes' | 'config';

export default function Dashboard() {
  const [activeTab, setActiveTab] = useState<Tab>('overview');

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      {/* Sidebar */}
      <nav style={{
        width: '240px',
        background: '#18181b',
        borderRight: '1px solid #27272a',
        padding: '24px 16px',
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
        flexShrink: 0,
      }}>
        <div style={{
          fontSize: '18px',
          fontWeight: 700,
          color: '#fafafa',
          padding: '0 12px 20px',
          borderBottom: '1px solid #27272a',
          marginBottom: '8px',
        }}>
          ⚡ Switchyard
        </div>
        <NavItem label="Overview" active={activeTab === 'overview'} onClick={() => setActiveTab('overview')} />
        <NavItem label="Routes" active={activeTab === 'routes'} onClick={() => setActiveTab('routes')} />
        <NavItem label="Config" active={activeTab === 'config'} onClick={() => setActiveTab('config')} />
      </nav>

      {/* Main Content */}
      <main style={{
        flex: 1,
        padding: '32px',
        overflowY: 'auto',
      }}>
        {activeTab === 'overview' && <Overview />}
        {activeTab === 'routes' && <Routes />}
        {activeTab === 'config' && <Config />}
      </main>
    </div>
  );
}

function NavItem({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'block',
        width: '100%',
        textAlign: 'left',
        padding: '10px 12px',
        borderRadius: '8px',
        border: 'none',
        cursor: 'pointer',
        fontSize: '14px',
        fontWeight: 500,
        color: active ? '#fafafa' : '#a1a1aa',
        background: active ? '#27272a' : 'transparent',
        transition: 'background 0.15s, color 0.15s',
      }}
      onMouseEnter={(e) => {
        if (!active) {
          e.currentTarget.style.background = '#27272a';
          e.currentTarget.style.color = '#fafafa';
        }
      }}
      onMouseLeave={(e) => {
        if (!active) {
          e.currentTarget.style.background = 'transparent';
          e.currentTarget.style.color = '#a1a1aa';
        }
      }}
    >
      {label}
    </button>
  );
}
