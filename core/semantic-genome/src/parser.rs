use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Behavior {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub r#type: String,
    pub target: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Constraint {
    pub r#type: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CapabilityGenome {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub behaviors: Option<Vec<Behavior>>,
    pub metrics: Option<Vec<Metric>>,
    pub dependencies: Option<Vec<String>>,
    pub constraints: Option<Vec<Constraint>>,
}

pub fn parse_genome(yaml_str: &str) -> Result<CapabilityGenome, serde_yaml::Error> {
    serde_yaml::from_str(yaml_str)
}
