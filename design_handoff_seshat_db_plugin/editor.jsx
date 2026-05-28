/* Seshat — SQL editor with syntax highlight, run bar, AI prompt.
   Custom-built tokenizer (lightweight) + textarea overlay. */

// ───────────────────────────────────────── Tokenizer
const SQL_KEYWORDS_SET = new Set([
  'select','from','where','group','by','order','having','limit','offset','as',
  'join','left','right','inner','outer','full','cross','on','using',
  'with','recursive','union','intersect','except','all','distinct',
  'insert','into','values','update','set','delete','returning','create','drop','alter','table','view','index',
  'and','or','not','in','is','null','true','false','case','when','then','else','end','exists','between',
  'asc','desc','nulls','first','last','interval','now','current_timestamp','primary','foreign','key','references','default','unique',
]);
const SQL_FUNCS = new Set(['count','sum','avg','min','max','coalesce','cast','to_char','to_date','date_trunc','extract','json_build_object','jsonb_agg','array_agg','row_number','rank','dense_rank','generate_series']);

function tokenizeSQL(src) {
  const tokens = [];
  let i = 0;
  const N = src.length;
  while (i < N) {
    const ch = src[i];
    // line comment
    if (ch === '-' && src[i + 1] === '-') {
      let j = i; while (j < N && src[j] !== '\n') j++;
      tokens.push({ t: 'comment', v: src.slice(i, j) }); i = j; continue;
    }
    // block comment
    if (ch === '/' && src[i + 1] === '*') {
      let j = i + 2; while (j < N - 1 && !(src[j] === '*' && src[j + 1] === '/')) j++;
      j = Math.min(N, j + 2);
      tokens.push({ t: 'comment', v: src.slice(i, j) }); i = j; continue;
    }
    // string
    if (ch === "'" || ch === '"') {
      const quote = ch; let j = i + 1;
      while (j < N && src[j] !== quote) { if (src[j] === '\\') j++; j++; }
      j = Math.min(N, j + 1);
      tokens.push({ t: quote === "'" ? 'string' : 'ident-quoted', v: src.slice(i, j) }); i = j; continue;
    }
    // number
    if (/[0-9]/.test(ch)) {
      let j = i; while (j < N && /[0-9.]/.test(src[j])) j++;
      tokens.push({ t: 'number', v: src.slice(i, j) }); i = j; continue;
    }
    // word
    if (/[a-z_]/i.test(ch)) {
      let j = i; while (j < N && /[a-z0-9_]/i.test(src[j])) j++;
      const word = src.slice(i, j);
      const lower = word.toLowerCase();
      if (SQL_KEYWORDS_SET.has(lower)) tokens.push({ t: 'keyword', v: word });
      else if (SQL_FUNCS.has(lower) && src[j] === '(') tokens.push({ t: 'function', v: word });
      else tokens.push({ t: 'ident', v: word });
      i = j; continue;
    }
    // punct
    if (/[(),;:.*=<>+\-/%!@#]/.test(ch)) {
      tokens.push({ t: 'punct', v: ch }); i++; continue;
    }
    // whitespace incl newline
    tokens.push({ t: 'ws', v: ch }); i++;
  }
  return tokens;
}

function tokenColor(t) {
  switch (t) {
    case 'comment':       return 'var(--overlay1)';
    case 'keyword':       return 'var(--primary)';
    case 'function':      return 'var(--secondary)';
    case 'string':        return 'var(--syn-string)';
    case 'ident-quoted':  return 'var(--syn-key)';
    case 'number':        return 'var(--syn-number)';
    case 'punct':         return 'var(--syn-bracket)';
    default:              return 'var(--text)';
  }
}

// ───────────────────────────────────────── HighlightedCode (read-only block)
function HighlightedSQL({ src, style }) {
  const tokens = React.useMemo(() => tokenizeSQL(src), [src]);
  return (
    <code style={{ fontFamily: 'var(--font-mono)', fontSize: 13, whiteSpace: 'pre-wrap', ...style }}>
      {tokens.map((t, i) => t.t === 'ws' ? t.v : (
        <span key={i} style={{ color: tokenColor(t.t), fontStyle: t.t === 'comment' ? 'italic' : 'normal' }}>{t.v}</span>
      ))}
    </code>
  );
}

// ───────────────────────────────────────── SqlEditor (textarea + overlay)
function SqlEditor({ value, onChange, onRun, onExplain, onAskAI, conn, executing }) {
  const taRef = React.useRef(null);
  const overlayRef = React.useRef(null);
  const [lineCount, setLineCount] = React.useState(() => Math.max(1, value.split('\n').length));
  const [selInfo, setSelInfo] = React.useState({ line: 1, col: 1 });

  // Sync scroll
  const onScroll = () => {
    if (overlayRef.current && taRef.current) {
      overlayRef.current.scrollTop = taRef.current.scrollTop;
      overlayRef.current.scrollLeft = taRef.current.scrollLeft;
    }
  };

  React.useEffect(() => {
    setLineCount(Math.max(1, value.split('\n').length));
  }, [value]);

  // Keyboard
  const onKeyDown = (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault(); onRun();
    } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === 'E' || e.key === 'e')) {
      e.preventDefault(); onExplain();
    } else if (e.key === '/' && value.trim() === '') {
      // intentionally let the slash type; but show AI hint
    } else if (e.key === 'Tab') {
      e.preventDefault();
      const ta = taRef.current;
      const s = ta.selectionStart, end = ta.selectionEnd;
      const next = value.slice(0, s) + '  ' + value.slice(end);
      onChange(next);
      requestAnimationFrame(() => { ta.selectionStart = ta.selectionEnd = s + 2; });
    }
  };

  const onSel = () => {
    const ta = taRef.current; if (!ta) return;
    const pos = ta.selectionStart;
    const before = value.slice(0, pos);
    const line = before.split('\n').length;
    const col = pos - (before.lastIndexOf('\n') + 1) + 1;
    setSelInfo({ line, col });
  };

  // Lines
  const lines = Array.from({ length: lineCount }, (_, i) => i + 1);
  const tokens = React.useMemo(() => tokenizeSQL(value), [value]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0, background: 'var(--base)' }}>
      <EditorToolbar conn={conn} executing={executing}
        onRun={onRun} onExplain={onExplain} onAskAI={onAskAI} />
      <div style={{ flex: 1, display: 'flex', minHeight: 0, position: 'relative' }}>
        {/* Gutter */}
        <div style={{
          background: 'var(--base)', borderRight: '1px solid var(--surface0)',
          padding: '12px 8px 12px 12px', userSelect: 'none', textAlign: 'right',
          fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text-disabled)',
          lineHeight: '20px', minWidth: 46, overflow: 'hidden',
        }}>
          {lines.map(n => <div key={n} style={{ height: 20 }}>{n}</div>)}
        </div>
        {/* Editor area */}
        <div style={{ position: 'relative', flex: 1, minWidth: 0 }}>
          <pre ref={overlayRef} aria-hidden="true" style={{
            position: 'absolute', inset: 0, margin: 0, padding: '12px 16px',
            fontFamily: 'var(--font-mono)', fontSize: 13, lineHeight: '20px',
            color: 'var(--text)', whiteSpace: 'pre', overflow: 'auto', pointerEvents: 'none',
          }}>
            {tokens.map((t, i) => t.t === 'ws' ? t.v : (
              <span key={i} style={{ color: tokenColor(t.t), fontStyle: t.t === 'comment' ? 'italic' : 'normal' }}>{t.v}</span>
            ))}
            {/* trailing space so caret at end is visible */}
            <span>{' '}</span>
          </pre>
          <textarea ref={taRef} value={value}
            onChange={(e) => onChange(e.target.value)}
            onScroll={onScroll} onKeyDown={onKeyDown}
            onSelect={onSel} onClick={onSel} onKeyUp={onSel}
            spellCheck={false}
            style={{
              position: 'absolute', inset: 0, margin: 0, padding: '12px 16px',
              border: 0, background: 'transparent', resize: 'none',
              fontFamily: 'var(--font-mono)', fontSize: 13, lineHeight: '20px',
              color: 'transparent', caretColor: 'var(--text)',
              outline: 'none', whiteSpace: 'pre', overflow: 'auto', tabSize: 2,
            }} />
        </div>
        {/* Autocomplete preview (static) */}
        <AutocompletePreview value={value} />
      </div>
      <EditorFooter selInfo={selInfo} conn={conn} />
    </div>
  );
}

function EditorToolbar({ conn, executing, onRun, onExplain, onAskAI }) {
  return (
    <div style={{
      height: 40, display: 'flex', alignItems: 'center', gap: 8,
      background: 'var(--mantle)', borderBottom: '1px solid var(--surface0)',
      padding: '0 10px', flexShrink: 0,
    }}>
      <RunButton executing={executing} onClick={onRun} />
      <button onClick={onExplain} style={{
        height: 26, padding: '0 10px', borderRadius: 4, cursor: 'pointer',
        background: 'var(--surface0)', border: '1px solid var(--surface1)',
        color: 'var(--text)', fontSize: 12, fontFamily: 'inherit',
        display: 'inline-flex', alignItems: 'center', gap: 6,
      }}>
        <Ph name="chart-bar" size={12} color="var(--info)" />
        Explain
        <span className="seshat-kbd">⌘⇧E</span>
      </button>
      <span style={{ width: 1, height: 18, background: 'var(--surface0)' }} />
      <button onClick={onAskAI} style={{
        height: 26, padding: '0 10px', borderRadius: 4, cursor: 'pointer',
        background: 'var(--surface0)', border: '1px solid var(--primary)',
        color: 'var(--primary)', fontSize: 12, fontFamily: 'inherit', fontWeight: 500,
        display: 'inline-flex', alignItems: 'center', gap: 6,
      }}>
        <Ph name="sparkle" size={12} color="var(--primary)" />
        Ask AI
        <span className="seshat-kbd" style={{ color: 'var(--primary)' }}>/</span>
      </button>
      <span style={{ flex: 1 }} />
      <button style={{
        height: 26, padding: '0 10px', borderRadius: 4, cursor: 'pointer',
        background: 'transparent', border: '1px solid var(--surface0)',
        color: 'var(--overlay1)', fontSize: 12, fontFamily: 'inherit',
        display: 'inline-flex', alignItems: 'center', gap: 6,
      }}>
        <Ph name="floppy-disk" size={12} color="currentColor" />
        Save
      </button>
      <button title="Format SQL" style={{
        height: 26, width: 26, borderRadius: 4, cursor: 'pointer',
        background: 'transparent', border: '1px solid var(--surface0)',
        color: 'var(--overlay1)', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      }}><Ph name="paint-brush-broad" size={12} color="currentColor" /></button>
      <button title="Share" style={{
        height: 26, width: 26, borderRadius: 4, cursor: 'pointer',
        background: 'transparent', border: '1px solid var(--surface0)',
        color: 'var(--overlay1)', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
      }}><Ph name="share-network" size={12} color="currentColor" /></button>
    </div>
  );
}

function RunButton({ executing, onClick }) {
  return (
    <button onClick={onClick} disabled={executing}
      style={{
        height: 26, padding: '0 12px', borderRadius: 4, cursor: executing ? 'wait' : 'pointer',
        background: executing ? 'var(--surface0)' : 'var(--primary)',
        border: 0, color: executing ? 'var(--overlay1)' : 'var(--crust)',
        fontSize: 12, fontWeight: 600, fontFamily: 'inherit',
        display: 'inline-flex', alignItems: 'center', gap: 6,
      }}>
      {executing
        ? <Ph name="circle-notch" size={12} color="currentColor" spin />
        : <Ph name="play-fill" size={12} color="currentColor" />}
      {executing ? 'Running…' : 'Run'}
      <span className="seshat-kbd" style={{ background: 'rgba(17,17,27,0.25)', color: executing ? 'var(--overlay1)' : 'var(--crust)', borderColor: 'rgba(17,17,27,0.4)' }}>⌘↵</span>
    </button>
  );
}

function EditorFooter({ selInfo, conn }) {
  return (
    <div style={{
      height: 22, background: 'var(--mantle)', borderTop: '1px solid var(--surface0)',
      display: 'flex', alignItems: 'center', gap: 12, padding: '0 12px',
      fontSize: 11, color: 'var(--overlay1)', fontFamily: 'var(--font-mono)', flexShrink: 0,
    }}>
      <span>Ln {selInfo.line}, Col {selInfo.col}</span>
      <span style={{ opacity: 0.5 }}>│</span>
      <span>{conn ? `${SESHAT.ENGINES[conn.engine]?.label || conn.engine}` : 'SQL'}</span>
      <span style={{ opacity: 0.5 }}>│</span>
      <span>2 spaces</span>
      <span style={{ flex: 1 }} />
      <span>Schema-aware autocomplete <span style={{ color: 'var(--success)' }}>●</span></span>
    </div>
  );
}

// Tiny static autocomplete preview to sell the feature
function AutocompletePreview({ value }) {
  // Show only if the value ends with a word boundary that "looks" like a column hint
  const tail = (value.match(/[a-z_]+$/i) || [''])[0];
  if (!tail || tail.length < 2) return null;
  const matches = [
    ...SESHAT.COLUMNS_VOCAB.filter(c => c.startsWith(tail.toLowerCase())).slice(0, 4).map(c => ({ kind: 'col', name: c, type: 'column' })),
    ...SESHAT.TABLES.filter(c => c.startsWith(tail.toLowerCase())).slice(0, 2).map(c => ({ kind: 'tbl', name: c, type: 'table' })),
    ...SESHAT.KEYWORDS.filter(k => k.toLowerCase().startsWith(tail.toLowerCase())).slice(0, 3).map(k => ({ kind: 'kw', name: k, type: 'keyword' })),
  ];
  if (matches.length === 0) return null;
  return (
    <div style={{
      position: 'absolute', bottom: 12, right: 16, width: 280, zIndex: 5,
      background: 'var(--mantle)', border: '1px solid var(--surface1)',
      borderRadius: 6, boxShadow: 'var(--shadow-menu)', overflow: 'hidden',
    }}>
      <div style={{ padding: '6px 10px', fontSize: 10, color: 'var(--overlay1)',
                    borderBottom: '1px solid var(--surface0)',
                    textTransform: 'uppercase', letterSpacing: '0.08em' }}>
        Suggestions · matching "{tail}"
      </div>
      {matches.map((m, i) => (
        <div key={i} style={{
          padding: '4px 10px', display: 'flex', alignItems: 'center', gap: 8,
          background: i === 0 ? 'var(--surface0)' : 'transparent',
          fontFamily: 'var(--font-mono)', fontSize: 12,
        }}>
          <span style={{
            fontSize: 9, padding: '1px 5px', borderRadius: 2,
            background: m.kind === 'col' ? 'var(--surface1)' : m.kind === 'tbl' ? 'var(--syn-string)' : 'var(--primary)',
            color: m.kind === 'col' ? 'var(--text)' : 'var(--crust)',
            fontWeight: 600, letterSpacing: '0.04em',
          }}>{m.kind.toUpperCase()}</span>
          <span style={{ color: 'var(--text)' }}>
            <span style={{ color: 'var(--warning)' }}>{m.name.slice(0, tail.length)}</span>{m.name.slice(tail.length)}
          </span>
          <span style={{ flex: 1 }} />
          <span style={{ fontSize: 10, color: 'var(--overlay1)' }}>{m.type}</span>
        </div>
      ))}
    </div>
  );
}

// ───────────────────────────────────────── AI Prompt overlay
function AiPromptOverlay({ open, onClose, onAccept, schema }) {
  const [prompt, setPrompt] = React.useState('top organizations by MRR with 30-day user growth');
  const [generated, setGenerated] = React.useState(null);
  const [thinking, setThinking] = React.useState(false);
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    if (open) {
      setGenerated(null);
      requestAnimationFrame(() => inputRef.current && inputRef.current.focus());
    }
  }, [open]);

  if (!open) return null;

  const run = () => {
    setThinking(true);
    setGenerated(null);
    setTimeout(() => {
      setThinking(false);
      setGenerated(SESHAT.HERO_QUERY);
    }, 1200);
  };

  return (
    <div style={{
      position: 'absolute', inset: 0, zIndex: 50,
      background: 'rgba(0,0,0,0.55)', display: 'flex', alignItems: 'flex-start',
      justifyContent: 'center', paddingTop: 120,
    }} onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} style={{
        width: 720, background: 'var(--mantle)', borderRadius: 10,
        border: '1px solid var(--surface1)', boxShadow: 'var(--shadow-modal)', overflow: 'hidden',
      }}>
        <div style={{ padding: 16, display: 'flex', alignItems: 'center', gap: 12, borderBottom: '1px solid var(--surface0)' }}>
          <div style={{
            width: 32, height: 32, borderRadius: 8,
            background: 'linear-gradient(135deg, var(--primary), var(--secondary))',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <Ph name="sparkle" size={16} color="var(--crust)" />
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--text)' }}>Ask AI to write SQL</div>
            <div style={{ fontSize: 11, color: 'var(--overlay1)' }}>Grounded in {schema || 'your schema'} · column types, indexes, FKs</div>
          </div>
          <span className="seshat-kbd">Esc</span>
        </div>
        <div style={{ padding: 16 }}>
          <textarea ref={inputRef} value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) run(); }}
            placeholder="Describe what you want…"
            style={{
              width: '100%', minHeight: 60, resize: 'vertical',
              background: 'var(--base)', border: '1px solid var(--surface1)', borderRadius: 6,
              padding: 12, color: 'var(--text)', fontFamily: 'inherit', fontSize: 13, outline: 'none',
            }} />
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 10 }}>
            {[
              'count users by plan',
              'failed webhooks in last 24h',
              'orgs with no activity in 90 days',
              'MRR cohort retention',
            ].map((s, i) => (
              <button key={i} onClick={() => setPrompt(s)} style={{
                background: 'var(--surface0)', border: '1px solid var(--surface1)',
                color: 'var(--overlay2)', padding: '3px 8px', borderRadius: 12,
                fontSize: 11, cursor: 'pointer', fontFamily: 'inherit',
              }}>{s}</button>
            ))}
          </div>
        </div>
        {(thinking || generated) && (
          <div style={{ padding: 16, borderTop: '1px solid var(--surface0)', background: 'var(--crust)' }}>
            {thinking && (
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--overlay1)', fontSize: 12 }}>
                <Ph name="circle-notch" size={12} spin color="var(--primary)" />
                Reading schema · planning joins · generating SQL…
              </div>
            )}
            {generated && (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                  <Ph name="check-circle" size={12} color="var(--success)" />
                  <span style={{ fontSize: 11, color: 'var(--overlay1)' }}>Generated 14 lines · 3 joins · estimated 142 ms on prod-postgres</span>
                </div>
                <div style={{
                  background: 'var(--base)', border: '1px solid var(--surface0)',
                  borderRadius: 4, padding: 10, maxHeight: 220, overflow: 'auto',
                }}>
                  <HighlightedSQL src={generated} />
                </div>
              </>
            )}
          </div>
        )}
        <div style={{ padding: 12, display: 'flex', gap: 8, justifyContent: 'flex-end',
                      borderTop: '1px solid var(--surface0)' }}>
          <button onClick={onClose} style={{
            background: 'transparent', border: '1px solid var(--surface1)',
            color: 'var(--overlay2)', padding: '6px 12px', borderRadius: 4,
            cursor: 'pointer', fontSize: 12, fontFamily: 'inherit',
          }}>Cancel</button>
          {!generated
            ? <button onClick={run} disabled={thinking} style={{
                background: 'var(--primary)', border: 0, color: 'var(--crust)',
                padding: '6px 14px', borderRadius: 4, cursor: 'pointer',
                fontSize: 12, fontWeight: 600, fontFamily: 'inherit',
                display: 'inline-flex', alignItems: 'center', gap: 6,
              }}>
                <Ph name="sparkle" size={11} color="var(--crust)" /> Generate
                <span className="seshat-kbd" style={{ background: 'rgba(17,17,27,0.25)', color: 'var(--crust)', borderColor: 'rgba(17,17,27,0.4)' }}>⌘↵</span>
              </button>
            : <button onClick={() => onAccept(generated)} style={{
                background: 'var(--success)', border: 0, color: 'var(--crust)',
                padding: '6px 14px', borderRadius: 4, cursor: 'pointer',
                fontSize: 12, fontWeight: 600, fontFamily: 'inherit',
                display: 'inline-flex', alignItems: 'center', gap: 6,
              }}>
                <Ph name="arrow-down" size={11} color="var(--crust)" /> Insert into editor
              </button>}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { SqlEditor, AiPromptOverlay, HighlightedSQL, tokenizeSQL });
