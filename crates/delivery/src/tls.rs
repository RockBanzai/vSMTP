#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Requirement {
    #[default]
    Required,
    Optional,
    Disabled,
}

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Tls {
    #[serde(default)]
    pub starttls: Requirement,
}
