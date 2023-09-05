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

#[derive(Debug, serde_with::SerializeDisplay, serde_with::DeserializeFromStr)]
pub struct Frequency {
    raw: String,
    frequency: std::time::Duration,
}

impl AsRef<std::time::Duration> for Frequency {
    fn as_ref(&self) -> &std::time::Duration {
        &self.frequency
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseErrorFrequency {
    #[error("expected format: <tick>/<duration>")]
    Format,
    #[error("invalid tick {0}")]
    Tick(#[from] std::num::ParseFloatError),
    #[error("invalid duration {0}")]
    Duration(#[from] humantime::DurationError),
}

impl std::str::FromStr for Frequency {
    type Err = ParseErrorFrequency;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (tick, per_unit) = s.split_once('/').ok_or(Self::Err::Format)?;
        let tick = tick.parse::<f64>()?;
        let per_unit = humantime::parse_duration(per_unit)?;

        Ok(Self {
            frequency: per_unit.div_f64(tick),
            raw: s.to_string(),
        })
    }
}

impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.raw.fmt(f)
    }
}

#[test]
fn frequency() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct S {
        v: Frequency,
    }

    assert_eq!(
        "60/1s".parse::<Frequency>().unwrap().frequency,
        std::time::Duration::from_nanos(16_666_667) // 16.67ms
    );
    assert_eq!(
        "1/1h".parse::<Frequency>().unwrap().frequency,
        std::time::Duration::from_secs(3600)
    );

    let input = r#"{"v":"60/1s"}"#;
    let output = serde_json::to_string(&serde_json::from_str::<S>(input).unwrap()).unwrap();
    assert_eq!(input, output);
}
