use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Deserialize, Serialize)]
pub struct CapabilityStatement {
    #[serde(rename = "resourceType")]
    resource_type: String,
    rest: Option<Vec<RestResource>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RestResource {
    mode: Option<String>,
    resource: Option<Vec<ResourceDefinition>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ResourceDefinition {
    #[serde(rename = "type")]
    resource_type: String,
    profile: Option<String>,
    interaction: Option<Vec<Interaction>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Interaction {
    code: String,
}

#[derive(Debug, Clone)]
pub struct ServerCapabilities {
    pub resources: Vec<String>,
    pub searchable_resources: HashSet<String>,
}

impl ServerCapabilities {
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            searchable_resources: HashSet::new(),
        }
    }

    pub fn from_capability_statement(statement: CapabilityStatement) -> Self {
        let mut capabilities = Self::new();

        if let Some(rest_resources) = statement.rest {
            for rest in rest_resources {
                if let Some(mode) = &rest.mode {
                    if mode == "server" {
                        if let Some(resources) = rest.resource {
                            for resource in resources {
                                capabilities.resources.push(resource.resource_type.clone());

                                if let Some(interactions) = &resource.interaction {
                                    for interaction in interactions {
                                        if interaction.code == "search-type" {
                                            capabilities
                                                .searchable_resources
                                                .insert(resource.resource_type.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        capabilities.resources.sort();
        capabilities
    }
}

pub fn fetch_capability_statement(fhir_base_url: &str) -> anyhow::Result<ServerCapabilities> {
    let url = format!("{}/metadata", fhir_base_url);
    println!("Fetching capability statement from: {}", url);

    let response = reqwest::blocking::get(&url)?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch capability statement: HTTP {}",
            response.status()
        ));
    }

    let capability_statement: CapabilityStatement = response.json()?;

    if capability_statement.resource_type != "CapabilityStatement" {
        return Err(anyhow::anyhow!(
            "Expected CapabilityStatement but got: {}",
            capability_statement.resource_type
        ));
    }

    let capabilities = ServerCapabilities::from_capability_statement(capability_statement);

    println!("Found {} resource types", capabilities.resources.len());
    println!(
        "Searchable resources: {}",
        capabilities.searchable_resources.len()
    );

    Ok(capabilities)
}

pub fn fetch_resources(
    fhir_base_url: &str,
    resource_type: &str,
    count: Option<usize>,
) -> anyhow::Result<Vec<Value>> {
    let mut url = format!("{}/{}", fhir_base_url, resource_type);

    if let Some(count) = count {
        url = format!("{}?_count={}", url, count);
    }

    println!("Fetching {} resources from: {}", resource_type, url);

    let response = reqwest::blocking::get(&url)?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch {} resources: HTTP {}",
            resource_type,
            response.status()
        ));
    }

    let bundle: Value = response.json()?;

    if bundle["resourceType"] != "Bundle" {
        return Err(anyhow::anyhow!(
            "Expected Bundle but got: {}",
            bundle["resourceType"]
        ));
    }

    let resources = bundle["entry"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| entry["resource"].as_object())
        .map(|resource| Value::Object(resource.clone()))
        .collect();

    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_capabilities_new() {
        let capabilities = ServerCapabilities::new();
        assert!(capabilities.resources.is_empty());
        assert!(capabilities.searchable_resources.is_empty());
    }

    #[test]
    fn test_from_capability_statement() {
        let json = r#"{
            "resourceType": "CapabilityStatement",
            "rest": [
                {
                    "mode": "server",
                    "resource": [
                        {
                            "type": "Patient",
                            "interaction": [
                                {"code": "read"},
                                {"code": "search-type"}
                            ]
                        },
                        {
                            "type": "Observation",
                            "interaction": [
                                {"code": "read"}
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let statement: CapabilityStatement = serde_json::from_str(json).unwrap();
        let capabilities = ServerCapabilities::from_capability_statement(statement);

        assert_eq!(capabilities.resources.len(), 2);
        assert!(capabilities.resources.contains(&"Patient".to_string()));
        assert!(capabilities.resources.contains(&"Observation".to_string()));
        assert_eq!(capabilities.searchable_resources.len(), 1);
        assert!(capabilities.searchable_resources.contains("Patient"));
    }
}
