use crate::parser::CapabilityGenome;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tracing::info;

#[derive(Serialize, Debug)]
struct Statement {
    statement: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Debug)]
struct TransactionRequest {
    statements: Vec<Statement>,
}

#[derive(Deserialize, Debug)]
struct Neo4jError {
    code: String,
    message: String,
}

#[derive(Deserialize, Debug)]
struct RowData {
    row: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct ResultData {
    #[allow(dead_code)]
    columns: Vec<String>,
    data: Vec<RowData>,
}

#[derive(Deserialize, Debug)]
struct TransactionResponse {
    results: Vec<ResultData>,
    errors: Vec<Neo4jError>,
}

pub struct Neo4jClient {
    url: String,
    user: String,
    pass: String,
    http: reqwest::Client,
}

impl Neo4jClient {
    pub fn new(url: &str, user: &str, pass: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            user: user.to_string(),
            pass: pass.to_string(),
            http: reqwest::Client::new(),
        }
    }

    async fn execute(
        &self,
        statements: Vec<Statement>,
    ) -> Result<TransactionResponse, Box<dyn Error>> {
        let endpoint = format!("{}/db/neo4j/tx/commit", self.url);
        let payload = TransactionRequest { statements };

        let response = self
            .http
            .post(&endpoint)
            .basic_auth(&self.user, Some(&self.pass))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(
                format!("Neo4j API request failed with status {}: {}", status, body).into(),
            );
        }

        let tx_resp: TransactionResponse = response.json().await?;
        if !tx_resp.errors.is_empty() {
            let first_err = &tx_resp.errors[0];
            return Err(format!(
                "Neo4j Cypher error [{}]: {}",
                first_err.code, first_err.message
            )
            .into());
        }

        Ok(tx_resp)
    }

    pub async fn sync_genome(&self, genome: &CapabilityGenome) -> Result<(), Box<dyn Error>> {
        info!("Syncing genome to Neo4j: {}", genome.id);
        let mut statements = Vec::new();

        // 1. Merge Capability node
        statements.push(Statement {
            statement: "MERGE (c:Capability {id: $id}) \
                        SET c.name = $name, c.version = $version, c.description = $description"
                .to_string(),
            parameters: serde_json::json!({
                "id": genome.id,
                "name": genome.name,
                "version": genome.version,
                "description": genome.description.clone().unwrap_or_default()
            }),
        });

        // 2. Clear old outbound relations from this capability
        statements.push(Statement {
            statement: "MATCH (c:Capability {id: $id}) \
                        OPTIONAL MATCH (c)-[r:HAS_BEHAVIOR|HAS_METRIC|HAS_CONSTRAINT|DEPENDS_ON]->() \
                        DELETE r"
                .to_string(),
            parameters: serde_json::json!({ "id": genome.id }),
        });

        // 3. Add behaviors
        if let Some(behaviors) = &genome.behaviors {
            for behavior in behaviors {
                statements.push(Statement {
                    statement: "MATCH (c:Capability {id: $id}) \
                                MERGE (b:Behavior {name: $b_name}) \
                                SET b.description = $b_desc \
                                MERGE (c)-[:HAS_BEHAVIOR]->(b)"
                        .to_string(),
                    parameters: serde_json::json!({
                        "id": genome.id,
                        "b_name": behavior.name,
                        "b_desc": behavior.description.clone().unwrap_or_default()
                    }),
                });
            }
        }

        // 4. Add metrics
        if let Some(metrics) = &genome.metrics {
            for metric in metrics {
                statements.push(Statement {
                    statement: "MATCH (c:Capability {id: $id}) \
                                MERGE (m:Metric {name: $m_name}) \
                                SET m.type = $m_type, m.target = $m_target \
                                MERGE (c)-[:HAS_METRIC]->(m)"
                        .to_string(),
                    parameters: serde_json::json!({
                        "id": genome.id,
                        "m_name": metric.name,
                        "m_type": metric.r#type,
                        "m_target": metric.target
                    }),
                });
            }
        }

        // 5. Add constraints
        if let Some(constraints) = &genome.constraints {
            for constraint in constraints {
                statements.push(Statement {
                    statement: "MATCH (c:Capability {id: $id}) \
                                MERGE (co:Constraint {type: $co_type, value: $co_value}) \
                                MERGE (c)-[:HAS_CONSTRAINT]->(co)"
                        .to_string(),
                    parameters: serde_json::json!({
                        "id": genome.id,
                        "co_type": constraint.r#type,
                        "co_value": constraint.value
                    }),
                });
            }
        }

        // 6. Add dependencies
        if let Some(dependencies) = &genome.dependencies {
            for dep_id in dependencies {
                statements.push(Statement {
                    statement: "MATCH (c:Capability {id: $id}) \
                                MERGE (dep:Capability {id: $dep_id}) \
                                MERGE (c)-[:DEPENDS_ON]->(dep)"
                        .to_string(),
                    parameters: serde_json::json!({
                        "id": genome.id,
                        "dep_id": dep_id
                    }),
                });
            }
        }

        self.execute(statements).await?;
        info!("Successfully synced genome {} to Neo4j.", genome.id);
        Ok(())
    }

    pub async fn get_recursive_dependencies(
        &self,
        capability_id: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        info!("Fetching recursive dependencies for {}", capability_id);
        let statement = Statement {
            statement: "MATCH (c:Capability {id: $id})-[:DEPENDS_ON*]->(dep:Capability) \
                        RETURN DISTINCT dep.id AS dep_id"
                .to_string(),
            parameters: serde_json::json!({ "id": capability_id }),
        };

        let tx_resp = self.execute(vec![statement]).await?;
        let mut deps = Vec::new();

        if let Some(result) = tx_resp.results.first() {
            for row_data in &result.data {
                if let Some(val) = row_data.row.first() {
                    if let Some(dep_id) = val.as_str() {
                        deps.push(dep_id.to_string());
                    }
                }
            }
        }

        Ok(deps)
    }
}
