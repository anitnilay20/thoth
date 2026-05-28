/* App shell — title bar, activity bar (left), root layout, status bar, tweaks. */

const { useState: appUseState, useEffect: appUseEffect } = React;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "dark",
  "activeStyle": "vscode",
  "showStatusbar": true
}/*EDITMODE-END*/;

function ActivityBar({ api, focusedGroupId, onTheme, dark }) {
  const items = [
    { id: 'files',    icon: 'files',          label: 'Open file' },
    { id: 'welcome',  icon: 'house',          label: 'Welcome' },
    { id: 'schema',   icon: 'check-circle',   label: 'Schema Validator' },
    { id: 'diff',     icon: 'git-diff',       label: 'Diff Viewer' },
    { id: 'jsonpath', icon: 'magnifying-glass', label: 'JSONPath' },
    { id: 'settings', icon: 'gear',           label: 'Settings' },
  ];
  const onClick = (id) => {
    if (id === 'files') return; // expand below
    if (id === 'schema') return api.open('plugin', 'schema-validator', focusedGroupId);
    if (id === 'jsonpath') return api.open('plugin', 'jsonpath', focusedGroupId);
    if (id === 'diff') return api.open('plugin', 'diff', focusedGroupId);
    api.open('plugin', id, focusedGroupId);
  };
  return (
    <div style={{
      width: 48, background: 'var(--mantle)', borderRight: '1px solid var(--surface0)',
      display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '8px 0', gap: 4, flexShrink: 0,
    }}>
      {items.map((it) => (
        <RailButton key={it.id} icon={it.icon} title={it.label} onClick={() => onClick(it.id)}>
          {it.id === 'files' && <FilesFlyout api={api} focusedGroupId={focusedGroupId} />}
        </RailButton>
      ))}
      <span style={{ flex: 1 }} />
      <RailButton icon={dark ? 'sun' : 'moon'} title="Toggle theme" onClick={onTheme} />
    </div>
  );
}
function RailButton({ icon, title, onClick, children }) {
  const [h, setH] = appUseState(false);
  return (
    <span
      onClick={onClick}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => setH(false)}
      title={title}
      style={{
        position: 'relative', width: 32, height: 32, display: 'flex',
        alignItems: 'center', justifyContent: 'center', borderRadius: 4,
        cursor: 'pointer',
        background: h ? 'var(--sidebar-hover)' : 'transparent',
        color: h ? 'var(--text)' : 'var(--overlay2)',
      }}
    >
      <i className={`ph ph-${icon}`} style={{ fontSize: 18 }} />
      {h && children}
    </span>
  );
}
function FilesFlyout({ api, focusedGroupId }) {
  const files = Object.keys(window.THOTH_FILES);
  return (
    <div
      onClick={(e) => e.stopPropagation()}
      style={{
        position: 'absolute', left: 40, top: 0, zIndex: 100,
        background: 'var(--surface0)', border: '1px solid var(--surface1)',
        borderRadius: 4, boxShadow: 'var(--shadow-menu)', padding: '4px 0',
        minWidth: 200, fontSize: 'var(--fs-md)', color: 'var(--text)',
      }}
    >
      <div style={{ padding: '4px 12px', color: 'var(--overlay1)', fontSize: 'var(--fs-xs)', textTransform: 'uppercase', letterSpacing: '0.06em', fontWeight: 700 }}>
        Open File
      </div>
      {files.map((f) => (
        <FlyoutItem key={f} onClick={() => api.open('file', f, focusedGroupId)}>
          <i className={`ph ph-${window.THOTH_FILES[f].type === 'NDJSON' ? 'rows' : 'brackets-curly'}`} style={{ fontSize: 14, color: 'var(--accent)' }} />
          <span>{f}</span>
          <span style={{ flex: 1 }} />
          <span style={{ color: 'var(--overlay1)', fontSize: 'var(--fs-xs)' }}>{window.THOTH_FILES[f].type}</span>
        </FlyoutItem>
      ))}
    </div>
  );
}
function FlyoutItem({ children, onClick }) {
  const [h, setH] = appUseState(false);
  return (
    <div
      onClick={onClick}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => setH(false)}
      style={{
        padding: '6px 12px', display: 'flex', alignItems: 'center', gap: 8,
        background: h ? 'var(--selection-bg)' : 'transparent', cursor: 'pointer',
      }}
    >{children}</div>
  );
}

function TitleBar({ focused }) {
  return (
    <div style={{
      height: 32, background: 'var(--crust)', color: 'var(--text)',
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      padding: '0 12px', fontSize: 'var(--fs-md)', fontWeight: 500, userSelect: 'none',
      borderBottom: '1px solid var(--mantle)', flexShrink: 0,
    }}>
      <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <img src="assets/thoth_icon_256.png" width="16" height="16" alt="" style={{ borderRadius: 3 }} />
        <span>Thoth — {focused}</span>
      </span>
      <span style={{ color: 'var(--overlay1)', display: 'flex', alignItems: 'center', gap: 14 }}>
        <i className="ph ph-minus" style={{ fontSize: 13 }} />
        <i className="ph ph-square" style={{ fontSize: 11 }} />
        <i className="ph ph-x" style={{ fontSize: 13 }} />
      </span>
    </div>
  );
}

function StatusBar({ state }) {
  const groups = window.allGroups(state.root);
  const allTabs = groups.flatMap((g) => g.tabs);
  const focusedGroup = groups.find((g) => g.id === state.focusedGroupId);
  const activeTab = focusedGroup?.tabs.find((t) => t.id === focusedGroup.activeId);
  let typeLabel = '—';
  if (activeTab) {
    if (activeTab.kind === 'file') typeLabel = window.THOTH_FILES[activeTab.key]?.type || 'JSON';
    else typeLabel = 'Plugin';
  }
  const sep = <span style={{ color: 'var(--overlay1)', opacity: 0.5 }}>│</span>;
  return (
    <div style={{
      height: 24, background: 'var(--crust)', color: 'var(--text)', fontSize: 'var(--fs-sm)',
      display: 'flex', alignItems: 'center', gap: 10, padding: '0 12px',
      borderTop: '1px solid var(--surface0)', flexShrink: 0,
    }}>
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
        <i className="ph ph-stack" style={{ fontSize: 12 }} />
        {groups.length} {groups.length === 1 ? 'group' : 'groups'}
      </span>{sep}
      <span>{allTabs.length} tabs open</span>{sep}
      <span>{typeLabel}</span>{sep}
      <span style={{ color: 'var(--success)', display: 'inline-flex', alignItems: 'center', gap: 6 }}>
        <i className="ph ph-lightning" style={{ fontSize: 12 }} />
        Ready
      </span>
      <span style={{ flex: 1 }} />
      <span style={{ color: 'var(--overlay1)' }}>Drag any tab to a pane edge to split.</span>
    </div>
  );
}

// ── Root ────────────────────────────────────────────────────────────────────
function ThothApp() {
  const [state, api] = window.useLayout();
  const [tweaks, setTweak] = useTweaks(TWEAK_DEFAULTS);
  // Apply theme to root.
  appUseEffect(() => {
    document.documentElement.setAttribute('data-theme', tweaks.theme === 'light' ? 'light' : 'dark');
  }, [tweaks.theme]);

  // Find a label for the title bar.
  const groups = window.allGroups(state.root);
  const focusedGroup = groups.find((g) => g.id === state.focusedGroupId);
  const activeTab = focusedGroup?.tabs.find((t) => t.id === focusedGroup.activeId);
  const titleLabel = activeTab
    ? (activeTab.kind === 'file' ? activeTab.key : (window.THOTH_PLUGINS[activeTab.key]?.title || 'Plugin'))
    : 'Untitled';

  return (
    <div style={{
      height: '100vh', display: 'flex', flexDirection: 'column',
      background: 'var(--base)', color: 'var(--text)',
      fontFamily: 'var(--font-ui)', fontSize: 'var(--fs-md)',
    }}>
      <TitleBar focused={titleLabel} />
      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
        <ActivityBar
          api={api}
          focusedGroupId={state.focusedGroupId}
          onTheme={() => setTweak('theme', tweaks.theme === 'light' ? 'dark' : 'light')}
          dark={tweaks.theme !== 'light'}
        />
        <div style={{ flex: 1, display: 'flex', minWidth: 0, minHeight: 0 }}>
          <window.SplitNode node={state.root} focusedGroupId={state.focusedGroupId} api={api} />
        </div>
      </div>
      {tweaks.showStatusbar && <StatusBar state={state} />}

      <TweaksPanel title="Tweaks">
        <TweakSection label="Theme">
          <TweakRadio
            label="Palette"
            value={tweaks.theme}
            onChange={(v) => setTweak('theme', v)}
            options={[{ value: 'dark', label: 'Mocha' }, { value: 'light', label: 'Latte' }]}
          />
        </TweakSection>
        <TweakSection label="Layout">
          <TweakButton label="Reset layout to default" onClick={() => api.reset()} />
          <TweakButton label="Open Welcome tab" secondary onClick={() => api.open('plugin', 'welcome', state.focusedGroupId)} />
          <TweakToggle
            label="Show status bar"
            value={tweaks.showStatusbar}
            onChange={(v) => setTweak('showStatusbar', v)}
          />
        </TweakSection>
        <TweakSection label="Quick open">
          {Object.keys(window.THOTH_FILES).map((f) => (
            <TweakButton key={f} label={f} secondary onClick={() => api.open('file', f, state.focusedGroupId)} />
          ))}
        </TweakSection>
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<ThothApp />);
