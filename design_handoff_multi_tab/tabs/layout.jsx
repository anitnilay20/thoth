/* Tab strip, group pane with drop zones, split renderer with resizable handles.
   Drag is implemented with HTML5 DnD + a window-level dragging-tab handle for cross-group state. */

const { useState: lyUseState, useRef: lyUseRef, useEffect: lyUseEffect, useMemo: lyUseMemo, useLayoutEffect } = React;

// ── drag handle (window-level) ─────────────────────────────────────────────
// Reading dataTransfer in dragover is restricted; we use a window-scoped handle.
window.__thothDrag = null;

function setDrag(d) { window.__thothDrag = d; }
function getDrag() { return window.__thothDrag; }
function clearDrag() { window.__thothDrag = null; }

// ── tab visuals ─────────────────────────────────────────────────────────────
function tabIcon(tab) {
  if (tab.kind === 'file') {
    const f = window.THOTH_FILES[tab.key];
    return f?.type === 'NDJSON' ? 'rows' : 'brackets-curly';
  }
  const p = window.THOTH_PLUGINS[tab.key];
  return p?.icon || 'puzzle-piece';
}
function tabAccent(tab) {
  if (tab.kind === 'file') return 'accent';
  return window.THOTH_PLUGINS[tab.key]?.accent || 'primary';
}
function tabTitle(tab) {
  if (tab.kind === 'file') return tab.key;
  return window.THOTH_PLUGINS[tab.key]?.title || tab.key;
}

// ── Tab ─────────────────────────────────────────────────────────────────────
function Tab({ tab, group, active, focused, api, onContext }) {
  const [hover, setHover] = lyUseState(false);
  const [closeHover, setCloseHover] = lyUseState(false);
  const accent = tabAccent(tab);
  const icon = tabIcon(tab);
  const isActive = active;
  // VSCode-style: top accent strip for active tab in focused group, dimmer for active-but-unfocused
  const topAccent = isActive
    ? (focused ? `var(--${accent})` : 'var(--surface2)')
    : 'transparent';
  const bg = isActive ? 'var(--base)' : hover ? 'var(--surface0)' : 'var(--mantle)';
  const color = isActive ? 'var(--text)' : 'var(--overlay1)';

  return (
    <div
      draggable
      onDragStart={(e) => {
        setDrag({ tabId: tab.id, fromGroupId: group.id });
        e.dataTransfer.effectAllowed = 'move';
        // Use a minimal ghost — let the browser show the tab itself
        try { e.dataTransfer.setData('text/plain', tab.id); } catch {}
      }}
      onDragEnd={() => clearDrag()}
      onClick={() => { api.select(group.id, tab.id); api.focus(group.id); }}
      onMouseDown={(e) => { if (e.button === 1) { e.preventDefault(); api.close(group.id, tab.id); } }}
      onContextMenu={(e) => { e.preventDefault(); onContext(e, tab); }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        height: 35,
        display: 'inline-flex', alignItems: 'center', gap: 6,
        padding: tab.pinned ? '0 8px 0 10px' : '0 8px 0 12px',
        background: bg,
        color,
        borderRight: '1px solid var(--surface0)',
        cursor: 'pointer',
        position: 'relative',
        fontSize: 'var(--fs-md)',
        userSelect: 'none',
        flexShrink: 0,
        maxWidth: 240,
        transition: 'background var(--d-fast)',
      }}
      data-tab-id={tab.id}
    >
      {/* top accent bar */}
      <span style={{
        position: 'absolute', top: 0, left: 0, right: 0, height: 2,
        background: topAccent, transition: 'background var(--d-fast)',
      }} />
      {/* pin marker (rotated icon) */}
      {tab.pinned && <i className="ph ph-push-pin-simple" style={{ fontSize: 11, color: 'var(--overlay2)', transform: 'rotate(45deg)' }} />}
      <i className={`ph ph-${icon}`} style={{ fontSize: 14, color: isActive ? `var(--${accent})` : color, flexShrink: 0 }} />
      <span style={{
        fontStyle: tab.kind === 'plugin' ? 'normal' : 'normal',
        whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
      }}>{tabTitle(tab)}</span>
      {/* close / modified dot — dot until hover, then × */}
      <span
        onClick={(e) => { e.stopPropagation(); api.close(group.id, tab.id); }}
        onMouseEnter={() => setCloseHover(true)}
        onMouseLeave={() => setCloseHover(false)}
        style={{
          width: 18, height: 18, borderRadius: 3, marginLeft: 4,
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
          background: closeHover ? 'var(--surface1)' : 'transparent',
          flexShrink: 0,
        }}
      >
        {tab.modified && !hover ? (
          <span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--text)' }} />
        ) : (
          <i className="ph ph-x" style={{ fontSize: 11, opacity: hover ? 1 : 0, transition: 'opacity var(--d-fast)', color: 'var(--text)' }} />
        )}
      </span>
    </div>
  );
}

// ── Tab strip with reorder ──────────────────────────────────────────────────
function TabStrip({ group, focused, api, onContext, onStripEmptyDrop }) {
  const [hoverIdx, setHoverIdx] = lyUseState(null); // insertion index (between tabs)
  const stripRef = lyUseRef(null);

  const onStripDragOver = (e) => {
    const d = getDrag();
    if (!d) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    // Compute insertion index based on cursor x relative to tab elements
    const tabs = Array.from(stripRef.current.querySelectorAll('[data-tab-id]'));
    let idx = tabs.length;
    for (let i = 0; i < tabs.length; i++) {
      const r = tabs[i].getBoundingClientRect();
      if (e.clientX < r.left + r.width / 2) { idx = i; break; }
    }
    setHoverIdx(idx);
  };
  const onStripDragLeave = (e) => {
    // Only clear if leaving the strip element (not a child)
    if (e.currentTarget.contains(e.relatedTarget)) return;
    setHoverIdx(null);
  };
  const onStripDrop = (e) => {
    const d = getDrag();
    if (!d) return;
    e.preventDefault();
    e.stopPropagation();
    const idx = hoverIdx ?? group.tabs.length;
    if (d.fromGroupId === group.id) {
      api.reorder(group.id, d.tabId, idx);
    } else {
      api.moveTab(d.tabId, d.fromGroupId, group.id, idx);
    }
    setHoverIdx(null);
    clearDrag();
  };

  return (
    <div
      ref={stripRef}
      onDragOver={onStripDragOver}
      onDragLeave={onStripDragLeave}
      onDrop={onStripDrop}
      style={{
        height: 35, display: 'flex', background: 'var(--mantle)',
        borderBottom: '1px solid var(--surface0)',
        position: 'relative', overflowX: 'auto', overflowY: 'hidden',
        flexShrink: 0,
      }}
    >
      {group.tabs.length === 0 && (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--overlay1)', fontSize: 'var(--fs-sm)' }}>
          Drop a tab here
        </div>
      )}
      {group.tabs.map((tab, i) => (
        <Tab
          key={tab.id}
          tab={tab}
          group={group}
          active={tab.id === group.activeId}
          focused={focused}
          api={api}
          onContext={onContext}
        />
      ))}
      {/* trailing space to enable drop at end */}
      <div style={{ flex: 1, minWidth: 80 }} />
      {/* insertion indicator */}
      {hoverIdx !== null && (
        <InsertionLine stripRef={stripRef} index={hoverIdx} tabCount={group.tabs.length} />
      )}
    </div>
  );
}
function InsertionLine({ stripRef, index, tabCount }) {
  const [x, setX] = lyUseState(0);
  useLayoutEffect(() => {
    if (!stripRef.current) return;
    const tabs = Array.from(stripRef.current.querySelectorAll('[data-tab-id]'));
    const stripRect = stripRef.current.getBoundingClientRect();
    let xPx;
    if (tabs.length === 0) xPx = 4;
    else if (index >= tabs.length) {
      const last = tabs[tabs.length - 1].getBoundingClientRect();
      xPx = last.right - stripRect.left;
    } else {
      const t = tabs[index].getBoundingClientRect();
      xPx = t.left - stripRect.left;
    }
    setX(xPx);
  }, [index, tabCount]);
  return (
    <div style={{
      position: 'absolute', top: 0, bottom: 0, left: x, width: 2,
      background: 'var(--primary)', pointerEvents: 'none', zIndex: 5,
    }} />
  );
}

// ── Drop zone overlay (4 edges + center) ────────────────────────────────────
function DropOverlay({ paneRef, group, api }) {
  const [zone, setZone] = lyUseState(null); // 'left'|'right'|'top'|'bottom'|'center'|null
  const [visible, setVisible] = lyUseState(false);

  lyUseEffect(() => {
    // Track global drag start/end so overlay activates only while dragging.
    const onDragStart = () => { if (getDrag()) setVisible(true); };
    const onDragEnd = () => { setVisible(false); setZone(null); };
    // Fallback: poll briefly because dragstart fires before window.__thothDrag is set in some browsers
    document.addEventListener('dragstart', onDragStart, true);
    document.addEventListener('dragend', onDragEnd, true);
    document.addEventListener('drop', onDragEnd, true);
    return () => {
      document.removeEventListener('dragstart', onDragStart, true);
      document.removeEventListener('dragend', onDragEnd, true);
      document.removeEventListener('drop', onDragEnd, true);
    };
  }, []);

  const computeZone = (e) => {
    const rect = paneRef.current.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const y = (e.clientY - rect.top) / rect.height;
    const edge = 0.22;
    // Edges win over center; closest edge wins among edges.
    const dl = x, dr = 1 - x, dt = y, db = 1 - y;
    const m = Math.min(dl, dr, dt, db);
    if (m > edge) return 'center';
    if (m === dl) return 'left';
    if (m === dr) return 'right';
    if (m === dt) return 'top';
    return 'bottom';
  };

  const onDragOver = (e) => {
    if (!getDrag()) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    setZone(computeZone(e));
  };
  const onDragLeave = (e) => {
    if (e.currentTarget.contains(e.relatedTarget)) return;
    setZone(null);
  };
  const onDrop = (e) => {
    const d = getDrag();
    if (!d) return;
    e.preventDefault();
    const z = computeZone(e);
    setZone(null);
    setVisible(false);
    if (z === 'center') {
      if (d.fromGroupId === group.id) { clearDrag(); return; }
      api.moveTab(d.tabId, d.fromGroupId, group.id, group.tabs.length);
    } else {
      api.split(group.id, d.tabId, d.fromGroupId, z);
    }
    clearDrag();
  };

  // Geometry of the highlighted region within the pane
  const highlight = lyUseMemo(() => {
    switch (zone) {
      case 'left':   return { left: 0, top: 0, width: '50%',  height: '100%' };
      case 'right':  return { left: '50%', top: 0, width: '50%', height: '100%' };
      case 'top':    return { left: 0, top: 0, width: '100%', height: '50%' };
      case 'bottom': return { left: 0, top: '50%', width: '100%', height: '50%' };
      case 'center': return { left: 0, top: 0, width: '100%', height: '100%' };
      default: return null;
    }
  }, [zone]);

  const zoneLabel = {
    left: 'Split Left',
    right: 'Split Right',
    top: 'Split Up',
    bottom: 'Split Down',
    center: 'Add to Group',
  }[zone];

  if (!visible) return null;
  return (
    <div
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      style={{
        position: 'absolute', inset: 0, zIndex: 10,
        // transparent capture surface — visuals only when zone is computed
      }}
    >
      {highlight && (
        <div style={{
          position: 'absolute',
          left: highlight.left, top: highlight.top, width: highlight.width, height: highlight.height,
          background: 'rgba(203, 166, 247, 0.16)',
          border: '2px dashed var(--primary)',
          boxSizing: 'border-box',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          pointerEvents: 'none',
          transition: 'left var(--d-fast), top var(--d-fast), width var(--d-fast), height var(--d-fast)',
        }}>
          <span style={{
            background: 'var(--primary)', color: 'var(--crust)',
            padding: '4px 12px', borderRadius: 4,
            fontSize: 'var(--fs-md)', fontWeight: 600,
            boxShadow: 'var(--shadow-menu)',
          }}>{zoneLabel}</span>
        </div>
      )}
    </div>
  );
}

// ── Context menu ────────────────────────────────────────────────────────────
function ContextMenu({ x, y, items, onClose }) {
  lyUseEffect(() => {
    const onDown = (e) => {
      // Close on any click outside the menu
      if (e.target.closest('[data-thoth-menu]')) return;
      onClose();
    };
    const onKey = (e) => { if (e.key === 'Escape') onClose(); };
    setTimeout(() => document.addEventListener('mousedown', onDown), 0);
    document.addEventListener('keydown', onKey);
    return () => { document.removeEventListener('mousedown', onDown); document.removeEventListener('keydown', onKey); };
  }, [onClose]);
  return (
    <div data-thoth-menu style={{
      position: 'fixed', left: x, top: y, zIndex: 1000,
      background: 'var(--surface0)', border: '1px solid var(--surface1)',
      borderRadius: 4, boxShadow: 'var(--shadow-menu)', minWidth: 220,
      padding: '4px 0', fontSize: 'var(--fs-md)', color: 'var(--text)',
    }}>
      {items.map((it, i) => {
        if (it.separator) return <div key={i} style={{ height: 1, background: 'var(--surface1)', margin: '4px 0' }} />;
        return <MenuItem key={i} {...it} onClose={onClose} />;
      })}
    </div>
  );
}
function MenuItem({ label, hint, onClick, disabled, onClose }) {
  const [h, setH] = lyUseState(false);
  return (
    <div
      onClick={() => { if (!disabled) { onClick(); onClose(); } }}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => setH(false)}
      style={{
        padding: '6px 14px',
        background: h && !disabled ? 'var(--selection-bg)' : 'transparent',
        color: disabled ? 'var(--text-disabled)' : 'var(--text)',
        cursor: disabled ? 'default' : 'pointer',
        display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 24,
      }}
    >
      <span>{label}</span>
      {hint && <span style={{ color: 'var(--overlay1)', fontSize: 'var(--fs-sm)', fontFamily: 'var(--font-mono)' }}>{hint}</span>}
    </div>
  );
}

// ── GroupPane ───────────────────────────────────────────────────────────────
function GroupPane({ group, focused, api, openWelcome }) {
  const ref = lyUseRef(null);
  const [menu, setMenu] = lyUseState(null);

  const active = group.tabs.find((t) => t.id === group.activeId) || group.tabs[0];

  const onTabContext = (e, tab) => {
    setMenu({
      x: e.clientX, y: e.clientY, tab,
      items: [
        { label: 'Close',          hint: '⌘W', onClick: () => api.close(group.id, tab.id) },
        { label: 'Close Others',              onClick: () => api.closeOthers(group.id, tab.id) },
        { label: 'Close to the Right',        onClick: () => api.closeRight(group.id, tab.id) },
        { label: 'Close All',                 onClick: () => api.closeAll(group.id) },
        { separator: true },
        { label: tab.pinned ? 'Unpin Tab' : 'Pin Tab', onClick: () => api.pin(group.id, tab.id) },
        { label: tab.modified ? 'Mark as Saved' : 'Mark as Modified', onClick: () => api.toggleModified(group.id, tab.id) },
        { separator: true },
        { label: 'Split Right',    hint: '⌘\\',     onClick: () => api.split(group.id, tab.id, group.id, 'right') },
        { label: 'Split Down',                       onClick: () => api.split(group.id, tab.id, group.id, 'bottom') },
      ],
    });
  };

  return (
    <div
      ref={ref}
      onMouseDown={() => api.focus(group.id)}
      style={{
        display: 'flex', flexDirection: 'column',
        background: 'var(--base)',
        position: 'relative', minWidth: 0, minHeight: 0,
        flex: '1 1 0', overflow: 'hidden',
        outline: focused ? '0px solid transparent' : 'none',
      }}
    >
      <TabStrip group={group} focused={focused} api={api} onContext={onTabContext} />
      <div style={{ flex: 1, minHeight: 0, display: 'flex', position: 'relative' }}>
        {active ? (
          <TabContent tab={active} api={api} groupId={group.id} />
        ) : (
          <EmptyGroup onOpenWelcome={() => { api.open('plugin', 'welcome', group.id); }} />
        )}
      </div>
      <DropOverlay paneRef={ref} group={group} api={api} />
      {menu && <ContextMenu {...menu} onClose={() => setMenu(null)} />}
    </div>
  );
}
function EmptyGroup({ onOpenWelcome }) {
  return (
    <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: 8, color: 'var(--overlay1)' }}>
      <i className="ph ph-app-window" style={{ fontSize: 32 }} />
      <div style={{ fontSize: 'var(--fs-md)' }}>No tab open in this group.</div>
      <button onClick={onOpenWelcome} style={{
        marginTop: 8, background: 'transparent', color: 'var(--accent)',
        border: '1px solid var(--surface1)', padding: '4px 12px', borderRadius: 4,
        cursor: 'pointer', fontFamily: 'inherit', fontSize: 'var(--fs-md)',
      }}>Open Welcome</button>
    </div>
  );
}

// ── TabContent dispatcher ───────────────────────────────────────────────────
function TabContent({ tab, api, groupId }) {
  if (tab.kind === 'file') {
    const f = window.THOTH_FILES[tab.key];
    if (!f) return <div style={{ flex: 1, padding: 24, color: 'var(--error)' }}>File not found: {tab.key}</div>;
    return <window.JsonTree data={f.value} query="" />;
  }
  // plugin
  switch (tab.key) {
    case 'welcome':
      return <window.WelcomePanel
        onOpenFile={(name) => api.open('file', name, groupId)}
        onOpenPlugin={(id) => api.open('plugin', id, groupId)} />;
    case 'settings':          return <window.SettingsPanel />;
    case 'schema-validator':  return <window.SchemaValidatorPanel />;
    case 'diff':              return <window.DiffPanel />;
    case 'jsonpath':          return <window.JsonPathPanel />;
    default: return <div style={{ flex: 1, padding: 24, color: 'var(--overlay1)' }}>Unknown plugin: {tab.key}</div>;
  }
}

// ── Split renderer ──────────────────────────────────────────────────────────
function SplitNode({ node, focusedGroupId, api }) {
  if (node.type === 'group') {
    return <GroupPane group={node} focused={node.id === focusedGroupId} api={api} />;
  }
  return <SplitContainer split={node} focusedGroupId={focusedGroupId} api={api} />;
}

function SplitContainer({ split, focusedGroupId, api }) {
  const ref = lyUseRef(null);
  const dirRow = split.dir === 'row';
  return (
    <div
      ref={ref}
      style={{
        display: 'flex', flexDirection: dirRow ? 'row' : 'column',
        flex: '1 1 0', minWidth: 0, minHeight: 0, position: 'relative',
      }}
    >
      {split.children.map((child, i) => (
        <React.Fragment key={child.id}>
          <div style={{
            flex: `${split.sizes[i]} 1 0`, minWidth: 0, minHeight: 0,
            display: 'flex', position: 'relative',
          }}>
            <SplitNode node={child} focusedGroupId={focusedGroupId} api={api} />
          </div>
          {i < split.children.length - 1 && (
            <ResizeHandle
              dir={split.dir}
              onResize={(deltaPx) => {
                const rect = ref.current.getBoundingClientRect();
                const total = dirRow ? rect.width : rect.height;
                const delta = deltaPx / total;
                const next = [...split.sizes];
                next[i] = Math.max(0.1, next[i] + delta);
                next[i + 1] = Math.max(0.1, next[i + 1] - delta);
                // Re-normalize so they sum to original
                const sum = next.reduce((a, b) => a + b, 0);
                api.resize(split.id, next.map((s) => s / sum));
              }}
            />
          )}
        </React.Fragment>
      ))}
    </div>
  );
}
function ResizeHandle({ dir, onResize }) {
  const dragging = lyUseRef(false);
  const last = lyUseRef(0);
  const onDown = (e) => {
    e.preventDefault();
    dragging.current = true;
    last.current = dir === 'row' ? e.clientX : e.clientY;
    document.body.style.cursor = dir === 'row' ? 'col-resize' : 'row-resize';
    const onMove = (ev) => {
      if (!dragging.current) return;
      const cur = dir === 'row' ? ev.clientX : ev.clientY;
      const delta = cur - last.current;
      last.current = cur;
      onResize(delta);
    };
    const onUp = () => {
      dragging.current = false;
      document.body.style.cursor = '';
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  };
  const [hover, setHover] = lyUseState(false);
  return (
    <div
      onMouseDown={onDown}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        flex: '0 0 4px',
        background: hover ? 'var(--selection-stroke)' : 'var(--surface0)',
        cursor: dir === 'row' ? 'col-resize' : 'row-resize',
        zIndex: 2,
        transition: 'background var(--d-fast)',
      }}
    />
  );
}

Object.assign(window, { SplitNode });
