/* Seshat — results grid + cell preview + EXPLAIN visualization + messages tab. */

// ────────────────────────────────────────────────────── Cell renderers
function fmtNumber(n) {
  if (typeof n !== 'number') return String(n);
  if (Math.abs(n) >= 1e6) return n.toLocaleString(undefined, { maximumFractionDigits: 1 });
  return n.toLocaleString();
}
function fmtCurrency(n) {
  if (typeof n !== 'number') return String(n);
  return '$' + n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
function tierChipColor(tier) {
  if (tier === 'enterprise') return { bg: 'rgba(203,166,247,0.18)', fg: 'var(--primary)' };
  if (tier === 'growth')     return { bg: 'rgba(137,180,250,0.18)', fg: 'var(--info)' };
  return                            { bg: 'rgba(166,227,161,0.15)', fg: 'var(--success)' };
}

function Cell({ col, value, onJsonClick, onFkClick }) {
  if (value === null || value === undefined) {
    return <span style={{ color: 'var(--text-disabled)', fontStyle: 'italic', fontFamily: 'var(--font-mono)', fontSize: 12 }}>NULL</span>;
  }
  if (col.kind === 'json' || (typeof value === 'object')) {
    const preview = typeof value === 'object' ? JSON.stringify(value) : String(value);
    return (
      <button onClick={(e) => onJsonClick && onJsonClick(e, value)} style={{
        background: 'rgba(116,199,236,0.08)', border: '1px solid var(--surface0)',
        color: 'var(--info)', borderRadius: 3, padding: '1px 6px',
        fontFamily: 'var(--font-mono)', fontSize: 11, cursor: 'pointer',
        maxWidth: '100%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        display: 'inline-flex', alignItems: 'center', gap: 4,
      }}>
        <Ph name="brackets-curly" size={10} color="var(--info)" />
        {preview.length > 38 ? preview.slice(0, 38) + '…' : preview}
      </button>
    );
  }
  if (col.chip) {
    const tc = tierChipColor(value);
    return (
      <span style={{
        background: tc.bg, color: tc.fg, padding: '1px 8px', borderRadius: 10,
        fontSize: 11, fontWeight: 500, textTransform: 'capitalize',
        fontFamily: 'var(--font-mono)',
      }}>{value}</span>
    );
  }
  if (col.fk) {
    return (
      <button onClick={(e) => onFkClick && onFkClick(e, col.fk, value)} style={{
        background: 'transparent', border: 0, color: 'var(--info)', cursor: 'pointer',
        fontFamily: 'var(--font-mono)', fontSize: 12, padding: 0,
        textDecoration: 'underline', textDecorationStyle: 'dotted',
        textUnderlineOffset: 3, textDecorationColor: 'var(--surface2)',
      }} title={`→ ${col.fk}`}>{value}</button>
    );
  }
  if (col.kind === 'currency') {
    return <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--syn-number)' }}>{fmtCurrency(value)}</span>;
  }
  if (col.type === 'bigint' || col.type === 'integer' || col.type === 'numeric') {
    return <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--syn-number)' }}>{fmtNumber(value)}</span>;
  }
  if (col.type === 'date' || col.type === 'timestamptz') {
    return <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--syn-string)' }}>{value}</span>;
  }
  return <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text)' }}>{value}</span>;
}

// ────────────────────────────────────────────────────── ResultsGrid
function ResultsGrid({ columns, rows, queryMs, onJsonClick, onFkClick, selectedRow, onSelectRow }) {
  const widths = columns.map(c => c.width || 140);
  const rowH = 30;
  return (
    <div style={{ flex: 1, overflow: 'auto', background: 'var(--base)', position: 'relative' }}>
      <div style={{ display: 'inline-block', minWidth: '100%' }}>
        {/* Header */}
        <div style={{
          display: 'flex', position: 'sticky', top: 0, zIndex: 2,
          background: 'var(--mantle)', borderBottom: '1px solid var(--surface1)',
        }}>
          <ColHeaderNum />
          {columns.map((c, i) => (
            <div key={c.name} style={{
              width: widths[i], padding: '0 10px', height: 28,
              borderRight: '1px solid var(--surface0)',
              display: 'flex', alignItems: 'center', gap: 6,
              fontSize: 11, color: 'var(--text)', fontWeight: 600,
              justifyContent: c.align === 'right' ? 'flex-end' : 'flex-start',
            }}>
              {c.isPk && <Ph name="key" size={10} color="var(--warning)" />}
              {c.fk && <Ph name="link" size={10} color="var(--info)" />}
              <span>{c.name}</span>
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 9, color: 'var(--overlay1)', fontWeight: 400 }}>
                {c.type}
              </span>
              <span style={{ flex: 1 }} />
              <Ph name="caret-up-down" size={9} color="var(--overlay1)" />
            </div>
          ))}
          <div style={{ flex: 1, borderBottom: 0, background: 'var(--mantle)' }} />
        </div>
        {/* Rows */}
        {rows.map((row, ri) => {
          const isSel = selectedRow === ri;
          return (
            <div key={ri} onClick={() => onSelectRow(ri)}
              style={{
                display: 'flex', height: rowH, alignItems: 'stretch',
                background: isSel ? 'var(--selection-bg)' : (ri % 2 === 1 ? 'rgba(255,255,255,0.012)' : 'transparent'),
                borderLeft: isSel ? '2px solid var(--selection-stroke)' : '2px solid transparent',
                borderBottom: '1px solid var(--surface0)',
                cursor: 'pointer',
              }}
              onMouseEnter={(e) => { if (!isSel) e.currentTarget.style.background = 'var(--sidebar-hover)'; }}
              onMouseLeave={(e) => { if (!isSel) e.currentTarget.style.background = (ri % 2 === 1 ? 'rgba(255,255,255,0.012)' : 'transparent'); }}>
              <CellNum n={ri + 1} />
              {columns.map((c, i) => (
                <div key={c.name} style={{
                  width: widths[i], padding: '0 10px',
                  borderRight: '1px solid var(--surface0)',
                  display: 'flex', alignItems: 'center',
                  justifyContent: c.align === 'right' ? 'flex-end' : 'flex-start',
                  overflow: 'hidden',
                }}>
                  <Cell col={c} value={row[i]} onJsonClick={onJsonClick} onFkClick={onFkClick} />
                </div>
              ))}
              <div style={{ flex: 1, borderBottom: '1px solid var(--surface0)' }} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
function ColHeaderNum() {
  return (
    <div style={{
      width: 44, height: 28, borderRight: '1px solid var(--surface0)',
      background: 'var(--mantle)', position: 'sticky', left: 0, zIndex: 1,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)',
    }}>#</div>
  );
}
function CellNum({ n }) {
  return (
    <div style={{
      width: 44, position: 'sticky', left: 0, zIndex: 1,
      background: 'var(--base)', borderRight: '1px solid var(--surface0)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      fontSize: 10, color: 'var(--text-disabled)', fontFamily: 'var(--font-mono)',
    }}>{n}</div>
  );
}

// ────────────────────────────────────────────────────── ResultsPanel — tabs above grid
function ResultsPanel({ columns, rows, queryMs, executing, onAskExplain, error }) {
  const [tab, setTab] = React.useState('results');
  const [selRow, setSelRow] = React.useState(0);
  const [jsonOpen, setJsonOpen] = React.useState(null);
  const [toast, setToast] = React.useState(null);

  const flashToast = (text, icon = 'info') => {
    setToast({ text, icon });
    setTimeout(() => setToast(null), 1800);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0, background: 'var(--base)', position: 'relative' }}>
      <ResultsTabBar tab={tab} setTab={setTab} queryMs={queryMs} rowCount={rows.length} executing={executing} error={error} />
      {executing && <ExecutingOverlay />}
      {!executing && tab === 'results' && (
        <ResultsGrid
          columns={columns} rows={rows} queryMs={queryMs}
          selectedRow={selRow} onSelectRow={setSelRow}
          onJsonClick={(e, v) => {
            const r = e.currentTarget.getBoundingClientRect();
            setJsonOpen({ value: v, x: r.left, y: r.bottom + 4 });
          }}
          onFkClick={(e, fk, val) => flashToast(`Would jump to ${fk} = ${val}`, 'arrow-square-out')}
        />
      )}
      {!executing && tab === 'messages' && <MessagesTab queryMs={queryMs} rowCount={rows.length} />}
      {!executing && tab === 'explain' && <ExplainTab />}
      {!executing && tab === 'stats' && <StatsTab rows={rows} columns={columns} />}
      {!executing && tab === 'chart' && <ChartTab rows={rows} columns={columns} />}

      {jsonOpen && (
        <CellPreviewPopover value={jsonOpen.value} onClose={() => setJsonOpen(null)} />
      )}
      {toast && (
        <div style={{
          position: 'absolute', bottom: 16, left: '50%', transform: 'translateX(-50%)',
          background: 'var(--surface1)', color: 'var(--text)', padding: '6px 12px', borderRadius: 4,
          fontSize: 12, display: 'inline-flex', alignItems: 'center', gap: 8,
          boxShadow: 'var(--shadow-menu)', zIndex: 30,
        }}>
          <Ph name={toast.icon} size={11} color="var(--info)" />
          {toast.text}
        </div>
      )}
    </div>
  );
}

function ResultsTabBar({ tab, setTab, queryMs, rowCount, executing, error }) {
  const tabs = [
    { id: 'results',  icon: 'table',           label: 'Results',  badge: executing ? null : rowCount.toLocaleString() },
    { id: 'messages', icon: 'chat-text',       label: 'Messages', badge: null },
    { id: 'explain',  icon: 'tree-structure',  label: 'Explain',  badge: null },
    { id: 'stats',    icon: 'chart-bar',       label: 'Stats',    badge: null },
    { id: 'chart',    icon: 'chart-line',      label: 'Chart',    badge: null },
  ];
  return (
    <div style={{
      height: 30, background: 'var(--mantle)', borderBottom: '1px solid var(--surface0)',
      display: 'flex', alignItems: 'stretch', flexShrink: 0,
    }}>
      {tabs.map(t => {
        const isActive = tab === t.id;
        return (
          <button key={t.id} onClick={() => setTab(t.id)} style={{
            height: '100%', padding: '0 12px',
            background: isActive ? 'var(--base)' : 'transparent',
            border: 0, borderRight: '1px solid var(--surface0)',
            color: isActive ? 'var(--text)' : 'var(--overlay1)',
            fontSize: 11, fontFamily: 'inherit', cursor: 'pointer',
            display: 'inline-flex', alignItems: 'center', gap: 6,
            position: 'relative',
          }}>
            <Ph name={t.icon} size={11} color="currentColor" />
            {t.label}
            {t.badge != null && (
              <span style={{
                background: 'var(--surface1)', color: 'var(--overlay2)',
                fontSize: 9, padding: '1px 5px', borderRadius: 8, fontFamily: 'var(--font-mono)',
              }}>{t.badge}</span>
            )}
            {isActive && <span style={{ position: 'absolute', left: 0, right: 0, top: 0, height: 1, background: 'var(--primary)' }} />}
          </button>
        );
      })}
      <span style={{ flex: 1 }} />
      {queryMs != null && !executing && (
        <div style={{ display: 'inline-flex', alignItems: 'center', gap: 10, padding: '0 12px', fontSize: 11, color: 'var(--overlay1)' }}>
          <span><Ph name="lightning" size={10} color="var(--warning)" /> <span style={{ color: 'var(--success)' }}>{queryMs} ms</span></span>
          <span>·</span>
          <span>{rowCount.toLocaleString()} rows</span>
        </div>
      )}
    </div>
  );
}

function ExecutingOverlay() {
  return (
    <div style={{
      flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center',
      color: 'var(--overlay1)', flexDirection: 'column', gap: 12,
    }}>
      <Ph name="circle-notch" size={28} color="var(--primary)" spin />
      <span style={{ fontSize: 12 }}>Executing query…</span>
      <div style={{
        width: 220, height: 3, background: 'var(--surface0)', borderRadius: 2, overflow: 'hidden',
      }}>
        <div style={{
          height: '100%', background: 'var(--primary)',
          animation: 'seshat-progress 1.6s ease-in-out infinite',
          width: '40%',
        }} />
      </div>
      <style>{`@keyframes seshat-progress { 0%{transform:translateX(-100%)} 100%{transform:translateX(280%)} }`}</style>
    </div>
  );
}

function MessagesTab({ queryMs, rowCount }) {
  const messages = [
    { t: 'NOTICE', color: 'var(--info)',    text: 'planner: chose Hash Join (cost 612.8) over Merge Join (cost 814.2)' },
    { t: 'OK',     color: 'var(--success)', text: `Query OK · ${rowCount} row${rowCount === 1 ? '' : 's'} returned in ${queryMs} ms` },
    { t: 'INFO',   color: 'var(--info)',    text: 'cache: 3 of 4 buffer pages reused' },
    { t: 'OK',     color: 'var(--success)', text: 'connection released to pool · 9/20 in use' },
  ];
  return (
    <div style={{ flex: 1, overflow: 'auto', padding: 12, fontFamily: 'var(--font-mono)', fontSize: 12 }}>
      {messages.map((m, i) => (
        <div key={i} style={{ display: 'flex', gap: 12, alignItems: 'baseline', padding: '4px 0' }}>
          <span style={{ width: 60, color: m.color, fontWeight: 600, fontSize: 10 }}>[{m.t}]</span>
          <span style={{ color: 'var(--text)' }}>{m.text}</span>
        </div>
      ))}
    </div>
  );
}

function ExplainTab() {
  const rows = SESHAT.HERO_EXPLAIN;
  const max = Math.max(...rows.map(r => r.ms));
  return (
    <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
      <div style={{
        display: 'flex', gap: 24, marginBottom: 16, fontSize: 12, color: 'var(--overlay2)',
      }}>
        <Stat label="Total" value="142 ms"   color="var(--success)" />
        <Stat label="Planning" value="2.1 ms" color="var(--overlay2)" />
        <Stat label="Execution" value="139.9 ms" color="var(--overlay2)" />
        <Stat label="Buffers" value="412 hit · 3 read" color="var(--overlay2)" />
        <Stat label="Plan" value="Hash Join" color="var(--info)" />
      </div>
      <div style={{ background: 'var(--mantle)', border: '1px solid var(--surface0)', borderRadius: 6 }}>
        {rows.map((r, i) => (
          <div key={i} style={{
            display: 'flex', alignItems: 'center', gap: 12, padding: '6px 12px',
            borderTop: i ? '1px solid var(--surface0)' : 0,
            fontFamily: 'var(--font-mono)', fontSize: 12,
          }}>
            <span style={{ width: 360, color: 'var(--text)', whiteSpace: 'pre' }}>{r.op}</span>
            <span style={{ width: 96, color: 'var(--syn-number)', textAlign: 'right' }}>{r.rows.toLocaleString()} rows</span>
            <span style={{ width: 100, color: 'var(--overlay1)', fontSize: 11 }}>cost {r.cost}</span>
            <div style={{ flex: 1, height: 14, background: 'var(--surface0)', borderRadius: 2, position: 'relative' }}>
              <div style={{
                position: 'absolute', left: 0, top: 0, bottom: 0,
                width: `${(r.ms / max) * 100}%`,
                background: r.ms > 100 ? 'var(--warning)' : r.ms > 50 ? 'var(--info)' : 'var(--success)',
                borderRadius: 2,
              }} />
            </div>
            <span style={{ width: 70, textAlign: 'right', color: 'var(--syn-number)' }}>{r.ms.toFixed(1)} ms</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Stat({ label, value, color }) {
  return (
    <div>
      <div style={{ fontSize: 10, color: 'var(--overlay1)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>{label}</div>
      <div style={{ fontSize: 14, color, fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{value}</div>
    </div>
  );
}

function StatsTab({ rows, columns }) {
  // Compute simple stats for numeric columns
  const numericCols = columns
    .map((c, i) => ({ c, i }))
    .filter(({ c }) => c.type === 'bigint' || c.type === 'integer' || c.type === 'numeric');
  return (
    <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
      <div style={{
        display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 12,
      }}>
        {numericCols.map(({ c, i }) => {
          const vals = rows.map(r => r[i]).filter(v => typeof v === 'number');
          const min = Math.min(...vals), max = Math.max(...vals);
          const sum = vals.reduce((a, b) => a + b, 0);
          const avg = sum / vals.length;
          return (
            <div key={c.name} style={{
              background: 'var(--surface0)', borderRadius: 8, padding: 14,
            }}>
              <div style={{ fontSize: 11, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)', marginBottom: 6 }}>
                {c.name}
              </div>
              <div style={{ fontSize: 18, color: 'var(--text)', fontFamily: 'var(--font-mono)', fontWeight: 600, marginBottom: 8 }}>
                {c.kind === 'currency' ? fmtCurrency(sum) : fmtNumber(sum)}
                <span style={{ fontSize: 10, color: 'var(--overlay1)', marginLeft: 6 }}>sum</span>
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 4, fontSize: 11, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>
                <span>min  {fmtNumber(min)}</span>
                <span>max  {fmtNumber(max)}</span>
                <span>avg  {fmtNumber(Math.round(avg))}</span>
                <span>n    {vals.length}</span>
              </div>
              {/* Tiny histogram */}
              <Sparkbars values={vals} />
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Sparkbars({ values, bins = 16 }) {
  if (!values.length) return null;
  const min = Math.min(...values), max = Math.max(...values);
  const buckets = Array(bins).fill(0);
  for (const v of values) {
    const idx = Math.min(bins - 1, Math.floor(((v - min) / (max - min || 1)) * bins));
    buckets[idx]++;
  }
  const maxB = Math.max(...buckets);
  return (
    <div style={{ display: 'flex', alignItems: 'flex-end', gap: 2, height: 28, marginTop: 10 }}>
      {buckets.map((b, i) => (
        <div key={i} style={{
          flex: 1, height: `${(b / maxB) * 100}%`, minHeight: 1,
          background: 'var(--secondary)', borderRadius: 1, opacity: 0.5 + (b / maxB) * 0.5,
        }} />
      ))}
    </div>
  );
}

function ChartTab({ rows, columns }) {
  // Plot mrr_usd by organization name (top 20)
  const labelIdx = columns.findIndex(c => c.name === 'organization');
  const valueIdx = columns.findIndex(c => c.name === 'mrr_usd');
  const data = rows.slice(0, 20).map(r => ({ label: r[labelIdx], value: r[valueIdx] }));
  const max = Math.max(...data.map(d => d.value));
  return (
    <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16, fontSize: 11, color: 'var(--overlay1)' }}>
        <ChartConfigPill label="X" value="organization" />
        <ChartConfigPill label="Y" value="mrr_usd (sum)" />
        <ChartConfigPill label="Group" value="—" />
        <ChartConfigPill label="Type" value="bar" />
        <span style={{ flex: 1 }} />
        <button style={{
          background: 'var(--surface0)', border: '1px solid var(--surface1)',
          color: 'var(--overlay2)', padding: '3px 10px', borderRadius: 4,
          fontSize: 11, cursor: 'pointer', fontFamily: 'inherit',
        }}>Save as visualization</button>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {data.map((d, i) => (
          <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10, height: 22 }}>
            <span style={{ width: 180, fontSize: 11, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontFamily: 'var(--font-mono)' }}>{d.label}</span>
            <div style={{ flex: 1, height: 16, background: 'var(--surface0)', borderRadius: 2, position: 'relative' }}>
              <div style={{
                width: `${(d.value / max) * 100}%`, height: '100%',
                background: 'linear-gradient(90deg, var(--primary), var(--secondary))',
                borderRadius: 2,
              }} />
            </div>
            <span style={{ width: 90, fontSize: 11, color: 'var(--syn-number)', fontFamily: 'var(--font-mono)', textAlign: 'right' }}>
              {fmtCurrency(d.value)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ChartConfigPill({ label, value }) {
  return (
    <span style={{
      background: 'var(--surface0)', border: '1px solid var(--surface1)',
      borderRadius: 4, padding: '3px 8px', display: 'inline-flex', alignItems: 'center', gap: 6,
      fontSize: 11, fontFamily: 'var(--font-mono)',
    }}>
      <span style={{ color: 'var(--overlay1)' }}>{label}</span>
      <span style={{ color: 'var(--text)' }}>{value}</span>
    </span>
  );
}

// ────────────────────────────────────────────────────── Cell preview popover (for JSON)
function CellPreviewPopover({ value, onClose }) {
  const pretty = typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value);
  return (
    <>
      <div onClick={onClose} style={{ position: 'absolute', inset: 0, zIndex: 40 }} />
      <div style={{
        position: 'absolute', bottom: 40, right: 40, width: 380, zIndex: 41,
        background: 'var(--mantle)', border: '1px solid var(--surface1)', borderRadius: 8,
        boxShadow: 'var(--shadow-menu)', overflow: 'hidden',
      }}>
        <div style={{
          padding: '8px 12px', borderBottom: '1px solid var(--surface0)',
          display: 'flex', alignItems: 'center', gap: 8, justifyContent: 'space-between',
          fontSize: 11, color: 'var(--overlay1)',
        }}>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <Ph name="brackets-curly" size={11} color="var(--info)" />
            jsonb · {pretty.length} bytes
          </span>
          <span style={{ display: 'inline-flex', gap: 6 }}>
            <button style={{ background: 'transparent', border: 0, color: 'var(--overlay1)', cursor: 'pointer', padding: 0 }} title="Copy"><Ph name="copy" size={11} /></button>
            <button onClick={onClose} style={{ background: 'transparent', border: 0, color: 'var(--overlay1)', cursor: 'pointer', padding: 0 }}><Ph name="x" size={11} /></button>
          </span>
        </div>
        <pre style={{
          margin: 0, padding: 12, maxHeight: 280, overflow: 'auto',
          fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text)',
        }}>{pretty}</pre>
      </div>
    </>
  );
}

Object.assign(window, { ResultsPanel, ResultsGrid, Cell, fmtNumber, fmtCurrency });
