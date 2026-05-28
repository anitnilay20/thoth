/* Seshat — Command palette (⌘K).
   Primary navigation. Searches connections, tables, saved queries, history, actions. */

function CommandPalette({ open, onClose, onCmd, conn }) {
  const [query, setQuery] = React.useState('');
  const [sel, setSel] = React.useState(0);
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    if (open) {
      setQuery(''); setSel(0);
      requestAnimationFrame(() => inputRef.current && inputRef.current.focus());
    }
  }, [open]);

  // Build command list — must run before early return to keep hooks order stable
  const commands = React.useMemo(() => {
    const list = [];
    // Quick actions
    list.push(
      { group: 'Actions', icon: 'play-fill',    iconColor: 'var(--primary)',   label: 'Run query',                       kbd: '⌘↵',  do: () => onCmd({ type: 'run' }) },
      { group: 'Actions', icon: 'sparkle',      iconColor: 'var(--primary)',   label: 'Ask AI to write SQL…',            kbd: '/',   do: () => onCmd({ type: 'ai' }) },
      { group: 'Actions', icon: 'terminal-window', iconColor: 'var(--info)',   label: 'New SQL editor tab',              kbd: '⌘T',  do: () => onCmd({ type: 'new-sql' }) },
      { group: 'Actions', icon: 'plus-circle',  iconColor: 'var(--success)',   label: 'New connection…',                 kbd: '⌘N',  do: () => onCmd({ type: 'new-conn' }) },
      { group: 'Actions', icon: 'chart-bar',    iconColor: 'var(--info)',      label: 'Explain current query',           kbd: '⌘⇧E', do: () => onCmd({ type: 'explain' }) },
      { group: 'Actions', icon: 'graph',        iconColor: 'var(--primary)',   label: 'Open ER diagram',                              do: () => onCmd({ type: 'open-er' }) },
      { group: 'Actions', icon: 'sun',          iconColor: 'var(--warning)',   label: 'Toggle light / dark theme',       kbd: '⌘⇧T', do: () => onCmd({ type: 'toggle-theme' }) },
      { group: 'Actions', icon: 'download-simple', iconColor: 'var(--info)',   label: 'Export current results…',         kbd: '⌘E',  do: () => onCmd({ type: 'export' }) },
    );
    // Connections
    SESHAT.CONNECTIONS.forEach(c => {
      list.push({
        group: 'Connections',
        icon: 'database', iconColor: c.color,
        label: c.name, subtitle: `${SESHAT.ENGINES[c.engine].label} · ${c.host}`,
        rightEl: <StatusDot state={c.status} />,
        kbd: c.id === 'prod-pg' ? '⌘1' : c.id === 'stage-pg' ? '⌘2' : '',
        do: () => onCmd({ type: 'switch-conn', conn: c }),
      });
    });
    // Tables in active connection
    if (conn) {
      const schemas = SESHAT.SCHEMAS[conn.id] || [];
      schemas.forEach(s => s.tables.forEach(t => {
        list.push({
          group: 'Tables',
          icon: t.kind === 'view' ? 'eye' : t.kind === 'matview' ? 'database' : 'table',
          iconColor: t.kind === 'view' ? 'var(--secondary)' : 'var(--syn-string)',
          label: `${s.name}.${t.name}`, subtitle: `${t.rows.toLocaleString()} rows`,
          do: () => onCmd({ type: 'open-table', conn, schema: s.name, table: t }),
        });
      }));
    }
    // Saved queries
    SESHAT.SAVED.forEach(q => {
      list.push({
        group: 'Saved queries',
        icon: q.starred ? 'star' : 'file-text',
        iconColor: q.starred ? 'var(--warning)' : 'var(--overlay2)',
        label: q.name, subtitle: `${q.folder} · ${q.conn}`,
        do: () => onCmd({ type: 'open-saved', q }),
      });
    });
    return list;
  }, [conn]);

  if (!open) return null;

  const q = query.trim().toLowerCase();
  const filtered = q
    ? commands.filter(c =>
        c.label.toLowerCase().includes(q) ||
        (c.subtitle || '').toLowerCase().includes(q) ||
        c.group.toLowerCase().includes(q))
    : commands;

  // Group filtered list by group, preserving stable indices for keyboard nav
  const groups = filtered.reduce((acc, c) => {
    (acc[c.group] = acc[c.group] || []).push(c); return acc;
  }, {});
  const flat = Object.values(groups).flat();

  const onKey = (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setSel(s => Math.min(flat.length - 1, s + 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setSel(s => Math.max(0, s - 1)); }
    else if (e.key === 'Enter') { e.preventDefault(); if (flat[sel]) { flat[sel].do(); onClose(); } }
    else if (e.key === 'Escape') { e.preventDefault(); onClose(); }
  };

  return (
    <div onClick={onClose} style={{
      position: 'fixed', inset: 0, zIndex: 60,
      background: 'rgba(0,0,0,0.55)',
      display: 'flex', alignItems: 'flex-start', justifyContent: 'center', paddingTop: 100,
    }}>
      <div onClick={(e) => e.stopPropagation()} style={{
        width: 680, maxHeight: 540, background: 'var(--mantle)', borderRadius: 10,
        border: '1px solid var(--surface1)', boxShadow: 'var(--shadow-modal)',
        overflow: 'hidden', display: 'flex', flexDirection: 'column',
      }}>
        <div style={{
          padding: '12px 16px', display: 'flex', alignItems: 'center', gap: 10,
          borderBottom: '1px solid var(--surface0)',
        }}>
          <Ph name="magnifying-glass" size={14} color="var(--overlay2)" />
          <input ref={inputRef} value={query}
            onChange={(e) => { setQuery(e.target.value); setSel(0); }}
            onKeyDown={onKey}
            placeholder="Search connections, tables, saved queries, actions…"
            style={{
              flex: 1, background: 'transparent', border: 0,
              color: 'var(--text)', fontFamily: 'inherit', fontSize: 14, outline: 'none',
            }} />
          <span className="seshat-kbd">esc</span>
        </div>
        <div style={{ overflowY: 'auto', flex: 1 }}>
          {Object.entries(groups).map(([g, items]) => (
            <div key={g}>
              <div style={{
                padding: '8px 16px 4px', fontSize: 10, color: 'var(--overlay1)',
                textTransform: 'uppercase', letterSpacing: '0.08em', fontWeight: 700,
              }}>{g}</div>
              {items.map(c => {
                const idx = flat.indexOf(c);
                const active = idx === sel;
                return (
                  <div key={g + '·' + c.label + '·' + idx} onClick={() => { c.do(); onClose(); }} onMouseEnter={() => setSel(idx)}
                    style={{
                      padding: '6px 16px', display: 'flex', alignItems: 'center', gap: 10,
                      background: active ? 'var(--surface0)' : 'transparent', cursor: 'pointer',
                      borderLeft: active ? '2px solid var(--primary)' : '2px solid transparent',
                    }}>
                    <Ph name={c.icon} size={14} color={c.iconColor} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 13, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.label}</div>
                      {c.subtitle && <div style={{ fontSize: 11, color: 'var(--overlay1)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.subtitle}</div>}
                    </div>
                    {c.rightEl}
                    {c.kbd && <span className="seshat-kbd">{c.kbd}</span>}
                  </div>
                );
              })}
            </div>
          ))}
          {flat.length === 0 && (
            <div style={{ padding: 30, color: 'var(--overlay1)', fontSize: 12, textAlign: 'center' }}>
              No matches.
            </div>
          )}
        </div>
        <div style={{
          padding: '6px 16px', borderTop: '1px solid var(--surface0)',
          display: 'flex', alignItems: 'center', gap: 14, fontSize: 10, color: 'var(--overlay1)',
        }}>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
            <span className="seshat-kbd">↑↓</span> navigate
          </span>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
            <span className="seshat-kbd">↵</span> select
          </span>
          <span style={{ flex: 1 }} />
          <span>Powered by Seshat</span>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { CommandPalette });
