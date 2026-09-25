/* ============================================================================
   Thoth — file viewer: visual query builder + the DataView node.
   Mirrors thoth-plugin-sdk DataView (table / json / raw, 1000-row read cap)
   and issue #149 (mode toggle, SQL editor, ⌘↵, row count + exec time, inline
   error). The SQL engine below is a real in-page engine over the loaded
   records — every row count and execution time on screen is measured, not
   invented.
   ========================================================================== */
'use strict';

const $ = (s, r = document) => r.querySelector(s);
const DV_LIMIT = 1000;          // DataView::LIMIT — max rows the host draws
const RAW_JSON_CAP = 300;       // raw-view render cap, disclosed in the body
const TREE_CHUNK = 120;         // lazy-scroll page size for the raw tree

const IS_MAC = /Mac|iP(hone|ad|od)/.test(navigator.platform || navigator.userAgent);
const MOD = IS_MAC ? '⌘' : 'Ctrl';

/* ── 1. The loaded file: events.ndjson ────────────────────────────────────────
   A real NDJSON stream is not one table. Each event type carries its own
   fields, so the file below is deliberately heterogeneous: twelve event types
   over nine distinct record shapes, sharing only ts / level / event / service.
   Nothing here declares a schema - the catalog and the shapes are both read
   back off the records, the way the app must read them off a user's file.
   ────────────────────────────────────────────────────────────────────────── */

const REGIONS = ['us-east-1', 'us-west-2', 'eu-west-1', 'ap-south-1'];
const METHODS = ['password', 'oauth-google', 'oauth-github', 'magic-link'];
const TYPES = ['image/png', 'application/pdf', 'text/csv', 'image/jpeg'];
const QUERIES = ['status:open', 'invoice overdue', 'region eu', 'user 3312', 'retry failed'];
const DECLINES = ['insufficient_funds', 'card_expired', 'do_not_honor', 'fraud_suspected'];
const ENDPOINTS = ['/v2/hooks/dispatch', '/v2/hooks/retry', '/v2/hooks/replay'];

/** Deterministic PRNG so the file is identical on every load. */
function lcg(seed) {
  let s = seed >>> 0;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
}

/* Each builder returns only the fields that event actually carries. Weight is
   how often the event appears in the stream. */
const EVENT_KINDS = [
  {
    event: 'checkout.completed', service: 'billing', level: 'info', weight: 9,
    build: (r, id) => ({
      status: 201,
      order_id: `ord_${(100000 + Math.floor(r() * 899999)).toString(36)}`,
      amount_usd: Math.round((8 + r() * 940) * 100) / 100,
      currency: r() < 0.82 ? 'USD' : 'EUR',
      items: 1 + Math.floor(r() * 6),
      user_id: id,
      duration_ms: Math.round((90 + r() * 380) * 10) / 10,
      region: REGIONS[Math.floor(r() * REGIONS.length)],
      trace: { trace_id: `t_${Math.floor(r() * 1e9).toString(16)}`, sampled: r() < 0.3 },
    }),
  },
  {
    event: 'payment.declined', service: 'billing', level: 'warn', weight: 4,
    build: (r, id) => ({
      status: 402,
      order_id: `ord_${(100000 + Math.floor(r() * 899999)).toString(36)}`,
      amount_usd: Math.round((8 + r() * 940) * 100) / 100,
      currency: r() < 0.82 ? 'USD' : 'EUR',
      decline_code: DECLINES[Math.floor(r() * DECLINES.length)],
      user_id: id,
      region: REGIONS[Math.floor(r() * REGIONS.length)],
    }),
  },
  {
    event: 'auth.login', service: 'auth-svc', level: 'info', weight: 14,
    build: (r, id) => authShape(r, id, 200),
  },
  {
    event: 'auth.token.refresh', service: 'auth-svc', level: 'info', weight: 11,
    build: (r, id) => authShape(r, id, 200),
  },
  {
    event: 'session.expired', service: 'auth-svc', level: 'warn', weight: 5,
    build: (r, id) => authShape(r, id, 401),
  },
  {
    event: 'search.query', service: 'search-svc', level: 'info', weight: 16,
    build: (r, id) => searchShape(r, id),
  },
  {
    event: 'cache.miss', service: 'search-svc', level: 'debug', weight: 7,
    build: (r, id) => searchShape(r, id),
  },
  {
    event: 'file.upload', service: 'api-gateway', level: 'info', weight: 8,
    build: (r, id) => ({
      status: 201,
      file_id: `f_${Math.floor(r() * 1e9).toString(36)}`,
      bytes: 2048 + Math.floor(r() * 9400000),
      content_type: TYPES[Math.floor(r() * TYPES.length)],
      duration_ms: Math.round((120 + r() * 1800) * 10) / 10,
      user_id: id,
      region: REGIONS[Math.floor(r() * REGIONS.length)],
    }),
  },
  {
    event: 'webhook.delivered', service: 'api-gateway', level: 'info', weight: 9,
    build: (r) => ({
      status: 202,
      hook_id: `hk_${Math.floor(r() * 1e6).toString(36)}`,
      endpoint: ENDPOINTS[Math.floor(r() * ENDPOINTS.length)],
      attempt: 1 + Math.floor(r() * 3),
      duration_ms: Math.round((20 + r() * 260) * 10) / 10,
      trace: { trace_id: `t_${Math.floor(r() * 1e9).toString(16)}`, sampled: r() < 0.3 },
    }),
  },
  {
    event: 'rate.limited', service: 'api-gateway', level: 'warn', weight: 6,
    build: (r) => ({
      status: 429,
      client_id: `cl_${Math.floor(r() * 9999).toString().padStart(4, '0')}`,
      endpoint: ENDPOINTS[Math.floor(r() * ENDPOINTS.length)],
      limit: [100, 500, 1000, 5000][Math.floor(r() * 4)],
      window_s: [1, 60, 3600][Math.floor(r() * 3)],
    }),
  },
  {
    event: 'index.rebuild', service: 'ingest-worker', level: 'info', weight: 4,
    build: (r) => ({
      shard: `shard-${Math.floor(r() * 16).toString().padStart(2, '0')}`,
      docs: 100 + Math.floor(r() * 480000),
      duration_ms: Math.round((900 + r() * 26000) * 10) / 10,
      retries: 0,
    }),
  },
  {
    event: 'ingest.retry', service: 'ingest-worker', level: 'error', weight: 7,
    build: (r) => ({
      shard: `shard-${Math.floor(r() * 16).toString().padStart(2, '0')}`,
      docs: 100 + Math.floor(r() * 480000),
      duration_ms: Math.round((900 + r() * 26000) * 10) / 10,
      retries: 1 + Math.floor(r() * 4),
      error: ['connection reset', 'shard locked', 'disk pressure', 'checksum mismatch'][Math.floor(r() * 4)],
      trace: { trace_id: `t_${Math.floor(r() * 1e9).toString(16)}`, sampled: true },
    }),
  },
];

/* Three event types share this shape, and two share the search one - which is
   why a shape cannot simply be named after its event. */
function authShape(r, id, status) {
  return {
    status,
    user_id: id,
    method: METHODS[Math.floor(r() * METHODS.length)],
    mfa: r() < 0.38,
    ip: `${10 + Math.floor(r() * 80)}.${Math.floor(r() * 256)}.${Math.floor(r() * 256)}.${1 + Math.floor(r() * 254)}`,
    duration_ms: Math.round((8 + r() * 120) * 10) / 10,
    region: REGIONS[Math.floor(r() * REGIONS.length)],
  };
}

function searchShape(r, id) {
  const hit = r() < 0.64;
  return {
    status: 200,
    query: QUERIES[Math.floor(r() * QUERIES.length)],
    hits: Math.floor(r() * 240),
    cache_hit: hit,
    duration_ms: Math.round((hit ? 4 + r() * 40 : 60 + r() * 420) * 10) / 10,
    user_id: 100000 + Math.floor(r() * 899999),
  };
}

function buildRecords(n) {
  const rnd = lcg(20260918);
  const bag = [];
  for (const k of EVENT_KINDS) for (let i = 0; i < k.weight; i++) bag.push(k);

  const out = new Array(n);
  let t = Date.UTC(2026, 8, 20, 9, 0, 0);
  for (let i = 0; i < n; i++) {
    t += 500 + Math.floor(rnd() * 39500);
    const kind = bag[Math.floor(rnd() * bag.length)];
    out[i] = Object.assign({
      ts: new Date(t).toISOString().replace('T', ' ').replace('Z', ''),
      level: kind.level,
      event: kind.event,
      service: kind.service,
    }, kind.build(rnd, 100000 + Math.floor(rnd() * 899999)));
  }
  return out;
}

const RECORDS = buildRecords(4812);

/* ── Field catalog — read off the records, not declared ──────────────────── */

const TS_RE = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/;

function inferType(value) {
  if (value === null || value === undefined) return null;
  if (typeof value === 'boolean') return 'BOOLEAN';
  if (typeof value === 'number') return Number.isInteger(value) ? 'BIGINT' : 'DOUBLE';
  if (typeof value === 'object') return 'JSON';
  return TS_RE.test(value) ? 'TIMESTAMP' : 'VARCHAR';
}

/* A field seen as both BIGINT and DOUBLE is a DOUBLE; anything else that
   disagrees falls back to VARCHAR rather than guessing. */
function mergeType(a, b) {
  if (!a) return b;
  if (!b || a === b) return a;
  const nums = ['BIGINT', 'DOUBLE'];
  if (nums.includes(a) && nums.includes(b)) return 'DOUBLE';
  return 'VARCHAR';
}

function scanFields(records) {
  const seen = new Map();
  let order = 0;
  for (const r of records) {
    for (const k in r) {
      let f = seen.get(k);
      if (!f) { f = { name: k, type: null, n: 0, order: order++, sample: null, values: new Set() }; seen.set(k, f); }
      f.n++;
      const v = r[k];
      f.type = mergeType(f.type, inferType(v));
      if (f.sample === null && v !== null && v !== undefined) f.sample = v;
      if (f.values.size <= 64 && typeof v !== 'object') f.values.add(v);
    }
  }
  return [...seen.values()]
    .map(f => ({ ...f, type: f.type || 'VARCHAR', present: f.n / records.length }))
    .sort((a, b) => (b.present - a.present) || (a.order - b.order));
}

const FIELDS = scanFields(RECORDS);

/* ── Record types — the tables inside one file ───────────────────────────────
   A file that mixes record kinds nearly always names them in a field; here it
   is `event`. Those values are the tables. Picking one writes an ordinary
   filter, so the builder, the SQL and the picker can never disagree.
   ────────────────────────────────────────────────────────────────────────── */

/* Several fields can look like the type field - `level` here has four values
   and is in every record - but a severity is something you filter by, not a
   table. Prefer the conventional names, then the one that draws the most
   distinctions, which is what a type field is for. */
const TYPE_NAMES = ['type', 'kind', 'event', '_type', 'event_type', 'record_type', '__typename'];

const TYPE_FIELD = (() => {
  const cands = FIELDS.filter(f => f.present >= 0.9 && f.type === 'VARCHAR'
    && f.values.size > 1 && f.values.size <= 24 && f.name !== 'ts');
  if (!cands.length) return null;
  const named = cands.find(f => TYPE_NAMES.includes(f.name.toLowerCase()));
  return named || cands.reduce((a, b) => (b.values.size > a.values.size ? b : a));
})();

/** One entry per distinct value, biggest first. */
const TABLES = (() => {
  if (!TYPE_FIELD) return [];
  const counts = new Map();
  for (const r of RECORDS) {
    const v = String(r[TYPE_FIELD.name] ?? '');
    counts.set(v, (counts.get(v) || 0) + 1);
  }
  return [...counts].sort((a, b) => b[1] - a[1]).map(([label, n]) => ({ label, n }));
})();

const CATALOG = FIELDS;
const COLUMNS = CATALOG.map(f => ({ name: f.name, type: f.type }));
const COLNAMES = COLUMNS.map(c => c.name);
const TYPE_OF = Object.fromEntries(COLUMNS.map(c => [c.name, c.type]));
const FIELD_OF = Object.fromEntries(CATALOG.map(f => [f.name, f]));

/** One accessor for every record read, so a derived field stays possible. */
function getField(r, name) {
  return r[name];
}

/* -- 2. Query engine --------------------------------------------------------
   The builder owns a structured spec - filters, groups, aggregates, sort. Two
   pure functions consume it: compileSql() renders the SQL a reader can copy,
   runSpec() executes that same spec over the loaded records. Executing the spec
   instead of re-parsing the rendered text is what keeps the two honest: there
   is one source of truth, and the SQL is its rendering.
   ------------------------------------------------------------------------- */

class QueryError extends Error {}

function typeClass(type) {
  if (type === 'BIGINT' || type === 'INTEGER' || type === 'DOUBLE') return 'num';
  if (type === 'TIMESTAMP') return 'time';
  if (type === 'JSON') return 'json';
  return 'text';
}

/* A JSON column cannot group or sort meaningfully, so those lanes skip it. */
const PLAIN_FIELDS = COLUMNS.filter(c => c.type !== 'JSON').map(c => c.name);
const NUM_FIELDS = COLUMNS.filter(c => typeClass(c.type) === 'num').map(c => c.name);

/* Conditions the Filter lane offers. `arity` is how many value inputs the
   condition needs; `on` narrows it to the column kinds it reads sensibly. */
const OPERATORS = [
  { op: '=',           label: 'is',           arity: 1, on: '*'   },
  { op: '!=',          label: 'is not',       arity: 1, on: '*'   },
  { op: 'in',          label: 'is any of',    arity: 1, on: '*'   },
  { op: '>',           label: 'more than',    arity: 1, on: 'ord' },
  { op: '>=',          label: 'at least',     arity: 1, on: 'ord' },
  { op: '<',           label: 'less than',    arity: 1, on: 'ord' },
  { op: '<=',          label: 'at most',      arity: 1, on: 'ord' },
  { op: 'between',     label: 'between',      arity: 2, on: 'ord' },
  { op: 'contains',    label: 'contains',     arity: 1, on: 'str' },
  { op: 'starts',      label: 'starts with',  arity: 1, on: 'str' },
  { op: 'is null',     label: 'is empty',     arity: 0, on: '*'   },
  { op: 'is not null', label: 'is not empty', arity: 0, on: '*'   },
];

function opsFor(type) {
  const k = typeClass(type);
  return OPERATORS.filter(o => o.on === '*'
    || (o.on === 'ord' && (k === 'num' || k === 'time'))
    || (o.on === 'str' && k !== 'num'));
}
function opMeta(op) { return OPERATORS.find(o => o.op === op) || OPERATORS[0]; }

/* Aggregates the Compute lane offers. `type` is the output column type; null
   means "same as the source column", which is what MIN / MAX return. */
const AGGS = [
  { fn: 'count',    label: 'Count of rows',   field: false, numeric: false, type: 'BIGINT' },
  { fn: 'distinct', label: 'Distinct values', field: true,  numeric: false, type: 'BIGINT' },
  { fn: 'sum',      label: 'Sum',             field: true,  numeric: true,  type: 'DOUBLE' },
  { fn: 'avg',      label: 'Average',         field: true,  numeric: true,  type: 'DOUBLE' },
  { fn: 'min',      label: 'Minimum',         field: true,  numeric: false, type: null     },
  { fn: 'max',      label: 'Maximum',         field: true,  numeric: false, type: null     },
];
function aggMeta(fn) { return AGGS.find(a => a.fn === fn) || AGGS[0]; }
function aggFields(fn) { return aggMeta(fn).numeric ? NUM_FIELDS : PLAIN_FIELDS; }

function aggName(a) {
  if (!aggMeta(a.fn).field) return 'count';
  return `${a.fn}_${a.field}`;
}
function aggType(a) {
  return aggMeta(a.fn).type || TYPE_OF[a.field] || 'VARCHAR';
}
function aggSql(a) {
  if (!aggMeta(a.fn).field) return 'count(*) AS count';
  if (a.fn === 'distinct') return `count(DISTINCT ${a.field}) AS ${aggName(a)}`;
  return `${a.fn}(${a.field}) AS ${aggName(a)}`;
}

/** A grouped result's columns are fully determined before anything runs. */
function groupedColumns(spec) {
  const cols = spec.groupBy.map(f => ({ name: f, type: TYPE_OF[f] }));
  for (const a of spec.aggs) cols.push({ name: aggName(a), type: aggType(a) });
  return cols;
}

const AUTO_PRESENCE = 0.6;   // a column has to be in most rows to earn its width
const AUTO_MAX = 14;

/* With mixed records there is no single right column set, so the default is
   the set actually populated in the rows on screen: narrow to one shape and
   that shape's fields appear on their own. Pinning a column in the Fields
   panel freezes the choice and nothing here overrides it. */
function autoColumns(rows, sameType) {
  if (!rows.length) return [];
  const count = new Map();
  for (const r of rows) for (const k in r) count.set(k, (count.get(k) || 0) + 1);
  return CATALOG
    /* One table selected means the type is the same in every row and the
       picker already says which - it does not need a column too. */
    .filter(f => !(sameType && TYPE_FIELD && f.name === TYPE_FIELD.name))
    .filter(f => (count.get(f.name) || 0) / rows.length >= AUTO_PRESENCE)
    .slice(0, AUTO_MAX)
    .map(f => ({ name: f.name, type: f.type }));
}

function resultColumns(spec, rows, sameType) {
  if (spec.groupBy.length || spec.aggs.length) return groupedColumns(spec);
  if (spec.columns.length) return spec.columns.map(n => ({ name: n, type: TYPE_OF[n] }));
  return autoColumns(rows, sameType);
}

function listOf(value) {
  return String(value ?? '').split(',').map(s => s.trim()).filter(Boolean);
}

/* -- spec to SQL -------------------------------------------------------------
   Renders while the user is mid-edit, so an unfilled value prints as `?`
   rather than throwing. Running is what validates. */

function sqlLit(field, value) {
  const s = String(value ?? '').trim();
  if (!s) return '?';
  const numeric = typeClass(TYPE_OF[field]) === 'num';
  if (numeric && /^-?\d+(\.\d+)?$/.test(s)) return s;
  return `'${s.replace(/'/g, "''")}'`;
}

function filterSql(f) {
  const raw = String(f.value ?? '').trim();
  switch (f.op) {
    case 'is null':     return `${f.field} IS NULL`;
    case 'is not null': return `${f.field} IS NOT NULL`;
    case 'between':     return `${f.field} BETWEEN ${sqlLit(f.field, f.value)} AND ${sqlLit(f.field, f.value2)}`;
    case 'contains':    return `${f.field} LIKE ${raw ? sqlLit(f.field, `%${raw}%`) : '?'}`;
    case 'starts':      return `${f.field} LIKE ${raw ? sqlLit(f.field, `${raw}%`) : '?'}`;
    case 'in': {
      const items = listOf(f.value);
      return `${f.field} IN (${items.length ? items.map(v => sqlLit(f.field, v)).join(', ') : '?'})`;
    }
    default: return `${f.field} ${f.op} ${sqlLit(f.field, f.value)}`;
  }
}

function compileSql(spec) {
  const grouped = spec.groupBy.length || spec.aggs.length;
  const select = grouped
    ? [...spec.groupBy, ...spec.aggs.map(aggSql)].join(', ')
    : (spec.columns.length ? spec.columns.join(', ') : '*');
  const lines = [`SELECT ${select}`, 'FROM data'];
  if (spec.filters.length) {
    const glue = spec.combine === 'any' ? '\n   OR ' : '\n  AND ';
    lines.push(`WHERE ${spec.filters.map(filterSql).join(glue)}`);
  }
  if (spec.groupBy.length) lines.push(`GROUP BY ${spec.groupBy.join(', ')}`);
  if (spec.sort.length) {
    lines.push(`ORDER BY ${spec.sort.map(s => `${s.field} ${s.dir.toUpperCase()}`).join(', ')}`);
  }
  lines.push(`LIMIT ${spec.limit}`);
  return lines.join('\n');
}

/* -- spec to rows --------------------------------------------------------- */

function likeToRegex(pattern) {
  const body = pattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
                      .replace(/%/g, '[\\s\\S]*').replace(/_/g, '[\\s\\S]');
  return new RegExp(`^${body}$`, 'i');
}

const asText = v => (v === null || v === undefined) ? ''
  : typeof v === 'object' ? JSON.stringify(v) : String(v);

/** Compile one builder condition into a row to boolean test. */
function compileFilter(f) {
  const meta = opMeta(f.op);
  const kind = typeClass(TYPE_OF[f.field]);
  const col = f.field;

  if (f.op === 'is null') return r => getField(r, col) === null || getField(r, col) === undefined;
  if (f.op === 'is not null') return r => getField(r, col) !== null && getField(r, col) !== undefined;

  /* Values come from a typed input, so the only failures left are an empty
     entry and a non-numeric one - both worth naming in the user's own words. */
  const need = (value, which) => {
    const s = String(value ?? '').trim();
    if (!s) {
      throw new QueryError(`"${col} ${meta.label}" is missing ${which || 'a value'} - fill it in or remove the filter.`);
    }
    if (kind === 'num') {
      const n = Number(s);
      if (!Number.isFinite(n)) {
        throw new QueryError(`${col} is ${TYPE_OF[col]} - "${s}" is not a number.`);
      }
      return n;
    }
    return s;
  };

  if (f.op === 'in') {
    const items = listOf(f.value);
    if (!items.length) {
      throw new QueryError(`"${col} is any of" needs at least one value, comma separated.`);
    }
    const set = new Set(items.map(v => v.toLowerCase()));
    return r => set.has(asText(getField(r, col)).toLowerCase());
  }
  if (f.op === 'contains' || f.op === 'starts') {
    const v = need(f.value);
    const re = likeToRegex(f.op === 'contains' ? `%${v}%` : `${v}%`);
    return r => re.test(asText(getField(r, col)));
  }
  if (f.op === 'between') {
    const lo = need(f.value, 'a lower bound');
    const hi = need(f.value2, 'an upper bound');
    return r => {
      const d1 = cmpCell(getField(r, col), lo, kind);
      const d2 = cmpCell(getField(r, col), hi, kind);
      return d1 !== null && d2 !== null && d1 >= 0 && d2 <= 0;
    };
  }

  const v = need(f.value);
  switch (f.op) {
    case '=':  return r => cmpCell(getField(r, col), v, kind) === 0;
    case '!=': return r => { const d = cmpCell(getField(r, col), v, kind); return d !== null && d !== 0; };
    case '>':  return r => { const d = cmpCell(getField(r, col), v, kind); return d !== null && d > 0; };
    case '>=': return r => { const d = cmpCell(getField(r, col), v, kind); return d !== null && d >= 0; };
    case '<':  return r => { const d = cmpCell(getField(r, col), v, kind); return d !== null && d < 0; };
    case '<=': return r => { const d = cmpCell(getField(r, col), v, kind); return d !== null && d <= 0; };
  }
  throw new QueryError(`Unsupported condition "${f.op}" on ${col}.`);
}

/** null when the cell is absent, so a comparison against it is never true. */
function cmpCell(cell, value, kind) {
  if (cell === null || cell === undefined) return null;
  if (kind === 'num') {
    const n = Number(cell);
    return Number.isNaN(n) ? null : (n < value ? -1 : n > value ? 1 : 0);
  }
  const a = asText(cell), b = String(value);
  return a < b ? -1 : a > b ? 1 : 0;
}

function aggregate(a, rows) {
  if (!aggMeta(a.fn).field) return rows.length;
  const field = a.field;
  if (a.fn === 'distinct') {
    const seen = new Set();
    for (const r of rows) {
      const v = getField(r, field);
      if (v !== null && v !== undefined) seen.add(asText(v));
    }
    return seen.size;
  }
  if (a.fn === 'sum' || a.fn === 'avg') {
    let total = 0, n = 0;
    for (const r of rows) {
      const v = Number(getField(r, field));
      if (Number.isFinite(v)) { total += v; n++; }
    }
    if (a.fn === 'sum') return Math.round(total * 10) / 10;
    return n ? Math.round((total / n) * 10) / 10 : null;
  }
  const kind = typeClass(TYPE_OF[field]);
  let best = null;
  for (const r of rows) {
    const v = getField(r, field);
    if (v === null || v === undefined) continue;
    if (best === null) { best = v; continue; }
    const d = cmpCell(v, kind === 'num' ? Number(best) : asText(best), kind);
    if (d === null) continue;
    if (a.fn === 'min' ? d < 0 : d > 0) best = v;
  }
  return best;
}

function sortRows(rows, sort, cols) {
  if (!sort.length) return rows;
  const typeOf = Object.fromEntries(cols.map(c => [c.name, c.type]));
  return rows.sort((x, y) => {
    for (const s of sort) {
      const numeric = typeClass(typeOf[s.field] || TYPE_OF[s.field] || 'VARCHAR') === 'num';
      const sign = s.dir === 'asc' ? 1 : -1;
      const a = getField(x, s.field), b = getField(y, s.field);
      if (a === b) continue;
      if (a === null || a === undefined) return 1;
      if (b === null || b === undefined) return -1;
      const d = numeric ? Number(a) - Number(b) : asText(a).localeCompare(asText(b));
      if (d) return sign * d;
    }
    return 0;
  });
}

/** Same result shape the DataView already renders: columns + rows + counts. */
function runSpec(spec) {
  const tests = spec.filters.map(compileFilter);
  const pick = spec.combine === 'any' ? 'some' : 'every';

  const t0 = performance.now();
  let rows = tests.length ? RECORDS.filter(r => tests[pick](fn => fn(r))) : RECORDS.slice();
  const scanned = rows.length;
  const filtered = rows;   // pre-grouping, for the facet counts and the timeline

  const grouped = spec.groupBy.length > 0 || spec.aggs.length > 0;
  if (grouped) {
    const groups = new Map();
    for (const r of rows) {
      const key = spec.groupBy.map(f => asText(getField(r, f))).join(' | ');
      let g = groups.get(key);
      if (!g) { g = { head: r, rows: [] }; groups.set(key, g); }
      g.rows.push(r);
    }
    rows = [...groups.values()].map(g => {
      const o = {};
      for (const f of spec.groupBy) o[f] = getField(g.head, f);
      for (const a of spec.aggs) o[aggName(a)] = aggregate(a, g.rows);
      return o;
    });
  }

  rows = sortRows(rows, spec.sort, grouped ? groupedColumns(spec) : []);
  const matched = rows.length;
  const limited = rows.slice(0, spec.limit);
  const objs = limited.slice(0, DV_LIMIT);
  /* One kind of record in the result means the picker is showing a table. */
  const sameType = !!TYPE_FIELD && spec.filters.some(f =>
    f.field === TYPE_FIELD.name && f.op === '=' && String(f.value || '').trim());
  const cols = resultColumns(spec, objs, sameType);
  const ms = performance.now() - t0;

  return {
    columns: cols, objs,
    matched, selected: limited.length, drawn: objs.length, scanned, filtered,
    grouped, auto: !grouped && !spec.columns.length,
    ms, sql: compileSql(spec),
  };
}

/* ── 3. SQL highlighting — for the read-only generated-SQL panel ─────────── */

const KEYWORD_SET = new Set([
  'select', 'from', 'where', 'order', 'by', 'group', 'having', 'limit', 'offset',
  'and', 'or', 'not', 'like', 'is', 'null', 'asc', 'desc', 'as', 'distinct',
  'true', 'false', 'between', 'in', 'case', 'when', 'then', 'else', 'end',
]);

const esc = s => String(s)
  .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/* Text nodes keep their quotes - renderRawJson colours JSON by matching on
   them - so quote escaping lives in the attribute helper instead. */
const escAttr = s => esc(s).replace(/"/g, '&quot;');

/**
 * One pass over the raw SQL, escaping each token as it is emitted. Chained
 * `.replace()` passes would corrupt the markup they had just inserted, since
 * `=` and `<` also appear inside the `<span class="…">` wrappers.
 */
const SQL_TOKENS = /('(?:[^']|'')*')|(--[^\n]*)|([A-Za-z_][A-Za-z0-9_]*)|(\d+(?:\.\d+)?)|(>=|<=|!=|<>|=|>|<|\*|,|;)/g;

function highlightSql(src) {
  let out = '', last = 0, m;
  SQL_TOKENS.lastIndex = 0;
  while ((m = SQL_TOKENS.exec(src))) {
    out += esc(src.slice(last, m.index));
    const [full, str, com, word, num, op] = m;
    if (str) out += `<span class="tok-str">${esc(str)}</span>`;
    else if (com) out += `<span class="tok-com">${esc(com)}</span>`;
    else if (word) out += KEYWORD_SET.has(word.toLowerCase())
      ? `<span class="tok-kw">${esc(word)}</span>`
      : esc(word);
    else if (num) out += `<span class="tok-num">${num}</span>`;
    else out += `<span class="tok-op">${esc(op)}</span>`;
    last = m.index + full.length;
  }
  return out + esc(src.slice(last)) + '\n';
}

/* ── 4. Cell + value formatting ───────────────────────────────────────────── */

const nf = new Intl.NumberFormat('en-US');

function cellClass(type) {
  switch (type) {
    case 'BIGINT': case 'INTEGER': case 'DOUBLE': return 'r t-num';
    case 'TIMESTAMP': return 'r t-time';
    case 'BOOLEAN': return 't-bool';
    case 'JSON': return 't-json';
    default: return '';
  }
}

function cellText(value, type) {
  if (value === null || value === undefined) return '';
  if (type === 'JSON' || typeof value === 'object') return JSON.stringify(value);
  if (type === 'DOUBLE') return value.toFixed(1);
  if (type === 'BIGINT' || type === 'INTEGER') return String(value);
  return String(value);
}

/* ── 5. Views: table, JSON tree, raw ──────────────────────────────────────── */

function renderTable(result) {
  const head = result.columns.map(c => {
    const right = ['BIGINT', 'INTEGER', 'DOUBLE', 'TIMESTAMP'].includes(c.type);
    return `<th class="${right ? 'r' : ''}">${esc(c.name)}<span class="ty">${c.type}</span></th>`;
  }).join('');

  const body = result.objs.map((row, i) => {
    const tds = result.columns.map(c => {
      const raw = getField(row, c.name);
      /* A field this record does not carry is absent, not blank - with mixed
         shapes that difference is the whole point of the table. */
      if (raw === undefined || raw === null) return '<td class="t-nil">&mdash;</td>';
      if (typeof raw === 'object') {
        const n = Array.isArray(raw) ? raw.length : Object.keys(raw).length;
        const glyph = Array.isArray(raw) ? `[${n}]` : `{${n}}`;
        return `<td class="t-json" title="${escAttr(JSON.stringify(raw))}">`
          + `<span class="jchip">${glyph}</span></td>`;
      }
      const text = cellText(raw, c.type);
      const q = COLNAMES.includes(c.name) && text !== ''
        ? ` data-f="${c.name}" data-v="${escAttr(text)}"` : '';
      if (c.name === 'level') {
        return `<td${q}><span class="lvl lvl-${escAttr(String(raw))}">${esc(text)}</span></td>`;
      }
      return `<td class="${cellClass(c.type)}"${q} title="${escAttr(text)}">${esc(text)}</td>`;
    }).join('');
    return `<tr data-i="${i}">${tds}</tr>`;
  }).join('');

  return `<table class="tv${result.grouped ? ' grouped' : ''}">`
    + `<thead><tr>${head}</tr></thead><tbody>${body}</tbody></table>`;
}

const CARET_DOWN = '<svg width="12" height="12"><use href="#i-caret-down"/></svg>';

function summary(value) {
  if (Array.isArray(value)) return `${value.length} ${value.length === 1 ? 'item' : 'items'}`;
  const n = Object.keys(value).length;
  return `${n} ${n === 1 ? 'key' : 'keys'}`;
}

function scalarHtml(value, query) {
  if (value === null) return '<span class="nl">null</span>';
  if (typeof value === 'boolean') return `<span class="b">${value}</span>`;
  if (typeof value === 'number') return `<span class="n">${hl(String(value), query)}</span>`;
  return `<span class="s">"${hl(String(value), query)}"</span>`;
}

function hl(text, query) {
  const safe = esc(text);
  if (!query) return safe;
  const re = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
  return safe.replace(re, m => `<mark class="hit">${m}</mark>`);
}

/**
 * One JSON node. `label` is the index badge for top-level rows, the object key
 * for nested rows. Open state is carried in the markup so toggling is local.
 */
function nodeHtml(label, key, value, depth, open, query) {
  const indent = depth * 16;
  const gx = 56 + indent + 8;
  const keyHtml = key === null ? '' : `<span class="k">"${hl(key, query)}"</span><span class="p">: </span>`;
  const idx = `<span class="idx">${label === null ? '' : label}</span>`;
  const ind = `<span class="ind" style="width:${indent}px"></span>`;

  if (value === null || typeof value !== 'object') {
    return `<div class="tnode"><div class="tline">${idx}${ind}` +
           `<span class="tw leaf">${CARET_DOWN}</span>${keyHtml}${scalarHtml(value, query)}</div></div>`;
  }
  const isArr = Array.isArray(value);
  const [o, c] = isArr ? ['[', ']'] : ['{', '}'];
  const entries = isArr ? value.map((v, i) => [String(i), v]) : Object.entries(value);
  const kids = entries.map(([k, v]) =>
    nodeHtml(null, isArr ? null : k, v, depth + 1, false, query)).join('');
  return `<div class="tnode">` +
    `<div class="tline" aria-expanded="${open}" data-kids="1">${idx}${ind}` +
    `<span class="tw">${CARET_DOWN}</span>${keyHtml}<span class="p">${o}</span>` +
    ` <span class="summ">${summary(value)}</span> <span class="p">${c}</span></div>` +
    `<div class="tkids" style="--gx:${gx}px"${open ? '' : ' hidden'}>${kids}</div></div>`;
}

function renderTreeRange(records, from, to, query, labelOffset) {
  let out = '';
  for (let i = from; i < to && i < records.length; i++) {
    out += nodeHtml(labelOffset === null ? null : i, null, records[i], 0, i === from && from === 0, query);
  }
  return out;
}

function renderRawJson(objs) {
  const shown = objs.slice(0, RAW_JSON_CAP);
  const json = esc(JSON.stringify(shown, null, 2));
  const colored = json
    .replace(/"([^"\\]*(?:\\.[^"\\]*)*)"(\s*:)/g, '<span class="k">"$1"</span>$2')
    .replace(/: "([^"\\]*(?:\\.[^"\\]*)*)"/g, ': <span class="s">"$1"</span>')
    .replace(/: (-?\d+(?:\.\d+)?)/g, ': <span class="n">$1</span>')
    .replace(/: (true|false)/g, ': <span class="b">$1</span>')
    .replace(/: (null)/g, ': <span class="nl">$1</span>');
  const note = objs.length > RAW_JSON_CAP
    ? `<div class="loadmore" style="padding-left:14px">Showing the first ${nf.format(RAW_JSON_CAP)} of ` +
      `${nf.format(objs.length)} rows as raw JSON — switch to Table for the whole page.</div>`
    : '';
  return note + `<pre class="code">${colored}</pre>`;
}

function emptyState(title, sub) {
  return `<div class="empty"><div class="ttl">${title}</div><div class="sub">${sub}</div></div>`;
}

/* -- 6. State ------------------------------------------------------------- */

/* The default question the viewer opens on: newest records first, no filter. */
const DEFAULT_SPEC = {
  combine: 'all',
  /* The biggest table, not every record: one type means real columns. */
  filters: (TYPE_FIELD && TABLES.length)
    ? [{ field: TYPE_FIELD.name, op: '=', value: TABLES[0].label, value2: '' }]
    : [],
  groupBy: [],
  aggs: [],
  sort: [{ field: 'ts', dir: 'desc' }],
  columns: [],          // empty = columns follow the result
  limit: 1000,
};

const store = {
  get(k, fb) { try { return localStorage.getItem('thoth.' + k) ?? fb; } catch { return fb; } },
  set(k, v) { try { localStorage.setItem('thoth.' + k, v); } catch { /* private mode */ } },
};

/* Every persisted spec goes back through normalizeSpec, so an older or
   hand-edited entry cannot boot the builder into an unrunnable shape. */
function normalizeSpec(raw) {
  const s = {
    combine: raw && raw.combine === 'any' ? 'any' : 'all',
    filters: Array.isArray(raw && raw.filters) ? raw.filters : [],
    groupBy: Array.isArray(raw && raw.groupBy) ? raw.groupBy : [],
    aggs: Array.isArray(raw && raw.aggs) ? raw.aggs : [],
    sort: Array.isArray(raw && raw.sort) ? raw.sort : [],
    columns: Array.isArray(raw && raw.columns) ? raw.columns : [],
    limit: Number(raw && raw.limit) || DEFAULT_SPEC.limit,
  };

  s.columns = [...new Set(s.columns.filter(n => COLNAMES.includes(n)))];

  s.filters = s.filters
    .filter(f => f && COLNAMES.includes(f.field))
    .map(f => {
      const allowed = opsFor(TYPE_OF[f.field]).map(o => o.op);
      return {
        field: f.field,
        op: allowed.includes(f.op) ? f.op : '=',
        value: f.value ?? '',
        value2: f.value2 ?? '',
      };
    });

  s.groupBy = [...new Set(s.groupBy.filter(f => PLAIN_FIELDS.includes(f)))];

  s.aggs = s.aggs
    .filter(a => a && AGGS.some(x => x.fn === a.fn))
    .map(a => {
      if (!aggMeta(a.fn).field) return { fn: a.fn };
      const fields = aggFields(a.fn);
      return { fn: a.fn, field: fields.includes(a.field) ? a.field : fields[0] };
    });

  /* Grouping changes the result's shape, so a sort on a column that is no
     longer produced is dropped rather than silently ignored at run time. */
  const sortable = new Set(sortFields(s));
  const seen = new Set();
  s.sort = s.sort.filter(x => {
    if (!x || !sortable.has(x.field) || seen.has(x.field)) return false;
    seen.add(x.field);
    return true;
  }).map(x => ({ field: x.field, dir: x.dir === 'asc' ? 'asc' : 'desc' }));

  s.limit = Math.min(10000, Math.max(1, Math.round(s.limit)));
  return s;
}

function sortFields(spec) {
  if (spec.groupBy.length || spec.aggs.length) {
    return groupedColumns(spec).filter(c => c.type !== 'JSON').map(c => c.name);
  }
  return PLAIN_FIELDS;
}

function loadSpec() {
  try {
    const raw = store.get('spec', '');
    return raw ? normalizeSpec(JSON.parse(raw)) : normalizeSpec(DEFAULT_SPEC);
  } catch {
    return normalizeSpec(DEFAULT_SPEC);
  }
}

const state = {
  theme: store.get('theme', 'mocha'),
  view: store.get('view', 'table'),
  spec: loadSpec(),
  sqlOpen: store.get('sqlOpen', '0') === '1',
  qbOpen: store.get('qbOpen', '1') === '1',
  result: null,
  error: null,
  running: false,
  detail: null,                                 // index of the record in the drawer
  facetShut: new Set(),                         // folded facet groups
  facetMore: new Set(),                         // facet groups showing every value
  section: 'fields',
};

function commitSpec({ builder = true } = {}) {
  /* The drawer points at a row index, and a new query renumbers the rows. */
  state.detail = null;
  state.spec = normalizeSpec(state.spec);
  store.set('spec', JSON.stringify(state.spec));
  execute({ builder });
}

const el = {
  html: document.documentElement,
  body: $('#body'), dvbody: $('#dvbody'), dvbar: $('#dvbar'), rcount: $('#rcount'),
  veil: $('#veil'), veilTxt: $('#veilTxt'),
  tableSel: $('#tableSel'), tableMenu: $('#tableMenu'),
  tableLbl: $('#tableLbl'), tableCnt: $('#tableCnt'),
  viewSel: $('#viewSel'), viewMenu: $('#viewMenu'), viewLbl: $('#viewLbl'), viewIcon: $('#viewIcon'),
  exportSel: $('#exportSel'), exportMenu: $('#exportMenu'),
  themeBtn: $('#themeBtn'), themeMenu: $('#themeMenu'), themeIcon: $('#themeIcon'),
  lanes: $('#qbLanes'), lnFilters: $('#lnFilters'), lnGroup: $('#lnGroup'),
  lnAggs: $('#lnAggs'), lnSort: $('#lnSort'), combineSeg: $('#combineSeg'),
  qstatus: $('#qstatus'), limIn: $('#limIn'),
  sqlDisc: $('#sqlDisc'), sqlBox: $('#sqlBox'), sqlHl: $('#sqlHl'), sqlCopyLbl: $('#sqlCopyLbl'),
  qbToggle: $('#qbToggle'), qbBody: $('#qbBody'), qbSum: $('#qbSum'), qbCount: $('#qbCount'),
  tlPlot: $('#tlPlot'), tlBars: $('#tlBars'), tlBand: $('#tlBand'), tlDrag: $('#tlDrag'),
  tlFrom: $('#tlFrom'), tlTo: $('#tlTo'), tlLegend: $('#tlLegend'),
  tlClear: $('#tlClear'), tlRange: $('#tlRange'),
  detail: $('#detail'), detailIdx: $('#detailIdx'), detailBody: $('#detailBody'),
  sideTitle: $('#sideTitle'), sideBody: $('#sideBody'), cellact: $('#cellact'),
  stItems: $('#stItems'), stMode: $('#stMode'),
  stSig: $('#stSig'), stSigVal: $('#stSigVal'),
  copyLbl: $('#copyLbl'),
};

/* View meta for the DataView selector */
const VIEW_META = {
  'table': ['Table', '#i-table'],
  'json': ['JSON', '#i-braces'],
  'raw': ['Raw', '#i-code'],
  'chart': ['Chart', '#i-chart'],
};

/* -- 7. Query builder -------------------------------------------------------
   Every lane renders from the spec, so the DOM is never the source of truth.
   Selects commit on change and re-render; text inputs mutate in place and only
   re-render the SQL, which keeps the caret where the user left it. */

const CARET_SM = '<svg class="caret" width="12" height="12"><use href="#i-caret-down"/></svg>';
const RM_BTN = '<button class="rm" data-role="remove" title="Remove" aria-label="Remove">'
  + '<svg width="11" height="11"><use href="#i-x"/></svg></button>';

/* A placeholder is the field's own first value, so it teaches the column. */
const EXAMPLE = Object.fromEntries(CATALOG.map(f => {
  const v = f.sample;
  return [f.name, (v === null || v === undefined || typeof v === 'object') ? '' : String(v).slice(0, 19)];
}));

function selHtml(role, value, options, aria) {
  const body = options.map(o => {
    const v = typeof o === 'string' ? o : o.v;
    const l = typeof o === 'string' ? o : o.l;
    return `<option value="${escAttr(v)}"${v === value ? ' selected' : ''}>${esc(l)}</option>`;
  }).join('');
  return `<span class="sel"><select data-role="${role}" aria-label="${escAttr(aria)}">${body}</select>${CARET_SM}</span>`;
}

function valInput(role, value, field, width) {
  const list = FACET_FIELDS.includes(field) ? ` list="dl-${field}"` : '';
  return `<input class="v" data-role="${role}" value="${escAttr(value ?? '')}" style="--w:${width}px"`
    + ` placeholder="${escAttr(EXAMPLE[field] || '')}" spellcheck="false" autocomplete="off"${list}`
    + ` aria-label="Value">`;
}

function filterPill(f, i) {
  const meta = opMeta(f.op);
  const ops = opsFor(TYPE_OF[f.field]).map(o => ({ v: o.op, l: o.label }));
  let value = '';
  if (meta.arity === 1) value = valInput('value', f.value, f.field, f.op === 'in' ? 132 : 96);
  if (meta.arity === 2) {
    value = valInput('value', f.value, f.field, 62)
      + '<span class="conj">and</span>'
      + valInput('value2', f.value2, f.field, 62);
  }
  return `<span class="pill" data-kind="filter" data-i="${i}">`
    + selHtml('field', f.field, COLNAMES, 'Filter field')
    + selHtml('op', f.op, ops, 'Condition')
    + value + RM_BTN + '</span>';
}

function groupPill(field, i) {
  const taken = new Set(state.spec.groupBy.filter(f => f !== field));
  const fields = PLAIN_FIELDS.filter(f => !taken.has(f));
  return `<span class="pill" data-kind="group" data-i="${i}">`
    + selHtml('field', field, fields, 'Group by field')
    + RM_BTN + '</span>';
}

function aggPill(a, i) {
  const meta = aggMeta(a.fn);
  const fns = AGGS.map(x => ({ v: x.fn, l: x.label }));
  const field = meta.field ? selHtml('field', a.field, aggFields(a.fn), 'Aggregate field') : '';
  return `<span class="pill" data-kind="agg" data-i="${i}">`
    + selHtml('fn', a.fn, fns, 'Aggregate')
    + field + RM_BTN + '</span>';
}

function sortPill(s, i) {
  const taken = new Set(state.spec.sort.map(x => x.field).filter(f => f !== s.field));
  const fields = sortFields(state.spec).filter(f => !taken.has(f));
  const asc = s.dir === 'asc';
  return `<span class="pill" data-kind="sort" data-i="${i}">`
    + selHtml('field', s.field, fields, 'Sort field')
    + `<button class="dir mono" data-role="dir" title="${asc ? 'Ascending' : 'Descending'} — click to flip"`
    + ` aria-label="Direction: ${asc ? 'ascending' : 'descending'}">${asc ? '↑' : '↓'}</button>`
    + RM_BTN + '</span>';
}

const hint = text => `<span class="lane-hint">${text}</span>`;

function renderBuilder() {
  const s = state.spec;

  el.lnFilters.innerHTML = s.filters.length
    ? s.filters.map(filterPill).join('')
    : hint(`no filters &middot; all ${nf.format(RECORDS.length)} records`);

  el.combineSeg.hidden = s.filters.length < 2;
  for (const b of el.combineSeg.querySelectorAll('button')) {
    b.setAttribute('aria-pressed', String(b.dataset.combine === s.combine));
  }

  el.lnGroup.innerHTML = s.groupBy.length
    ? s.groupBy.map(groupPill).join('')
    : hint('no grouping &middot; one row per record');

  el.lnAggs.innerHTML = s.aggs.length
    ? s.aggs.map(aggPill).join('')
    : hint('nothing computed');

  el.lnSort.innerHTML = s.sort.length
    ? s.sort.map(sortPill).join('')
    : hint('file order');

  if (el.limIn.value !== String(s.limit)) el.limIn.value = s.limit;
}

function renderSql() {
  el.sqlHl.innerHTML = highlightSql(compileSql(state.spec));
  el.sqlBox.hidden = !state.sqlOpen;
  el.sqlDisc.setAttribute('aria-expanded', String(state.sqlOpen));
}

/* -- 7b. Explorer surfaces: facets, timeline, query summary, record drawer --
   Three ways to ask the same question, all writing to the one spec: click a
   facet value, drag a time range, or edit a lane. Whichever you use, the other
   two update, so the query is never in two places at once.
   ------------------------------------------------------------------------- */

/* Shape leads; after it, any low-cardinality field is worth a facet. An id is
   never a measure - the median of a user id means nothing. */
const ID_RE = /(^|_)id$/;
const FACET_FIELDS = FIELDS
  .filter(f => f.name !== 'ts' && f.type !== 'JSON'
    && f.name !== (TYPE_FIELD && TYPE_FIELD.name)   // the table picker owns this one
    && f.values.size > 1 && f.values.size <= 24)
  .map(f => f.name).slice(0, 15);
const MEASURES = FIELDS
  .filter(f => typeClass(f.type) === 'num' && f.values.size > 24 && !ID_RE.test(f.name))
  .slice(0, 3).map(f => f.name);
const FACET_TOP = 5;
const LEVELS = [['error', 'lv-error'], ['warn', 'lv-warn'], ['info', 'lv-info'], ['debug', 'lv-debug']];

/* Value lists so a filter value can be picked instead of typed. */
function renderDatalists() {
  $('#datalists').innerHTML = FACET_FIELDS.map(field => {
    const vals = [...new Set(RECORDS.map(r => asText(getField(r, field))))].sort();
    return `<datalist id="dl-${field}">`
      + vals.map(v => `<option value="${escAttr(v)}"></option>`).join('')
      + '</datalist>';
  }).join('');
}

/* ---- The query, read back in words ------------------------------------- */

function filterWords(f) {
  const meta = opMeta(f.op);
  const val = v => esc(String(v ?? '').trim() || '?');
  if (meta.arity === 0) return `${f.field} <em>${meta.label}</em>`;
  if (meta.arity === 2) return `${f.field} <em>between</em> ${val(f.value)} <em>and</em> ${val(f.value2)}`;
  return `${f.field} <em>${meta.label}</em> ${val(f.value)}`;
}

function renderHead() {
  const s = state.spec, r = state.result;
  el.qbBody.hidden = !state.qbOpen;
  el.qbToggle.setAttribute('aria-expanded', String(state.qbOpen));
  el.qbToggle.title = state.qbOpen
    ? `Hide the query builder (${MOD}/)`
    : `Show the query builder (${MOD}/)`;

  el.qbCount.textContent = state.error ? 'query failed'
    : r ? `${nf.format(r.selected)} ${r.grouped ? 'groups' : 'rows'} · ${r.ms.toFixed(1)} ms`
    : '';

  if (state.qbOpen) { el.qbSum.innerHTML = ''; return; }

  const chips = [];
  if (s.filters.length) {
    if (s.combine === 'any' && s.filters.length > 1) chips.push('<span class="qchip plain">any of</span>');
    for (const f of s.filters) chips.push(`<span class="qchip">${filterWords(f)}</span>`);
  } else {
    chips.push('<span class="qchip plain">every record</span>');
  }
  if (s.groupBy.length) chips.push(`<span class="qchip"><em>by</em> ${esc(s.groupBy.join(', '))}</span>`);
  if (s.aggs.length) chips.push(`<span class="qchip"><em>compute</em> ${esc(s.aggs.map(aggName).join(', '))}</span>`);
  el.qbSum.innerHTML = chips.join('');
}

/* ---- Facets ------------------------------------------------------------ */

function facetActive(field, value) {
  return state.spec.filters.some(x => x.field === field
    && ((x.op === '=' && asText(x.value) === value)
      || (x.op === 'in' && listOf(x.value).includes(value))));
}

/* Clicking a second value on the same field widens `=` into `IN (…)`, the way a
   checkbox list behaves — rather than replacing the first choice. */
function toggleFacet(field, value) {
  const s = state.spec;
  const f = s.filters.find(x => x.field === field && (x.op === '=' || x.op === 'in'));
  if (!f) {
    s.filters.push({ field, op: '=', value, value2: '' });
  } else {
    const vals = f.op === 'in' ? listOf(f.value) : [asText(f.value)];
    const at = vals.indexOf(value);
    if (at >= 0) vals.splice(at, 1); else vals.push(value);
    if (!vals.length) s.filters.splice(s.filters.indexOf(f), 1);
    else if (vals.length === 1) { f.op = '='; f.value = vals[0]; }
    else { f.op = 'in'; f.value = vals.join(', '); }
  }
  commitSpec();
}

function excludeValue(field, value) {
  const s = state.spec;
  const has = s.filters.some(x => x.field === field && x.op === '!=' && asText(x.value) === value);
  if (!has) s.filters.push({ field, op: '!=', value, value2: '' });
  commitSpec();
}

/* The pin is the whole column picker: no new panel, one affordance per field. */
function facetHead(field, badge) {
  const pinned = state.spec.columns.includes(field);
  return '<div class="facet-h">'
    + '<button class="facet-t" data-role="facet-head">'
    + '<svg class="tw" width="12" height="12"><use href="#i-caret-down"/></svg>'
    + ` ${esc(field)}${badge ? `<span class="n">${badge}</span>` : ''}</button>`
    + `<button class="facet-pin" data-role="facet-pin" aria-pressed="${pinned}"`
    + ` title="${pinned ? 'Remove this column' : 'Show as a column'}">${pinned ? '\u2713' : '+'}</button>`
    + '</div>';
}

/* Pinning starts from whatever is already on screen, so the first `+` adds a
   column instead of blanking the table down to one. */
function toggleColumn(field) {
  const s = state.spec;
  if (!s.columns.length && state.result) s.columns = state.result.columns.map(c => c.name);
  const at = s.columns.indexOf(field);
  if (at >= 0) s.columns.splice(at, 1);
  else s.columns.push(field);
  commitSpec();
}

function columnsGroup() {
  const pinned = state.spec.columns;
  const shown = state.result ? state.result.columns.length : 0;
  const head = (badge, extra) => '<div class="facet-h">'
    + `<span class="facet-t static">columns<span class="n">${badge}</span></span>${extra}</div>`;

  if (!pinned.length) {
    return '<div class="facet cols" data-field="__cols" aria-expanded="true">'
      + head(shown, '')
      + '<div class="facet-vals"><div class="cols-hint">following the result</div></div></div>';
  }
  return '<div class="facet cols" data-field="__cols" aria-expanded="true">'
    + head(pinned.length, '<button class="facet-pin wide" data-role="cols-auto"'
        + ' title="Go back to automatic columns">auto</button>')
    + '<div class="facet-vals">'
    + pinned.map(n => `<button class="fval col" data-col="${escAttr(n)}" title="Remove ${escAttr(n)}">`
        + `<span class="nm">${esc(n)}</span><span class="x">\u00d7</span></button>`).join('')
    + '</div></div>';
}

function quantile(sorted, p) {
  if (!sorted.length) return null;
  const i = Math.min(sorted.length - 1, Math.floor(p * sorted.length));
  return sorted[i];
}

/* Counts describe the current result, not the file — the same contract every
   log explorer uses, so a facet always tells you what one more click would do. */
function renderFacets() {
  const rows = state.result ? state.result.filtered : RECORDS;
  const total = rows.length || 1;
  let html = '<div class="facets">' + columnsGroup();

  for (const field of FACET_FIELDS) {
    const counts = new Map();
    for (const r of rows) {
      const v = asText(getField(r, field));
      counts.set(v, (counts.get(v) || 0) + 1);
    }
    const vals = [...counts].sort((a, b) => b[1] - a[1]);
    const open = !state.facetShut.has(field);
    const all = state.facetMore.has(field);
    const shown = all ? vals : vals.slice(0, FACET_TOP);

    html += `<div class="facet" data-field="${field}" aria-expanded="${open}">`
      + facetHead(field, nf.format(vals.length))
      + '<div class="facet-vals">'
      + shown.map(([v, n]) => {
        const pct = (n / total * 100);
        return `<button class="fval" data-value="${escAttr(v)}" aria-pressed="${facetActive(field, v)}"`
          + ` title="${escAttr(v)} — ${nf.format(n)} of ${nf.format(total)} (${pct.toFixed(1)}%)">`
          + '<span class="box"></span>'
          + `<span class="nm">${esc(v || '(empty)')}</span>`
          + `<span class="n">${nf.format(n)}</span>`
          + `<span class="bar"><i style="--p:${pct.toFixed(1)}%"></i></span></button>`;
      }).join('')
      + (vals.length > FACET_TOP
        ? `<button class="fmore" data-role="facet-more">${all ? 'show top 5' : `+${nf.format(vals.length - FACET_TOP)} more`}</button>`
        : '')
      + '</div></div>';
  }

  /* Measures get percentiles rather than values — clicking one filters to the
     slow tail, which is the reason anyone opens a latency column. */
  for (const field of MEASURES) {
    const nums = rows.map(r => Number(getField(r, field))).filter(Number.isFinite).sort((a, b) => a - b);
    const p50 = quantile(nums, 0.5), p95 = quantile(nums, 0.95), max = nums[nums.length - 1];
    html += `<div class="facet" data-field="${field}" aria-expanded="${!state.facetShut.has(field)}">`
      + facetHead(field, '')
      + '<div class="facet-vals"><div class="fstat">'
      + [['p50', p50], ['p95', p95], ['max', max]].map(([k, v]) => v === null || v === undefined
        ? `<span>${k} —</span>`
        : `<button data-role="measure" data-value="${escAttr(v)}" title="Filter to ${field} at least ${v}">`
          + `${k} <b>${v.toFixed(1)}</b></button>`).join('')
      + '</div></div></div>';
  }

  el.sideBody.innerHTML = html + '</div>';
}

/* ---- Timeline ---------------------------------------------------------- */

const TL_BUCKETS = 96;
const tsToMs = ts => Date.parse(String(ts).replace(' ', 'T') + 'Z');
const msToTs = ms => new Date(ms).toISOString().replace('T', ' ').replace('Z', '').slice(0, 19);
const shortTs = ts => String(ts).slice(5, 16);

const TS_MS = new WeakMap();
let tsLo = Infinity, tsHi = -Infinity;
for (const r of RECORDS) {
  const ms = tsToMs(r.ts);
  TS_MS.set(r, ms);
  if (ms < tsLo) tsLo = ms;
  if (ms > tsHi) tsHi = ms;
}
const TS_MIN_MS = tsLo;
const TS_MAX_MS = tsHi;
const TS_SPAN = (TS_MAX_MS - TS_MIN_MS) || 1;

function timeFilter() {
  return state.spec.filters.find(x => x.field === 'ts' && x.op === 'between') || null;
}

function setTimeRange(from, to) {
  const f = timeFilter();
  if (f) { f.value = from; f.value2 = to; }
  else state.spec.filters.push({ field: 'ts', op: 'between', value: from, value2: to });
  commitSpec();
}

function clearTimeRange() {
  state.spec.filters = state.spec.filters.filter(x => !(x.field === 'ts' && x.op === 'between'));
  commitSpec();
}

function renderTimeline() {
  const rows = state.result ? state.result.filtered : RECORDS;
  const buckets = Array.from({ length: TL_BUCKETS },
    () => ({ error: 0, warn: 0, info: 0, debug: 0, n: 0 }));

  for (const r of rows) {
    const ms = TS_MS.get(r) ?? tsToMs(r.ts);
    const at = Math.floor((ms - TS_MIN_MS) / TS_SPAN * TL_BUCKETS);
    const b = buckets[Math.max(0, Math.min(TL_BUCKETS - 1, at))];
    if (b[r.level] !== undefined) b[r.level]++;
    b.n++;
  }
  const peak = Math.max(1, ...buckets.map(b => b.n));
  const width = TS_SPAN / TL_BUCKETS;

  el.tlBars.innerHTML = buckets.map((b, i) => {
    const h = b.n / peak * 100;
    const segs = LEVELS
      .map(([lv, cls]) => b[lv] ? `<i class="${cls}" style="height:${(b[lv] / b.n * h).toFixed(2)}%"></i>` : '')
      .join('');
    const worst = LEVELS.find(([lv]) => b[lv]);
    const tip = `${shortTs(msToTs(TS_MIN_MS + i * width))} · ${nf.format(b.n)} events`
      + (b.error ? ` · ${nf.format(b.error)} error` : worst ? ` · ${worst[0]}` : '');
    return `<div class="b" title="${escAttr(tip)}">${segs}</div>`;
  }).join('');

  el.tlFrom.textContent = shortTs(msToTs(TS_MIN_MS));
  el.tlTo.textContent = shortTs(msToTs(TS_MAX_MS));
  el.tlLegend.innerHTML = LEVELS
    .map(([lv, cls]) => `<span><i class="${cls}"></i>${lv}</span>`).join('')
    + `<span>peak <b class="mono">${nf.format(peak)}</b></span>`;

  const f = timeFilter();
  if (f && f.value && f.value2) {
    const a = (tsToMs(f.value) - TS_MIN_MS) / TS_SPAN * 100;
    const b = (tsToMs(f.value2) - TS_MIN_MS) / TS_SPAN * 100;
    el.tlBand.hidden = false;
    el.tlBand.style.left = `${Math.max(0, a).toFixed(2)}%`;
    el.tlBand.style.width = `${Math.min(100, b - a).toFixed(2)}%`;
    el.tlClear.hidden = false;
    el.tlRange.textContent = `${shortTs(f.value)} → ${shortTs(f.value2)}`;
  } else {
    el.tlBand.hidden = true;
    el.tlClear.hidden = true;
  }
}

/* Drag across the plot to set the range. Offsets are measured against the plot
   rect, not the event target, because the bars are children. */
let tlDrag = null;
el.tlPlot.addEventListener('pointerdown', e => {
  const rect = el.tlPlot.getBoundingClientRect();
  tlDrag = { x0: e.clientX - rect.left, rect };
  el.tlPlot.setPointerCapture(e.pointerId);
});
el.tlPlot.addEventListener('pointermove', e => {
  if (!tlDrag) return;
  const x = e.clientX - tlDrag.rect.left;
  const a = Math.max(0, Math.min(tlDrag.x0, x));
  const b = Math.min(tlDrag.rect.width, Math.max(tlDrag.x0, x));
  el.tlDrag.hidden = (b - a) < 3;
  el.tlDrag.style.left = `${a}px`;
  el.tlDrag.style.width = `${b - a}px`;
});
el.tlPlot.addEventListener('pointerup', e => {
  if (!tlDrag) return;
  const { rect, x0 } = tlDrag;
  tlDrag = null;
  el.tlDrag.hidden = true;
  const x = e.clientX - rect.left;
  const a = Math.max(0, Math.min(x0, x));
  const b = Math.min(rect.width, Math.max(x0, x));
  if ((b - a) < 3) return;
  setTimeRange(msToTs(TS_MIN_MS + (a / rect.width) * TS_SPAN),
               msToTs(TS_MIN_MS + (b / rect.width) * TS_SPAN));
});
el.tlClear.addEventListener('click', e => { e.stopPropagation(); clearTimeRange(); });

/* ---- Record drawer ----------------------------------------------------- */

function renderDetail() {
  const r = state.result;
  const i = state.detail;
  const row = r && i !== null ? r.objs[i] : null;
  el.detail.hidden = !row;
  if (!row) return;
  el.detailIdx.textContent = `#${nf.format(i)}`;
  el.detailBody.innerHTML = renderTreeRange([row], 0, 1, '', null);
  for (const tr of el.dvbody.querySelectorAll('tbody tr')) {
    tr.setAttribute('aria-selected', String(Number(tr.dataset.i) === i));
  }
}

/* A grouped row has no single record behind it, so clicking one drills in:
   the group's values become filters and the grouping drops away. */
function drillInto(i) {
  const r = state.result;
  const row = r && r.objs[i];
  if (!row) return;
  const s = state.spec;
  for (const field of s.groupBy) {
    const v = asText(row[field]);
    const f = s.filters.find(x => x.field === field && (x.op === '=' || x.op === 'in'));
    if (f) { f.op = '='; f.value = v; f.value2 = ''; }
    else s.filters.push({ field, op: '=', value: v, value2: '' });
  }
  s.groupBy = [];
  s.aggs = [];
  commitSpec();
}

/* -- 8. Render ------------------------------------------------------------ */

function renderStatusLine() {
  if (state.running) {
    el.qstatus.innerHTML = '<span class="spin"></span><span class="live">Running…</span>';
    return;
  }
  if (state.error) {
    el.qstatus.innerHTML =
      '<svg width="14" height="14" style="color:var(--error)"><use href="#i-warn"/></svg>'
      + `<span class="err">${esc(state.error)}</span>`;
    return;
  }
  const r = state.result;
  if (!r) {
    el.qstatus.innerHTML = `<span class="ok">Ready &middot; ${nf.format(RECORDS.length)} records`
      + ` &middot; ${CATALOG.length} fields &middot; ${TABLES.length} types</span>`;
    return;
  }
  const noun = r.grouped ? (r.selected === 1 ? 'group' : 'groups') : (r.selected === 1 ? 'row' : 'rows');
  const capped = r.matched > r.selected ? ` of ${nf.format(r.matched)}` : '';
  const scan = r.grouped ? ` from ${nf.format(r.scanned)} records` : '';
  el.qstatus.innerHTML = `<span class="ok">${nf.format(r.selected)}${capped} ${noun}${scan}`
    + ` &middot; ${r.ms.toFixed(1)} ms</span>`;
}

function renderResult() {
  if (state.error) {
    el.rcount.textContent = '—';
    el.dvbody.innerHTML = emptyState('Query failed',
      'The reason is spelled out under the builder. Fix that lane and it runs again.');
    return;
  }
  const r = state.result;
  if (!r) {
    el.rcount.textContent = '—';
    el.dvbody.innerHTML = emptyState('Nothing has run yet',
      `Press <code>${MOD}↵</code> to run the query above.`);
    return;
  }
  el.rcount.textContent = r.drawn < r.selected
    ? `${nf.format(r.drawn)} of ${nf.format(r.selected)} rows`
    : `${nf.format(r.selected)} ${r.selected === 1 ? 'row' : 'rows'}`;

  if (!r.objs.length) {
    el.dvbody.innerHTML = emptyState('0 rows', timeFilter()
      ? 'Nothing in this time range matches. Widen it on the timeline, or drop a filter in the Fields panel.'
      : 'The query is valid — no record satisfied every filter. Untick a value in the Fields panel to widen it.');
    return;
  }
  if (state.view === 'table') {
    const union = !currentTable() && TYPE_FIELD && TABLES.length > 1;
    const note = union
      ? `<div class="tnote">${TABLES.length} record types in view &middot; a table can only show`
        + ' the fields all of them share &middot; pick one above, or switch to JSON to read records whole</div>'
      : '';
    el.dvbody.innerHTML = note + renderTable(r);
  } else if (state.view === 'json') {
    el.dvbody.innerHTML = `<div class="tree" id="tree">${renderTreeRange(r.objs, 0, Math.min(r.objs.length, TREE_CHUNK), '', 0)}</div>`
      + (r.objs.length > TREE_CHUNK
        ? `<div class="loadmore" style="padding-left:66px">${nf.format(TREE_CHUNK)} of ${nf.format(r.objs.length)} rows &middot; narrow the query to see the rest</div>`
        : '');
  } else if (state.view === 'raw') {
    el.dvbody.innerHTML = renderRawJson(r.objs);
  } else if (state.view === 'chart') {
    el.dvbody.innerHTML = renderChart(r);
  }
}

/* ---- Table picker ------------------------------------------------------ */

/* The picker is a view onto one ordinary filter, never a second source of
   truth: filter the type from a table cell and the picker moves with it. */
function currentTable() {
  if (!TYPE_FIELD) return null;
  const f = state.spec.filters.find(x => x.field === TYPE_FIELD.name && (x.op === '=' || x.op === 'in'));
  if (!f) return null;
  return f.op === 'in' ? { many: listOf(f.value).length } : { one: asText(f.value) };
}

function setTable(value) {
  if (!TYPE_FIELD) return;
  const s = state.spec;
  s.filters = s.filters.filter(x => !(x.field === TYPE_FIELD.name && (x.op === '=' || x.op === 'in')));
  if (value) s.filters.unshift({ field: TYPE_FIELD.name, op: '=', value, value2: '' });
  commitSpec();
}

function renderTableMenu() {
  if (!TYPE_FIELD) {
    el.tableSel.disabled = true;
    el.tableSel.title = 'Every record in this file is the same kind';
    return;
  }
  el.tableSel.title = `Switch table \u2014 ${TABLES.length} kinds of record in this file`;
  const item = (value, label, n) =>
    `<button role="menuitemradio" data-table="${escAttr(value)}" aria-checked="false">`
    + `${esc(label)}<span class="n">${nf.format(n)}</span>`
    + '<svg class="tick" width="15" height="15"><use href="#i-check"/></svg></button>';
  el.tableMenu.innerHTML = item('', 'All types', RECORDS.length)
    + '<div class="sep"></div>'
    + TABLES.map(t => item(t.label, t.label, t.n)).join('');
}

const CHART_ROWS = 40;
const isNumType = t => ['BIGINT', 'INTEGER', 'DOUBLE'].includes(t);

/* A chart of ungrouped rows would be 1,000 bars of nothing, so this view
   charts what grouping already produced and says so when there is none. */
function renderChart(r) {
  const label = r.columns.find(c => !isNumType(c.type));
  const value = r.columns.find(c => isNumType(c.type));
  if (!r.grouped || !label || !value) {
    return emptyState('Nothing to chart yet',
      'A bar needs one row per group. Add a field to <code>Group by</code>'
      + ' and a number to <code>Compute</code>.');
  }

  const rows = r.objs.slice(0, CHART_ROWS);
  const peak = Math.max(...rows.map(o => Math.abs(Number(o[value.name])) || 0), 1);
  const bars = rows.map(o => {
    const raw = o[value.name];
    const n = Number(raw);
    const pct = Number.isFinite(n) ? Math.abs(n) / peak * 100 : 0;
    const text = !Number.isFinite(n) ? '—'
      : Number.isInteger(n) ? nf.format(n) : cellText(n, value.type);
    const name = asText(o[label.name]);
    return `<div class="crow">`
      + `<span class="clabel" title="${escAttr(name)}">${esc(name)}</span>`
      + `<span class="cbar"><i style="--p:${pct.toFixed(1)}%"></i></span>`
      + `<span class="cval">${esc(text)}</span></div>`;
  }).join('');

  const note = r.objs.length > CHART_ROWS
    ? ` &middot; top ${CHART_ROWS} of ${nf.format(r.objs.length)}` : '';
  return `<div class="chart"><div class="chead">${esc(value.name)} by ${esc(label.name)}${note}</div>${bars}</div>`;
}

function renderChrome() {
  el.html.dataset.theme = state.theme;
  el.themeIcon.setAttribute('href', state.theme === 'latte' ? '#i-sun' : '#i-moon');
  for (const b of el.themeMenu.querySelectorAll('button')) {
    b.setAttribute('aria-checked', String(b.dataset.themeSet === state.theme));
  }

  const t = currentTable();
  if (TYPE_FIELD) {
    el.tableLbl.textContent = !t ? 'All types' : t.many ? `${t.many} types` : t.one;
    el.tableCnt.textContent = state.result ? nf.format(state.result.scanned) : '';
    for (const b of el.tableMenu.querySelectorAll('button')) {
      b.setAttribute('aria-checked', String(b.dataset.table === (t && t.one ? t.one : '')));
    }
  }

  const [label, icon] = VIEW_META[state.view] || VIEW_META.table;
  el.viewLbl.textContent = label;
  el.viewIcon.setAttribute('href', icon);
  for (const b of el.viewMenu.querySelectorAll('button')) {
    b.setAttribute('aria-checked', String(b.dataset.view === state.view));
  }

  const r = state.result;
  const filtered = state.spec.filters.length > 0;
  el.stMode.textContent = r && r.grouped ? 'Grouped' : 'Rows';
  el.stItems.innerHTML = r && (filtered || r.grouped)
    ? `<svg width="13" height="13"><use href="#i-funnel"/></svg> <span class="v">${nf.format(r.matched)}</span>`
      + ` of ${nf.format(RECORDS.length)} items`
    : `<svg width="13" height="13"><use href="#i-list"/></svg> <span class="v">${nf.format(RECORDS.length)}</span> items`;
  el.stSig.classList.toggle('live', state.running);
  el.stSigVal.textContent = state.running ? 'running' : state.error ? 'error' : 'ready';
}

function render({ builder = true } = {}) {
  renderChrome();
  if (builder) renderBuilder();
  renderHead();
  renderSql();
  renderStatusLine();
  renderResult();
  renderTimeline();
  if (state.section === 'fields') renderFacets();
  renderDetail();
}

/* -- 9. Actions ----------------------------------------------------------- */

/* `loud` is the explicit Run: it shows the veil and lets the engine warm up for
   a frame. Builder edits run silently, because a spinner on every keystroke
   reads as breakage rather than progress. */
function execute({ loud = false, builder = true } = {}) {
  const finish = () => {
    try {
      state.result = runSpec(state.spec);
      state.error = null;
    } catch (err) {
      state.result = null;
      state.error = err instanceof QueryError ? err.message : `Internal Error: ${err.message}`;
    }
    state.running = false;
    el.veil.hidden = true;
    render({ builder });
  };

  if (!loud) { state.running = false; finish(); return; }

  state.running = true;
  state.error = null;
  el.veilTxt.textContent = 'Running query…';
  el.veil.hidden = false;
  el.rcount.innerHTML = '<span class="spin" style="display:inline-block;vertical-align:-2px"></span>';
  renderChrome();
  renderStatusLine();
  setTimeout(finish, 200);
}

function setView(view) {
  state.view = view;
  store.set('view', view);
  render();
}

function currentRows() {
  return state.result ? state.result.objs : [];
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0';
    document.body.appendChild(ta); ta.select();
    try { document.execCommand('copy'); } catch { /* clipboard unavailable */ }
    ta.remove();
  }
}

async function copyRows() {
  await copyText(JSON.stringify(currentRows(), null, 2));
  el.copyLbl.textContent = 'Copied';
  setTimeout(() => { el.copyLbl.textContent = 'Copy'; }, 1400);
}

function exportRows(format) {
  const rows = currentRows();
  const cols = state.result ? state.result.columns.map(c => c.name) : COLNAMES;
  let text, mime;
  if (format === 'csv') {
    const cell = v => {
      const s = v === null || v === undefined ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v);
      return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
    };
    text = [cols.join(','), ...rows.map(r => cols.map(c => cell(r[c])).join(','))].join('\n');
    mime = 'text/csv';
  } else if (format === 'ndjson') {
    text = rows.map(r => JSON.stringify(r)).join('\n');
    mime = 'application/x-ndjson';
  } else {
    text = JSON.stringify(rows, null, 2);
    mime = 'application/json';
  }
  const url = URL.createObjectURL(new Blob([text], { type: mime }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `events-${state.result && state.result.grouped ? 'grouped' : 'records'}.${format}`;
  a.click();
  URL.revokeObjectURL(url);
}

/* -- 10. Events ----------------------------------------------------------- */

/* Add one lane item. Each default is the next useful thing rather than an
   empty slot, so a click produces a runnable query straight away. */
function addItem(kind) {
  const s = state.spec;
  if (kind === 'filter') {
    s.filters.push({ field: 'level', op: '=', value: '', value2: '' });
  } else if (kind === 'group') {
    const next = PLAIN_FIELDS.find(f => !s.groupBy.includes(f));
    if (!next) return;
    s.groupBy.push(next);
    if (!s.aggs.length) s.aggs.push({ fn: 'count' });
  } else if (kind === 'agg') {
    s.aggs.push(s.aggs.some(a => a.fn === 'count')
      ? { fn: 'avg', field: 'duration_ms' }
      : { fn: 'count' });
  } else if (kind === 'sort') {
    const taken = new Set(s.sort.map(x => x.field));
    const next = sortFields(s).find(f => !taken.has(f));
    if (!next) return;
    s.sort.push({ field: next, dir: 'desc' });
  }
  commitSpec();
  if (kind === 'filter') {
    const last = el.lnFilters.querySelector('.pill:last-child input');
    if (last) last.focus();
  }
}

let runTimer = 0;
function scheduleRun() {
  clearTimeout(runTimer);
  runTimer = setTimeout(() => commitSpec({ builder: false }), 280);
}

/* One listener for the whole builder: pills are replaced on every render, so
   per-pill listeners would leak with each edit. */
el.lanes.addEventListener('change', e => {
  const sel = e.target.closest('select');
  if (!sel) return;
  const pill = sel.closest('.pill');
  if (!pill) return;
  const i = Number(pill.dataset.i);
  const s = state.spec;
  const role = sel.dataset.role;

  if (pill.dataset.kind === 'filter') {
    if (role === 'field') s.filters[i].field = sel.value;
    if (role === 'op') s.filters[i].op = sel.value;
  } else if (pill.dataset.kind === 'group') {
    s.groupBy[i] = sel.value;
  } else if (pill.dataset.kind === 'agg') {
    if (role === 'fn') s.aggs[i] = { fn: sel.value, field: s.aggs[i].field };
    if (role === 'field') s.aggs[i].field = sel.value;
  } else if (pill.dataset.kind === 'sort') {
    s.sort[i].field = sel.value;
  }
  commitSpec();
});

el.lanes.addEventListener('input', e => {
  const input = e.target.closest('input.v');
  if (!input) return;
  const pill = input.closest('.pill');
  const f = state.spec.filters[Number(pill.dataset.i)];
  if (!f) return;
  f[input.dataset.role] = input.value;
  renderSql();
  scheduleRun();
});

el.lanes.addEventListener('click', e => {
  const add = e.target.closest('[data-add]');
  if (add) { addItem(add.dataset.add); return; }

  const combine = e.target.closest('[data-combine]');
  if (combine) { state.spec.combine = combine.dataset.combine; commitSpec(); return; }

  const pill = e.target.closest('.pill');
  if (!pill) return;
  const i = Number(pill.dataset.i);
  const s = state.spec;

  if (e.target.closest('[data-role="remove"]')) {
    if (pill.dataset.kind === 'filter') s.filters.splice(i, 1);
    if (pill.dataset.kind === 'group') s.groupBy.splice(i, 1);
    if (pill.dataset.kind === 'agg') s.aggs.splice(i, 1);
    if (pill.dataset.kind === 'sort') s.sort.splice(i, 1);
    commitSpec();
    return;
  }
  if (e.target.closest('[data-role="dir"]')) {
    s.sort[i].dir = s.sort[i].dir === 'asc' ? 'desc' : 'asc';
    commitSpec();
  }
});

el.limIn.addEventListener('change', () => {
  state.spec.limit = Number(el.limIn.value) || DEFAULT_SPEC.limit;
  commitSpec();
});

$('#runBtn').addEventListener('click', () => execute({ loud: true }));
$('#resetBtn').addEventListener('click', () => {
  state.spec = normalizeSpec(DEFAULT_SPEC);
  commitSpec();
});

/* SQL is a read-only rendering of the builder — open it to read or copy, not
   to type into. */
el.sqlDisc.addEventListener('click', () => {
  state.sqlOpen = !state.sqlOpen;
  store.set('sqlOpen', state.sqlOpen ? '1' : '0');
  renderSql();
});
$('#sqlCopy').addEventListener('click', async () => {
  await copyText(compileSql(state.spec));
  el.sqlCopyLbl.textContent = 'Copied';
  setTimeout(() => { el.sqlCopyLbl.textContent = 'Copy SQL'; }, 1400);
});

/* Hide / show the builder. The head keeps reading the query back either way,
   so collapsing loses the controls but never the context. */
el.qbToggle.addEventListener('click', () => {
  state.qbOpen = !state.qbOpen;
  store.set('qbOpen', state.qbOpen ? '1' : '0');
  render();
});

/* Facets — a value toggles a filter, a header folds the group, a measure
   filters to its tail. */
el.sideBody.addEventListener('click', e => {
  if (state.section !== 'fields') return;
  const facet = e.target.closest('.facet');
  if (!facet) return;
  const field = facet.dataset.field;

  if (e.target.closest('[data-role="cols-auto"]')) {
    state.spec.columns = [];
    commitSpec();
    return;
  }
  if (e.target.closest('[data-role="facet-pin"]')) { toggleColumn(field); return; }
  const col = e.target.closest('[data-col]');
  if (col) { toggleColumn(col.dataset.col); return; }
  if (e.target.closest('[data-role="facet-head"]')) {
    if (state.facetShut.has(field)) state.facetShut.delete(field);
    else state.facetShut.add(field);
    renderFacets();
    return;
  }
  if (e.target.closest('[data-role="facet-more"]')) {
    if (state.facetMore.has(field)) state.facetMore.delete(field);
    else state.facetMore.add(field);
    renderFacets();
    return;
  }
  const measure = e.target.closest('[data-role="measure"]');
  if (measure) {
    const f = state.spec.filters.find(x => x.field === field && x.op === '>=');
    if (f) f.value = measure.dataset.value;
    else state.spec.filters.push({ field, op: '>=', value: measure.dataset.value, value2: '' });
    commitSpec();
    return;
  }
  const val = e.target.closest('.fval');
  if (val) toggleFacet(field, val.dataset.value);
});

/* A record row opens the drawer; a grouped row drills into its records. */
el.dvbody.addEventListener('click', e => {
  const tr = e.target.closest('tbody tr[data-i]');
  if (!tr) return;
  if (!window.getSelection().isCollapsed) return;   // the click ended a text selection
  const i = Number(tr.dataset.i);
  if (state.result && state.result.grouped) { drillInto(i); return; }
  state.detail = state.detail === i ? null : i;
  renderDetail();
});

/* One pair of quick-filter buttons follows the pointer from cell to cell,
   rather than 18,000 buttons waiting in the table for a hover. */
el.dvbody.addEventListener('mouseover', e => {
  const td = e.target.closest('td[data-f]');
  if (td && el.cellact.parentElement !== td) td.appendChild(el.cellact);
});
el.cellact.addEventListener('click', e => {
  const btn = e.target.closest('[data-fq]');
  const td = el.cellact.parentElement;
  if (!btn || !td) return;
  e.stopPropagation();
  if (btn.dataset.fq === 'out') excludeValue(td.dataset.f, td.dataset.v);
  else toggleFacet(td.dataset.f, td.dataset.v);
});

$('#detailClose').addEventListener('click', () => { state.detail = null; renderDetail(); });
$('#detailCopy').addEventListener('click', async () => {
  const row = state.result && state.detail !== null ? state.result.objs[state.detail] : null;
  if (row) await copyText(JSON.stringify(row, null, 2));
});

/* Menus — one open at a time, closed by outside click or Escape. */
const MENUS = [['#tableSel', '#tableMenu'], ['#viewSel', '#viewMenu'],
  ['#exportSel', '#exportMenu'], ['#themeBtn', '#themeMenu']];
function closeMenus(except) {
  for (const [t, m] of MENUS) {
    const menu = $(m);
    if (menu === except) continue;
    menu.hidden = true;
    $(t).setAttribute('aria-expanded', 'false');
  }
}
for (const [trigger, menu] of MENUS) {
  $(trigger).addEventListener('click', (e) => {
    e.stopPropagation();
    const m = $(menu);
    const open = m.hidden;
    closeMenus(open ? m : null);
    m.hidden = !open;
    $(trigger).setAttribute('aria-expanded', String(open));
  });
}
document.addEventListener('click', () => closeMenus());

el.tableMenu.addEventListener('click', e => {
  const b = e.target.closest('[data-table]');
  if (!b) return;
  setTable(b.dataset.table);
  closeMenus();
});

for (const b of el.viewMenu.querySelectorAll('button')) {
  b.addEventListener('click', () => { setView(b.dataset.view); closeMenus(); });
}
for (const b of el.exportMenu.querySelectorAll('button')) {
  b.addEventListener('click', () => { exportRows(b.dataset.export); closeMenus(); });
}
for (const b of el.themeMenu.querySelectorAll('button')) {
  b.addEventListener('click', () => {
    state.theme = b.dataset.themeSet;
    store.set('theme', state.theme);
    closeMenus();
    render();
  });
}

$('#copyBtn').addEventListener('click', copyRows);
$('#collapseBtn').addEventListener('click', () => el.body.classList.toggle('collapsed'));

/* Tree twisties — delegated, so a 120-row page costs one listener. */
el.dvbody.addEventListener('click', e => {
  const line = e.target.closest('.tline[data-kids]');
  if (!line) return;
  const open = line.getAttribute('aria-expanded') === 'true';
  line.setAttribute('aria-expanded', String(!open));
  const kids = line.nextElementSibling;
  if (kids && kids.classList.contains('tkids')) kids.hidden = open;
});

/* Sidebar sections — real product panes, one per rail icon. */
const PANES = {
  bookmarks: ['Bookmarks', `
    <div class="sgroup"><h3>events.ndjson</h3></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-bookmark"/></svg><span class="nm">$[0].trace.trace_id</span><span class="pth">record 0</span></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-bookmark"/></svg><span class="nm">$[1633].decline_code</span><span class="pth">record 1633</span></div>
    <div class="sgroup"><h3>spans.ndjson</h3></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-bookmark"/></svg><span class="nm">$[88].trace_id</span><span class="pth">record 88</span></div>`],
  seshat: ['Seshat — databases', `
    <div class="sgroup"><h3>Connections</h3></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-db"/></svg><span class="nm">analytics</span><span class="pth">postgres · read-only</span></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-db"/></svg><span class="nm">local.duckdb</span><span class="pth">~/data/local.duckdb</span></div>
    <div class="sgroup"><h3>Saved queries</h3></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-code"/></svg><span class="nm">errors_by_service</span><span class="pth">2 params</span></div>`],
  url: ['url-source — fetch', `
    <div style="padding:10px 12px 4px">
      <label class="field" style="width:100%"><svg width="14" height="14"><use href="#i-plug"/></svg>
        <input placeholder="https://api.example.com/events" autocomplete="off"></label>
    </div>
    <div style="padding:0 12px 10px"><button class="btn btn-solid" style="width:100%;justify-content:center">Fetch</button></div>
    <div class="sgroup"><h3>Recent endpoints</h3></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-plug"/></svg><span class="nm">/v2/events</span><span class="pth">200 · 41 KB</span></div>
    <div class="frow"><svg width="14" height="14"><use href="#i-plug"/></svg><span class="nm">/v2/services</span><span class="pth">200 · 2 KB</span></div>`],
};
const recentPane = $('#sideBody').innerHTML;

function renderSidebar() {
  if (state.section === 'fields') {
    el.sideTitle.textContent = 'Fields';
    renderFacets();
    return;
  }
  const [title, html] = state.section === 'recent'
    ? ['Recent files', recentPane]
    : PANES[state.section];
  el.sideTitle.textContent = title;
  el.sideBody.innerHTML = html;
}

for (const btn of document.querySelectorAll('.railbtn')) {
  btn.addEventListener('click', () => {
    if (el.body.classList.contains('collapsed')) el.body.classList.remove('collapsed');
    for (const b of document.querySelectorAll('.railbtn')) {
      b.setAttribute('aria-selected', String(b === btn));
    }
    state.section = btn.dataset.section;
    renderSidebar();
  });
}

/* Tabs — selection only; each tab keeps its own query in the app. */
for (const tab of document.querySelectorAll('.tab')) {
  tab.addEventListener('click', e => {
    if (e.target.closest('.x')) return;
    for (const t of document.querySelectorAll('.tab')) t.setAttribute('aria-selected', String(t === tab));
  });
}

/* Keyboard — keyboard-first, every shortcut visible in a tooltip or hint. */
document.addEventListener('keydown', e => {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.key === 'f') { e.preventDefault(); addItem('filter'); return; }
  if (mod && e.key === 'g') { e.preventDefault(); addItem('group'); return; }
  if (mod && e.key === 'b') { e.preventDefault(); el.body.classList.toggle('collapsed'); return; }
  if (mod && e.key === '/') { e.preventDefault(); el.qbToggle.click(); return; }
  if (mod && e.key === 'Enter') { e.preventDefault(); execute({ loud: true }); return; }
  if (e.key === 'Escape') {
    closeMenus();
    if (state.detail !== null) { state.detail = null; renderDetail(); }
  }
});

/* -- 11. Boot ------------------------------------------------------------- */

$('#runKbd').textContent = IS_MAC ? '⌘↵' : 'Ctrl↵';
$('#filterKbd').textContent = IS_MAC ? '⌘F' : 'Ctrl F';
$('#qbKbd').textContent = IS_MAC ? '⌘/' : 'Ctrl /';
/* A session that last used the removed card view would land on no view at
   all, so an unknown view falls back rather than rendering nothing. */
if (!VIEW_META[state.view]) { state.view = 'table'; store.set('view', 'table'); }

for (const f of FACET_FIELDS.slice(4)) state.facetShut.add(f);
renderTableMenu();
renderDatalists();
renderSidebar();
execute();
