use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

// Maximum resources to fetch per resource type
const MAX_RESOURCES: usize = 1000;
// Resources per page (FHIR _count parameter)
const PAGE_SIZE: usize = 100;
// Maximum concurrent page fetches
const MAX_CONCURRENT_FETCHES: usize = 10;

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

pub async fn fetch_capability_statement(
    client: &Client,
    fhir_base_url: &str,
) -> anyhow::Result<ServerCapabilities> {
    let url = format!("{}/metadata", fhir_base_url);
    println!("[FHIR] Fetching capability statement...");

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch capability statement: HTTP {}",
            response.status()
        ));
    }

    let capability_statement: CapabilityStatement = response.json().await?;

    if capability_statement.resource_type != "CapabilityStatement" {
        return Err(anyhow::anyhow!(
            "Expected CapabilityStatement but got: {}",
            capability_statement.resource_type
        ));
    }

    let capabilities = ServerCapabilities::from_capability_statement(capability_statement);

    println!(
        "[FHIR] Found {} resource types",
        capabilities.resources.len()
    );

    Ok(capabilities)
}

/// Fetch a single page of resources and return (resources, next_page_url)
async fn fetch_page(
    client: &Client,
    url: &str,
) -> anyhow::Result<(Vec<Value>, Option<String>)> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch resources: HTTP {}",
            response.status()
        ));
    }

    let bundle: Value = response.json().await?;

    if bundle["resourceType"] != "Bundle" {
        return Err(anyhow::anyhow!(
            "Expected Bundle but got: {}",
            bundle["resourceType"]
        ));
    }

    // Extract resources from entries
    let resources: Vec<Value> = bundle["entry"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| entry["resource"].as_object())
        .map(|resource| Value::Object(resource.clone()))
        .collect();

    // Find "next" link for pagination
    let next_url = bundle["link"]
        .as_array()
        .and_then(|links| {
            links.iter().find(|link| link["relation"] == "next")
        })
        .and_then(|link| link["url"].as_str())
        .map(|s| s.to_string());

    Ok((resources, next_url))
}

/// Fetch all resources of a type with pagination, up to MAX_RESOURCES
/// Sequential version - kept for reference/fallback
#[allow(dead_code)]
async fn fetch_resources(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
) -> anyhow::Result<Vec<Value>> {
    let initial_url = format!("{}?_count={}",
        format!("{}/{}", fhir_base_url, resource_type),
        PAGE_SIZE
    );

    // First, fetch the initial page to get the total and first batch of next URLs
    let (first_resources, first_next_url) = fetch_page(client, &initial_url).await?;

    if first_resources.is_empty() {
        return Ok(vec![]);
    }

    let mut all_resources = first_resources;

    // If no next page or we already have enough, return
    if first_next_url.is_none() || all_resources.len() >= MAX_RESOURCES {
        all_resources.truncate(MAX_RESOURCES);
        return Ok(all_resources);
    }

    // Collect all page URLs we need to fetch
    // We'll do this by fetching pages sequentially first to discover URLs,
    // then fetch remaining pages in parallel batches
    let mut next_urls: Vec<String> = vec![first_next_url.unwrap()];
    let mut current_url = next_urls[0].clone();

    // Discover page URLs (we need to do this sequentially as each page tells us the next)
    // But we'll limit discovery to avoid fetching too many pages
    let max_pages = MAX_RESOURCES / PAGE_SIZE;

    while next_urls.len() < max_pages {
        // Fetch just to get the next URL
        let response = client.get(&current_url).send().await?;
        if !response.status().is_success() {
            break;
        }

        let bundle: Value = response.json().await?;

        // Extract resources from this page
        let page_resources: Vec<Value> = bundle["entry"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|entry| entry["resource"].as_object())
            .map(|resource| Value::Object(resource.clone()))
            .collect();

        all_resources.extend(page_resources);

        if all_resources.len() >= MAX_RESOURCES {
            break;
        }

        // Find next URL
        let next_url = bundle["link"]
            .as_array()
            .and_then(|links| links.iter().find(|link| link["relation"] == "next"))
            .and_then(|link| link["url"].as_str())
            .map(|s| s.to_string());

        match next_url {
            Some(url) => {
                next_urls.push(url.clone());
                current_url = url;
            }
            None => break,
        }
    }

    all_resources.truncate(MAX_RESOURCES);
    Ok(all_resources)
}

/// Extract next page URL from bundle links
fn extract_next_url(bundle: &Value) -> Option<String> {
    // Try "link" array (standard FHIR)
    bundle["link"]
        .as_array()
        .and_then(|links| links.iter().find(|link| link["relation"] == "next"))
        .and_then(|link| link["url"].as_str())
        .map(|s| s.to_string())
        // Also try "links" array (some servers use this)
        .or_else(|| {
            bundle["links"]
                .as_array()
                .and_then(|links| links.iter().find(|link| link["relation"] == "next"))
                .and_then(|link| link["url"].as_str())
                .map(|s| s.to_string())
        })
}

/// Extract last page number from bundle links to calculate total pages
fn extract_last_page(bundle: &Value) -> Option<usize> {
    let last_url = bundle["link"]
        .as_array()
        .and_then(|links| links.iter().find(|link| link["relation"] == "last"))
        .and_then(|link| link["url"].as_str())
        .or_else(|| {
            bundle["links"]
                .as_array()
                .and_then(|links| links.iter().find(|link| link["relation"] == "last"))
                .and_then(|link| link["url"].as_str())
        })?;

    // Extract _page=N from URL
    last_url
        .split('&')
        .find(|param| param.starts_with("_page="))
        .and_then(|param| param.strip_prefix("_page="))
        .and_then(|num| num.parse().ok())
}

/// Fetch a single page by URL
async fn fetch_page_by_url(
    client: &Client,
    url: &str,
) -> anyhow::Result<Vec<Value>> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(anyhow::anyhow!(
            "Failed to fetch\nURL: {}\nHTTP {}\nBody: {}",
            url,
            status,
            body
        ));
    }

    let bundle: Value = response.json().await?;

    let resources: Vec<Value> = bundle["entry"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| entry["resource"].as_object())
        .map(|resource| Value::Object(resource.clone()))
        .collect();

    Ok(resources)
}

/// Fetch resources with parallel page fetching using link-based pagination
pub async fn fetch_resources_parallel(
    client: &Client,
    fhir_base_url: &str,
    resource_type: &str,
) -> anyhow::Result<Vec<Value>> {
    let initial_url = format!("{}/{}?_count={}", fhir_base_url, resource_type, PAGE_SIZE);

    // First, fetch initial page to discover pagination structure
    let response = client.get(&initial_url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch {}: HTTP {}",
            resource_type,
            response.status()
        ));
    }

    let bundle: Value = response.json().await?;

    // Extract first page resources
    let first_resources: Vec<Value> = bundle["entry"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| entry["resource"].as_object())
        .map(|resource| Value::Object(resource.clone()))
        .collect();

    if first_resources.is_empty() {
        return Ok(vec![]);
    }

    // Get total count from bundle
    let total = bundle["total"].as_u64().unwrap_or(0) as usize;
    let total_to_fetch = if total > 0 { total.min(MAX_RESOURCES) } else { MAX_RESOURCES };

    // Check if we have all resources already
    if first_resources.len() >= total_to_fetch {
        let mut result = first_resources;
        result.truncate(total_to_fetch);
        return Ok(result);
    }

    // Try to get last page number for parallel fetching
    let last_page = extract_last_page(&bundle);
    let next_url = extract_next_url(&bundle);

    // If we have last page info and _page pattern, we can fetch in parallel
    if let Some(last_page_num) = last_page {
        // Calculate max pages based on resource limit
        let max_pages = (MAX_RESOURCES + PAGE_SIZE - 1) / PAGE_SIZE;
        let pages_to_fetch = last_page_num.min(max_pages);

        // Generate all page URLs (pages 2 to last)
        let base_url = format!("{}/{}?_count={}", fhir_base_url, resource_type, PAGE_SIZE);
        let page_urls: Vec<String> = (2..=pages_to_fetch)
            .map(|page| format!("{}&_page={}", base_url, page))
            .collect();

        // Fetch all pages in parallel
        let results: Vec<anyhow::Result<Vec<Value>>> = stream::iter(page_urls)
            .map(|url| {
                let client = client.clone();
                async move {
                    fetch_page_by_url(&client, &url).await
                }
            })
            .buffer_unordered(MAX_CONCURRENT_FETCHES)
            .collect()
            .await;

        // Collect all resources
        let mut all_resources = first_resources;

        for result in results {
            match result {
                Ok(resources) => {
                    all_resources.extend(resources);
                }
                Err(e) => {
                    eprintln!("[FHIR] Error fetching page: {}", e);
                }
            }
        }

        all_resources.truncate(total_to_fetch);
        return Ok(all_resources);
    }

    // Fallback: Follow next links sequentially if no _page pattern found
    if next_url.is_none() {
        return Ok(first_resources);
    }

    let mut all_resources = first_resources;
    let mut current_next_url = next_url;

    while let Some(url) = current_next_url {
        if all_resources.len() >= total_to_fetch {
            break;
        }

        let response = client.get(&url).send().await?;
        if !response.status().is_success() {
            break;
        }

        let bundle: Value = response.json().await?;

        let page_resources: Vec<Value> = bundle["entry"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|entry| entry["resource"].as_object())
            .map(|resource| Value::Object(resource.clone()))
            .collect();

        if page_resources.is_empty() {
            break;
        }

        all_resources.extend(page_resources);
        current_next_url = extract_next_url(&bundle);
    }

    all_resources.truncate(total_to_fetch);
    Ok(all_resources)
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
