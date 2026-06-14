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
#[allow(dead_code)]
struct RowData {
    row: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct ResultData {
    #[allow(dead_code)]
    columns: Vec<String>,
    data: Vec<RowData>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
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

    pub async fn delete_capability(&self, capability_id: &str) -> Result<(), Box<dyn Error>> {
        info!("Deleting capability node from Neo4j: {}", capability_id);
        let statement = Statement {
            statement: "MATCH (c:Capability {id: $id}) DETACH DELETE c".to_string(),
            parameters: serde_json::json!({ "id": capability_id }),
        };

        self.execute(vec![statement]).await?;
        info!(
            "Successfully deleted capability {} from Neo4j.",
            capability_id
        );
        Ok(())
    }
}
