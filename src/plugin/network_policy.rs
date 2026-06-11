use std::sync::{Arc, Mutex};

use reqwest::Url;

use crate::{plugin::NetworkDeclarations, settings::PluginNetworkPolicy};

pub struct NetworkPolicy {
    config: PluginNetworkPolicy,
    request_count: u32,
    window_start: std::time::Instant,
    /// Domains approved at runtime via "Remember this choice" — checked after
    /// the static policy so consent-modal approvals take effect immediately.
    runtime_allowed: Arc<Mutex<Vec<String>>>,
    /// Original normalised plugin-manifest domain list. Runtime consent can
    /// only relax user-side restrictions; it cannot expand the plugin's own
    /// declared scope.
    plugin_declared_domains: Vec<String>,
}

pub enum CheckOutcome {
    Allowed,
    NeedsConsent { domain: String }, // reason
}

#[derive(Debug)]
pub enum PolicyViolation {
    RateLimitExceeded,
    HttpNotAllowed,
    UserBlocked,
    InvalidUrl(String),
}

impl NetworkPolicy {
    pub fn from_plugin_and_settings(
        plugin: &NetworkDeclarations,
        user: &PluginNetworkPolicy,
    ) -> Self {
        // Intersection of allowed domains:
        // - If user has allowed no domains, no domains are allowed
        // - If plugin declares no allowed_domains, inherit from user (plugin has no restrictions)
        // - If plugin allows wildcard "*", use user's allowed domains
        // - If user allows wildcard "*", use plugin's allowed domains
        // - Otherwise, intersect: plugin can only access domains the user also allows
        // Normalise once so all comparisons and dedup work regardless of
        // how the caller formatted the domain strings.
        let norm = |v: &[String]| -> Vec<String> {
            v.iter().map(|d| d.trim().to_ascii_lowercase()).collect()
        };
        let user_domains = norm(&user.allowed_domains);
        let plugin_domains = norm(&plugin.allowed_domains);

        let allowed_domains: Vec<String> = if user_domains.is_empty() {
            // User hasn't permitted any domains
            Vec::new()
        } else if plugin_domains.is_empty() {
            // Plugin has no restrictions, use user's allowed domains
            let mut v = user_domains;
            v.sort_unstable();
            v.dedup();
            v
        } else if plugin_domains.contains(&"*".to_string()) {
            // Plugin allows wildcard, use user's permissions
            let mut v = user_domains;
            v.sort_unstable();
            v.dedup();
            v
        } else if user_domains.contains(&"*".to_string()) {
            // User allows wildcard, use plugin's declarations
            let mut v = plugin_domains;
            v.sort_unstable();
            v.dedup();
            v
        } else {
            // Both have specific restrictions, intersect
            let mut v: Vec<String> = plugin_domains
                .iter()
                .filter(|d| user_domains.contains(d))
                .cloned()
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };

        // User's blocked list is authoritative — normalise for consistent matching.
        let mut blocked_domains = norm(&user.blocked_domains);
        blocked_domains.sort_unstable();
        blocked_domains.dedup();

        let config = PluginNetworkPolicy {
            allowed_domains: allowed_domains.clone(),
            blocked_domains: blocked_domains.clone(),
            require_https: user.require_https || plugin.require_https,
            // Minimum rate limit — honour whichever side is more restrictive.
            rate_limit_rpm: user.rate_limit_rpm.min(plugin.rate_limit_rpm),
        };

        Self {
            config,
            request_count: 0,
            window_start: std::time::Instant::now(),
            runtime_allowed: Arc::new(Mutex::new(Vec::new())),
            plugin_declared_domains: norm(&plugin.allowed_domains),
        }
    }

    /// Returns a handle to the runtime-approved domain list so it can be shared
    /// into consent callbacks and populated when the user checks "Remember".
    pub fn runtime_allowed_handle(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.runtime_allowed)
    }

    pub fn check(&mut self, url: &str) -> Result<CheckOutcome, PolicyViolation> {
        let parsed_url =
            Url::parse(url).map_err(|err| PolicyViolation::InvalidUrl(err.to_string()))?;

        let host = parsed_url
            .host()
            .ok_or_else(|| PolicyViolation::InvalidUrl(format!("URL has no host: {url}")))?
            .to_string();

        if self.is_rate_limited() {
            return Err(PolicyViolation::RateLimitExceeded);
        }

        if self.config.require_https && !url.starts_with("https://") {
            return Err(PolicyViolation::HttpNotAllowed);
        }

        // Check if domain is explicitly blocked
        if self.is_blocked(&host) {
            return Err(PolicyViolation::UserBlocked);
        }

        // Static allowed list passes unconditionally.
        // Runtime approvals only apply within the plugin's declared scope so a
        // user approval cannot expand the plugin beyond its manifest.
        if self.is_allowed(&host)
            || (self.is_within_plugin_scope(&host) && self.is_runtime_allowed(&host))
        {
            return Ok(CheckOutcome::Allowed);
        }

        // Domain not in allowed list - ask for consent
        Ok(CheckOutcome::NeedsConsent {
            domain: host.to_string(),
        })
    }

    /// Gate a raw TCP connect (the `tcp-client` import) by host. The user-supplied
    /// host is the trust boundary: it's allowed if in the plugin's manifest
    /// allowlist (or runtime-approved within scope), otherwise consent is required.
    ///
    /// Unlike [`check`] this does NOT enforce HTTPS or block private/loopback
    /// addresses — database clients legitimately connect to `localhost` and
    /// internal networks, so per-host allowlist + consent is the gate instead.
    pub fn check_tcp(&mut self, host: &str) -> Result<CheckOutcome, PolicyViolation> {
        // Normalize to match the allow/block lists, which `from_plugin_and_settings`
        // stores trimmed + lowercased — otherwise `LOCALHOST` or ` db ` would miss.
        let host = host.trim().to_ascii_lowercase();
        if host.is_empty() {
            return Err(PolicyViolation::InvalidUrl("empty host".to_string()));
        }
        // No rate limiting for TCP: a database client opens a fresh connection
        // per operation (every query, plus each schema-introspection call), so a
        // per-minute connect cap would reject ordinary use. The security boundary
        // for TCP is the host allowlist + per-host consent below, not a rate cap.
        if self.is_blocked(&host) {
            return Err(PolicyViolation::UserBlocked);
        }
        if self.is_allowed(&host)
            || (self.is_within_plugin_scope(&host) && self.is_runtime_allowed(&host))
        {
            return Ok(CheckOutcome::Allowed);
        }
        Ok(CheckOutcome::NeedsConsent { domain: host })
    }

    fn is_blocked(&self, host: &str) -> bool {
        self.config
            .blocked_domains
            .iter()
            .any(|blocked| self.domain_matches(host, blocked))
    }

    fn is_allowed(&self, host: &str) -> bool {
        self.config
            .allowed_domains
            .iter()
            .any(|domain| self.domain_matches(host, domain))
    }

    /// Returns true when `host` falls within what the plugin's manifest declared.
    /// An empty or wildcard manifest means the plugin imposed no domain
    /// restrictions, so any host is considered within scope.
    pub fn is_within_plugin_scope(&self, host: &str) -> bool {
        if self.plugin_declared_domains.is_empty()
            || self.plugin_declared_domains.contains(&"*".to_string())
        {
            return true;
        }
        self.plugin_declared_domains
            .iter()
            .any(|d| self.domain_matches(host, d))
    }

    fn is_runtime_allowed(&self, host: &str) -> bool {
        if let Ok(list) = self.runtime_allowed.lock() {
            list.iter().any(|d| self.domain_matches(host, d))
        } else {
            false
        }
    }

    fn domain_matches(&self, host: &str, pattern: &str) -> bool {
        // Bare wildcard — matches everything
        if pattern == "*" {
            return true;
        }

        // Exact match
        if host == pattern {
            return true;
        }

        // Wildcard subdomain (e.g. "*.example.com" matches "api.example.com")
        if let Some(pattern_domain) = pattern.strip_prefix("*.") {
            return host.ends_with(&format!(".{}", pattern_domain)) || host == pattern_domain;
        }

        false
    }

    fn is_rate_limited(&mut self) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.window_start) > std::time::Duration::from_secs(60) {
            self.request_count = 0;
            self.window_start = now;
        }
        self.request_count += 1;
        self.request_count > self.config.rate_limit_rpm
    }
}

#[cfg(test)]
mod test {
    use crate::{
        plugin::{
            NetworkDeclarations,
            network_policy::{CheckOutcome, PolicyViolation},
        },
        settings::PluginNetworkPolicy,
    };

    use super::NetworkPolicy;

    #[test]
    fn user_wildcard_allows_all_requests() {
        // User has allowed wildcard, plugin declares no restrictions
        // Result: all requests should be allowed
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let resp = np.check("https://abc.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));

        let resp = np.check("https://example.org").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));
    }

    #[test]
    fn user_allows_specific_domain_plugin_no_restrictions() {
        // User allows specific domain, plugin declares no restrictions
        // Result: user's allowed domain should be allowed
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["abc.com".to_string()],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let resp = np.check("https://abc.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));
    }

    #[test]
    fn user_allows_specific_domain_other_domain_needs_consent() {
        // User allows abc.com, plugin declares no restrictions
        // Result: abc.com allowed, other domain needs consent
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["abc.com".to_string()],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let resp = np.check("https://other.com").unwrap();
        assert!(matches!(resp, CheckOutcome::NeedsConsent { domain } if domain == "other.com"));
    }

    #[test]
    fn user_blocks_domain_returns_error() {
        // User has blocked abc.com
        // Result: should return UserBlocked error
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            blocked_domains: vec!["abc.com".to_string()],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let result = np.check("https://abc.com");
        assert!(matches!(result, Err(PolicyViolation::UserBlocked)));
    }

    #[test]
    fn user_blocks_overrides_plugin_permission() {
        // User blocks abc.com but plugin requests it
        // Result: abc.com should be blocked, example.com needs consent (not in user's allowed)
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec![],
            blocked_domains: vec!["abc.com".to_string()],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec!["abc.com".to_string(), "example.com".to_string()],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        // abc.com is explicitly blocked by user
        let result = np.check("https://abc.com");
        assert!(matches!(result, Err(PolicyViolation::UserBlocked)));

        // example.com is not blocked, but user has no allowed_domains, so needs consent
        let result = np.check("https://example.com");
        assert!(
            matches!(result, Ok(CheckOutcome::NeedsConsent { domain }) if domain == "example.com")
        );
    }

    #[test]
    fn plugin_wildcard_uses_user_permissions() {
        // Plugin allows wildcard "*", user allows specific domains
        // Result: should use user's permissions
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["abc.com".to_string(), "example.com".to_string()],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec!["*".to_string()],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        // User allowed domains should be permitted
        let resp = np.check("https://abc.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));

        let resp = np.check("https://example.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));

        // Other domains need consent
        let resp = np.check("https://other.com").unwrap();
        assert!(matches!(resp, CheckOutcome::NeedsConsent { domain } if domain == "other.com"));
    }

    #[test]
    fn user_no_permissions_nothing_allowed() {
        // User has empty allowed_domains, plugin declares permissions
        // Result: nothing should be allowed directly
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec![],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec!["abc.com".to_string(), "example.com".to_string()],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let resp = np.check("https://abc.com").unwrap();
        assert!(matches!(resp, CheckOutcome::NeedsConsent { domain } if domain == "abc.com"));
    }

    #[test]
    fn wildcard_subdomain_pattern() {
        // User allows *.example.com
        // Result: api.example.com should be allowed, other.com should need consent
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["*.example.com".to_string()],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let resp = np.check("https://api.example.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));

        let resp = np.check("https://example.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));

        let resp = np.check("https://other.com").unwrap();
        assert!(matches!(resp, CheckOutcome::NeedsConsent { domain } if domain == "other.com"));
    }

    #[test]
    fn require_https_enforcement() {
        // require_https is true
        // Result: http should be blocked, https should be allowed
        let user_setting = PluginNetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            blocked_domains: vec![],
            require_https: true,
            rate_limit_rpm: 10,
        };

        let plugin_declaration = NetworkDeclarations {
            allowed_domains: vec![],
            require_https: false,
            rate_limit_rpm: 10,
        };

        let mut np = NetworkPolicy::from_plugin_and_settings(&plugin_declaration, &user_setting);

        let result = np.check("http://abc.com");
        assert!(matches!(result, Err(PolicyViolation::HttpNotAllowed)));

        let resp = np.check("https://abc.com").unwrap();
        assert!(matches!(resp, CheckOutcome::Allowed));
    }
}
