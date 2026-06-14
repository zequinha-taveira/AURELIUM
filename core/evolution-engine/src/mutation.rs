use rand::Rng;
use serde::{Deserialize, Serialize};
use std::error::Error;

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

pub fn mutate_genome(yaml_str: &str) -> Result<(CapabilityGenome, String), Box<dyn Error>> {
    let mut genome: CapabilityGenome = serde_yaml::from_str(yaml_str)?;
    let mut rng = rand::thread_rng();

    // 1. Generate mutation suffix
    let mut_id = rng.gen_range(1000..9999);
    let parent_id = genome.id.clone();
    let old_version = genome.version.clone();
    let new_version = format!("{}-mut-{}", old_version, mut_id);
    let new_id = format!("{}@v{}", parent_id, new_version);

    genome.id = new_id;
    genome.version = new_version;
    genome.name = format!("{} (Mutation {})", genome.name, mut_id);

    // 2. Apply target metrics mutations (e.g. reduce latency target by 15%)
    if let Some(metrics) = &mut genome.metrics {
        let mut mutated_metrics = Vec::new();
        for m in metrics {
            let mut new_target = m.target;
            if m.r#type == "latency" || m.name.contains("latency") || m.name.contains("time") {
                // pressure to optimize latency: reduce target by 15%
                new_target = (m.target * 0.85 * 100.0).round() / 100.0;
            }
            mutated_metrics.push(Metric {
                name: m.name.clone(),
                r#type: m.r#type.clone(),
                target: new_target,
            });
        }
        genome.metrics = Some(mutated_metrics);
    }

    // 3. Apply constraints mutations (e.g. increase concurrency limit by 20%)
    if let Some(constraints) = &mut genome.constraints {
        let mut mutated_constraints = Vec::new();
        for c in constraints {
            let mut new_value = c.value.clone();
            if c.r#type == "concurrency_limit" {
                if let Ok(limit) = c.value.parse::<i32>() {
                    let mutated_limit = (limit as f64 * 1.20) as i32;
                    new_value = mutated_limit.to_string();
                }
            }
            mutated_constraints.push(Constraint {
                r#type: c.r#type.clone(),
                value: new_value,
            });
        }
        genome.constraints = Some(mutated_constraints);
    }

    // Serialize back to YAML
    let mutated_yaml = serde_yaml::to_string(&genome)?;
    Ok((genome, mutated_yaml))
}
