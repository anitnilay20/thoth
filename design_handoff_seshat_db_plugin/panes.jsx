/* Seshat — side panes:
   ConnectionsPane, SchemaPane, SavedPane, HistoryPane, ImportPane. */

const { useState: useStateP } = React; // alias to avoid hoisting issues

// ────────────────────────────────────────────── shared row primitives
function PaneHeader({ title, action, kbd }) {
  return (
    <div style={{
      padding: '10px 14px 6px', borderBottom: '1px solid var(--surface0)',
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    }}>
      <span className="t-section">{title}</span>
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
        {kbd && <span className="seshat-kbd">{kbd}</span>}
        {action}
      </span>
    </div>
  );
}

function SearchField({ value, onChange, placeholder = 'Filter…' }) {
  return (
    <div style={{
      margin: '8px 10px', padding: '4px 8px',
      background: 'var(--crust)', border: '1px solid var(--surface0)',
      borderRadius: 4, display: 'flex', alignItems: 'center', gap: 6, height: 24,
    }}>
      <Ph name="magnifying-glass" size={11} color="var(--overlay1)" />
      <input value={value} onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder} className="nofocus"
        style={{ flex: 1, background: 'transparent', border: 0, color: 'var(--text)',
                 fontSize: 12, fontFamily: 'inherit', minWidth: 0 }} />
    </div>
  );
}

function PaneRow({ children, selected, onClick, indent = 0, height, onMouseEnter, onMouseLeave }) {
  const [h, setH] = React.useState(false);
  return (
    <div onClick={onClick}
      onMouseEnter={(e) => { setH(true); onMouseEnter && onMouseEnter(e); }}
      onMouseLeave={(e) => { setH(false); onMouseLeave && onMouseLeave(e); }}
      style={{
        height: height || 24, paddingLeft: 10 + indent * 14, paddingRight: 10,
        display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer',
        background: selected ? 'var(--selection-bg)' : h ? 'var(--sidebar-hover)' : 'transparent',
        borderLeft: selected ? '2px solid var(--selection-stroke)' : '2px solid transparent',
        fontSize: 13, color: 'var(--text)',
      }}>
      {children}
    </div>
  );
}

// ──────────────────────────────────────────────────────────── ConnectionsPane
function ConnectionsPane({ active, onPick, onNew }) {
  const [filter, setFilter] = React.useState('');
  const items = SESHAT.CONNECTIONS.filter(c =>
    !filter || c.name.toLowerCase().includes(filter.toLowerCase()) || c.host.toLowerCase().includes(filter.toLowerCase()));
  const byEnv = items.reduce((acc, c) => ({ ...acc, [c.env]: [...(acc[c.env] || []), c] }), {});
  const envOrder = ['prod', 'stage', 'dev'];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <PaneHeader title="Connections" action={
        <button onClick={onNew} title="New connection (⌘N)" style={{
          background: 'transparent', border: 0, color: 'var(--overlay1)', cursor: 'pointer', padding: 2,
        }}><Ph name="plus" size={13} /></button>
      } />
      <SearchField value={filter} onChange={setFilter} placeholder="Filter connections…" />
      <div style={{ overflowY: 'auto', flex: 1, paddingBottom: 8 }}>
        {envOrder.map(env => byEnv[env] && (
          <div key={env}>
            <div style={{ padding: '8px 14px 4px', display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{
                fontFamily: 'var(--font-mono)', fontSize: 10, fontWeight: 700,
                color: env === 'prod' ? 'var(--red)' : env === 'stage' ? 'var(--warning)' : 'var(--success)',
                textTransform: 'uppercase', letterSpacing: '0.1em',
              }}>{env}</span>
              <span style={{ height: 1, background: 'var(--surface0)', flex: 1 }} />
              <span style={{ fontSize: 11, color: 'var(--overlay1)' }}>{byEnv[env].length}</span>
            </div>
            {byEnv[env].map(c => {
              const isActive = active && active.id === c.id;
              return (
                <PaneRow key={c.id} selected={isActive} onClick={() => onPick(c)} height={32}>
                  <EngineGlyph engine={c.engine} size={18} />
                  <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', justifyContent: 'center' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <StatusDot state={c.status} />
                      <span style={{ fontSize: 12, color: 'var(--text)', fontWeight: 500,
                                     overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.name}</span>
                    </div>
                    <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)',
                                   overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {c.host}{c.port ? ':' + c.port : ''}
                    </span>
                  </div>
                  {c.latency != null && (
                    <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>{c.latency}ms</span>
                  )}
                </PaneRow>
              );
            })}
          </div>
        ))}
        <div style={{ padding: '12px 14px 0' }}>
          <button onClick={onNew} style={{
            width: '100%', background: 'transparent', border: '1px dashed var(--surface1)',
            color: 'var(--overlay1)', padding: '8px 12px', borderRadius: 6, cursor: 'pointer',
            fontSize: 12, fontFamily: 'inherit',
            display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 6,
          }}><Ph name="plus" size={11} color="currentColor" /> New connection</button>
        </div>
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────── SchemaPane
function tableIcon(kind) {
  if (kind === 'view')    return { name: 'eye',          color: 'var(--secondary)' };
  if (kind === 'matview') return { name: 'database',     color: 'var(--peach)' };
  return                          { name: 'table',        color: 'var(--syn-string)' };
}
function fmtCount(n) {
  if (n >= 1e6) return (n / 1e6).toFixed(n >= 10e6 ? 0 : 1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(n >= 10e3 ? 0 : 1) + 'k';
  return String(n);
}
function SchemaPane({ conn, onOpenTable, onOpenStructure, onAskAI }) {
  const [filter, setFilter] = React.useState('');
  const [expanded, setExpanded] = React.useState(() => new Set(['public']));
  const [expandedTables, setExpandedTables] = React.useState(() => new Set());

  if (!conn) return <Empty icon="plugs" title="No active connection" sub="Pick a connection to browse its schema." />;
  const schemas = SESHAT.SCHEMAS[conn.id] || [];
  const q = filter.toLowerCase();

  const toggleSchema = (n) => setExpanded(prev => {
    const next = new Set(prev); next.has(n) ? next.delete(n) : next.add(n); return next;
  });
  const toggleTable = (n) => setExpandedTables(prev => {
    const next = new Set(prev); next.has(n) ? next.delete(n) : next.add(n); return next;
  });

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <PaneHeader title={`Schema · ${conn.db}`} kbd="⌘P" action={
        <button title="Refresh" style={{ background: 'transparent', border: 0, color: 'var(--overlay1)', cursor: 'pointer', padding: 2 }}>
          <Ph name="arrows-clockwise" size={12} />
        </button>
      } />
      <SearchField value={filter} onChange={setFilter} placeholder="Search tables, columns…" />
      <div style={{ overflowY: 'auto', flex: 1, paddingBottom: 8 }}>
        {schemas.map(schema => {
          const open = expanded.has(schema.name);
          const matchingTables = schema.tables.filter(t => !q || t.name.toLowerCase().includes(q));
          if (q && matchingTables.length === 0) return null;
          return (
            <div key={schema.name}>
              <PaneRow onClick={() => toggleSchema(schema.name)}>
                <Ph name={open ? 'caret-down' : 'caret-right'} size={10} color="var(--overlay2)" />
                <Ph name="folder" size={13} color="var(--overlay2)" />
                <span style={{ flex: 1, fontWeight: 500 }}>{schema.name}</span>
                <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>
                  {schema.tables.length}
                </span>
              </PaneRow>
              {open && (q ? matchingTables : schema.tables).map(t => {
                const ico = tableIcon(t.kind);
                const tOpen = expandedTables.has(`${schema.name}.${t.name}`);
                const hasCols = !!t.cols;
                return (
                  <div key={t.name} style={{ display: 'contents' }}>
                    <PaneRow indent={1}
                      onClick={() => hasCols ? toggleTable(`${schema.name}.${t.name}`) : onOpenTable(conn, schema.name, t)}>
                      {hasCols
                        ? <Ph name={tOpen ? 'caret-down' : 'caret-right'} size={10} color="var(--overlay2)" />
                        : <span style={{ width: 10 }} />}
                      <Ph name={ico.name} size={12} color={ico.color} />
                      <span style={{ flex: 1, fontSize: 13, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{t.name}</span>
                      <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>
                        {fmtCount(t.rows)}
                      </span>
                      <RowActions
                        onData={(e) => { e.stopPropagation(); onOpenTable(conn, schema.name, t); }}
                        onStruct={(e) => { e.stopPropagation(); onOpenStructure(conn, schema.name, t); }}
                        onAsk={(e) => { e.stopPropagation(); onAskAI(`${schema.name}.${t.name}`); }} />
                    </PaneRow>
                    {tOpen && hasCols && t.cols.map(col => (
                      <PaneRow key={col.name} indent={2} height={22}
                        onClick={() => onOpenTable(conn, schema.name, t)}>
                        <span style={{ width: 10 }} />
                        <Ph name={col.pk ? 'key' : col.fk ? 'link' : 'circle'}
                            size={col.pk || col.fk ? 11 : 6}
                            color={col.pk ? 'var(--warning)' : col.fk ? 'var(--info)' : 'var(--text-disabled)'} />
                        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12,
                                       color: col.pk ? 'var(--warning)' : 'var(--text)' }}>{col.name}</span>
                        <span style={{ flex: 1 }} />
                        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--overlay1)' }}>
                          {col.type}{col.nn && !col.pk ? '' : ''}
                        </span>
                      </PaneRow>
                    ))}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function RowActions({ onData, onStruct, onAsk }) {
  return (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 2, marginLeft: 4 }}
      onClick={(e) => e.stopPropagation()}>
      <MicroAction title="Data" icon="table" onClick={onData} />
      <MicroAction title="Structure" icon="list-numbers" onClick={onStruct} />
      <MicroAction title="Ask AI about this table" icon="sparkle" onClick={onAsk} color="var(--primary)" />
    </span>
  );
}
function MicroAction({ icon, title, onClick, color = 'var(--overlay1)' }) {
  const [h, setH] = React.useState(false);
  return (
    <span title={title} onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        width: 18, height: 18, borderRadius: 3, display: 'inline-flex',
        alignItems: 'center', justifyContent: 'center', cursor: 'pointer',
        background: h ? 'var(--surface0)' : 'transparent', color: h ? 'var(--text)' : color,
      }}>
      <Ph name={icon} size={10} color="currentColor" />
    </span>
  );
}

function Empty({ icon, title, sub }) {
  return (
    <div style={{ padding: 32, color: 'var(--overlay1)', display: 'flex', flexDirection: 'column',
                  alignItems: 'center', justifyContent: 'center', flex: 1, textAlign: 'center', gap: 10 }}>
      <Ph name={icon} size={36} color="var(--surface2)" />
      <div style={{ color: 'var(--overlay2)', fontSize: 13, fontWeight: 500 }}>{title}</div>
      {sub && <div style={{ fontSize: 12, lineHeight: 1.5 }}>{sub}</div>}
    </div>
  );
}

// ──────────────────────────────────────────────────────────── SavedPane
function SavedPane({ onOpenQuery }) {
  const [filter, setFilter] = React.useState('');
  const items = SESHAT.SAVED.filter(q => !filter || q.name.toLowerCase().includes(filter.toLowerCase()));
  const folders = [...new Set(items.map(i => i.folder))];
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <PaneHeader title="Saved queries" />
      <SearchField value={filter} onChange={setFilter} placeholder="Search saved…" />
      <div style={{ overflowY: 'auto', flex: 1, paddingBottom: 8 }}>
        {folders.map(folder => (
          <div key={folder}>
            <PaneRow>
              <Ph name="folder" size={12} color="var(--overlay2)" />
              <span style={{ flex: 1, fontWeight: 500 }}>{folder}</span>
              <span style={{ fontSize: 10, color: 'var(--overlay1)' }}>{items.filter(i => i.folder === folder).length}</span>
            </PaneRow>
            {items.filter(i => i.folder === folder).map(q => (
              <PaneRow key={q.id} indent={1} onClick={() => onOpenQuery(q)} height={26}>
                <Ph name={q.starred ? 'star' : 'file-text'} size={11}
                    color={q.starred ? 'var(--warning)' : 'var(--overlay2)'} />
                <span style={{ flex: 1, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{q.name}</span>
                <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>{q.conn}</span>
              </PaneRow>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────── HistoryPane
function HistoryPane({ onOpenQuery }) {
  const [filter, setFilter] = React.useState('');
  const items = SESHAT.HISTORY.filter(h => !filter || h.query.toLowerCase().includes(filter.toLowerCase()));
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <PaneHeader title="Query history" />
      <SearchField value={filter} onChange={setFilter} placeholder="Search history…" />
      <div style={{ overflowY: 'auto', flex: 1, paddingBottom: 8 }}>
        {items.map((h, i) => (
          <div key={i} onClick={() => onOpenQuery(h)}
            style={{
              padding: '8px 14px', borderBottom: '1px solid var(--surface0)',
              cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 4,
            }}
            onMouseEnter={(e) => e.currentTarget.style.background = 'var(--sidebar-hover)'}
            onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Ph name={h.status === 'ok' ? 'check-circle' : 'x-circle'} size={11}
                  color={h.status === 'ok' ? 'var(--success)' : 'var(--error)'} />
              <span style={{ fontSize: 11, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>{h.conn}</span>
              <span style={{ flex: 1 }} />
              <span style={{ fontSize: 10, color: 'var(--overlay1)' }}>{h.t}</span>
            </div>
            <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text)',
              lineHeight: 1.4, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical',
              overflow: 'hidden',
            }}>{h.query}</div>
            <div style={{ display: 'flex', gap: 10, fontSize: 10, color: 'var(--overlay1)' }}>
              {h.ms != null ? <span>{h.ms} ms</span> : <span style={{ color: 'var(--error)' }}>{h.error}</span>}
              {h.rows != null && <span>· {h.rows.toLocaleString()} rows</span>}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────── ImportPane
function ImportPane() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <PaneHeader title="Import / Export" />
      <div style={{ padding: 14, display: 'flex', flexDirection: 'column', gap: 8 }}>
        {[
          { icon: 'upload-simple', label: 'Import CSV / TSV',          sub: 'Append or replace a table' },
          { icon: 'file-arrow-up', label: 'Import JSON / NDJSON',      sub: 'Map nested fields to columns' },
          { icon: 'database',      label: 'Restore SQL dump',          sub: '.sql, .dump, .backup' },
          { icon: 'download-simple', label: 'Export query results',    sub: 'CSV · JSON · Parquet · SQL' },
          { icon: 'file-zip',      label: 'Export table → dump',       sub: 'schema + data' },
        ].map((a, i) => (
          <div key={i} style={{
            padding: 10, background: 'var(--surface0)', borderRadius: 6,
            display: 'flex', alignItems: 'center', gap: 10, cursor: 'pointer',
          }}>
            <Ph name={a.icon} size={18} color="var(--text)" />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 12, fontWeight: 500, color: 'var(--text)' }}>{a.label}</div>
              <div style={{ fontSize: 11, color: 'var(--overlay1)' }}>{a.sub}</div>
            </div>
            <Ph name="caret-right" size={11} color="var(--overlay1)" />
          </div>
        ))}
      </div>
    </div>
  );
}

Object.assign(window, {
  ConnectionsPane, SchemaPane, SavedPane, HistoryPane, ImportPane,
  PaneHeader, SearchField, PaneRow, Empty,
});
