/* JSON tree viewer for file tabs.
   Uses CSS variables from colors_and_type.css so theme swaps automatically. */

const { useState: jtUseState, useMemo: jtUseMemo, Fragment: JtFragment } = React;

function jtValueType(v) {
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'array';
  return typeof v;
}
function jtFormatScalar(v) {
  const t = jtValueType(v);
  if (t === 'string')  return <span style={{ color: 'var(--syn-string)' }}>"{v}"</span>;
  if (t === 'number')  return <span style={{ color: 'var(--syn-number)' }}>{String(v)}</span>;
  if (t === 'boolean') return <span style={{ color: 'var(--syn-boolean)' }}>{String(v)}</span>;
  if (t === 'null')    return <span style={{ color: 'var(--syn-null)' }}>null</span>;
  return <span>{String(v)}</span>;
}
function jtHighlight(text, query) {
  if (!query) return text;
  const t = String(text);
  const i = t.toLowerCase().indexOf(query.toLowerCase());
  if (i === -1) return t;
  return (
    <JtFragment>
      {t.slice(0, i)}
      <span style={{ background: 'var(--warning)', color: 'var(--crust)', borderRadius: 2, padding: '0 1px' }}>
        {t.slice(i, i + query.length)}
      </span>
      {t.slice(i + query.length)}
    </JtFragment>
  );
}
function jtRowMatches(key, value, q) {
  if (!q) return true;
  const s = q.toLowerCase();
  if (key !== null && String(key).toLowerCase().includes(s)) return true;
  const t = jtValueType(value);
  if (t === 'string' || t === 'number' || t === 'boolean') return String(value).toLowerCase().includes(s);
  return false;
}
function jtSubtreeMatches(value, q) {
  if (!q) return true;
  if (Array.isArray(value)) return value.some((v, i) => jtRowMatches(i, v, q) || jtSubtreeMatches(v, q));
  if (value && typeof value === 'object') {
    return Object.entries(value).some(([k, v]) => jtRowMatches(k, v, q) || jtSubtreeMatches(v, q));
  }
  return jtRowMatches(null, value, q);
}

function JtRow({ depth, label, value, query, expanded, onToggle, selected, onSelect }) {
  const [hover, setHover] = jtUseState(false);
  const t = jtValueType(value);
  const expandable = t === 'array' || t === 'object';
  const bg = selected
    ? 'var(--selection-bg)'
    : hover
      ? 'rgba(255,255,255,0.04)'
      : 'transparent';
  return (
    <div
      onClick={onSelect}
      onDoubleClick={expandable ? onToggle : undefined}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        height: 22, display: 'flex', alignItems: 'center',
        position: 'relative', background: bg, paddingRight: 8, cursor: 'pointer',
        borderLeft: selected ? '2px solid var(--selection-stroke)' : '2px solid transparent',
      }}
    >
      {Array.from({ length: depth }).map((_, i) => (
        <span key={i} style={{ width: 16, height: 22, marginLeft: i === 0 ? 4 : 0, borderRight: '1px solid var(--indent-guide)' }} />
      ))}
      <span
        onClick={(e) => { if (expandable) { e.stopPropagation(); onToggle(); } }}
        style={{ width: 14, color: 'var(--overlay2)', display: 'inline-flex', justifyContent: 'center', alignItems: 'center' }}
      >
        {expandable ? <i className={`ph ph-caret-${expanded ? 'down' : 'right'}`} style={{ fontSize: 11 }} /> : null}
      </span>
      <span style={{ marginLeft: 4, whiteSpace: 'nowrap' }}>
        {label !== null && (typeof label === 'number' ? (
          <span style={{ color: 'var(--syn-bracket)' }}>[{label}]</span>
        ) : (
          <span style={{ color: 'var(--syn-key)' }}>"{jtHighlight(label, query)}"</span>
        ))}
        {label !== null && <span style={{ color: 'var(--syn-bracket)' }}>: </span>}
        {expandable ? (
          <span style={{ color: 'var(--syn-bracket)' }}>
            {t === 'array' ? '[' : '{'}
            {!expanded && <span style={{ color: 'var(--overlay1)', fontStyle: 'italic' }}>
              {t === 'array' ? ` ${value.length} items ` : ` ${Object.keys(value).length} fields `}
            </span>}
            {!expanded && (t === 'array' ? ']' : '}')}
          </span>
        ) : (
          <span>
            {t === 'string'
              ? <span style={{ color: 'var(--syn-string)' }}>"{jtHighlight(String(value), query)}"</span>
              : jtFormatScalar(value)
            }
          </span>
        )}
      </span>
    </div>
  );
}

function jtRender(value, query, expanded, toggle, selected, setSelected, depth = 0, path = '$', label = null) {
  const t = jtValueType(value);
  const isExp = expanded.has(path);
  const expandable = t === 'array' || t === 'object';
  const matchesHere = label !== null && jtRowMatches(label, value, query);
  const childMatches = expandable && jtSubtreeMatches(value, query);
  if (query && !matchesHere && !childMatches) return null;

  const out = [];
  out.push(
    <JtRow key={path} depth={depth} label={label} value={value} query={query}
           expanded={isExp} onToggle={() => toggle(path)}
           selected={selected === path} onSelect={() => setSelected(path)} />
  );
  if (expandable && (isExp || query)) {
    const entries = t === 'array' ? value.map((v, i) => [i, v]) : Object.entries(value);
    entries.forEach(([k, v]) => {
      out.push(...(jtRender(v, query, expanded, toggle, selected, setSelected, depth + 1, `${path}.${k}`, k) || []));
    });
    out.push(
      <div key={path + '_close'} style={{ height: 22, display: 'flex', alignItems: 'center' }}>
        {Array.from({ length: depth }).map((_, i) => (
          <span key={i} style={{ width: 16, height: 22, marginLeft: i === 0 ? 4 : 0, borderRight: '1px solid var(--indent-guide)' }} />
        ))}
        <span style={{ width: 14 }} />
        <span style={{ color: 'var(--syn-bracket)', marginLeft: 4 }}>{t === 'array' ? ']' : '}'}</span>
      </div>
    );
  }
  return out;
}

function JsonTree({ data, query }) {
  const [expanded, setExpanded] = jtUseState(() => new Set(['$', '$.0', '$.1', '$.2', '$.3', '$.4', '$.5']));
  const [selected, setSelected] = jtUseState(null);
  const toggle = (path) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path); else next.add(path);
      return next;
    });
  };
  const tree = jtUseMemo(
    () => jtRender(data, query, expanded, toggle, selected, setSelected),
    [data, query, expanded, selected]
  );
  return (
    <div style={{
      flex: 1, background: 'var(--base)', color: 'var(--text)', overflow: 'auto',
      fontFamily: 'var(--font-mono)', fontSize: 'var(--fs-md)', padding: '8px 4px',
    }}>
      {tree}
    </div>
  );
}

window.JsonTree = JsonTree;
