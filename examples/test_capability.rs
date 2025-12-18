use std::env;

// Add the parent directory modules to the path
#[path = "../src/capability.rs"]
mod capability;

use capability::{fetch_capability_statement, fetch_resources};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <fhir_base_url>", args[0]);
        eprintln!("Example: {} http://localhost:8080/fhir", args[0]);
        std::process::exit(1);
    }

    let fhir_base_url = &args[1];

    println!("Testing FHIR server at: {}", fhir_base_url);
    println!("{}", "=".repeat(50));

    // Test fetching capability statement
    println!("\n1. Fetching Capability Statement...");
    match fetch_capability_statement(fhir_base_url) {
        Ok(capabilities) => {
            println!("✓ Successfully fetched capability statement");
            println!("  Found {} resource types:", capabilities.resources.len());

            // Display first 10 resource types
            for (i, resource) in capabilities.resources.iter().take(10).enumerate() {
                println!("    {}. {}", i + 1, resource);
            }

            if capabilities.resources.len() > 10 {
                println!("    ... and {} more", capabilities.resources.len() - 10);
            }

            println!(
                "\n  Searchable resources: {}",
                capabilities.searchable_resources.len()
            );

            // Test fetching some resources if available
            println!("\n2. Testing Resource Fetching...");

            // Try to fetch Patient resources
            if capabilities.resources.contains(&"Patient".to_string()) {
                println!("\n  Fetching Patient resources (limit 5)...");
                match fetch_resources(fhir_base_url, "Patient", Some(5)) {
                    Ok(patients) => {
                        println!("  ✓ Found {} Patient resources", patients.len());
                        for patient in patients.iter().take(3) {
                            if let Some(id) = patient.get("id").and_then(|v| v.as_str()) {
                                println!("    - Patient ID: {}", id);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Failed to fetch Patient resources: {}", e);
                    }
                }
            }

            // Try to fetch Observation resources if available
            if capabilities.resources.contains(&"Observation".to_string()) {
                println!("\n  Fetching Observation resources (limit 5)...");
                match fetch_resources(fhir_base_url, "Observation", Some(5)) {
                    Ok(observations) => {
                        println!("  ✓ Found {} Observation resources", observations.len());
                        for obs in observations.iter().take(3) {
                            if let Some(id) = obs.get("id").and_then(|v| v.as_str()) {
                                println!("    - Observation ID: {}", id);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Failed to fetch Observation resources: {}", e);
                    }
                }
            }

            // Try to fetch Encounter resources if available
            if capabilities.resources.contains(&"Encounter".to_string()) {
                println!("\n  Fetching Encounter resources (limit 5)...");
                match fetch_resources(fhir_base_url, "Encounter", Some(5)) {
                    Ok(encounters) => {
                        println!("  ✓ Found {} Encounter resources", encounters.len());
                        for enc in encounters.iter().take(3) {
                            if let Some(id) = enc.get("id").and_then(|v| v.as_str()) {
                                println!("    - Encounter ID: {}", id);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Failed to fetch Encounter resources: {}", e);
                    }
                }
            }

            println!("\n3. Summary");
            println!("{}", "=".repeat(50));
            println!(
                "Server supports {} resource types",
                capabilities.resources.len()
            );
            println!(
                "Server supports searching on {} resource types",
                capabilities.searchable_resources.len()
            );
        }
        Err(e) => {
            println!("✗ Failed to fetch capability statement");
            println!("  Error: {:#?}", e);
            println!("\nPossible causes:");
            println!("  - Server is not running");
            println!("  - Incorrect FHIR base URL");
            println!("  - Server doesn't support capability statement");
            std::process::exit(1);
        }
    }

    println!("\n✓ All tests completed successfully!");
}
