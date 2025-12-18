#[allow(dead_code)]
pub fn get_from_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    resource_id: &str,
) -> anyhow::Result<String> {
    let url = format!(
        "{}/{}/{}?_pretty=true",
        fhir_base_url, resource_type, resource_id
    );

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/fhir+json")
        .send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!("FHIR GET {}/{} -> {}", resource_type, resource_id, status);

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

pub fn put_to_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
    content: &str,
) -> anyhow::Result<String> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    let client = reqwest::blocking::Client::new();
    let response = client
        .put(&url)
        .header("Content-Type", "application/fhir+json")
        .body(content.to_string())
        .send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!("FHIR PUT {}/{} -> {}", resource_type, resource_id, status);

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

pub fn delete_from_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
) -> anyhow::Result<()> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    let client = reqwest::blocking::Client::new();
    let response = client.delete(&url).send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!(
        "FHIR DELETE {}/{} -> {}",
        resource_type, resource_id, status
    );

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
