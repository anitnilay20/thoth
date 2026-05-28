/* Seshat — auxiliary views: TableDataView, TableStructureView, ERDiagramView,
   ConnectionManagerView, NewConnectionDialog. */

// ────────────────────────────────────────────────────── TableDataView
// Lightweight inline grid for browsing arbitrary tables. Uses the hero data
// when the table is `public.users`, otherwise generates plausible-looking rows.
function generateRowsFor(table, count = 50) {
  const seed = table.name.charCodeAt(0) + table.rows;
  let s = seed;
  const rnd = () => (s = (s * 9301 + 49297) % 233280) / 233280;
  const names = ['Alex','Jordan','Sam','Riley','Casey','Morgan','Taylor','Drew','Avery','Quinn','Hayden','Skyler','Reese','Jamie','Cameron','Charlie','Robin','Toby','Frankie','Hollis'];
  const domains = ['acme.io','northwind.co','foxtrot.health','helix.ai','stratus.cloud','beacon.io','quill.press','orbit.studio'];
  const plans = ['free','starter','pro','enterprise'];
  return Array.from({ length: count }, (_, i) => {
    const fn = names[Math.floor(rnd() * names.length)];
    const ln = names[Math.floor(rnd() * names.length)];
    return [
      1000 + i,
      `${fn.toLowerCase()}.${ln.toLowerCase()}@${domains[Math.floor(rnd() * domains.length)]}`,
      `${fn} ${ln}`,
      plans[Math.floor(rnd() * plans.length)],
      Math.floor(rnd() * 38291) + 1,
      rnd() > 0.4 ? null : `https://avatars.acme.io/${1000 + i}.png`,
      { source: ['signup','invite','sso'][Math.floor(rnd()*3)], beta: rnd() > 0.8 },
      `2024-${String(Math.floor(rnd()*12)+1).padStart(2,'0')}-${String(Math.floor(rnd()*28)+1).padStart(2,'0')}`,
      `2026-05-${String(Math.floor(rnd()*15)+1).padStart(2,'0')}`,
    ];
  });
}

function TableDataView({ conn, schema, table }) {
  const rows = React.useMemo(() => generateRowsFor(table, 100), [table.name]);
  const columns = table.cols || [
    { name: 'id', type: 'bigint', width: 88, align: 'right', isPk: true },
    { name: 'email', type: 'text', width: 260 },
    { name: 'name', type: 'text', width: 180 },
    { name: 'plan', type: 'text', width: 110, chip: true },
    { name: 'org_id', type: 'bigint', width: 110, align: 'right', fk: 'organizations.id' },
    { name: 'avatar_url', type: 'text', width: 220 },
    { name: 'metadata', type: 'jsonb', width: 240, kind: 'json' },
    { name: 'created_at', type: 'date', width: 130 },
    { name: 'last_seen_at', type: 'date', width: 130 },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}>
      <TableDataToolbar table={table} schema={schema} rowCount={table.rows} />
      <ResultsPanel columns={columns} rows={rows} queryMs={28} executing={false} />
    </div>
  );
}

function TableDataToolbar({ table, schema, rowCount }) {
  return (
    <div style={{
      height: 40, display: 'flex', alignItems: 'center', gap: 8,
      background: 'var(--mantle)', borderBottom: '1px solid var(--surface0)',
      padding: '0 10px', flexShrink: 0,
    }}>
      <Ph name="table" size={14} color="var(--syn-string)" />
      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 13, color: 'var(--overlay2)' }}>
        {schema}.<span style={{ color: 'var(--text)', fontWeight: 600 }}>{table.name}</span>
      </span>
      <span style={{ fontSize: 11, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>
        · {rowCount.toLocaleString()} rows
      </span>
      <span style={{ width: 1, height: 20, background: 'var(--surface0)', margin: '0 4px' }} />
      <FilterPill icon="funnel" label="WHERE" placeholder="add filter…" />
      <FilterPill icon="sort-ascending" label="ORDER BY" placeholder="—" />
      <span style={{ flex: 1 }} />
      <button style={ghostBtn}>
        <Ph name="plus" size={11} color="currentColor" /> Add row
      </button>
      <button style={ghostBtn}>
        <Ph name="arrows-clockwise" size={11} color="currentColor" /> Refresh
      </button>
      <button style={ghostBtn}>
        <Ph name="download-simple" size={11} color="currentColor" /> Export
      </button>
    </div>
  );
}
const ghostBtn = {
  height: 26, padding: '0 10px', borderRadius: 4, cursor: 'pointer',
  background: 'transparent', border: '1px solid var(--surface0)',
  color: 'var(--overlay2)', fontSize: 12, fontFamily: 'inherit',
  display: 'inline-flex', alignItems: 'center', gap: 6,
};

function FilterPill({ icon, label, placeholder }) {
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 6,
      background: 'var(--crust)', border: '1px solid var(--surface0)',
      borderRadius: 4, padding: '3px 8px', height: 24,
    }}>
      <Ph name={icon} size={10} color="var(--overlay1)" />
      <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)', fontWeight: 600 }}>{label}</span>
      <span style={{ fontSize: 11, color: 'var(--text-disabled)', fontFamily: 'var(--font-mono)' }}>{placeholder}</span>
    </span>
  );
}

// ────────────────────────────────────────────────────── TableStructureView
function TableStructureView({ conn, schema, table }) {
  const [tab, setTab] = React.useState('columns');
  const t = table.cols ? table : { ...table, cols: [
    { name: 'id', type: 'bigint', pk: true, nn: true },
    { name: 'name', type: 'text', nn: true },
  ]};
  return (
    <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0, background: 'var(--base)' }}>
      <div style={{
        padding: '14px 20px', borderBottom: '1px solid var(--surface0)',
        display: 'flex', alignItems: 'center', gap: 12,
      }}>
        <div style={{
          width: 40, height: 40, borderRadius: 8, background: 'var(--surface0)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <Ph name="table" size={20} color="var(--syn-string)" />
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--overlay1)' }}>{schema}</div>
          <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--text)' }}>{t.name}</div>
        </div>
        <Stat label="Rows"     value={t.rows.toLocaleString()} color="var(--text)" />
        <Stat label="Columns"  value={String(t.cols.length)}   color="var(--text)" />
        <Stat label="Indexes"  value={String((t.indexes || []).length)} color="var(--text)" />
        <Stat label="Size"     value="318 MB" color="var(--text)" />
      </div>
      <div style={{ display: 'flex', borderBottom: '1px solid var(--surface0)', background: 'var(--mantle)' }}>
        {['columns','indexes','constraints','foreign-keys','ddl','triggers'].map(id => (
          <button key={id} onClick={() => setTab(id)} style={{
            height: 36, padding: '0 14px', background: 'transparent', border: 0,
            borderRight: '1px solid var(--surface0)',
            color: tab === id ? 'var(--text)' : 'var(--overlay1)',
            borderBottom: tab === id ? '2px solid var(--primary)' : '2px solid transparent',
            fontSize: 12, fontFamily: 'inherit', cursor: 'pointer', textTransform: 'capitalize',
          }}>{id.replace('-', ' ')}</button>
        ))}
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: 20 }}>
        {tab === 'columns' && <ColumnsTable cols={t.cols} />}
        {tab === 'indexes' && <IndexesTable idx={t.indexes || []} />}
        {tab === 'constraints' && <ConstraintsView cols={t.cols} />}
        {tab === 'foreign-keys' && <ForeignKeysView cols={t.cols} />}
        {tab === 'ddl' && <DdlView table={t} schema={schema} />}
        {tab === 'triggers' && <Empty icon="lightning" title="No triggers defined on this table." />}
      </div>
    </div>
  );
}

function ColumnsTable({ cols }) {
  const head = ['', 'Column', 'Type', 'Nullable', 'Default', 'Constraints', ''];
  return (
    <div style={{ background: 'var(--mantle)', borderRadius: 6, border: '1px solid var(--surface0)', overflow: 'hidden' }}>
      <div style={{ display: 'grid', gridTemplateColumns: '28px 1fr 1.4fr 90px 1.2fr 1.4fr 60px',
                    background: 'var(--surface0)', borderBottom: '1px solid var(--surface1)' }}>
        {head.map((h, i) => (
          <div key={i} style={{ padding: '8px 10px', fontSize: 10, fontWeight: 700, color: 'var(--overlay2)',
                                textTransform: 'uppercase', letterSpacing: '0.06em' }}>{h}</div>
        ))}
      </div>
      {cols.map((c, i) => (
        <div key={c.name} style={{
          display: 'grid', gridTemplateColumns: '28px 1fr 1.4fr 90px 1.2fr 1.4fr 60px',
          borderTop: i ? '1px solid var(--surface0)' : 0, alignItems: 'center',
        }}>
          <div style={{ padding: '8px 0 8px 10px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <Ph name={c.pk ? 'key' : c.fk ? 'link' : 'circle'}
                size={c.pk || c.fk ? 12 : 6}
                color={c.pk ? 'var(--warning)' : c.fk ? 'var(--info)' : 'var(--text-disabled)'} />
          </div>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 13,
                        color: c.pk ? 'var(--warning)' : 'var(--text)', fontWeight: c.pk ? 600 : 400 }}>
            {c.name}
          </div>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--secondary)' }}>
            {c.type}
          </div>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 11, color: c.nn ? 'var(--overlay1)' : 'var(--syn-number)' }}>
            {c.nn ? 'NOT NULL' : 'nullable'}
          </div>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 11, color: c.default ? 'var(--syn-string)' : 'var(--text-disabled)' }}>
            {c.default || '—'}
          </div>
          <div style={{ padding: '8px 10px', display: 'flex', gap: 4, flexWrap: 'wrap' }}>
            {c.pk && <Badge color="var(--warning)" text="PRIMARY KEY" />}
            {c.unique && !c.pk && <Badge color="var(--secondary)" text="UNIQUE" />}
            {c.fk && <Badge color="var(--info)" text={`FK → ${c.fk}`} />}
          </div>
          <div style={{ padding: '8px 10px', display: 'flex', justifyContent: 'flex-end' }}>
            <MicroAction title="Edit column" icon="pencil-simple" />
          </div>
        </div>
      ))}
    </div>
  );
}

function Badge({ text, color }) {
  return (
    <span style={{
      fontFamily: 'var(--font-mono)', fontSize: 9, fontWeight: 700,
      padding: '2px 6px', borderRadius: 3,
      background: 'transparent', border: `1px solid ${color}`, color,
      letterSpacing: '0.04em',
    }}>{text}</span>
  );
}

function IndexesTable({ idx }) {
  if (!idx.length) return <Empty icon="list-numbers" title="No indexes besides the PK." />;
  return (
    <div style={{ background: 'var(--mantle)', borderRadius: 6, border: '1px solid var(--surface0)', overflow: 'hidden' }}>
      <div style={{ display: 'grid', gridTemplateColumns: '1.4fr 2fr 0.8fr 1fr',
                    background: 'var(--surface0)', borderBottom: '1px solid var(--surface1)' }}>
        {['Index','Columns','Unique','Condition'].map((h, i) => (
          <div key={i} style={{ padding: '8px 10px', fontSize: 10, fontWeight: 700, color: 'var(--overlay2)',
                                textTransform: 'uppercase', letterSpacing: '0.06em' }}>{h}</div>
        ))}
      </div>
      {idx.map((idx, i) => (
        <div key={idx.name} style={{
          display: 'grid', gridTemplateColumns: '1.4fr 2fr 0.8fr 1fr',
          borderTop: i ? '1px solid var(--surface0)' : 0, padding: 0,
        }}>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 13, color: 'var(--text)' }}>
            <Ph name="list-numbers" size={11} color="var(--overlay2)" style={{ marginRight: 6 }} />
            {idx.name}
          </div>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--syn-string)' }}>
            ({idx.cols.join(', ')})
          </div>
          <div style={{ padding: '8px 10px' }}>
            {idx.unique && <Badge color="var(--secondary)" text="UNIQUE" />}
          </div>
          <div style={{ padding: '8px 10px', fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--overlay1)' }}>
            {idx.partial || '—'}
          </div>
        </div>
      ))}
    </div>
  );
}

function ConstraintsView({ cols }) {
  const pk = cols.filter(c => c.pk).map(c => c.name);
  const unique = cols.filter(c => c.unique && !c.pk);
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <ConstraintCard color="var(--warning)" icon="key" label="PRIMARY KEY"
        body={`(${pk.join(', ')})`} />
      {unique.map(u => (
        <ConstraintCard key={u.name} color="var(--secondary)" icon="fingerprint"
          label="UNIQUE" body={`(${u.name})`} />
      ))}
      <ConstraintCard color="var(--info)" icon="check-square" label="CHECK"
        body={`char_length(email) > 3`} />
    </div>
  );
}
function ConstraintCard({ color, icon, label, body }) {
  return (
    <div style={{
      background: 'var(--mantle)', border: '1px solid var(--surface0)',
      borderRadius: 6, padding: 12, display: 'flex', alignItems: 'center', gap: 12,
    }}>
      <span style={{
        width: 28, height: 28, borderRadius: 4, background: 'var(--surface0)',
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      }}>
        <Ph name={icon} size={14} color={color} />
      </span>
      <Badge color={color} text={label} />
      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 13, color: 'var(--text)' }}>{body}</span>
    </div>
  );
}

function ForeignKeysView({ cols }) {
  const fks = cols.filter(c => c.fk);
  if (!fks.length) return <Empty icon="link" title="No foreign keys on this table." />;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      {fks.map(c => (
        <div key={c.name} style={{
          background: 'var(--mantle)', border: '1px solid var(--surface0)', borderRadius: 6,
          padding: 14, display: 'flex', alignItems: 'center', gap: 12,
        }}>
          <Ph name="link" size={18} color="var(--info)" />
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 13, color: 'var(--text)' }}>
            <span style={{ color: 'var(--warning)' }}>{c.name}</span>
            <span style={{ color: 'var(--overlay1)', margin: '0 8px' }}>→</span>
            <span style={{ color: 'var(--syn-string)' }}>{c.fk}</span>
          </span>
          <span style={{ flex: 1 }} />
          <Badge color="var(--info)" text="ON DELETE RESTRICT" />
          <Badge color="var(--info)" text="ON UPDATE CASCADE" />
        </div>
      ))}
    </div>
  );
}

function DdlView({ table, schema }) {
  const ddl = `-- ${schema}.${table.name}\nCREATE TABLE ${schema}.${table.name} (\n${
    (table.cols || []).map(c =>
      `  ${c.name.padEnd(14)} ${c.type}${c.nn ? ' NOT NULL' : ''}${c.default ? ' DEFAULT ' + c.default : ''}${c.pk ? ' PRIMARY KEY' : ''}${c.unique && !c.pk ? ' UNIQUE' : ''}${c.fk ? ` REFERENCES ${c.fk.split('.')[0]}(${c.fk.split('.')[1]})` : ''}`
    ).join(',\n')
  }\n);\n\n${(table.indexes || []).filter(i => !i.name.endsWith('_pkey')).map(i =>
    `CREATE${i.unique ? ' UNIQUE' : ''} INDEX ${i.name}\n  ON ${schema}.${table.name} (${i.cols.join(', ')})${i.partial ? '\n  ' + i.partial : ''};`
  ).join('\n\n')}`;
  return (
    <div style={{
      background: 'var(--mantle)', border: '1px solid var(--surface0)', borderRadius: 6,
      padding: 16, overflow: 'auto',
    }}>
      <HighlightedSQL src={ddl} />
    </div>
  );
}

// ────────────────────────────────────────────────────── ERDiagramView
function ERDiagramView({ conn }) {
  const nodes = SESHAT.ER_NODES;
  const edges = SESHAT.ER_EDGES;
  const W = 1080, H = 820;
  const [hover, setHover] = React.useState(null);
  const [zoom, setZoom] = React.useState(1);

  // Compute edge paths
  const nodeMap = Object.fromEntries(nodes.map(n => [n.id, n]));
  const NODE_W = 240;
  const HEADER_H = 32, ROW_H = 22;
  const nodeHeight = (n) => HEADER_H + n.cols.length * ROW_H;
  const colY = (n, colName) => HEADER_H + n.cols.indexOf(colName) * ROW_H + ROW_H / 2;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0, background: 'var(--base)' }}>
      <div style={{
        height: 40, display: 'flex', alignItems: 'center', gap: 8,
        background: 'var(--mantle)', borderBottom: '1px solid var(--surface0)', padding: '0 10px',
      }}>
        <Ph name="graph" size={14} color="var(--primary)" />
        <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--text)' }}>ER Diagram</span>
        <span style={{ fontSize: 11, color: 'var(--overlay1)' }}>· {nodes.length} tables · {edges.length} relationships</span>
        <span style={{ flex: 1 }} />
        <span style={{ display: 'inline-flex', gap: 2, background: 'var(--crust)', border: '1px solid var(--surface0)', borderRadius: 4 }}>
          <ZoomBtn icon="minus" onClick={() => setZoom(z => Math.max(0.5, z - 0.1))} />
          <span style={{ padding: '0 8px', display: 'inline-flex', alignItems: 'center', fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--overlay2)' }}>
            {Math.round(zoom * 100)}%
          </span>
          <ZoomBtn icon="plus" onClick={() => setZoom(z => Math.min(2, z + 0.1))} />
        </span>
        <button style={ghostBtn}><Ph name="layout" size={11} /> Auto-layout</button>
        <button style={ghostBtn}><Ph name="download-simple" size={11} /> SVG</button>
      </div>
      <div style={{ flex: 1, overflow: 'auto', background: `radial-gradient(circle at 20px 20px, var(--surface0) 1px, transparent 1px) 0 0 / 24px 24px var(--base)` }}>
        <div style={{ width: W * zoom, height: H * zoom, position: 'relative' }}>
          <svg width={W * zoom} height={H * zoom} viewBox={`0 0 ${W} ${H}`} style={{ position: 'absolute', inset: 0 }}>
            <defs>
              <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
                <path d="M0,0 L10,5 L0,10 Z" fill="var(--info)" />
              </marker>
            </defs>
            {edges.map((e, i) => {
              const [fromTable, fromCol] = e.from.split('.');
              const [toTable, toCol] = e.to.split('.');
              const a = nodeMap[fromTable], b = nodeMap[toTable];
              if (!a || !b) return null;
              const aLeft = a.x + NODE_W / 2 < b.x + NODE_W / 2;
              const x1 = aLeft ? a.x + NODE_W : a.x;
              const x2 = aLeft ? b.x : b.x + NODE_W;
              const y1 = a.y + colY(a, fromCol);
              const y2 = b.y + colY(b, toCol);
              const mx = (x1 + x2) / 2;
              const isHovered = hover === fromTable || hover === toTable;
              return (
                <path key={i}
                  d={`M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`}
                  fill="none"
                  stroke={isHovered ? 'var(--primary)' : 'var(--surface2)'}
                  strokeWidth={isHovered ? 2 : 1.5}
                  markerEnd="url(#arrow)" />
              );
            })}
          </svg>
          {nodes.map(n => (
            <ERNode key={n.id} n={n} hover={hover === n.id} onHover={setHover} />
          ))}
        </div>
      </div>
    </div>
  );
}

function ERNode({ n, hover, onHover }) {
  return (
    <div onMouseEnter={() => onHover(n.id)} onMouseLeave={() => onHover(null)}
      style={{
        position: 'absolute', left: n.x, top: n.y, width: 240,
        background: 'var(--mantle)', border: `1px solid ${hover ? 'var(--primary)' : 'var(--surface1)'}`,
        borderRadius: 6, overflow: 'hidden',
        boxShadow: hover ? '0 0 0 1px var(--primary), 0 4px 16px rgba(203,166,247,0.15)' : 'none',
        transition: 'box-shadow 100ms, border-color 100ms',
      }}>
      <div style={{
        height: 32, padding: '0 12px', background: 'var(--surface0)',
        borderBottom: '1px solid var(--surface1)',
        display: 'flex', alignItems: 'center', gap: 6,
      }}>
        <Ph name="table" size={11} color="var(--syn-string)" />
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text)', fontWeight: 600 }}>{n.id}</span>
      </div>
      {n.cols.map(c => {
        const isPk = c === n.pk;
        const isFk = c.endsWith('_id') && !isPk;
        return (
          <div key={c} style={{
            height: 22, padding: '0 12px', display: 'flex', alignItems: 'center', gap: 6,
            borderBottom: '1px solid var(--surface0)',
            fontFamily: 'var(--font-mono)', fontSize: 11,
          }}>
            <Ph name={isPk ? 'key' : isFk ? 'link' : 'circle'} size={isPk || isFk ? 10 : 5}
                color={isPk ? 'var(--warning)' : isFk ? 'var(--info)' : 'var(--text-disabled)'} />
            <span style={{ color: isPk ? 'var(--warning)' : 'var(--text)' }}>{c}</span>
          </div>
        );
      })}
    </div>
  );
}

function ZoomBtn({ icon, onClick }) {
  return (
    <button onClick={onClick} style={{
      width: 24, height: 24, background: 'transparent', border: 0, cursor: 'pointer',
      color: 'var(--overlay2)', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    }}><Ph name={icon} size={11} color="currentColor" /></button>
  );
}

// ────────────────────────────────────────────────────── ConnectionManagerView
function ConnectionManagerView({ onPick, onNew }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0, background: 'var(--base)', overflow: 'auto' }}>
      <div style={{ padding: '24px 32px 8px' }}>
        <div style={{ fontSize: 20, fontWeight: 600, color: 'var(--text)' }}>Connections</div>
        <div style={{ fontSize: 13, color: 'var(--overlay1)', marginTop: 4 }}>
          {SESHAT.CONNECTIONS.length} saved · {SESHAT.CONNECTIONS.filter(c => c.status === 'connected').length} connected
        </div>
      </div>
      <div style={{ padding: '8px 32px 32px', display: 'grid',
                    gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 12 }}>
        <NewConnectionCard onClick={onNew} />
        {SESHAT.CONNECTIONS.map(c => (
          <ConnectionCard key={c.id} c={c} onClick={() => onPick(c)} />
        ))}
      </div>
    </div>
  );
}

function NewConnectionCard({ onClick }) {
  return (
    <button onClick={onClick} style={{
      background: 'transparent', border: '1px dashed var(--surface2)', borderRadius: 12,
      padding: 16, cursor: 'pointer', minHeight: 132,
      display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
      gap: 6, color: 'var(--overlay1)', fontFamily: 'inherit',
    }}>
      <Ph name="plus-circle" size={24} color="var(--primary)" />
      <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--text)' }}>New connection</span>
      <span style={{ fontSize: 11 }}>13 engines supported</span>
    </button>
  );
}

function ConnectionCard({ c, onClick }) {
  const e = SESHAT.ENGINES[c.engine];
  return (
    <div onClick={onClick} style={{
      background: 'var(--mantle)', border: '1px solid var(--surface0)', borderRadius: 12,
      padding: 16, cursor: 'pointer', position: 'relative', overflow: 'hidden',
    }}
      onMouseEnter={(e) => e.currentTarget.style.borderColor = 'var(--surface2)'}
      onMouseLeave={(e) => e.currentTarget.style.borderColor = 'var(--surface0)'}>
      <span style={{
        position: 'absolute', top: 0, left: 0, right: 0, height: 3, background: c.color,
      }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
        <EngineGlyph engine={c.engine} size={32} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {c.name}
          </div>
          <div style={{ fontSize: 11, color: 'var(--overlay1)' }}>{e.label} · {c.env}</div>
        </div>
        <StatusDot state={c.status} size={9} />
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, fontSize: 11, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>
        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          <Ph name="globe" size={10} color="currentColor" /> {c.host}{c.port ? ':' + c.port : ''}
        </span>
        <span><Ph name="database" size={10} color="currentColor" /> {c.db}</span>
        <span><Ph name="user" size={10} color="currentColor" /> {c.user || '—'} {c.ssl && <span style={{ color: 'var(--success)' }}>· TLS</span>}</span>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 10, paddingTop: 10, borderTop: '1px solid var(--surface0)' }}>
        <span style={{ fontSize: 10, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)' }}>
          {c.latency != null ? `${c.latency} ms` : 'offline'}
        </span>
        <span style={{ flex: 1 }} />
        <span style={{ fontSize: 11, color: 'var(--info)', fontWeight: 500 }}>Open →</span>
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────── NewConnectionDialog
function NewConnectionDialog({ open, onClose, onConnect }) {
  const [engine, setEngine] = React.useState('postgres');
  const [step, setStep] = React.useState(0); // 0 = engine, 1 = form
  const [form, setForm] = React.useState({
    name: '', host: 'localhost', port: 5432, db: '', user: '', password: '', ssl: true,
  });
  const [testing, setTesting] = React.useState(null); // null | 'pending' | 'ok' | 'err'

  React.useEffect(() => {
    if (open) { setStep(0); setEngine('postgres'); setTesting(null); }
  }, [open]);

  if (!open) return null;

  const engines = Object.entries(SESHAT.ENGINES);
  const test = () => {
    setTesting('pending');
    setTimeout(() => setTesting('ok'), 900);
  };

  return (
    <div style={{
      position: 'fixed', inset: 0, zIndex: 50,
      background: 'rgba(0,0,0,0.6)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
    }} onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} style={{
        width: 640, maxHeight: '80vh', background: 'var(--mantle)', borderRadius: 10,
        border: '1px solid var(--surface1)', boxShadow: 'var(--shadow-modal)', overflow: 'hidden',
        display: 'flex', flexDirection: 'column',
      }}>
        <div style={{ padding: '14px 18px', borderBottom: '1px solid var(--surface0)',
                      display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div>
            <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--text)' }}>New connection</div>
            <div style={{ fontSize: 11, color: 'var(--overlay1)' }}>
              {step === 0 ? 'Pick a database engine' : `${SESHAT.ENGINES[engine].label} · enter credentials`}
            </div>
          </div>
          <button onClick={onClose} style={{
            background: 'transparent', border: 0, color: 'var(--overlay1)', cursor: 'pointer', padding: 4,
          }}><Ph name="x" size={14} /></button>
        </div>
        {step === 0 ? (
          <div style={{ padding: 18, overflow: 'auto' }}>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 8 }}>
              {engines.map(([id, e]) => (
                <button key={id} onClick={() => { setEngine(id); setStep(1); }}
                  style={{
                    background: 'var(--surface0)', border: '1px solid var(--surface1)',
                    borderRadius: 6, padding: 12, cursor: 'pointer', textAlign: 'left',
                    display: 'flex', alignItems: 'center', gap: 10, fontFamily: 'inherit',
                  }}
                  onMouseEnter={(ev) => ev.currentTarget.style.borderColor = e.dot}
                  onMouseLeave={(ev) => ev.currentTarget.style.borderColor = 'var(--surface1)'}>
                  <EngineGlyph engine={id} size={28} />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 13, color: 'var(--text)', fontWeight: 500 }}>{e.label}</div>
                    <div style={{ fontSize: 10, color: 'var(--overlay1)', textTransform: 'uppercase' }}>{e.kind}</div>
                  </div>
                </button>
              ))}
            </div>
          </div>
        ) : (
          <div style={{ padding: 18, overflow: 'auto', flex: 1 }}>
            <Field label="Connection name"
              value={form.name} onChange={v => setForm({ ...form, name: v })}
              placeholder="my-database" mono />
            <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 12 }}>
              <Field label="Host"
                value={form.host} onChange={v => setForm({ ...form, host: v })} mono />
              <Field label="Port"
                value={form.port} onChange={v => setForm({ ...form, port: +v })} mono />
            </div>
            <Field label="Database"
              value={form.db} onChange={v => setForm({ ...form, db: v })} placeholder="postgres" mono />
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
              <Field label="User"
                value={form.user} onChange={v => setForm({ ...form, user: v })} mono />
              <Field label="Password"
                value={form.password} onChange={v => setForm({ ...form, password: v })} type="password" mono />
            </div>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 8, cursor: 'pointer' }}>
              <input type="checkbox" checked={form.ssl} onChange={e => setForm({ ...form, ssl: e.target.checked })} />
              <span style={{ fontSize: 12, color: 'var(--text)' }}>Require TLS</span>
            </label>
            <details style={{ marginTop: 14, fontSize: 12, color: 'var(--overlay1)' }}>
              <summary style={{ cursor: 'pointer' }}>Advanced (SSH tunnel, pool size, options)</summary>
              <div style={{ paddingTop: 8, color: 'var(--text-disabled)' }}>
                <code style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>—</code>
              </div>
            </details>
          </div>
        )}
        <div style={{ padding: 14, borderTop: '1px solid var(--surface0)', display: 'flex', alignItems: 'center', gap: 10 }}>
          {step === 1 && (
            <button onClick={() => setStep(0)} style={ghostBtn}>
              <Ph name="caret-left" size={11} /> Back
            </button>
          )}
          {step === 1 && (
            <button onClick={test} style={{
              ...ghostBtn,
              borderColor: testing === 'ok' ? 'var(--success)' : 'var(--surface0)',
              color: testing === 'ok' ? 'var(--success)' : 'var(--overlay2)',
            }}>
              {testing === 'pending' && <Ph name="circle-notch" size={11} spin color="currentColor" />}
              {testing === 'ok' && <Ph name="check-circle" size={11} color="var(--success)" />}
              {!testing && <Ph name="plug" size={11} color="currentColor" />}
              {testing === 'pending' ? 'Testing…' : testing === 'ok' ? 'Connection OK · 38 ms' : 'Test connection'}
            </button>
          )}
          <span style={{ flex: 1 }} />
          <button onClick={onClose} style={ghostBtn}>Cancel</button>
          <button onClick={() => onConnect && onConnect({ ...form, engine })} disabled={step === 0}
            style={{
              height: 26, padding: '0 14px', borderRadius: 4,
              cursor: step === 0 ? 'not-allowed' : 'pointer',
              background: step === 0 ? 'var(--surface0)' : 'var(--primary)',
              border: 0, color: step === 0 ? 'var(--overlay1)' : 'var(--crust)',
              fontSize: 12, fontWeight: 600, fontFamily: 'inherit',
              display: 'inline-flex', alignItems: 'center', gap: 6,
            }}>
            <Ph name="plug" size={11} color="currentColor" /> Connect
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({ label, value, onChange, placeholder, type = 'text', mono }) {
  return (
    <label style={{ display: 'block', marginBottom: 10 }}>
      <div style={{ fontSize: 11, color: 'var(--overlay1)', marginBottom: 4, textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 600 }}>{label}</div>
      <input value={value} onChange={e => onChange(e.target.value)} type={type} placeholder={placeholder}
        style={{
          width: '100%', background: 'var(--base)', border: '1px solid var(--surface1)',
          borderRadius: 4, padding: '6px 10px', height: 30,
          color: 'var(--text)', fontFamily: mono ? 'var(--font-mono)' : 'inherit', fontSize: 12, outline: 'none',
        }} />
    </label>
  );
}

Object.assign(window, {
  TableDataView, TableStructureView, ERDiagramView,
  ConnectionManagerView, NewConnectionDialog,
});
