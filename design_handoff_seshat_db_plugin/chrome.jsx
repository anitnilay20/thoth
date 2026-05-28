/* Seshat — chrome layer: TitleBar, ActivityBar, TabBar, StatusBar.
   Each component takes pure props; styling uses thoth-tokens CSS vars. */

const { useState, useEffect, useRef, useMemo, Fragment, useCallback } = React;

// ───────────────────────────────────────────────────────── Phosphor icon
function Ph({ name, size = 16, color, style, className = '', spin }) {
  return <i className={`ph ph-${name} ${spin ? 'seshat-spin' : ''} ${className}`}
    style={{ fontSize: size, color, lineHeight: 1, display: 'inline-flex', ...style }} />;
}

// Engine glyph: small rounded chip with 2 letters + engine dot
function EngineGlyph({ engine, size = 18 }) {
  const e = SESHAT.ENGINES[engine];
  if (!e) return null;
  return (
    <span style={{
      width: size, height: size, borderRadius: 4,
      background: 'var(--surface0)',
      border: `1px solid var(--surface1)`,
      display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      position: 'relative', flexShrink: 0,
    }}>
      <span style={{
        fontFamily: 'var(--font-mono)', fontSize: Math.round(size * 0.5),
        fontWeight: 700, color: e.dot, letterSpacing: '-0.05em',
      }}>{e.short}</span>
    </span>
  );
}

// Status dot — connection state
function StatusDot({ state, size = 8 }) {
  const color = state === 'connected' ? 'var(--success)' : state === 'connecting' ? 'var(--warning)' : 'var(--text-disabled)';
  return (
    <span className={state === 'connecting' ? 'seshat-pulse' : ''} style={{
      width: size, height: size, borderRadius: '50%', background: color,
      boxShadow: state === 'connected' ? '0 0 6px rgba(166, 227, 161, 0.55)' : 'none',
      flexShrink: 0,
    }} />
  );
}

// ───────────────────────────────────────────────────────── TitleBar
// Thoth host chrome — feather wordmark on the left, plugin breadcrumb in the
// middle (shows Seshat is the currently-active plugin), and Thoth's standard
// minimize / maximize / close glyphs on the right (NO macOS traffic lights —
// this is a host-app chrome, not a standalone window).
function TitleBar({ activeConn, pluginName, onOpenPalette, onSwitchConn }) {
  return (
    <div style={{
      height: 'var(--h-titlebar)', background: 'var(--crust)',
      borderBottom: '1px solid var(--mantle)',
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      padding: '0 8px 0 12px', userSelect: 'none', flexShrink: 0,
    }}>
      {/* Thoth wordmark */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
        <img src="assets/thoth_icon_256.png" width="16" height="16" alt="" style={{ borderRadius: 3 }} />
        <span style={{ fontSize: 13, color: 'var(--text)', fontWeight: 600 }}>Thoth</span>
        <span style={{ color: 'var(--overlay1)' }}>›</span>
        {/* Plugin chip — makes the host/plugin relationship explicit */}
        <span style={{
          display: 'inline-flex', alignItems: 'center', gap: 6,
          padding: '2px 8px', borderRadius: 4,
          background: 'linear-gradient(135deg, rgba(203,166,247,0.18), rgba(180,190,254,0.10))',
          border: '1px solid rgba(203,166,247,0.35)',
        }}>
          <Ph name="puzzle-piece" size={10} color="var(--primary)" />
          <span style={{ fontSize: 11, color: 'var(--primary)', fontWeight: 600 }}>{pluginName}</span>
          <span style={{ fontSize: 10, color: 'var(--overlay1)', textTransform: 'uppercase', letterSpacing: '0.06em' }}>plugin</span>
        </span>
        {activeConn && (
          <>
            <span style={{ color: 'var(--overlay1)' }}>·</span>
            <button onClick={onSwitchConn} style={{
              background: 'var(--surface0)', border: '1px solid var(--surface1)',
              color: 'var(--text)', fontSize: 12, padding: '3px 8px', borderRadius: 4,
              display: 'inline-flex', alignItems: 'center', gap: 6, cursor: 'pointer',
              fontFamily: 'inherit',
            }}>
              <StatusDot state={activeConn.status} />
              <EngineGlyph engine={activeConn.engine} size={14} />
              <span style={{ fontWeight: 500 }}>{activeConn.name}</span>
              <Ph name="caret-down" size={10} color="var(--overlay1)" />
            </button>
          </>
        )}
      </div>

      {/* Right: palette hint + Thoth window controls */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <button onClick={onOpenPalette} style={{
          background: 'var(--surface0)', border: '1px solid var(--surface1)',
          color: 'var(--overlay2)', fontSize: 11, padding: '3px 8px', borderRadius: 4,
          display: 'inline-flex', alignItems: 'center', gap: 8, cursor: 'pointer',
          fontFamily: 'inherit',
        }}>
          <Ph name="magnifying-glass" size={11} color="var(--overlay1)" />
          <span>Go to anything</span>
          <span className="seshat-kbd">⌘K</span>
        </button>
        <span style={{ width: 1, height: 18, background: 'var(--surface0)' }} />
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 14, color: 'var(--overlay1)' }}>
          <Ph name="minus" size={13} color="currentColor" />
          <Ph name="square" size={11} color="currentColor" />
          <Ph name="x" size={13} color="currentColor" />
        </span>
      </div>
    </div>
  );
}

// ───────────────────────────────────────────────────────── ActivityBar
// Side rail — a *single* plugin icon, not the whole sub-nav. The plugin's
// internal sections (Schema, Connections, Saved, History, ER, Import) live
// one level in, inside the sidebar pane itself, so this rail stays scannable
// even with several plugins installed.
const THOTH_RAIL = [
  { id: 'thoth-recent',   icon: 'folders',         label: 'Recent files (Thoth)' },
  { id: 'thoth-clip',     icon: 'clipboard-text',  label: 'Clipboard (Thoth)' },
  { id: 'thoth-search',   icon: 'magnifying-glass',label: 'Search files (Thoth)' },
];
// The plugin sections — exposed via SESHAT_RAIL so the inner sub-nav can
// reuse the same list. Order is the user-perceived priority.
const SESHAT_RAIL = [
  { id: 'schema',      icon: 'tree-structure',          label: 'Schema browser' },
  { id: 'connections', icon: 'plugs-connected',         label: 'Connections' },
  { id: 'saved',       icon: 'bookmark-simple',         label: 'Saved queries' },
  { id: 'history',     icon: 'clock-counter-clockwise', label: 'Query history' },
  { id: 'er',          icon: 'graph',                   label: 'ER diagram' },
  { id: 'import',      icon: 'database',                label: 'Import / Export' },
];
const ACTIVITY = SESHAT_RAIL;
// Is this id one of the plugin's sub-sections?
function isPluginPane(id) { return SESHAT_RAIL.some(s => s.id === id); }

function ActivityBar({ active, onSelect, pluginName }) {
  // Rail collapses the plugin to ONE icon. We treat any plugin sub-pane as
  // "the plugin rail icon is active". Clicking it toggles the sidebar AND
  // returns to the default plugin pane ('schema').
  const pluginActive = isPluginPane(active);
  return (
    <div style={{
      width: 52, background: 'var(--crust)', borderRight: '1px solid var(--mantle)',
      display: 'flex', flexDirection: 'column', alignItems: 'stretch',
      flexShrink: 0,
    }}>
      <RailSection>
        {THOTH_RAIL.map(item => (
          <RailButton key={item.id} item={item} active={active === item.id} dim
            onClick={() => onSelect(item.id)} />
        ))}
      </RailSection>
      <div style={{
        margin: '6px 8px 4px', padding: '4px 0',
        borderTop: '1px solid var(--surface0)',
        display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2,
      }}>
        <span style={{
          fontSize: 8, fontWeight: 700, color: 'var(--overlay1)',
          textTransform: 'uppercase', letterSpacing: '0.08em',
        }}>Plugins</span>
      </div>
      {/* Single plugin button — purple accent ring when active */}
      <button onClick={() => onSelect(pluginActive ? null : 'schema')}
        title={`${pluginName} — Database plugin`}
        style={{
          margin: '0 8px', height: 40,
          background: pluginActive ? 'linear-gradient(135deg, rgba(203,166,247,0.20), rgba(180,190,254,0.10))' : 'transparent',
          border: pluginActive ? '1px solid rgba(203,166,247,0.45)' : '1px solid transparent',
          borderRadius: 6, cursor: 'pointer', position: 'relative',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          color: pluginActive ? 'var(--primary)' : 'var(--overlay1)',
          transition: 'color 100ms, background 100ms, border-color 100ms',
        }}
        onMouseEnter={(e) => { if (!pluginActive) e.currentTarget.style.color = 'var(--text)'; }}
        onMouseLeave={(e) => { if (!pluginActive) e.currentTarget.style.color = 'var(--overlay1)'; }}>
        {pluginActive && (
          <span style={{ position: 'absolute', left: -8, top: 6, bottom: 6, width: 2, background: 'var(--primary)', borderRadius: 1 }} />
        )}
        <Ph name="database" size={20} color="currentColor" />
      </button>
      <div style={{ textAlign: 'center', fontSize: 8, color: pluginActive ? 'var(--primary)' : 'var(--text-disabled)',
                    fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.08em', marginTop: 4 }}>
        {pluginName}
      </div>
      <span style={{ flex: 1 }} />
      <div style={{
        padding: '8px 0 10px', textAlign: 'center',
        fontSize: 9, color: 'var(--text-disabled)',
        fontFamily: 'var(--font-mono)', borderTop: '1px solid var(--surface0)',
      }}>
        v1.0.0
      </div>
      <button title="Settings" style={{
        width: '100%', height: 36, background: 'transparent', border: 0, cursor: 'pointer',
        color: 'var(--overlay1)', display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}><Ph name="gear" size={18} color="currentColor" /></button>
    </div>
  );
}

function RailSection({ label, children }) {
  return (
    <div style={{ padding: '6px 0 2px', display: 'flex', flexDirection: 'column', alignItems: 'stretch' }}>
      {children}
    </div>
  );
}

function RailButton({ item, active, onClick, dim }) {
  const [h, setH] = React.useState(false);
  const color = active ? 'var(--primary)' : dim
    ? (h ? 'var(--overlay2)' : 'var(--text-disabled)')
    : (h ? 'var(--text)' : 'var(--overlay1)');
  return (
    <button onClick={onClick} title={item.label}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        width: '100%', height: 36, position: 'relative',
        background: 'transparent', border: 0, cursor: 'pointer',
        color, display: 'flex', alignItems: 'center', justifyContent: 'center',
        transition: 'color 100ms',
      }}>
      {active && (
        <span style={{ position: 'absolute', left: 0, top: 6, bottom: 6, width: 2, background: 'var(--primary)', borderRadius: 1 }} />
      )}
      <Ph name={item.icon} size={dim ? 16 : 18} color="currentColor" />
    </button>
  );
}

// ───────────────────────────────────────────────────────── PluginSubNav
// One of four layouts for picking among the plugin's 6 sub-sections.
// Driven by the `subnav` tweak so the user can compare side-by-side.
function PluginSubNav({ mode, active, onSelect, sections }) {
  if (mode === 'sub-rail') return <SubRail active={active} onSelect={onSelect} sections={sections} />;
  if (mode === 'top-tabs') return <TopTabs active={active} onSelect={onSelect} sections={sections} />;
  if (mode === 'dropdown') return <SubDropdown active={active} onSelect={onSelect} sections={sections} />;
  if (mode === 'minimal')  return null; // tabs/palette only
  return null;
}

// (A) Narrow inner rail — 36px wide column of icons inside the sidebar pane.
function SubRail({ active, onSelect, sections }) {
  return (
    <div style={{
      width: 40, background: 'var(--crust)', borderRight: '1px solid var(--surface0)',
      display: 'flex', flexDirection: 'column', alignItems: 'stretch',
      flexShrink: 0, paddingTop: 4,
    }}>
      {sections.map(s => {
        const isActive = active === s.id;
        return (
          <button key={s.id} onClick={() => onSelect(s.id)} title={s.label}
            style={{
              width: '100%', height: 36, position: 'relative',
              background: isActive ? 'var(--surface0)' : 'transparent',
              border: 0, cursor: 'pointer',
              color: isActive ? 'var(--primary)' : 'var(--overlay1)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              transition: 'color 100ms, background 100ms',
            }}
            onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.color = 'var(--text)'; }}
            onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.color = 'var(--overlay1)'; }}>
            {isActive && (
              <span style={{ position: 'absolute', left: 0, top: 6, bottom: 6, width: 2, background: 'var(--primary)', borderRadius: 1 }} />
            )}
            <Ph name={s.icon} size={15} color="currentColor" />
          </button>
        );
      })}
    </div>
  );
}

// (B) Top tabs — horizontal compact pills at the top of the pane.
function TopTabs({ active, onSelect, sections }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'stretch', flexShrink: 0,
      borderBottom: '1px solid var(--surface0)', background: 'var(--crust)',
      overflowX: 'auto',
    }}>
      {sections.map(s => {
        const isActive = active === s.id;
        return (
          <button key={s.id} onClick={() => onSelect(s.id)}
            style={{
              flex: 1, minWidth: 0, height: 32, padding: '0 6px',
              background: isActive ? 'var(--base)' : 'transparent',
              border: 0, borderRight: '1px solid var(--surface0)',
              color: isActive ? 'var(--primary)' : 'var(--overlay1)',
              cursor: 'pointer', position: 'relative',
              display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 4,
              fontSize: 10, fontFamily: 'inherit',
            }}
            onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.color = 'var(--text)'; }}
            onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.color = 'var(--overlay1)'; }}
            title={s.label}>
            <Ph name={s.icon} size={12} color="currentColor" />
            {isActive && <span style={{
              position: 'absolute', left: 0, right: 0, top: 0, height: 2, background: 'var(--primary)',
            }} />}
          </button>
        );
      })}
    </div>
  );
}

// (C) Dropdown — single "View: Schema browser ▾" with a menu.
function SubDropdown({ active, onSelect, sections }) {
  const [open, setOpen] = React.useState(false);
  const cur = sections.find(s => s.id === active) || sections[0];
  return (
    <div style={{
      borderBottom: '1px solid var(--surface0)', padding: 8, position: 'relative', flexShrink: 0,
    }}>
      <button onClick={() => setOpen(v => !v)} style={{
        width: '100%', height: 32, padding: '0 10px',
        background: 'var(--crust)', border: '1px solid var(--surface0)',
        borderRadius: 4, cursor: 'pointer', color: 'var(--text)',
        display: 'flex', alignItems: 'center', gap: 8,
        fontSize: 12, fontFamily: 'inherit',
      }}>
        <Ph name={cur.icon} size={12} color="var(--primary)" />
        <span style={{ fontWeight: 500 }}>{cur.label}</span>
        <span style={{ flex: 1 }} />
        <Ph name="caret-down" size={10} color="var(--overlay1)" />
      </button>
      {open && (
        <>
          <div onClick={() => setOpen(false)} style={{ position: 'fixed', inset: 0, zIndex: 30 }} />
          <div style={{
            position: 'absolute', top: 44, left: 8, right: 8, zIndex: 31,
            background: 'var(--mantle)', border: '1px solid var(--surface1)',
            borderRadius: 6, boxShadow: 'var(--shadow-menu)', overflow: 'hidden',
          }}>
            {sections.map(s => {
              const isActive = s.id === active;
              return (
                <button key={s.id} onClick={() => { onSelect(s.id); setOpen(false); }}
                  style={{
                    width: '100%', padding: '8px 12px', display: 'flex', alignItems: 'center', gap: 10,
                    background: isActive ? 'var(--surface0)' : 'transparent',
                    border: 0, cursor: 'pointer', color: 'var(--text)',
                    fontSize: 12, fontFamily: 'inherit',
                  }}
                  onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.background = 'var(--surface0)'; }}
                  onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.background = 'transparent'; }}>
                  <Ph name={s.icon} size={12} color="var(--primary)" />
                  <span style={{ flex: 1, textAlign: 'left' }}>{s.label}</span>
                  {isActive && <Ph name="check" size={11} color="var(--success)" />}
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

Object.assign(window, { PluginSubNav, SubRail, TopTabs, SubDropdown });

// ───────────────────────────────────────────────────────── TabBar
function TabIcon({ tab }) {
  if (tab.kind === 'sql')        return <Ph name="terminal-window" size={12} color="var(--info)" />;
  if (tab.kind === 'table')      return <Ph name="table" size={12} color="var(--syn-string)" />;
  if (tab.kind === 'structure')  return <Ph name="list-numbers" size={12} color="var(--overlay2)" />;
  if (tab.kind === 'er')         return <Ph name="graph" size={12} color="var(--primary)" />;
  if (tab.kind === 'connections')return <Ph name="plugs-connected" size={12} color="var(--secondary)" />;
  return <Ph name="file" size={12} color="var(--overlay1)" />;
}
function TabBar({ tabs, activeId, onActivate, onClose, onAddSql }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'stretch', background: 'var(--crust)',
      borderBottom: '1px solid var(--surface0)', height: 34,
      paddingLeft: 0, overflowX: 'auto', overflowY: 'hidden', flexShrink: 0,
    }}>
      {tabs.map(t => {
        const isActive = t.id === activeId;
        return (
          <div key={t.id} onClick={() => onActivate(t.id)}
            style={{
              display: 'flex', alignItems: 'center', gap: 8, padding: '0 12px',
              cursor: 'pointer', position: 'relative',
              background: isActive ? 'var(--base)' : 'transparent',
              color: isActive ? 'var(--text)' : 'var(--overlay1)',
              borderRight: '1px solid var(--surface0)',
              fontSize: 12, fontWeight: 500, minWidth: 0,
              whiteSpace: 'nowrap',
            }}
            onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.color = 'var(--text)'; }}
            onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.color = 'var(--overlay1)'; }}>
            <TabIcon tab={t} />
            <span>{t.title}</span>
            {t.dirty && <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--warning)' }} />}
            <span onClick={(e) => { e.stopPropagation(); onClose(t.id); }}
              style={{ marginLeft: 4, opacity: 0.5, cursor: 'pointer', display: 'inline-flex',
                       padding: 2, borderRadius: 2 }}
              onMouseEnter={(e) => { e.currentTarget.style.background = 'var(--surface0)'; e.currentTarget.style.opacity = '1'; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.opacity = '0.5'; }}>
              <Ph name="x" size={11} color="currentColor" />
            </span>
            {isActive && (
              <span style={{ position: 'absolute', left: 0, right: 0, top: 0, height: 1, background: 'var(--primary)' }} />
            )}
          </div>
        );
      })}
      <button onClick={onAddSql} title="New SQL editor (⌘T)" style={{
        background: 'transparent', border: 0, color: 'var(--overlay1)',
        cursor: 'pointer', padding: '0 12px', display: 'flex', alignItems: 'center',
      }}>
        <Ph name="plus" size={12} color="currentColor" />
      </button>
      <span style={{ flex: 1 }} />
    </div>
  );
}

// ───────────────────────────────────────────────────────── StatusBar
function StatusBar({ activeConn, activeTab, queryMs, rowCount, selection, onToggleResults, showResults }) {
  const sep = <span style={{ color: 'var(--overlay1)', opacity: 0.35 }}>│</span>;
  return (
    <div style={{
      height: 'var(--h-status)', background: 'var(--crust)',
      borderTop: '1px solid var(--surface0)', display: 'flex', alignItems: 'center',
      gap: 12, padding: '0 12px', fontSize: 12, color: 'var(--text)', flexShrink: 0,
    }}>
      {activeConn ? (
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
          <StatusDot state={activeConn.status} />
          <span style={{ color: 'var(--text)' }}>{activeConn.name}</span>
          <span style={{ color: 'var(--overlay1)' }}>{activeConn.host}{activeConn.port ? ':' + activeConn.port : ''}</span>
        </span>
      ) : <span style={{ color: 'var(--overlay1)' }}>No connection</span>}
      {sep}
      {activeConn && (
        <>
          <span style={{ color: 'var(--overlay1)' }}>{activeConn.env}</span>
          {sep}
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
            <Ph name="lightning" size={11} color="var(--warning)" />
            <span style={{ color: 'var(--overlay1)' }}>{activeConn.latency != null ? `${activeConn.latency} ms` : '—'}</span>
          </span>
          {sep}
        </>
      )}
      {queryMs != null && (
        <>
          <span style={{ color: 'var(--success)' }}>{queryMs} ms</span>
          {sep}
          <span style={{ color: 'var(--overlay1)' }}>{rowCount.toLocaleString()} rows</span>
          {sep}
        </>
      )}
      {selection && (
        <>
          <span style={{ color: 'var(--info)' }}>{selection}</span>
          {sep}
        </>
      )}
      <span style={{ flex: 1 }} />
      <span style={{ color: 'var(--overlay1)' }}>UTF-8</span>
      {sep}
      <span style={{ color: 'var(--overlay1)' }}>LF</span>
      {sep}
      <button onClick={onToggleResults} style={{
        background: 'transparent', border: 0, color: showResults ? 'var(--text)' : 'var(--overlay1)',
        cursor: 'pointer', fontSize: 12, fontFamily: 'inherit',
        display: 'inline-flex', alignItems: 'center', gap: 4, padding: 0,
      }}>
        <Ph name={showResults ? 'caret-down' : 'caret-up'} size={10} color="currentColor" />
        Results
      </button>
    </div>
  );
}

// Make available globally
Object.assign(window, { Ph, EngineGlyph, StatusDot, TitleBar, ActivityBar, TabBar, StatusBar, ACTIVITY, SESHAT_RAIL, THOTH_RAIL, isPluginPane });
