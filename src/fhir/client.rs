pub fn put_to_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
    content: &str,
) -> anyhow::Result<String> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    println!("Sending resource to FHIR server:");
    println!("  Method: PUT");
    println!("  URL: {}", url);
    println!("  Resource Type: {}", resource_type);
    println!("  Resource ID: {}", resource_id);
    println!("  Content size: {} bytes", content.len());

    let client = reqwest::blocking::Client::new();
    let response = client
        .put(&url)
        .header("Content-Type", "application/fhir+json")
        .body(content.to_string())
        .send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!("  Response status: {}", status);

    if status.is_success() {
        println!("  ✓ Successfully sent to FHIR server (PUT)");
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                println!("  Resource ID in response: {}", id);
            }
        }
        Ok(response_text)
    } else {
        println!("  ✗ Failed to send to FHIR server");
        println!("  Error response: {}", response_text);
        Err(anyhow::anyhow!(
            "Failed to PUT resource to FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}

pub fn delete_from_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
) -> anyhow::Result<()> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    println!("Deleting resource from FHIR server:");
    println!("  Method: DELETE");
    println!("  URL: {}", url);
    println!("  Resource Type: {}", resource_type);
    println!("  Resource ID: {}", resource_id);

    let client = reqwest::blocking::Client::new();
    let response = client.delete(&url).send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!("  Response status: {}", status);

    if status.is_success() || status.as_u16() == 404 {
        println!("  ✓ Successfully deleted from FHIR server");
        Ok(())
    } else {
        println!("  ✗ Failed to delete from FHIR server");
        println!("  Error response: {}", response_text);
        Err(anyhow::anyhow!(
            "Failed to DELETE resource from FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}
