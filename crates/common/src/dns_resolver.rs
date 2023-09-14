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

impl DnsResolver {
    #[must_use]
    pub fn google() -> Self {
        let config = trust_dns_resolver::config::ResolverConfig::google();
        let option = trust_dns_resolver::config::ResolverOpts::default();

        Self {
            config: config.clone(),
            option,
            resolver: trust_dns_resolver::TokioAsyncResolver::tokio(config, option),
        }
    }
}

impl<'de> serde::Deserialize<'de> for DnsResolver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let Inner { config, option } = Inner::deserialize(deserializer)?;
        Ok(Self {
            config: config.clone(),
            option,
            resolver: trust_dns_resolver::TokioAsyncResolver::tokio(config, option),
        })
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Inner {
    #[serde(default, deserialize_with = "deserialize_config")]
    config: trust_dns_resolver::config::ResolverConfig,
    #[serde(default)]
    option: trust_dns_resolver::config::ResolverOpts,
}

fn deserialize_config<'de, D>(
    deserialize: D,
) -> Result<trust_dns_resolver::config::ResolverConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = trust_dns_resolver::config::ResolverConfig;

        fn expecting(
            &self,
            fmt: &mut std::fmt::Formatter<'_>,
        ) -> std::result::Result<(), std::fmt::Error> {
            write!(
                fmt,
                "either a build-in config among '{}' or a map following the `ResolverConfig` scheme",
                <BuildIn as strum::VariantNames>::VARIANTS.join("|")
            )
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            <BuildIn as std::str::FromStr>::from_str(v)
                .map(Self::Value::from)
                .map_err(|e| serde::de::Error::custom(e))
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            <trust_dns_resolver::config::ResolverConfig as serde::Deserialize>::deserialize(
                serde::de::value::MapAccessDeserializer::new(map),
            )
        }
    }

    deserialize.deserialize_any(Visitor)
}

#[derive(strum::EnumString, strum::EnumVariantNames)]
#[strum(serialize_all = "snake_case")]
enum BuildIn {
    Google,
    GoogleTls,
    Cloudflare,
    CloudflareTls,
    Quad9,
    Quad9Tls,
}

impl From<BuildIn> for trust_dns_resolver::config::ResolverConfig {
    fn from(val: BuildIn) -> Self {
        match val {
            BuildIn::Google => Self::google(),
            BuildIn::GoogleTls => Self::google_tls(),
            BuildIn::Cloudflare => Self::cloudflare(),
            BuildIn::CloudflareTls => Self::cloudflare_tls(),
            BuildIn::Quad9 => Self::quad9(),
            BuildIn::Quad9Tls => Self::quad9_tls(),
        }
    }
}
