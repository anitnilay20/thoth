/* Seshat — mock data for the DB plugin prototype.
   Realistic-feeling connections, schemas, query results, history. */

window.SESHAT = (function () {
  // ─────── Engine catalogue ────────────────────────────────
  // Each engine: id, label, dotColor (for status & icons), category
  const ENGINES = {
    postgres:  { label: 'PostgreSQL',  short: 'PG',   dot: '#74c7ec', kind: 'sql' },
    mysql:     { label: 'MySQL',       short: 'MY',   dot: '#fab387', kind: 'sql' },
    mariadb:   { label: 'MariaDB',     short: 'MA',   dot: '#fab387', kind: 'sql' },
    sqlite:    { label: 'SQLite',      short: 'SL',   dot: '#94e2d5', kind: 'sql' },
    mssql:     { label: 'SQL Server',  short: 'MS',   dot: '#cba6f7', kind: 'sql' },
    oracle:    { label: 'Oracle',      short: 'OR',   dot: '#f38ba8', kind: 'sql' },
    bigquery:  { label: 'BigQuery',    short: 'BQ',   dot: '#89b4fa', kind: 'sql' },
    snowflake: { label: 'Snowflake',   short: 'SF',   dot: '#b4befe', kind: 'sql' },
    clickhouse:{ label: 'ClickHouse',  short: 'CH',   dot: '#f9e2af', kind: 'sql' },
    duckdb:    { label: 'DuckDB',      short: 'DD',   dot: '#fab387', kind: 'sql' },
    mongodb:   { label: 'MongoDB',     short: 'MG',   dot: '#a6e3a1', kind: 'doc' },
    redis:     { label: 'Redis',       short: 'RD',   dot: '#f38ba8', kind: 'kv' },
    cassandra: { label: 'Cassandra',   short: 'CS',   dot: '#74c7ec', kind: 'wide' },
  };

  // ─────── Saved connections ────────────────────────────────
  const CONNECTIONS = [
    { id: 'prod-pg',    name: 'prod-postgres',    engine: 'postgres',  host: 'db.prod.acme.io',           port: 5432,  user: 'reader',     db: 'acme_production',  ssl: true,  status: 'connected', latency: 38, env: 'prod', color: '#f38ba8' },
    { id: 'stage-pg',   name: 'staging-postgres', engine: 'postgres',  host: 'db.stage.acme.io',          port: 5432,  user: 'developer',  db: 'acme_staging',     ssl: true,  status: 'connected', latency: 71, env: 'stage', color: '#f9e2af' },
    { id: 'analytics',  name: 'analytics-warehouse', engine: 'snowflake', host: 'acme.snowflakecomputing.com', port: 443, user: 'analyst', db: 'ANALYTICS',     ssl: true,  status: 'connected', latency: 215, env: 'prod', color: '#b4befe' },
    { id: 'events',     name: 'events-clickhouse',engine: 'clickhouse',host: 'ch.events.acme.io',         port: 9000,  user: 'reader',     db: 'events',           ssl: true,  status: 'connected', latency: 24,  env: 'prod', color: '#f9e2af' },
    { id: 'local-pg',   name: 'local-dev',        engine: 'postgres',  host: 'localhost',                 port: 5432,  user: 'postgres',   db: 'acme_dev',         ssl: false, status: 'connected', latency: 2,   env: 'dev',  color: '#a6e3a1' },
    { id: 'sessions',   name: 'sessions-redis',   engine: 'redis',     host: 'cache.prod.acme.io',        port: 6379,  user: '',           db: '0',                ssl: true,  status: 'disconnected', latency: null, env: 'prod', color: '#f38ba8' },
    { id: 'docs-mongo', name: 'docs-mongo',       engine: 'mongodb',   host: 'mongo.acme.io',             port: 27017, user: 'app',        db: 'documents',        ssl: true,  status: 'disconnected', latency: null, env: 'stage', color: '#a6e3a1' },
    { id: 'reports',    name: 'reports-bigquery', engine: 'bigquery',  host: 'bigquery.googleapis.com',   port: 443,   user: 'svc-reports',db: 'acme-prod',        ssl: true,  status: 'connected', latency: 312, env: 'prod', color: '#89b4fa' },
    { id: 'warehouse-duck', name: 'duck-local',  engine: 'duckdb',     host: '~/data/warehouse.duckdb',   port: null,  user: '',           db: 'main',             ssl: false, status: 'connected', latency: 1,   env: 'dev',  color: '#fab387' },
  ];

  // ─────── Schemas, tables, columns ────────────────────────
  // prod-postgres schema (most detailed; others are summaries)
  const SCHEMAS = {
    'prod-pg': [
      { name: 'public', tables: [
        { name: 'users',          rows: 4823017, kind: 'table', cols: [
          { name: 'id',           type: 'bigint',        pk: true,  nn: true,  fk: null,             default: 'nextval(...)' },
          { name: 'email',        type: 'text',          pk: false, nn: true,  fk: null,             unique: true },
          { name: 'name',         type: 'text',          pk: false, nn: true,  fk: null },
          { name: 'plan',         type: 'plan_enum',     pk: false, nn: true,  fk: null,             default: "'free'" },
          { name: 'org_id',       type: 'bigint',        pk: false, nn: true,  fk: 'organizations.id' },
          { name: 'avatar_url',   type: 'text',          pk: false, nn: false, fk: null },
          { name: 'metadata',     type: 'jsonb',         pk: false, nn: false, fk: null,             default: "'{}'::jsonb" },
          { name: 'created_at',   type: 'timestamptz',   pk: false, nn: true,  fk: null,             default: 'now()' },
          { name: 'last_seen_at', type: 'timestamptz',   pk: false, nn: false, fk: null },
        ], indexes: [
          { name: 'users_pkey',          cols: ['id'],            unique: true },
          { name: 'users_email_uniq',    cols: ['email'],         unique: true },
          { name: 'users_org_id_idx',    cols: ['org_id'],        unique: false },
          { name: 'users_plan_idx',      cols: ['plan'],          unique: false, partial: "WHERE plan != 'free'" },
        ]},
        { name: 'organizations',  rows: 38291,   kind: 'table', cols: [
          { name: 'id',           type: 'bigint',        pk: true,  nn: true },
          { name: 'name',         type: 'text',          pk: false, nn: true },
          { name: 'domain',       type: 'text',          pk: false, nn: false, unique: true },
          { name: 'tier',         type: 'tier_enum',     pk: false, nn: true,  default: "'standard'" },
          { name: 'seat_count',   type: 'integer',       pk: false, nn: true,  default: '1' },
          { name: 'created_at',   type: 'timestamptz',   pk: false, nn: true,  default: 'now()' },
        ]},
        { name: 'sessions',       rows: 18230114, kind: 'table' },
        { name: 'api_keys',       rows: 9874,    kind: 'table' },
        { name: 'invoices',       rows: 142390,  kind: 'table' },
        { name: 'invoice_items',  rows: 481922,  kind: 'table' },
        { name: 'subscriptions',  rows: 38104,   kind: 'table' },
        { name: 'events',         rows: 91374821,kind: 'table' },
        { name: 'audit_log',      rows: 4823171, kind: 'table' },
        { name: 'feature_flags',  rows: 47,      kind: 'table' },
        { name: 'active_users',   rows: 192347,  kind: 'view'  },
        { name: 'revenue_daily',  rows: 730,     kind: 'view'  },
      ]},
      { name: 'billing', tables: [
        { name: 'payment_methods', rows: 81203, kind: 'table' },
        { name: 'charges',         rows: 412908, kind: 'table' },
        { name: 'refunds',         rows: 8124, kind: 'table' },
        { name: 'webhooks',        rows: 982341, kind: 'table' },
      ]},
      { name: 'analytics_mat', tables: [
        { name: 'retention_cohorts', rows: 8420, kind: 'matview' },
        { name: 'mrr_history',       rows: 730,  kind: 'matview' },
      ]},
    ],
    'stage-pg': [
      { name: 'public', tables: [
        { name: 'users', rows: 12480, kind: 'table' },
        { name: 'organizations', rows: 234, kind: 'table' },
        { name: 'sessions', rows: 89421, kind: 'table' },
        { name: 'events', rows: 1284921, kind: 'table' },
      ]},
    ],
    'analytics': [
      { name: 'ANALYTICS', tables: [
        { name: 'FACT_REVENUE_DAILY', rows: 3287, kind: 'table' },
        { name: 'DIM_USER',           rows: 4823017, kind: 'table' },
        { name: 'DIM_ORG',            rows: 38291, kind: 'table' },
        { name: 'FACT_EVENTS',        rows: 91374821, kind: 'table' },
      ]},
    ],
    'events': [
      { name: 'events', tables: [
        { name: 'page_views',  rows: 89412874, kind: 'table' },
        { name: 'click_stream',rows: 412874912, kind: 'table' },
        { name: 'errors',      rows: 8240914, kind: 'table' },
      ]},
    ],
    'local-pg': [
      { name: 'public', tables: [
        { name: 'users', rows: 12, kind: 'table' },
        { name: 'organizations', rows: 3, kind: 'table' },
      ]},
    ],
    'reports': [
      { name: 'acme-prod.reporting', tables: [
        { name: 'monthly_revenue', rows: 36, kind: 'table' },
        { name: 'user_funnel',     rows: 9824, kind: 'table' },
      ]},
    ],
    'warehouse-duck': [
      { name: 'main', tables: [
        { name: 'orders', rows: 482910, kind: 'table' },
        { name: 'line_items', rows: 1928410, kind: 'table' },
      ]},
    ],
  };

  // ─────── Hero query + results ────────────────────────────
  const HERO_QUERY = `-- Top organizations by MRR with 30-day user growth
SELECT
  o.id,
  o.name              AS organization,
  o.tier,
  COUNT(u.id)         AS active_users,
  SUM(s.mrr_cents)/100.0 AS mrr_usd,
  o.created_at::date  AS onboarded
FROM organizations o
JOIN users u           ON u.org_id = o.id
JOIN subscriptions s   ON s.org_id = o.id AND s.status = 'active'
WHERE u.last_seen_at > now() - interval '30 days'
GROUP BY o.id, o.name, o.tier, o.created_at
ORDER BY mrr_usd DESC
LIMIT 50;`;

  const HERO_COLUMNS = [
    { name: 'id',           type: 'bigint',      width: 88,  align: 'right',  fk: 'organizations.id', isPk: true },
    { name: 'organization', type: 'text',        width: 240 },
    { name: 'tier',         type: 'tier_enum',   width: 110, chip: true },
    { name: 'active_users', type: 'bigint',      width: 130, align: 'right' },
    { name: 'mrr_usd',      type: 'numeric',     width: 140, align: 'right', kind: 'currency' },
    { name: 'onboarded',    type: 'date',        width: 140 },
    { name: 'metadata',     type: 'jsonb',       width: 280, kind: 'json' },
  ];

  // Some realistic-ish rows
  const ORG_NAMES = ['Northwind Trading','Acme Robotics','Foxtrot Health','Lambda Logistics','Helix Labs','Quill & Press','Stratus Cloud','Beacon Analytics','Pioneer Pay','Magnolia Foods','Crescent Bank','Orbit Studios','Junction Capital','Lattice AI','Frost Industries','Vector Mobile','Mosaic Realty','Brevity Mail','Cobalt Energy','Atlas Freight','Vellum Press','Riverstone Group','Pebble Studios','Coastline Bio','Granite Build','Indigo Audio','Tessera Toys','Bramble Pets','Sequoia Books','Cypher Security','Hearth & Home','Wildwood Records','Magnetic North','Halcyon Wines','Bramble & Co','Aerie Outdoor','Vespa Lighting','Plinth Furniture','Mellow Coffee','Foundry Tools','Beacon Hill Realty','Lantern Press','Pinion Bikes','Sable Optics','Quokka Foods','Treble Music','Vault & Key','Birch Apparel','Ember Heating','Wisp Materials'];
  const TIERS = ['enterprise','growth','growth','standard','standard','standard'];
  function seedRand(seed) { let s = seed; return () => (s = (s * 9301 + 49297) % 233280) / 233280; }
  const rnd = seedRand(42);
  const HERO_ROWS = ORG_NAMES.map((name, i) => {
    const tier = TIERS[Math.floor(rnd() * TIERS.length)];
    const users = Math.floor(20 + rnd() * (tier === 'enterprise' ? 12000 : tier === 'growth' ? 2200 : 380));
    const mrr = +(users * (tier === 'enterprise' ? 48 : tier === 'growth' ? 24 : 9) * (0.7 + rnd() * 0.6)).toFixed(2);
    const dayOffset = Math.floor(rnd() * 900);
    const d = new Date(2024, 0, 1); d.setDate(d.getDate() - dayOffset);
    const meta = {
      region: ['us-east','us-west','eu-west','ap-south'][Math.floor(rnd()*4)],
      onboarded_by: ['self-serve','sales','partner'][Math.floor(rnd()*3)],
      flags: rnd() > 0.7 ? ['beta-ui','sso'] : rnd() > 0.4 ? ['sso'] : [],
    };
    return [
      1000 + i,
      name,
      tier,
      users,
      mrr,
      d.toISOString().slice(0,10),
      meta,
    ];
  }).sort((a,b) => b[4] - a[4]);

  // EXPLAIN ANALYZE-style mini breakdown
  const HERO_EXPLAIN = [
    { op: 'Limit',          rows: 50,     ms: 142.3, cost: '0.43..1024.5' },
    { op: '  Sort',         rows: 50,     ms: 141.9, cost: '0.42..1023.9' },
    { op: '    HashAggregate', rows: 38291, ms: 139.0, cost: '0.41..980.2' },
    { op: '      Hash Join: subscriptions', rows: 38104, ms: 112.4, cost: '0.40..820.1' },
    { op: '        Hash Join: users',       rows: 192347,ms: 78.1,  cost: '0.30..612.8' },
    { op: '          Seq Scan organizations', rows: 38291, ms: 12.4, cost: '0.10..120.4' },
    { op: '          Index Scan users_org_id_idx', rows: 192347, ms: 51.2, cost: '0.20..420.1' },
    { op: '        Seq Scan subscriptions', rows: 38104, ms: 26.8, cost: '0.20..180.3' },
  ];

  // ─────── Query history ────────────────────────────────────
  const HISTORY = [
    { t: 'just now',   conn: 'prod-pg',   ms: 142,  rows: 50,    status: 'ok',   query: HERO_QUERY.split('\n')[1].trim() + ' …' },
    { t: '3 min ago',  conn: 'prod-pg',   ms: 18,   rows: 1,     status: 'ok',   query: 'SELECT count(*) FROM users WHERE plan = \'enterprise\';' },
    { t: '14 min ago', conn: 'events',    ms: 412,  rows: 100,   status: 'ok',   query: 'SELECT event_name, count() FROM events.page_views WHERE date = today() GROUP BY 1 ORDER BY 2 DESC' },
    { t: '32 min ago', conn: 'prod-pg',   ms: 8,    rows: 12,    status: 'ok',   query: 'SELECT * FROM feature_flags;' },
    { t: '1 hour ago', conn: 'analytics', ms: 1840, rows: 365,   status: 'ok',   query: 'SELECT date, sum(revenue_cents)/100 FROM ANALYTICS.FACT_REVENUE_DAILY GROUP BY date ORDER BY date DESC LIMIT 365' },
    { t: '2 hours ago',conn: 'prod-pg',   ms: null, rows: null,  status: 'err',  query: 'UPDATE users SET plan = \'enterprise\' WHERE id = ?;', error: 'cancelled by user' },
    { t: 'yesterday',  conn: 'stage-pg',  ms: 24,   rows: 12480, status: 'ok',   query: 'SELECT * FROM users;' },
    { t: 'yesterday',  conn: 'reports',   ms: 2104, rows: 36,    status: 'ok',   query: 'SELECT month, gross_revenue FROM `acme-prod.reporting.monthly_revenue` ORDER BY month DESC' },
    { t: '2 days ago', conn: 'prod-pg',   ms: 71,   rows: 4823017, status: 'ok', query: 'SELECT count(*) FROM users;' },
    { t: '3 days ago', conn: 'warehouse-duck', ms: 412, rows: 1928410, status: 'ok', query: 'SELECT order_id, sum(line_total) FROM line_items GROUP BY 1' },
  ];

  // ─────── Saved queries ─────────────────────────────────────
  const SAVED = [
    { id: 'q1', name: 'MRR by tier',                    folder: 'Revenue',     conn: 'prod-pg' },
    { id: 'q2', name: 'Daily signups (last 90 days)',   folder: 'Growth',      conn: 'prod-pg' },
    { id: 'q3', name: 'Top orgs by MRR',                folder: 'Revenue',     conn: 'prod-pg', starred: true },
    { id: 'q4', name: 'Failed webhooks',                folder: 'Ops',         conn: 'prod-pg' },
    { id: 'q5', name: 'Cohort retention — month',       folder: 'Growth',      conn: 'analytics' },
    { id: 'q6', name: 'Event funnel: signup → invite',  folder: 'Growth',      conn: 'events' },
    { id: 'q7', name: 'Stale sessions',                 folder: 'Ops',         conn: 'prod-pg' },
    { id: 'q8', name: 'Org seat-utilization',           folder: 'Revenue',     conn: 'prod-pg', starred: true },
  ];

  // ─────── ER diagram nodes ──────────────────────────────────
  // Curated positions for a tidy "starter" diagram.
  const ER_NODES = [
    { id: 'organizations', x:  60, y:  60, cols: ['id','name','domain','tier','seat_count','created_at'], pk: 'id' },
    { id: 'users',         x: 380, y:  60, cols: ['id','email','name','plan','org_id','last_seen_at','created_at'], pk: 'id' },
    { id: 'subscriptions', x: 380, y: 320, cols: ['id','org_id','plan','status','mrr_cents','renews_at'], pk: 'id' },
    { id: 'sessions',      x: 720, y:  60, cols: ['id','user_id','ip','user_agent','created_at','expires_at'], pk: 'id' },
    { id: 'invoices',      x:  60, y: 320, cols: ['id','org_id','number','status','total_cents','due_at'], pk: 'id' },
    { id: 'invoice_items', x:  60, y: 560, cols: ['id','invoice_id','description','quantity','unit_price','amount_cents'], pk: 'id' },
    { id: 'audit_log',     x: 720, y: 320, cols: ['id','user_id','action','target_id','target_type','at'], pk: 'id' },
  ];
  const ER_EDGES = [
    { from: 'users.org_id',           to: 'organizations.id',      kind: 'many-one' },
    { from: 'subscriptions.org_id',   to: 'organizations.id',      kind: 'many-one' },
    { from: 'sessions.user_id',       to: 'users.id',              kind: 'many-one' },
    { from: 'invoices.org_id',        to: 'organizations.id',      kind: 'many-one' },
    { from: 'invoice_items.invoice_id', to: 'invoices.id',         kind: 'many-one' },
    { from: 'audit_log.user_id',      to: 'users.id',              kind: 'many-one' },
  ];

  // ─────── Autocomplete vocabulary ───────────────────────────
  const KEYWORDS = ['SELECT','FROM','WHERE','GROUP BY','ORDER BY','HAVING','LIMIT','JOIN','LEFT JOIN','INNER JOIN','ON','AS','WITH','RETURNING','INSERT','UPDATE','DELETE','CASE','WHEN','THEN','END','AND','OR','NOT','IN','EXISTS','UNION','DISTINCT','COUNT','SUM','AVG','MIN','MAX','NOW','INTERVAL','COALESCE'];
  const TABLES = ['users','organizations','subscriptions','sessions','invoices','invoice_items','audit_log','events','feature_flags','api_keys','payment_methods'];
  const COLUMNS_VOCAB = ['id','email','name','plan','tier','org_id','user_id','mrr_cents','status','created_at','last_seen_at','metadata','total_cents'];

  return {
    ENGINES,
    CONNECTIONS,
    SCHEMAS,
    HERO_QUERY,
    HERO_COLUMNS,
    HERO_ROWS,
    HERO_EXPLAIN,
    HISTORY,
    SAVED,
    ER_NODES,
    ER_EDGES,
    KEYWORDS,
    TABLES,
    COLUMNS_VOCAB,
  };
})();
