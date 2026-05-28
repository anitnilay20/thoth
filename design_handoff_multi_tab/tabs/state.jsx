/* Layout state for multi-tab Thoth.
   Tree model:
     group:  { type: 'group', id, tabs: [Tab], activeId }
     split:  { type: 'split', id, dir: 'row' | 'col', sizes: [n, n], children: [Node, Node] }
   Tab:      { id, kind: 'file'|'plugin', key, pinned?, modified? }
   Convention: every group has a unique id; tab ids are unique across the whole tree.
*/

const { useReducer, useCallback, useMemo } = React;

let _uid = 0;
const nid = (p) => `${p}_${++_uid}_${Math.random().toString(36).slice(2, 6)}`;

// ── builders ───────────────────────────────────────────────────────────────
function makeTab(kind, key, opts = {}) {
  return { id: nid('t'), kind, key, pinned: false, modified: false, ...opts };
}
function makeGroup(tabs = [], activeId = null) {
  return {
    type: 'group',
    id: nid('g'),
    tabs,
    activeId: activeId || (tabs[0] && tabs[0].id) || null,
  };
}
function makeSplit(dir, children, sizes = null) {
  return {
    type: 'split',
    id: nid('s'),
    dir,
    sizes: sizes || children.map(() => 1 / children.length),
    children,
  };
}

// ── tree walking ──────────────────────────────────────────────────────────
function clone(node) {
  if (!node) return node;
  if (node.type === 'group') {
    return { ...node, tabs: node.tabs.map((t) => ({ ...t })) };
  }
  return { ...node, sizes: [...node.sizes], children: node.children.map(clone) };
}
function findGroup(root, groupId, _path = []) {
  if (!root) return null;
  if (root.type === 'group' && root.id === groupId) return { node: root, path: _path };
  if (root.type === 'split') {
    for (let i = 0; i < root.children.length; i++) {
      const r = findGroup(root.children[i], groupId, [..._path, { split: root, index: i }]);
      if (r) return r;
    }
  }
  return null;
}
function findTab(root, tabId) {
  if (root.type === 'group') {
    const tab = root.tabs.find((t) => t.id === tabId);
    if (tab) return { tab, group: root };
    return null;
  }
  for (const ch of root.children) {
    const r = findTab(ch, tabId);
    if (r) return r;
  }
  return null;
}
function firstGroup(root) {
  if (root.type === 'group') return root;
  for (const ch of root.children) {
    const r = firstGroup(ch);
    if (r) return r;
  }
  return null;
}
function allGroups(root, out = []) {
  if (root.type === 'group') out.push(root);
  else root.children.forEach((c) => allGroups(c, out));
  return out;
}

// Replace nodeId with newNode anywhere in tree. Returns new tree (mutates clone path).
function replaceNode(root, nodeId, newNode) {
  if (root.id === nodeId) return newNode;
  if (root.type === 'split') {
    root.children = root.children.map((c) => replaceNode(c, nodeId, newNode));
  }
  return root;
}

// Remove an empty group; collapse the parent split (replace split with its surviving child).
function collapseIfEmpty(root, groupId) {
  // Walk and find parent. If group has 0 tabs, remove. If after removal a split has 1 child, replace split with that child.
  function walk(node, parent, indexInParent) {
    if (node.type === 'group') {
      if (node.id === groupId && node.tabs.length === 0) {
        // remove from parent
        if (!parent) {
          // it's the root and it's empty — keep a fresh empty group as root
          return { remove: true, replacement: null };
        }
        return { remove: true };
      }
      return null;
    }
    for (let i = 0; i < node.children.length; i++) {
      const r = walk(node.children[i], node, i);
      if (r && r.remove) {
        node.children.splice(i, 1);
        node.sizes.splice(i, 1);
        // re-normalize sizes
        const sum = node.sizes.reduce((a, b) => a + b, 0) || 1;
        node.sizes = node.sizes.map((s) => s / sum);
        if (node.children.length === 1) {
          // collapse split → single child
          return { remove: true, replacement: node.children[0] };
        }
        return null;
      }
      if (r && r.replacement) {
        node.children[i] = r.replacement;
        return null;
      }
    }
    return null;
  }
  const r = walk(root, null, 0);
  if (r && r.replacement) return r.replacement;
  if (r && r.remove && r.replacement === null) return makeGroup([]); // whole tree empty
  return root;
}

// ── operations ─────────────────────────────────────────────────────────────
function reducer(state, action) {
  const root = clone(state.root);
  switch (action.type) {
    case 'select': {
      const r = findGroup(root, action.groupId);
      if (!r) return state;
      r.node.activeId = action.tabId;
      return { ...state, root, focusedGroupId: action.groupId };
    }
    case 'focus': {
      return { ...state, focusedGroupId: action.groupId };
    }
    case 'open': {
      // Open a new tab in the focused group (or first group), or focus existing.
      const targetGroupId = action.groupId || state.focusedGroupId || firstGroup(root).id;
      const target = findGroup(root, targetGroupId).node;
      // De-dupe: if a tab with same kind+key exists, just focus it.
      for (const g of allGroups(root)) {
        const existing = g.tabs.find((t) => t.kind === action.kind && t.key === action.key);
        if (existing) {
          g.activeId = existing.id;
          return { ...state, root, focusedGroupId: g.id };
        }
      }
      const tab = makeTab(action.kind, action.key);
      target.tabs.push(tab);
      target.activeId = tab.id;
      return { ...state, root, focusedGroupId: target.id };
    }
    case 'close': {
      const g = findGroup(root, action.groupId).node;
      const idx = g.tabs.findIndex((t) => t.id === action.tabId);
      if (idx === -1) return state;
      g.tabs.splice(idx, 1);
      if (g.activeId === action.tabId) {
        g.activeId = g.tabs[Math.min(idx, g.tabs.length - 1)]?.id || null;
      }
      const next = collapseIfEmpty(root, g.id);
      return { ...state, root: next, focusedGroupId: findGroup(next, state.focusedGroupId) ? state.focusedGroupId : firstGroup(next).id };
    }
    case 'closeOthers': {
      const g = findGroup(root, action.groupId).node;
      g.tabs = g.tabs.filter((t) => t.id === action.tabId || t.pinned);
      g.activeId = action.tabId;
      return { ...state, root };
    }
    case 'closeAll': {
      const g = findGroup(root, action.groupId).node;
      g.tabs = g.tabs.filter((t) => t.pinned);
      g.activeId = g.tabs[0]?.id || null;
      const next = collapseIfEmpty(root, g.id);
      return { ...state, root: next, focusedGroupId: findGroup(next, state.focusedGroupId) ? state.focusedGroupId : firstGroup(next).id };
    }
    case 'closeRight': {
      const g = findGroup(root, action.groupId).node;
      const idx = g.tabs.findIndex((t) => t.id === action.tabId);
      if (idx === -1) return state;
      const keep = g.tabs.slice(0, idx + 1);
      const drop = g.tabs.slice(idx + 1).filter((t) => t.pinned);
      g.tabs = [...keep, ...drop];
      if (!g.tabs.find((t) => t.id === g.activeId)) g.activeId = action.tabId;
      return { ...state, root };
    }
    case 'pin': {
      const g = findGroup(root, action.groupId).node;
      const t = g.tabs.find((t) => t.id === action.tabId);
      if (!t) return state;
      t.pinned = !t.pinned;
      // Move pinned tabs to the front, preserving order otherwise
      g.tabs.sort((a, b) => (b.pinned - a.pinned));
      return { ...state, root };
    }
    case 'reorder': {
      // Move tab within same group to a new index.
      const g = findGroup(root, action.groupId).node;
      const from = g.tabs.findIndex((t) => t.id === action.tabId);
      if (from === -1) return state;
      const [tab] = g.tabs.splice(from, 1);
      let to = action.toIndex;
      if (from < to) to -= 1; // adjust because we removed
      // Pinned tabs always come before unpinned; keep that invariant
      g.tabs.splice(to, 0, tab);
      g.tabs.sort((a, b) => (b.pinned - a.pinned));
      return { ...state, root };
    }
    case 'moveTab': {
      // Move tab from one group to another (or another position in same group).
      // action: { tabId, fromGroupId, toGroupId, toIndex }
      const from = findGroup(root, action.fromGroupId).node;
      const to = findGroup(root, action.toGroupId).node;
      const idx = from.tabs.findIndex((t) => t.id === action.tabId);
      if (idx === -1) return state;
      const [tab] = from.tabs.splice(idx, 1);
      // restore active in source group
      if (from.activeId === tab.id) {
        from.activeId = from.tabs[Math.min(idx, from.tabs.length - 1)]?.id || null;
      }
      let insertAt = Math.max(0, Math.min(to.tabs.length, action.toIndex ?? to.tabs.length));
      to.tabs.splice(insertAt, 0, tab);
      to.activeId = tab.id;
      to.tabs.sort((a, b) => (b.pinned - a.pinned));
      const next = collapseIfEmpty(root, from.id);
      return { ...state, root: next, focusedGroupId: to.id };
    }
    case 'split': {
      // action: { targetGroupId, tabId, fromGroupId, edge: 'left'|'right'|'top'|'bottom' }
      const sourceGroup = findGroup(root, action.fromGroupId).node;
      // No-op: dragging the only tab from a group into a split of itself.
      if (action.fromGroupId === action.targetGroupId && sourceGroup.tabs.length === 1) return state;
      const tabIdx = sourceGroup.tabs.findIndex((t) => t.id === action.tabId);
      if (tabIdx === -1) return state;
      const [tab] = sourceGroup.tabs.splice(tabIdx, 1);
      if (sourceGroup.activeId === tab.id) {
        sourceGroup.activeId = sourceGroup.tabs[Math.min(tabIdx, sourceGroup.tabs.length - 1)]?.id || null;
      }
      const newGroup = makeGroup([tab], tab.id);
      // Find target group (may be same as source). It may have been emptied by removal but not yet collapsed — we'll handle that after.
      const t = findGroup(root, action.targetGroupId);
      if (!t) return state;
      const targetGroup = t.node;
      const dir = (action.edge === 'left' || action.edge === 'right') ? 'row' : 'col';
      const order = (action.edge === 'left' || action.edge === 'top')
        ? [newGroup, targetGroup]
        : [targetGroup, newGroup];
      const split = makeSplit(dir, order);
      const next = replaceNode(root, targetGroup.id, split);
      const cleaned = collapseIfEmpty(next, sourceGroup.id);
      return { ...state, root: cleaned, focusedGroupId: newGroup.id };
    }
    case 'resize': {
      // action: { splitId, sizes: number[] }
      function walk(n) {
        if (n.type === 'split') {
          if (n.id === action.splitId) n.sizes = action.sizes;
          n.children.forEach(walk);
        }
      }
      walk(root);
      return { ...state, root };
    }
    case 'reset': {
      return action.state;
    }
    case 'toggleModified': {
      const g = findGroup(root, action.groupId).node;
      const t = g.tabs.find((t) => t.id === action.tabId);
      if (t) t.modified = !t.modified;
      return { ...state, root };
    }
    default:
      return state;
  }
}

// ── initial layout ────────────────────────────────────────────────────────
function makeInitialState() {
  const left = makeGroup([
    makeTab('file', 'users.json', { pinned: true }),
    makeTab('file', 'events.ndjson'),
    makeTab('file', 'config.json', { modified: true }),
  ]);
  left.activeId = left.tabs[1].id;
  const right = makeGroup([
    makeTab('plugin', 'schema-validator'),
  ]);
  const root = makeSplit('row', [left, right], [0.62, 0.38]);
  return { root, focusedGroupId: left.id };
}

// ── hook ──────────────────────────────────────────────────────────────────
function useLayout() {
  const [state, dispatch] = useReducer(reducer, null, makeInitialState);
  const api = useMemo(() => ({
    select: (groupId, tabId) => dispatch({ type: 'select', groupId, tabId }),
    focus: (groupId) => dispatch({ type: 'focus', groupId }),
    open: (kind, key, groupId) => dispatch({ type: 'open', kind, key, groupId }),
    close: (groupId, tabId) => dispatch({ type: 'close', groupId, tabId }),
    closeOthers: (groupId, tabId) => dispatch({ type: 'closeOthers', groupId, tabId }),
    closeAll: (groupId) => dispatch({ type: 'closeAll', groupId }),
    closeRight: (groupId, tabId) => dispatch({ type: 'closeRight', groupId, tabId }),
    pin: (groupId, tabId) => dispatch({ type: 'pin', groupId, tabId }),
    reorder: (groupId, tabId, toIndex) => dispatch({ type: 'reorder', groupId, tabId, toIndex }),
    moveTab: (tabId, fromGroupId, toGroupId, toIndex) => dispatch({ type: 'moveTab', tabId, fromGroupId, toGroupId, toIndex }),
    split: (targetGroupId, tabId, fromGroupId, edge) => dispatch({ type: 'split', targetGroupId, tabId, fromGroupId, edge }),
    resize: (splitId, sizes) => dispatch({ type: 'resize', splitId, sizes }),
    reset: () => dispatch({ type: 'reset', state: makeInitialState() }),
    toggleModified: (groupId, tabId) => dispatch({ type: 'toggleModified', groupId, tabId }),
  }), []);
  return [state, api];
}

Object.assign(window, { useLayout, makeTab, makeGroup, makeSplit, findGroup, findTab, allGroups, makeInitialState });
