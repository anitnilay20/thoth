/* Plugin tab content panels. Each plugin is its own little app inside a tab. */

const { useState: plUseState, useMemo: plUseMemo } = React;

function PluginShell({ children, title, subtitle, accent = 'primary' }) {
  return (
    <div style={{
      flex: 1, background: 'var(--base)', color: 'var(--text)', overflow: 'auto',
      display: 'flex', flexDirection: 'column',
    }}>
      <div style={{
        padding: '20px 24px 16px',
        borderBottom: '1px solid var(--surface0)',
        display: 'flex', alignItems: 'baseline', gap: 12,
      }}>
        <h1 style={{
          margin: 0, fontSize: 'var(--fs-2xl)', fontWeight: 700,
          color: `var(--${accent})`, letterSpacing: '-0.01em',
        }}>{title}</h1>
        {subtitle && <span style={{ color: 'var(--overlay1)', fontSize: 'var(--fs-md)' }}>{subtitle}</span>}
      </div>
      <div style={{ flex: 1, padding: 24, overflow: 'auto' }}>{children}</div>
    </div>
  );
}

// ── Welcome ─────────────────────────────────────────────────────────────────
function WelcomePanel({ onOpenFile, onOpenPlugin }) {
  const files = Object.keys(window.THOTH_FILES);
  return (
    <PluginShell title="Welcome to Thoth" subtitle="Wisdom for your JSON." accent="primary">
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 32, maxWidth: 880 }}>
        <div>
          <div className="t-section" style={{ marginBottom: 8 }}>Start</div>
          <ActionRow icon="folder-open" label="Open file…" hint="⌘O" onClick={() => onOpenFile(files[0])} />
          <ActionRow icon="app-window" label="New window" hint="⌘N" onClick={() => {}} />
          <ActionRow icon="puzzle-piece" label="Browse plugins…" onClick={() => onOpenPlugin('settings')} />

          <div className="t-section" style={{ marginTop: 24, marginBottom: 8 }}>Recent</div>
          {files.map((f) => (
            <ActionRow key={f} icon="file-text" label={f}
              hint={window.THOTH_FILES[f].type}
              onClick={() => onOpenFile(f)} />
          ))}
        </div>
        <div>
          <div className="t-section" style={{ marginBottom: 8 }}>Tips</div>
          <Tip kbd="Drag tab → edge" body="Drop on the left, right, top, or bottom of any pane to split it." />
          <Tip kbd="Drag tab → strip" body="Move a tab between groups, or reorder within the same strip." />
          <Tip kbd="Right-click tab" body="Pin, close others, split right, split down — full VSCode-style menu." />
          <Tip kbd="⌘W" body="Close active tab. Empty groups collapse automatically." />
          <Tip kbd="⌘\\" body="Split active editor to the right." />
        </div>
      </div>
    </PluginShell>
  );
}
function ActionRow({ icon, label, hint, onClick }) {
  const [h, setH] = plUseState(false);
  return (
    <div
      onClick={onClick}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => setH(false)}
      style={{
        display: 'flex', alignItems: 'center', gap: 10,
        padding: '8px 10px', borderRadius: 4, cursor: 'pointer',
        background: h ? 'var(--surface0)' : 'transparent',
        fontSize: 'var(--fs-md)',
      }}
    >
      <i className={`ph ph-${icon}`} style={{ fontSize: 16, color: 'var(--accent)' }} />
      <span style={{ flex: 1 }}>{label}</span>
      {hint && <span style={{ color: 'var(--overlay1)', fontSize: 'var(--fs-sm)', fontFamily: 'var(--font-mono)' }}>{hint}</span>}
    </div>
  );
}
function Tip({ kbd, body }) {
  return (
    <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12, padding: '8px 0' }}>
      <span style={{
        fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-sm)',
        background: 'var(--surface0)', padding: '2px 8px', borderRadius: 4,
        color: 'var(--text)', flexShrink: 0, marginTop: 1,
      }}>{kbd}</span>
      <span style={{ color: 'var(--overlay1)', fontSize: 'var(--fs-md)', lineHeight: 1.5 }}>{body}</span>
    </div>
  );
}

// ── Settings ────────────────────────────────────────────────────────────────
function SettingsPanel() {
  const [tab, setTab] = plUseState('appearance');
  const sections = [
    { id: 'appearance', label: 'Appearance', icon: 'paint-brush' },
    { id: 'editor',     label: 'Editor',     icon: 'code' },
    { id: 'plugins',    label: 'Plugins',    icon: 'puzzle-piece' },
    { id: 'shortcuts',  label: 'Shortcuts',  icon: 'keyboard' },
  ];
  return (
    <PluginShell title="Settings" accent="overlay2">
      <div style={{ display: 'flex', gap: 24, maxWidth: 880 }}>
        <div style={{ width: 200, flexShrink: 0 }}>
          {sections.map((s) => (
            <div key={s.id} onClick={() => setTab(s.id)} style={{
              padding: '8px 12px', borderRadius: 4, cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 8,
              background: tab === s.id ? 'var(--selection-bg)' : 'transparent',
              borderLeft: tab === s.id ? '2px solid var(--selection-stroke)' : '2px solid transparent',
              fontSize: 'var(--fs-md)',
              color: tab === s.id ? 'var(--text)' : 'var(--overlay1)',
            }}>
              <i className={`ph ph-${s.icon}`} style={{ fontSize: 16 }} />
              {s.label}
            </div>
          ))}
        </div>
        <div style={{ flex: 1 }}>
          {tab === 'appearance' && <SettingsAppearance />}
          {tab === 'editor' && <SettingsEditor />}
          {tab === 'plugins' && <SettingsPlugins />}
          {tab === 'shortcuts' && <SettingsShortcuts />}
        </div>
      </div>
    </PluginShell>
  );
}
function SettingRow({ label, hint, children }) {
  return (
    <div style={{ padding: '12px 0', borderBottom: '1px solid var(--surface0)' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 16 }}>
        <div>
          <div style={{ fontSize: 'var(--fs-md)', fontWeight: 500 }}>{label}</div>
          {hint && <div style={{ fontSize: 'var(--fs-sm)', color: 'var(--overlay1)', marginTop: 2 }}>{hint}</div>}
        </div>
        <div>{children}</div>
      </div>
    </div>
  );
}
function SettingsAppearance() {
  const [theme, setTheme] = plUseState('mocha');
  const [fontSize, setFontSize] = plUseState(13);
  return (
    <div>
      <SettingRow label="Color theme" hint="Catppuccin variants">
        <select value={theme} onChange={(e) => setTheme(e.target.value)} style={selectStyle}>
          <option value="mocha">Mocha (dark)</option>
          <option value="latte">Latte (light)</option>
        </select>
      </SettingRow>
      <SettingRow label="Editor font size" hint="JSON tree rows">
        <input type="number" value={fontSize} onChange={(e) => setFontSize(+e.target.value)}
          style={{ ...selectStyle, width: 80 }} />
      </SettingRow>
      <SettingRow label="Show indent guides">
        <Toggle initial={true} />
      </SettingRow>
      <SettingRow label="Enable animations" hint="Tab open/close, panel transitions">
        <Toggle initial={true} />
      </SettingRow>
    </div>
  );
}
function SettingsEditor() {
  return (
    <div>
      <SettingRow label="Lazy load threshold" hint="Files above this size parse on-demand">
        <input defaultValue="50 MB" style={{ ...selectStyle, width: 120 }} />
      </SettingRow>
      <SettingRow label="LRU cache size" hint="Recently expanded nodes kept in memory">
        <input defaultValue="2048" style={{ ...selectStyle, width: 120 }} />
      </SettingRow>
      <SettingRow label="Parallel search workers">
        <input defaultValue="4" style={{ ...selectStyle, width: 80 }} />
      </SettingRow>
      <SettingRow label="Auto-detect NDJSON">
        <Toggle initial={true} />
      </SettingRow>
    </div>
  );
}
function SettingsPlugins() {
  const all = Object.values(window.THOTH_PLUGINS);
  return (
    <div>
      {all.map((p) => (
        <div key={p.id} style={{
          padding: 16, borderRadius: 12, background: 'var(--surface0)',
          marginBottom: 12, display: 'flex', gap: 12, alignItems: 'center',
        }}>
          <div style={{
            width: 48, height: 48, borderRadius: 8,
            background: `var(--${p.accent})`,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            flexShrink: 0,
          }}>
            <i className={`ph ph-${p.icon}`} style={{ fontSize: 24, color: 'var(--crust)' }} />
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 'var(--fs-xl)', fontWeight: 600 }}>{p.title}</div>
            <div style={{ fontSize: 'var(--fs-md)', color: 'var(--overlay1)', marginTop: 2 }}>
              {p.id === 'welcome' && 'Quick start screen for new sessions.'}
              {p.id === 'settings' && 'Configure appearance, editor, and shortcuts.'}
              {p.id === 'schema-validator' && 'Validate JSON files against JSON Schema.'}
              {p.id === 'diff' && 'Compare two JSON documents side-by-side.'}
              {p.id === 'jsonpath' && 'Query JSON files with JSONPath expressions.'}
            </div>
          </div>
          <span style={{
            fontSize: 'var(--fs-xs)', fontWeight: 700, textTransform: 'uppercase',
            letterSpacing: '0.06em', padding: '4px 8px', borderRadius: 4,
            background: 'var(--success)', color: 'var(--crust)',
          }}>ENABLED</span>
        </div>
      ))}
    </div>
  );
}
function SettingsShortcuts() {
  const rows = [
    ['Open file', '⌘O'],
    ['New tab', '⌘T'],
    ['Close tab', '⌘W'],
    ['Split right', '⌘\\'],
    ['Split down', '⌘K ⌘\\'],
    ['Focus next group', '⌘K ⌘→'],
    ['Search in file', '⌘F'],
    ['Toggle theme', '⌘⇧T'],
    ['Pin tab', '⌘K ⇧Enter'],
  ];
  return (
    <div style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-md)' }}>
      {rows.map(([label, key]) => (
        <div key={label} style={{
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          padding: '8px 0', borderBottom: '1px solid var(--surface0)',
        }}>
          <span style={{ fontFamily: 'var(--font-ui)' }}>{label}</span>
          <span style={{ background: 'var(--surface0)', padding: '2px 10px', borderRadius: 4, color: 'var(--text)' }}>{key}</span>
        </div>
      ))}
    </div>
  );
}
function Toggle({ initial }) {
  const [on, setOn] = plUseState(initial);
  return (
    <span onClick={() => setOn((v) => !v)} style={{
      width: 32, height: 18, borderRadius: 9,
      background: on ? 'var(--success)' : 'var(--surface2)',
      position: 'relative', cursor: 'pointer', display: 'inline-block',
      transition: 'background var(--d-fast)',
    }}>
      <span style={{
        position: 'absolute', top: 2, left: on ? 16 : 2,
        width: 14, height: 14, borderRadius: 7, background: 'var(--text)',
        transition: 'left var(--d-fast) var(--ease-out)',
      }} />
    </span>
  );
}
const selectStyle = {
  background: 'var(--surface0)', color: 'var(--text)', border: 0,
  padding: '4px 10px', borderRadius: 4, fontSize: 'var(--fs-md)',
  fontFamily: 'var(--font-ui)', outline: 'none', cursor: 'pointer',
};

// ── Schema Validator ────────────────────────────────────────────────────────
function SchemaValidatorPanel() {
  const files = Object.keys(window.THOTH_FILES);
  const [target, setTarget] = plUseState('users.json');
  const [schema, setSchema] = plUseState('schema.json');
  // Faux validation — for the demo, validate that target is an array of objects with id/name/email.
  const results = plUseMemo(() => {
    const data = window.THOTH_FILES[target]?.value;
    if (!data) return [];
    const items = Array.isArray(data) ? data : [data];
    const out = [];
    items.forEach((item, i) => {
      const path = Array.isArray(data) ? `$[${i}]` : '$';
      if (!item || typeof item !== 'object') {
        out.push({ severity: 'error', path, msg: 'expected object' }); return;
      }
      if (!('id' in item)) out.push({ severity: 'error', path: `${path}.id`, msg: 'required property missing' });
      else if (typeof item.id !== 'number') out.push({ severity: 'error', path: `${path}.id`, msg: 'expected integer' });
      if (!('name' in item)) out.push({ severity: 'error', path: `${path}.name`, msg: 'required property missing' });
      if (!('email' in item)) out.push({ severity: 'warning', path: `${path}.email`, msg: 'recommended property missing' });
      else if (typeof item.email === 'string' && !item.email.includes('@')) {
        out.push({ severity: 'warning', path: `${path}.email`, msg: 'value does not match format "email"' });
      }
    });
    return out;
  }, [target, schema]);
  const errors = results.filter((r) => r.severity === 'error').length;
  const warnings = results.filter((r) => r.severity === 'warning').length;
  return (
    <PluginShell title="Schema Validator" subtitle={`${errors} errors · ${warnings} warnings`} accent="info">
      <div style={{ display: 'flex', gap: 16, marginBottom: 16, alignItems: 'center' }}>
        <Field label="Target">
          <select value={target} onChange={(e) => setTarget(e.target.value)} style={selectStyle}>
            {files.map((f) => <option key={f}>{f}</option>)}
          </select>
        </Field>
        <Field label="Schema">
          <select value={schema} onChange={(e) => setSchema(e.target.value)} style={selectStyle}>
            {files.filter((f) => f.includes('schema')).map((f) => <option key={f}>{f}</option>)}
            <option>(inline)</option>
          </select>
        </Field>
        <span style={{ flex: 1 }} />
        <button style={btnPrimary}>Re-validate</button>
      </div>
      <div style={{ background: 'var(--mantle)', borderRadius: 8, overflow: 'hidden', border: '1px solid var(--surface0)' }}>
        {results.length === 0 && (
          <div style={{ padding: 32, textAlign: 'center', color: 'var(--success)' }}>
            <i className="ph ph-check-circle" style={{ fontSize: 32 }} />
            <div style={{ marginTop: 8, fontSize: 'var(--fs-lg)' }}>No issues found.</div>
          </div>
        )}
        {results.map((r, i) => (
          <div key={i} style={{
            padding: '10px 16px', display: 'flex', alignItems: 'center', gap: 12,
            borderBottom: i < results.length - 1 ? '1px solid var(--surface0)' : 0,
            fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-md)',
          }}>
            <i className={`ph ph-${r.severity === 'error' ? 'x-circle' : 'warning'}`}
               style={{ fontSize: 16, color: `var(--${r.severity === 'error' ? 'error' : 'warning'})` }} />
            <span style={{ color: 'var(--syn-key)' }}>{r.path}</span>
            <span style={{ color: 'var(--overlay1)' }}>—</span>
            <span style={{ color: 'var(--text)' }}>{r.msg}</span>
          </div>
        ))}
      </div>
    </PluginShell>
  );
}
function Field({ label, children }) {
  return (
    <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 'var(--fs-md)' }}>
      <span style={{ color: 'var(--overlay1)' }}>{label}</span>
      {children}
    </label>
  );
}
const btnPrimary = {
  background: 'var(--primary)', color: 'var(--crust)', border: 0,
  padding: '6px 14px', borderRadius: 4, fontSize: 'var(--fs-md)',
  fontFamily: 'var(--font-ui)', fontWeight: 600, cursor: 'pointer',
};

// ── Diff Viewer ─────────────────────────────────────────────────────────────
function DiffPanel() {
  const files = Object.keys(window.THOTH_FILES);
  const [left, setLeft] = plUseState('users.json');
  const [right, setRight] = plUseState('config.json');
  const leftStr = plUseMemo(() => JSON.stringify(window.THOTH_FILES[left]?.value, null, 2), [left]);
  const rightStr = plUseMemo(() => JSON.stringify(window.THOTH_FILES[right]?.value, null, 2), [right]);
  // Naive line-by-line marking
  const ll = leftStr.split('\n');
  const rl = rightStr.split('\n');
  const max = Math.max(ll.length, rl.length);
  return (
    <PluginShell title="Diff Viewer" subtitle="JSON ↔ JSON comparison" accent="secondary">
      <div style={{ display: 'flex', gap: 12, marginBottom: 12 }}>
        <select value={left} onChange={(e) => setLeft(e.target.value)} style={selectStyle}>{files.map((f) => <option key={f}>{f}</option>)}</select>
        <span style={{ color: 'var(--overlay1)', alignSelf: 'center' }}>↔</span>
        <select value={right} onChange={(e) => setRight(e.target.value)} style={selectStyle}>{files.map((f) => <option key={f}>{f}</option>)}</select>
      </div>
      <div style={{
        display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 1,
        background: 'var(--surface0)', borderRadius: 8, overflow: 'hidden',
        border: '1px solid var(--surface0)', maxHeight: 'calc(100vh - 280px)',
      }}>
        <DiffCol lines={ll} other={rl} side="left" />
        <DiffCol lines={rl} other={ll} side="right" />
      </div>
    </PluginShell>
  );
}
function DiffCol({ lines, other, side }) {
  return (
    <div style={{
      background: 'var(--base)', overflow: 'auto', fontFamily: 'var(--font-mono)',
      fontSize: 'var(--fs-md)', padding: '8px 0',
    }}>
      {lines.map((line, i) => {
        const otherLine = other[i];
        const same = line === otherLine;
        const bg = same ? 'transparent' : (side === 'left' ? 'rgba(243,139,168,0.10)' : 'rgba(166,227,161,0.10)');
        const marker = same ? ' ' : (side === 'left' ? '−' : '+');
        const markerColor = same ? 'var(--overlay1)' : (side === 'left' ? 'var(--error)' : 'var(--success)');
        return (
          <div key={i} style={{ display: 'flex', background: bg, minHeight: 20, lineHeight: '20px' }}>
            <span style={{ width: 36, textAlign: 'right', color: 'var(--overlay1)', flexShrink: 0, paddingRight: 8 }}>{i + 1}</span>
            <span style={{ width: 16, color: markerColor, textAlign: 'center', flexShrink: 0 }}>{marker}</span>
            <span style={{ whiteSpace: 'pre', color: 'var(--text)' }}>{line ?? ''}</span>
          </div>
        );
      })}
    </div>
  );
}

// ── JSONPath ────────────────────────────────────────────────────────────────
function JsonPathPanel() {
  const files = Object.keys(window.THOTH_FILES);
  const [target, setTarget] = plUseState('users.json');
  const [query, setQuery] = plUseState('$[*].name');
  const data = window.THOTH_FILES[target]?.value;
  // Tiny JSONPath: only supports $, $.key, $[*], $[*].key, $.key.key
  const results = plUseMemo(() => {
    try { return evalPath(data, query); } catch (e) { return { error: e.message }; }
  }, [target, query, data]);
  return (
    <PluginShell title="JSONPath" subtitle="Query against the active dataset" accent="accent">
      <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
        <select value={target} onChange={(e) => setTarget(e.target.value)} style={selectStyle}>
          {files.map((f) => <option key={f}>{f}</option>)}
        </select>
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="$.users[*].name"
          style={{ ...selectStyle, flex: 1, fontFamily: 'var(--font-mono)' }} />
      </div>
      <div className="t-section" style={{ marginBottom: 8 }}>Results {Array.isArray(results) && `(${results.length})`}</div>
      <div style={{
        background: 'var(--mantle)', borderRadius: 8, padding: 12,
        fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-md)',
        maxHeight: 'calc(100vh - 280px)', overflow: 'auto', whiteSpace: 'pre',
        border: '1px solid var(--surface0)',
      }}>
        {results && results.error
          ? <span style={{ color: 'var(--error)' }}>Error: {results.error}</span>
          : <span style={{ color: 'var(--syn-string)' }}>{JSON.stringify(results, null, 2)}</span>
        }
      </div>
      <div style={{ marginTop: 16, fontSize: 'var(--fs-sm)', color: 'var(--overlay1)' }}>
        Try:
        {[' $', ' $[*]', ' $[*].name', ' $.shortcuts', ' $.regions[*]'].map((q, i) => (
          <span key={i} onClick={() => setQuery(q.trim())} style={{
            display: 'inline-block', marginLeft: 8, padding: '2px 8px',
            background: 'var(--surface0)', borderRadius: 4, cursor: 'pointer',
            color: 'var(--text)', fontFamily: 'var(--font-mono)',
          }}>{q.trim()}</span>
        ))}
      </div>
    </PluginShell>
  );
}
function evalPath(data, q) {
  q = q.trim();
  if (!q.startsWith('$')) throw new Error('path must start with $');
  let cur = [data];
  let rest = q.slice(1);
  while (rest.length) {
    if (rest.startsWith('[*]')) {
      cur = cur.flatMap((v) => Array.isArray(v) ? v : (v && typeof v === 'object' ? Object.values(v) : []));
      rest = rest.slice(3);
    } else if (rest.startsWith('.')) {
      const m = rest.match(/^\.([A-Za-z_][\w-]*)/);
      if (!m) throw new Error(`bad token at "${rest}"`);
      const key = m[1];
      cur = cur.map((v) => (v && typeof v === 'object' ? v[key] : undefined)).filter((v) => v !== undefined);
      rest = rest.slice(m[0].length);
    } else if (rest.startsWith('[') ) {
      const m = rest.match(/^\[(\d+)\]/);
      if (!m) throw new Error(`bad index at "${rest}"`);
      const idx = +m[1];
      cur = cur.map((v) => Array.isArray(v) ? v[idx] : undefined).filter((v) => v !== undefined);
      rest = rest.slice(m[0].length);
    } else {
      throw new Error(`unexpected "${rest}"`);
    }
  }
  return cur;
}

Object.assign(window, { WelcomePanel, SettingsPanel, SchemaValidatorPanel, DiffPanel, JsonPathPanel });
