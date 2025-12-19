use reqwest::Client;
use serde_json::json;

#[allow(dead_code)]
pub async fn get_from_fhir_server(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
    resource_id: &str,
) -> anyhow::Result<String> {
    let url = format!(
        "{}/{}/{}?_pretty=true",
        fhir_base_url, resource_type, resource_id
    );

    let response = client
        .get(&url)
        .header("Accept", "application/fhir+json")
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;

    if status.is_success() {
        Ok(response_text)
    } else {
        Err(anyhow::anyhow!(
            "Failed to GET resource from FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}

pub async fn put_to_fhir_server(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
    content: &str,
) -> anyhow::Result<String> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    let response = client
        .put(&url)
        .header("Content-Type", "application/fhir+json")
        .body(content.to_string())
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;

    if status.is_success() {
        Ok(response_text)
    } else {
        Err(anyhow::anyhow!(
            "Failed to PUT resource to FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}

pub async fn delete_from_fhir_server(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
) -> anyhow::Result<()> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    let response = client.delete(&url).send().await?;

    let status = response.status();
    let response_text = response.text().await?;

    if status.is_success() || status.as_u16() == 404 {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to DELETE resource from FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}

pub async fn execute_operation(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
    resource_id: &str,
    operation: &str,
    format: &str,
) -> anyhow::Result<String> {
    let url = format!(
        "{}/{}/{}/{}",
        fhir_base_url, resource_type, resource_id, operation
    );

    let parameters = json!({
        "resourceType": "Parameters",
        "parameter": [
            {
                "name": "_format",
                "valueCode": format
            }
        ]
    });

    // Set Accept header based on requested format
    let accept_header = match format {
        "csv" => "text/csv",
        _ => "application/json", // Default to JSON
    };

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", accept_header)
        .body(parameters.to_string())
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;

    if status.is_success() {
        // Format JSON responses with 2-space indentation
        if format == "json" {
            match serde_json::from_str::<serde_json::Value>(&response_text) {
                Ok(json_value) => {
                    match serde_json::to_string_pretty(&json_value) {
                        Ok(formatted) => Ok(formatted),
                        Err(_) => Ok(response_text), // Fall back to original if formatting fails
                    }
                }
                Err(_) => Ok(response_text), // Fall back to original if parsing fails
            }
        } else {
            Ok(response_text)
        }
    } else {
        Err(anyhow::anyhow!(
            "Failed to execute operation {} on {}/{}: HTTP {} - {}",
            operation,
            resource_type,
            resource_id,
            status,
            response_text
        ))
    }
}

/// Search for resources using FHIR search API
/// Returns resources grouped by resource type (important for _include/_revinclude)
pub async fn search_fhir_resources(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
    query: &str,
) -> anyhow::Result<std::collections::HashMap<String, Vec<serde_json::Value>>> {
    use std::collections::HashMap;

    let url = format!("{}/{}?{}", fhir_base_url, resource_type, query);
    println!("[FHIR] Search: {}", url);

    let response = client
        .get(&url)
        .header("Accept", "application/fhir+json")
        .send()
        .await?;

    let status = response.status();
    let response_text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to search FHIR server: HTTP {} - {}",
            status,
            response_text
        ));
    }

    let bundle: serde_json::Value = serde_json::from_str(&response_text)?;
    let mut grouped_resources: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

    if let Some(entries) = bundle.get("entry").and_then(|e| e.as_array()) {
        for entry in entries {
            if let Some(resource) = entry.get("resource") {
                let res_type = resource
                    .get("resourceType")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                grouped_resources
                    .entry(res_type)
                    .or_insert_with(Vec::new)
                    .push(resource.clone());
            }
        }
    }

    let total: usize = grouped_resources.values().map(|v| v.len()).sum();
    println!(
        "[FHIR] Search returned {} resources across {} types",
        total,
        grouped_resources.len()
    );

    Ok(grouped_resources)
}
