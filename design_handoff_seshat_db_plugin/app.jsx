/* Seshat — root app.
   State: active connection, tabs (multi-tab workspace), active pane, tweaks, palette. */

// ─────────────────────────────────────────── Default tweaks (persisted)
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "dark",
  "density": "compact",
  "showResults": true,
  "sidebarWidth": 280,
  "sidebarCollapsed": false,
  "engine": "postgres",
  "layout": "vscode",
  "subnav": "sub-rail",
  "name": "Seshat"
}/*EDITMODE-END*/;

// Alternative product names (for tweak)
const NAME_OPTIONS = ['Seshat', 'Apis', 'Ibis', 'Scribe', 'Papyrus', 'Codex'];

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);

  // Apply theme + density to <html> so CSS vars cascade
  React.useEffect(() => {
    document.documentElement.setAttribute('data-theme', t.theme === 'light' ? 'light' : 'dark');
    document.documentElement.setAttribute('data-density', t.density);
  }, [t.theme, t.density]);

  // ── connection state
  const [activeConnId, setActiveConnId] = React.useState('prod-pg');
  const activeConn = SESHAT.CONNECTIONS.find(c => c.id === activeConnId);

  // ── activity pane
  const [pane, setPane] = React.useState('schema'); // null = collapsed
  const sidebarOpen = pane !== null && !t.sidebarCollapsed;

  // ── tabs
  const [tabs, setTabs] = React.useState([
    { id: 'tab-1', kind: 'sql',       title: 'top-orgs.sql',   query: SESHAT.HERO_QUERY, hasRun: true,  dirty: false },
    { id: 'tab-2', kind: 'table',     title: 'public.users',   conn: 'prod-pg', schema: 'public', tableName: 'users' },
    { id: 'tab-3', kind: 'structure', title: 'users · structure', conn: 'prod-pg', schema: 'public', tableName: 'users' },
    { id: 'tab-4', kind: 'er',        title: 'ER · prod-postgres', conn: 'prod-pg' },
  ]);
  const [activeTabId, setActiveTabId] = React.useState('tab-1');
  const activeTab = tabs.find(t => t.id === activeTabId);
  let tabCounter = React.useRef(5);

  // ── execution state per tab
  const [executing, setExecuting] = React.useState(false);
  const [queryMs, setQueryMs] = React.useState(142);

  // ── overlays
  const [paletteOpen, setPaletteOpen] = React.useState(false);
  const [aiOpen, setAiOpen] = React.useState(false);
  const [newConnOpen, setNewConnOpen] = React.useState(false);

  // ── derive table for table/structure tabs
  const findTable = (tab) => {
    if (!tab || !tab.conn) return null;
    const schemas = SESHAT.SCHEMAS[tab.conn] || [];
    const s = schemas.find(s => s.name === tab.schema);
    return s ? s.tables.find(t => t.name === tab.tableName) : null;
  };

  // ── handlers
  const openTab = (newTab) => {
    setTabs(prev => {
      const existing = prev.find(p => p.id === newTab.id);
      if (existing) return prev;
      return [...prev, newTab];
    });
    setActiveTabId(newTab.id);
  };
  const closeTab = (id) => {
    setTabs(prev => {
      const idx = prev.findIndex(p => p.id === id);
      const next = prev.filter(p => p.id !== id);
      if (id === activeTabId && next.length) {
        setActiveTabId(next[Math.max(0, idx - 1)].id);
      }
      return next;
    });
  };
  const newSqlTab = () => {
    const id = `tab-${tabCounter.current++}`;
    openTab({ id, kind: 'sql', title: `query-${tabCounter.current - 1}.sql`, query: '', hasRun: false, dirty: false });
  };
  const openTable = (conn, schema, table) => {
    const id = `tbl-${conn.id}-${schema}-${table.name}`;
    openTab({ id, kind: 'table', title: `${schema}.${table.name}`, conn: conn.id, schema, tableName: table.name });
  };
  const openStructure = (conn, schema, table) => {
    const id = `str-${conn.id}-${schema}-${table.name}`;
    openTab({ id, kind: 'structure', title: `${table.name} · structure`, conn: conn.id, schema, tableName: table.name });
  };

  const runQuery = () => {
    if (!activeTab || activeTab.kind !== 'sql') return;
    setExecuting(true);
    setQueryMs(null);
    setTimeout(() => {
      setExecuting(false);
      setQueryMs(80 + Math.floor(Math.random() * 200));
      setTabs(prev => prev.map(t => t.id === activeTabId ? { ...t, hasRun: true } : t));
    }, 900);
  };

  // ── keyboard shortcuts
  React.useEffect(() => {
    const onKey = (e) => {
      const meta = e.metaKey || e.ctrlKey;
      if (meta && e.key === 'k') { e.preventDefault(); setPaletteOpen(true); }
      else if (meta && e.key === 'p' && !e.shiftKey) { e.preventDefault(); setPaletteOpen(true); }
      else if (meta && e.key === 't') { e.preventDefault(); newSqlTab(); }
      else if (meta && e.key === 'n' && !e.shiftKey) { e.preventDefault(); setNewConnOpen(true); }
      else if (meta && e.key === 'Enter' && activeTab?.kind === 'sql') { e.preventDefault(); runQuery(); }
      else if (meta && e.shiftKey && (e.key === 'T' || e.key === 't')) { e.preventDefault(); setTweak('theme', t.theme === 'dark' ? 'light' : 'dark'); }
      else if (e.key === 'Escape') {
        if (paletteOpen) setPaletteOpen(false);
        else if (aiOpen) setAiOpen(false);
        else if (newConnOpen) setNewConnOpen(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [activeTab, paletteOpen, aiOpen, newConnOpen, t.theme]);

  // ── palette commands
  const onCmd = (c) => {
    if (c.type === 'switch-conn')   setActiveConnId(c.conn.id);
    else if (c.type === 'new-conn') setNewConnOpen(true);
    else if (c.type === 'new-sql')  newSqlTab();
    else if (c.type === 'open-table') openTable(c.conn, c.schema, c.table);
    else if (c.type === 'open-er')  openTab({ id: `er-${activeConn?.id}`, kind: 'er', title: `ER · ${activeConn?.name}`, conn: activeConn?.id });
    else if (c.type === 'open-saved') {
      const id = `q-${c.q.id}`;
      openTab({ id, kind: 'sql', title: c.q.name + '.sql', query: SESHAT.HERO_QUERY, hasRun: false, dirty: false });
    }
    else if (c.type === 'run')      runQuery();
    else if (c.type === 'ai')       setAiOpen(true);
    else if (c.type === 'toggle-theme') setTweak('theme', t.theme === 'dark' ? 'light' : 'dark');
    else if (c.type === 'explain')  { /* would switch results tab to explain */ }
  };

  // ── update tab query
  const updateTabQuery = (q) => {
    setTabs(prev => prev.map(p => p.id === activeTabId ? { ...p, query: q, dirty: true } : p));
  };

  // ── currently shown results
  const showResults = t.showResults && activeTab?.kind === 'sql' && activeTab.hasRun;
  const heroColumns = SESHAT.HERO_COLUMNS;
  const heroRows = SESHAT.HERO_ROWS;

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      <TitleBar activeConn={activeConn} pluginName={t.name}
        onOpenPalette={() => setPaletteOpen(true)}
        onSwitchConn={() => { setPane('connections'); setPaletteOpen(true); }} />
      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
        <ActivityBar active={pane} onSelect={setPane} pluginName={t.name} />
        {sidebarOpen && (
          <div style={{
            width: t.sidebarWidth, background: 'var(--mantle)',
            borderRight: '1px solid var(--surface0)', display: 'flex',
            flexDirection: t.subnav === 'sub-rail' ? 'row' : 'column',
            flexShrink: 0, minWidth: 0,
          }}>
            {pane && pane.startsWith('thoth-') && <ThothHostPane pane={pane} onReturn={() => setPane('schema')} />}
            {isPluginPane(pane) && (
              <>
                {/* Plugin sub-nav: A (sub-rail / horizontal) renders aside;
                    B (top-tabs) / C (dropdown) render above; D (minimal) renders nothing */}
                {t.subnav === 'sub-rail' && (
                  <PluginSubNav mode={t.subnav} active={pane} onSelect={setPane} sections={SESHAT_RAIL} />
                )}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, minWidth: 0 }}>
                  {(t.subnav === 'top-tabs' || t.subnav === 'dropdown') && (
                    <PluginSubNav mode={t.subnav} active={pane} onSelect={setPane} sections={SESHAT_RAIL} />
                  )}
                  {pane === 'connections' && <ConnectionsPane active={activeConn} onPick={c => setActiveConnId(c.id)} onNew={() => setNewConnOpen(true)} />}
                  {pane === 'schema'      && <SchemaPane conn={activeConn} onOpenTable={openTable} onOpenStructure={openStructure} onAskAI={() => setAiOpen(true)} />}
                  {pane === 'saved'       && <SavedPane onOpenQuery={(q) => onCmd({ type: 'open-saved', q })} />}
                  {pane === 'history'     && <HistoryPane onOpenQuery={(h) => {
                    const id = `tab-${tabCounter.current++}`;
                    openTab({ id, kind: 'sql', title: `history-${tabCounter.current - 1}.sql`, query: h.query, hasRun: false });
                  }} />}
                  {pane === 'er'          && <ERPane onOpen={() => openTab({ id: `er-${activeConn?.id}`, kind: 'er', title: `ER · ${activeConn?.name}`, conn: activeConn?.id })} conn={activeConn} />}
                  {pane === 'import'      && <ImportPane />}
                </div>
              </>
            )}
          </div>
        )}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, minWidth: 0 }}>
          <TabBar tabs={tabs} activeId={activeTabId}
            onActivate={setActiveTabId} onClose={closeTab} onAddSql={newSqlTab} />
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, position: 'relative' }}>
            {activeTab?.kind === 'sql' && (
              <>
                <div style={{ flex: showResults ? '1 1 50%' : '1 1 100%', display: 'flex', minHeight: 0 }}>
                  <SqlEditor value={activeTab.query} onChange={updateTabQuery}
                    onRun={runQuery} onExplain={runQuery} onAskAI={() => setAiOpen(true)}
                    conn={activeConn} executing={executing} />
                </div>
                {showResults && (
                  <>
                    <ResultsSplitter />
                    <div style={{ flex: '1 1 50%', display: 'flex', minHeight: 0, borderTop: '1px solid var(--surface0)' }}>
                      <ResultsPanel columns={heroColumns} rows={heroRows}
                        queryMs={queryMs} executing={executing} />
                    </div>
                  </>
                )}
              </>
            )}
            {activeTab?.kind === 'table' && (
              <TableDataView conn={activeConn} schema={activeTab.schema} table={findTable(activeTab) || { name: activeTab.tableName, rows: 0 }} />
            )}
            {activeTab?.kind === 'structure' && (
              <TableStructureView conn={activeConn} schema={activeTab.schema} table={findTable(activeTab) || { name: activeTab.tableName, rows: 0, cols: [] }} />
            )}
            {activeTab?.kind === 'er' && (
              <ERDiagramView conn={activeConn} />
            )}
            {activeTab?.kind === 'connections' && (
              <ConnectionManagerView onPick={(c) => setActiveConnId(c.id)} onNew={() => setNewConnOpen(true)} />
            )}
            {!activeTab && (
              <ConnectionManagerView onPick={(c) => setActiveConnId(c.id)} onNew={() => setNewConnOpen(true)} />
            )}
            <AiPromptOverlay open={aiOpen} onClose={() => setAiOpen(false)}
              onAccept={(sql) => { updateTabQuery(sql); setAiOpen(false); }}
              schema={activeConn ? `${activeConn.name} · ${activeConn.db}` : null} />
          </div>
          <StatusBar activeConn={activeConn} activeTab={activeTab}
            queryMs={showResults ? queryMs : null}
            rowCount={heroRows.length}
            selection={activeTab?.kind === 'sql' && showResults ? null : null}
            onToggleResults={() => setTweak('showResults', !t.showResults)}
            showResults={t.showResults} />
        </div>
      </div>

      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} onCmd={onCmd} conn={activeConn} />
      <NewConnectionDialog open={newConnOpen} onClose={() => setNewConnOpen(false)} onConnect={() => setNewConnOpen(false)} />

      <Tweaks t={t} setTweak={setTweak} />
    </div>
  );
}

// ThothHostPane — what the sidebar shows when a Thoth (host) rail icon is
// selected. Makes the host/plugin relationship feel real: the user can see
// they've stepped "out" of the plugin into Thoth's own features.
function ThothHostPane({ pane, onReturn }) {
  const labels = {
    'thoth-recent': { label: 'Recent files', icon: 'folders', body: 'Thoth shows recently-opened JSON / NDJSON files here.' },
    'thoth-clip':   { label: 'Clipboard',    icon: 'clipboard-text', body: 'Recently-copied JSON paths and values from the host.' },
    'thoth-search': { label: 'Search files', icon: 'magnifying-glass', body: 'Find values and keys across the open JSON file.' },
  }[pane] || { label: '', icon: 'file', body: '' };

  // Mock recent JSON files as if the host had them open
  const recent = ['analytics-export.json', 'users.ndjson', 'event-stream.json', 'config.json'];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <div style={{
        padding: '10px 14px 8px', borderBottom: '1px solid var(--surface0)',
        display: 'flex', alignItems: 'center', gap: 8,
      }}>
        <span style={{
          width: 18, height: 18, borderRadius: 4, background: 'var(--surface0)',
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <img src="assets/thoth_icon_256.png" width="12" height="12" alt="" style={{ borderRadius: 2 }} />
        </span>
        <span style={{
          fontSize: 10, color: 'var(--overlay1)',
          textTransform: 'uppercase', letterSpacing: '0.08em', fontWeight: 700,
        }}>Thoth host</span>
        <span style={{ flex: 1 }} />
        <button onClick={onReturn} title="Return to plugin" style={{
          background: 'transparent', border: 0, color: 'var(--primary)',
          cursor: 'pointer', padding: '2px 6px', borderRadius: 3,
          display: 'inline-flex', alignItems: 'center', gap: 4, fontSize: 10, fontFamily: 'inherit',
        }}>
          <Ph name="puzzle-piece" size={9} color="var(--primary)" />
          Back to plugin
        </button>
      </div>
      <div style={{ padding: '10px 14px 6px' }}>
        <span className="t-section">{labels.label}</span>
      </div>
      <div style={{ padding: '0 14px 12px', fontSize: 12, color: 'var(--overlay1)', lineHeight: 1.5 }}>
        {labels.body}
      </div>
      {pane === 'thoth-recent' && (
        <div style={{ overflowY: 'auto', flex: 1 }}>
          {recent.map(f => (
            <div key={f} style={{
              padding: '6px 14px', display: 'flex', alignItems: 'center', gap: 8,
              fontSize: 12, color: 'var(--text)',
            }}
              onMouseEnter={(e) => e.currentTarget.style.background = 'var(--sidebar-hover)'}
              onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}>
              <Ph name="file-text" size={11} color="var(--syn-string)" />
              <span>{f}</span>
            </div>
          ))}
        </div>
      )}
      <div style={{ flex: 1 }} />
      <div style={{
        margin: 14, padding: 12, background: 'var(--surface0)',
        borderRadius: 6, border: '1px dashed var(--surface2)',
        display: 'flex', alignItems: 'flex-start', gap: 10,
      }}>
        <Ph name="info" size={14} color="var(--info)" />
        <div style={{ fontSize: 11, color: 'var(--overlay2)', lineHeight: 1.5 }}>
          You're viewing a <strong style={{ color: 'var(--text)' }}>Thoth host</strong> feature.
          Click any plugin icon below the divider to return to Seshat.
        </div>
      </div>
    </div>
  );
}
function ERPane({ onOpen, conn }) {
  if (!conn) return <Empty icon="graph" title="No active connection" />;
  const nodes = SESHAT.ER_NODES;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <PaneHeader title="Entity-relationships" />
      <div style={{ padding: 14, color: 'var(--overlay1)', fontSize: 12, lineHeight: 1.5 }}>
        Visualize tables and foreign keys for <strong style={{ color: 'var(--text)' }}>{conn.name}</strong>.
      </div>
      <div style={{ padding: '0 14px', display: 'flex', flexDirection: 'column', gap: 6 }}>
        {nodes.map(n => (
          <div key={n.id} style={{
            padding: '6px 10px', background: 'var(--surface0)', borderRadius: 4,
            fontFamily: 'var(--font-mono)', fontSize: 12, display: 'flex', alignItems: 'center', gap: 8,
          }}>
            <Ph name="table" size={10} color="var(--syn-string)" />
            {n.id}
            <span style={{ flex: 1 }} />
            <span style={{ fontSize: 10, color: 'var(--overlay1)' }}>{n.cols.length} cols</span>
          </div>
        ))}
      </div>
      <div style={{ padding: 14 }}>
        <button onClick={onOpen} style={{
          width: '100%', background: 'var(--primary)', border: 0, color: 'var(--crust)',
          padding: '8px 12px', borderRadius: 6, cursor: 'pointer',
          fontSize: 12, fontWeight: 600, fontFamily: 'inherit',
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 6,
        }}>
          <Ph name="graph" size={12} color="var(--crust)" /> Open ER diagram
        </button>
      </div>
    </div>
  );
}

function ResultsSplitter() {
  return (
    <div style={{
      height: 4, background: 'var(--surface0)', cursor: 'row-resize', position: 'relative',
    }}>
      <div style={{
        position: 'absolute', left: '50%', top: 1, transform: 'translateX(-50%)',
        width: 30, height: 2, background: 'var(--surface1)', borderRadius: 1,
      }} />
    </div>
  );
}

// ─────────────────────────────────────────── Tweaks panel
function Tweaks({ t, setTweak }) {
  return (
    <TweaksPanel title="Tweaks">
      <TweakSection label="Appearance">
        <TweakRadio label="Theme" value={t.theme}
          options={[{ value: 'dark', label: 'Mocha' }, { value: 'light', label: 'Latte' }]}
          onChange={v => setTweak('theme', v)} />
        <TweakRadio label="Density" value={t.density}
          options={[{ value: 'compact', label: 'Compact' }, { value: 'comfortable', label: 'Comfy' }]}
          onChange={v => setTweak('density', v)} />
      </TweakSection>
      <TweakSection label="Plugin nav">
        <TweakSelect label="Sub-nav layout" value={t.subnav}
          options={[
            { value: 'sub-rail', label: 'A · Inner rail (icons left)' },
            { value: 'top-tabs', label: 'B · Top tab strip' },
            { value: 'dropdown', label: 'C · Dropdown switcher' },
            { value: 'minimal',  label: 'D · Hide (palette only)' },
          ]} onChange={v => setTweak('subnav', v)} />
      </TweakSection>
      <TweakSection label="Layout">
        <TweakSlider label="Sidebar width" value={t.sidebarWidth}
          min={220} max={420} step={10} unit="px"
          onChange={v => setTweak('sidebarWidth', v)} />
        <TweakToggle label="Collapse sidebar" value={t.sidebarCollapsed}
          onChange={v => setTweak('sidebarCollapsed', v)} />
        <TweakToggle label="Show results panel" value={t.showResults}
          onChange={v => setTweak('showResults', v)} />
        <TweakSelect label="Layout preset" value={t.layout}
          options={[
            { value: 'vscode',    label: 'VS Code (activity bar)' },
            { value: 'dbeaver',   label: 'DBeaver (tree-left)' },
            { value: 'tableplus', label: 'TablePlus (minimal)' },
          ]} onChange={v => setTweak('layout', v)} />
      </TweakSection>
      <TweakSection label="Brand">
        <TweakSelect label="Plugin name" value={t.name}
          options={NAME_OPTIONS.map(n => ({ value: n, label: n }))}
          onChange={v => {
            setTweak('name', v);
            document.title = `${v} — Database plugin for Thoth`;
          }} />
        <TweakSelect label="Default engine" value={t.engine}
          options={Object.entries(SESHAT.ENGINES).map(([id, e]) => ({ value: id, label: e.label }))}
          onChange={v => setTweak('engine', v)} />
      </TweakSection>
    </TweaksPanel>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
