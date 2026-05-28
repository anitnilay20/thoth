// Sample data + plugin registry for the Thoth multi-tab prototype.

window.THOTH_FILES = {
  "users.json": {
    type: "JSON",
    items: 4,
    value: [
      {
        id: 1001,
        name: "John Doe",
        email: "john@example.com",
        active: true,
        address: { street: "1 Main St", city: "Boston", zip: "02101" },
        roles: ["admin", "developer", "reviewer"]
      },
      {
        id: 1002,
        name: "Jane Smith",
        email: "jane@example.com",
        active: true,
        address: { street: "742 Evergreen Tce", city: "Springfield", zip: "00000" },
        roles: ["editor"]
      },
      {
        id: 1003,
        name: "Sam Park",
        email: "sam@example.com",
        active: false,
        address: null,
        roles: []
      },
      { id: 1004, name: "Ada Lovelace", email: "ada@analytical.engine", active: true, address: null, roles: ["admin"] }
    ]
  },
  "events.ndjson": {
    type: "NDJSON",
    items: 6,
    value: [
      { event: "login",  user: "ada",   ts: 1714000001, ip: "10.0.0.1" },
      { event: "view",   user: "ada",   ts: 1714000042, path: "/dashboard" },
      { event: "click",  user: "ada",   ts: 1714000110, target: "btn-export" },
      { event: "view",   user: "john",  ts: 1714000180, path: "/users" },
      { event: "logout", user: "ada",   ts: 1714000300 },
      { event: "login",  user: "sam",   ts: 1714000420, ip: "10.0.0.7" }
    ]
  },
  "config.json": {
    type: "JSON",
    items: 5,
    value: {
      app: "thoth",
      version: "0.2.4",
      theme: "catppuccin-mocha",
      shortcuts: { open: "Cmd+O", search: "Cmd+F", theme: "Cmd+Shift+T", newTab: "Cmd+T", splitRight: "Cmd+\\" },
      plugins: { enabled: true, count: 5, autoload: ["schema-validator", "jsonpath"] }
    }
  },
  "schema.json": {
    type: "JSON",
    items: 3,
    value: {
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "title": "User",
      "type": "object",
      "required": ["id", "name", "email"],
      "properties": {
        "id": { "type": "integer", "minimum": 1 },
        "name": { "type": "string", "minLength": 1 },
        "email": { "type": "string", "format": "email" },
        "active": { "type": "boolean", "default": true },
        "roles": { "type": "array", "items": { "type": "string" } }
      }
    }
  },
  "metrics.json": {
    type: "JSON",
    items: 8,
    value: {
      uptime_s: 1840293,
      requests_total: 8432910,
      errors_total: 142,
      latency_p50_ms: 18,
      latency_p95_ms: 64,
      latency_p99_ms: 211,
      regions: ["us-east-1", "eu-west-2", "ap-south-1"],
      build: { commit: "a1f2e9c", branch: "main", at: "2025-12-18T14:22:01Z" }
    }
  }
};

// Plugin definitions — each becomes a tab kind. The viewer is rendered by panels.jsx.
window.THOTH_PLUGINS = {
  "welcome": {
    id: "welcome",
    title: "Welcome",
    icon: "house",
    accent: "primary",
  },
  "settings": {
    id: "settings",
    title: "Settings",
    icon: "gear",
    accent: "overlay2",
  },
  "schema-validator": {
    id: "schema-validator",
    title: "Schema Validator",
    icon: "check-circle",
    accent: "info",
  },
  "diff": {
    id: "diff",
    title: "Diff Viewer",
    icon: "git-diff",
    accent: "secondary",
  },
  "jsonpath": {
    id: "jsonpath",
    title: "JSONPath",
    icon: "magnifying-glass",
    accent: "accent",
  },
};
