use reqwest::Url;

use crate::{plugin::NetworkDeclarations, settings::PluginNetworkPolicy};

pub struct NetworkPolicy {
    config: PluginNetworkPolicy,
    request_count: u32,
    window_start: std::time::Instant,
}

pub enum CheckOutcome {
    Allowed,
    NeedsConsent { domain: String }, // reason
}

#[derive(Debug)]
pub enum PolicyViolation {
    PrivateAddress,
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
        // Intersection of allowed domains: plugin cannot access a domain the user
        // hasn't explicitly permitted. If either side is empty, no domains are allowed.
        let allowed_domains: Vec<String> =
            if user.allowed_domains.is_empty() || plugin.allowed_domains.is_empty() {
                Vec::new()
            } else {
                let mut v: Vec<String> = plugin
                    .allowed_domains
                    .iter()
                    .filter(|d| user.allowed_domains.contains(d))
                    .cloned()
                    .collect();
                v.dedup();
                v
            };

        // User's blocked list is authoritative — plugins declare no blocked domains.
        let mut blocked_domains = user.blocked_domains.clone();
        blocked_domains.sort_unstable();
        blocked_domains.dedup();

        let config = PluginNetworkPolicy {
            allowed_domains,
            blocked_domains,
            require_https: user.require_https || plugin.require_https,
            // Minimum rate limit — honour whichever side is more restrictive.
            rate_limit_rpm: user.rate_limit_rpm.min(plugin.rate_limit_rpm),
        };

        Self {
            config,
            request_count: 0,
            window_start: std::time::Instant::now(),
        }
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
        if self.is_private_address(url) {
            return Err(PolicyViolation::PrivateAddress);
        }
        if self.matches(&host) {
            return Ok(CheckOutcome::Allowed);
        }
        Ok(CheckOutcome::NeedsConsent {
            domain: host.to_string(),
        })
    }

    fn is_private_address(&self, url: &str) -> bool {
        let parsed_url = match Url::parse(url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let host = match parsed_url.host() {
            Some(h) => h.to_string(),
            None => return false,
        };

        // Check if it's an IP address (either private or localhost)
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            return match ip {
                std::net::IpAddr::V4(addr) => {
                    addr.is_private() || addr.is_loopback() || addr.is_link_local()
                }
                std::net::IpAddr::V6(addr) => addr.is_loopback() || addr.is_unicast_link_local(),
            };
        }

        // Resolve the hostname and check all returned addresses.
        // A single private answer in any position is enough to block the request.
        if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&format!("{}:80", host)) {
            for addr in addrs {
                let is_private = match addr.ip() {
                    std::net::IpAddr::V4(ip) => {
                        ip.is_private() || ip.is_loopback() || ip.is_link_local()
                    }
                    std::net::IpAddr::V6(ip) => {
                        ip.is_loopback()
                            || ip.is_unicast_link_local()
                            // Unique-local range fc00::/7
                            || (ip.segments()[0] & 0xfe00) == 0xfc00
                    }
                };
                if is_private {
                    return true;
                }
            }
        }

        false
    }

    fn matches(&self, host: &str) -> bool {
        // Check if explicitly blocked by user
        if self
            .config
            .blocked_domains
            .iter()
            .any(|blocked| self.domain_matches(host, blocked))
        {
            return false;
        }

        self.config
            .allowed_domains
            .iter()
            .any(|domain| self.domain_matches(host, domain))
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
