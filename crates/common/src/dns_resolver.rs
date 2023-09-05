/*
 * vSMTP mail transfer agent
 *
 * Copyright (C) 2003 - viridIT SAS
 * Licensed under the Elastic License 2.0
 *
 * You should have received a copy of the Elastic License 2.0 along with
 * this program. If not, see https://www.elastic.co/licensing/elastic-license.
 *
 */

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
pub struct DnsResolver {
    config: trust_dns_resolver::config::ResolverConfig,
    option: trust_dns_resolver::config::ResolverOpts,
    #[serde(skip)]
    pub resolver: trust_dns_resolver::TokioAsyncResolver,
}

impl<'de> serde::Deserialize<'de> for DnsResolver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Inner {
            #[serde(default)]
            config: trust_dns_resolver::config::ResolverConfig,
            #[serde(default)]
            option: trust_dns_resolver::config::ResolverOpts,
        }
        let Inner { config, option } = Inner::deserialize(deserializer)?;

        let resolver = trust_dns_resolver::TokioAsyncResolver::tokio(config.clone(), option);
        Ok(Self {
            config,
            option,
            resolver,
        })
    }
}
