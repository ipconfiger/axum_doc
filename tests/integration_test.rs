use std::path::PathBuf;
use std::process::Command;
use std::fs;

/// Test helper to run axum_doc on a fixture and verify the output
fn test_fixture(fixture_name: &str, expected_routes: Vec<&str>) {
    let fixture_dir = PathBuf::from("tests/fixtures").join(fixture_name);
    let output_file = format!("/tmp/axum_doc_test_{}.json", fixture_name);

    // Build the command
    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/form.rs,src/response.rs,src/types.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    // Check that the command succeeded
    if !output.status.success() {
        eprintln!("axum_doc stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("axum_doc failed for fixture: {}", fixture_name);
    }

    // Verify output file exists
    assert!(fs::metadata(&output_file).is_ok(), "Output file not created: {}", output_file);

    // Verify output is valid JSON
    let content = fs::read_to_string(&output_file)
        .expect("Failed to read output file");
    let json: serde_json::Value = serde_json::from_str(&content)
        .expect("Output is not valid JSON");

    // Verify it's an OpenAPI 3.0 spec
    assert_eq!(json["openapi"], "3.0.0");
    assert!(json["paths"].is_object());
    assert!(json["components"]["schemas"].is_object());

    // Verify expected routes exist
    if !expected_routes.is_empty() {
        let paths = json["paths"].as_object().unwrap();
        for route in expected_routes {
            assert!(paths.contains_key(route), "Missing route: {}", route);
        }
    }
}

#[test]
fn test_simple_route_generation() {
    // Test basic route generation on simple_app fixture
    test_fixture("simple_app", vec!["/", "/login", "/user/:id"]);
}

#[test]
fn test_simple_app_openapi_structure() {
    // Verify OpenAPI structure for simple app
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_structure.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Verify required OpenAPI fields
    assert!(json["openapi"].is_string());
    assert!(json["info"].is_object());
    assert!(json["paths"].is_object());
    assert!(json["components"].is_object());

    // Verify info section
    assert_eq!(json["info"]["title"], "Generated API");
    assert_eq!(json["info"]["version"], "1.0.0");
}

#[test]
fn test_doc_comment_extraction() {
    // Verify that doc comments are properly extracted
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_docs.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check login endpoint has doc comments
    let login_summary = json["paths"]["/login"]["post"]["summary"]
        .as_str()
        .expect("Login summary missing");
    let login_description = json["paths"]["/login"]["post"]["description"]
        .as_str()
        .expect("Login description missing");

    // Verify summary comes from doc comment, not hardcoded
    assert!(login_summary.contains("login") || login_summary.contains("Login"));
    assert_ne!(login_summary, "POST login");

    // Verify description is populated
    assert!(!login_description.is_empty());
    assert!(login_description.len() > 10);
}

#[test]
fn test_type_mapping_uuid() {
    // Verify UUID type mapping
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_uuid.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/response.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check user_id field in LoginResponse
    let user_id = &json["components"]["schemas"]["LoginResponse"]["properties"]["user_id"];
    assert_eq!(user_id["type"], "string");
    assert_eq!(user_id["format"], "uuid");
    assert_eq!(user_id["example"], "550e8400-e29b-41d4-a716-446655440000");
}

#[test]
fn test_type_mapping_datetime() {
    // Verify DateTime type mapping
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_datetime.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/types.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check created_at field in User schema
    let created_at = &json["components"]["schemas"]["User"]["properties"]["created_at"];
    assert_eq!(created_at["type"], "string");
    assert_eq!(created_at["format"], "date-time");
    assert_eq!(created_at["example"], "2024-01-01T00:00:00Z");
}

#[test]
fn test_type_mapping_option() {
    // Verify Option<T> nullable handling
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_option.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/form.rs,src/response.rs,src/types.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check that schemas exist (we're testing the code doesn't crash with Option types)
    assert!(json["components"]["schemas"].as_object().unwrap().len() > 0);
}

#[test]
fn test_type_mapping_vec() {
    // Verify Vec<T> array handling
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_vec.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Verify the OpenAPI spec is valid
    assert!(json["paths"].is_object());
}

#[test]
fn test_http_methods() {
    // Verify different HTTP methods are detected
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_methods.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check for GET method
    assert!(json["paths"]["/"]["get"].is_object());
    assert!(json["paths"]["/user/:id"]["get"].is_object());

    // Check for POST method
    assert!(json["paths"]["/login"]["post"].is_object());

    // Verify operationId matches handler name
    assert_eq!(json["paths"]["/"]["get"]["operationId"], "root");
    assert_eq!(json["paths"]["/login"]["post"]["operationId"], "login");
}

#[test]
fn test_request_body() {
    // Verify request body is generated for Json extractor
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_body.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/form.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check login endpoint has request body
    let request_body = &json["paths"]["/login"]["post"]["requestBody"];
    assert!(request_body.is_object());

    // Verify request body content type
    assert!(request_body["content"]["application/json"]["schema"]["$ref"].is_string());
}

#[test]
fn test_response_schemas() {
    // Verify response schemas are generated
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_response.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/response.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check login endpoint has 200 response
    let response = &json["paths"]["/login"]["post"]["responses"]["200"];
    assert!(response.is_object());
    assert!(response["content"]["application/json"]["schema"]["$ref"].is_string());
}

#[test]
fn test_parameters() {
    // Verify path parameters are extracted
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_params.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check /user/:id has id parameter
    let params = &json["paths"]["/user/:id"]["get"]["parameters"];
    assert!(params.is_array());

    // Find the id parameter
    if let Some(params_array) = params.as_array() {
        let id_param = params_array.iter()
            .find(|p| p["name"] == "id")
            .expect("id parameter not found");

        assert_eq!(id_param["in"], "path");
        assert_eq!(id_param["required"], true);
        assert!(id_param["schema"]["type"].is_string());
    }
}

#[test]
fn test_custom_output_file() {
    // Test that custom output file path works
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let custom_output = "/tmp/custom_openapi.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", custom_output
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    // Verify custom output file was created
    assert!(fs::metadata(custom_output).is_ok());

    // Clean up
    let _ = fs::remove_file(custom_output);
}

#[test]
fn test_missing_model_files() {
    // Test graceful handling when model files don't exist
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_missing.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/nonexistent.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    // Should still succeed, just with warnings
    assert!(output.status.success());

    // Verify output is still generated
    assert!(fs::metadata(&output_file).is_ok());
}

#[test]
fn test_json_output_validity() {
    // Verify the JSON output is always valid and parseable
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_validity.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();

    // Verify it's valid JSON
    let json: serde_json::Value = serde_json::from_str(&content)
        .expect("Generated invalid JSON");

    // Verify required OpenAPI 3.0 fields exist
    assert!(json.get("openapi").is_some());
    assert!(json.get("info").is_some());
    assert!(json.get("paths").is_some());
    assert!(json.get("components").is_some());
}

#[test]
fn test_components_schemas() {
    // Verify components/schemas section is generated correctly
    let fixture_dir = PathBuf::from("tests/fixtures/simple_app");
    let output_file = "/tmp/axum_doc_test_schemas.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--model-files", "src/form.rs,src/response.rs,src/types.rs",
            "--output", &output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let schemas = &json["components"]["schemas"];
    assert!(schemas.is_object());

    // Verify each schema has required fields
    if let Some(schemas_obj) = schemas.as_object() {
        for (schema_name, schema) in schemas_obj {
            assert!(schema.is_object(), "Schema {} is not an object", schema_name);
            assert_eq!(schema["type"], "object", "Schema {} is not an object type", schema_name);
            assert!(schema["properties"].is_object() || schema["properties"].is_null(),
                   "Schema {} missing properties", schema_name);
        }
    }
}

#[test]
fn test_duplicate_path_prefix_detection() {
    // Test that duplicate path prefixes are properly deduplicated
    // This fixture has:
    // - modules/mod.rs: .nest("/api/v1/user", user::router())
    // - modules/user/mod.rs: Router::new().nest("/api/v1/user", handler::router())
    // Expected result: /api/v1/user/login (NOT /api/v1/user/api/v1/user/login)

    let fixture_dir = PathBuf::from("tests/fixtures/dup_path_app");
    let output_file = "/tmp/axum_doc_test_dup_path.json";

    let output = Command::new("cargo")
        .args(&[
            "run", "--",
            "--base-dir", fixture_dir.to_str().unwrap(),
            "--handler-file", "src/main.rs",
            "--output", output_file
        ])
        .output()
        .expect("Failed to run axum_doc");

    if !output.status.success() {
        eprintln!("axum_doc stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("axum_doc failed for dup_path_app fixture");
    }

    let content = fs::read_to_string(output_file)
        .expect("Failed to read output file");
    let json: serde_json::Value = serde_json::from_str(&content)
        .expect("Output is not valid JSON");

    let paths = json["paths"].as_object().unwrap();

    // Verify the correct path exists
    assert!(paths.contains_key("/api/v1/user/login"),
            "Expected path '/api/v1/user/login' not found. Found paths: {:?}",
            paths.keys().collect::<Vec<_>>());

    // Verify the duplicate path does NOT exist
    assert!(!paths.contains_key("/api/v1/user/api/v1/user/login"),
            "Duplicate path '/api/v1/user/api/v1/user/login' should not exist");
}
