use std::cmp::max;

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
        let config = PluginNetworkPolicy {
            allowed_domains: [
                user.allowed_domains.as_slice(),
                plugin.allowed_domains.as_slice(),
            ]
            .concat(),
            blocked_domains: user.blocked_domains.clone(),
            require_https: user.require_https || plugin.require_https,
            rate_limit_rpm: max(user.rate_limit_rpm, plugin.rate_limit_rpm),
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

        let host = parsed_url.host().unwrap().to_string();

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
            domain: url.to_string(),
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

        // Try to resolve hostname to see if it resolves to a private IP
        // Using std::net::ToSocketAddrs (blocking call) - for a production system,
        // consider using async DNS resolution
        if let Ok(mut addrs) = format!("{}:80", host)
            .parse::<std::net::SocketAddr>()
            .map(|addr| vec![addr].into_iter())
            .or_else(|_| std::net::ToSocketAddrs::to_socket_addrs(&format!("{}:80", host)))
        {
            if let Some(addr) = addrs.next() {
                return matches!(addr.ip(), std::net::IpAddr::V4(ip) if ip.is_private() || ip.is_loopback() || ip.is_link_local());
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
