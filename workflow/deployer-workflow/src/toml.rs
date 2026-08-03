use crate::generated::obelisk_flyio::workflow::types::ObeliskConfig;
use crate::{HEALTHCHECK_INTERNAL_PORT, SQLITE_DIRECTORY_PATH, VOLUME_MOUNT_PATH};
use anyhow::{Context, anyhow};
use toml::Table;

pub(crate) fn serialize_obelisk_toml(
    config: &ObeliskConfig,
) -> Result<(String, String), anyhow::Error> {
    const HEALTHCHECK_SERVER_NAME: &str = "healthcheck_server";
    const OBELISK_HEALTHCHECK_OCI_TOML: &str =
        include_str!("../../../obelisk-healthcheck-oci.toml");

    let webhook_healthcheck_location = {
        let val: toml::Value = toml::from_str(OBELISK_HEALTHCHECK_OCI_TOML).expect("Invalid TOML");

        let endpoint = val["webhook_endpoint_wasm"]
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|item| item["name"].as_str() == Some("webhook_healthcheck"))
            })
            .expect("endpoint not found");

        endpoint["location"]
            .as_str()
            .expect("missing location")
            .to_string()
    };

    let server_toml_template = format!(
        r#"
wasm.cache_directory = "{VOLUME_MOUNT_PATH}/wasm"
wasm.codegen_cache.directory = "{VOLUME_MOUNT_PATH}/codegen"

wasm.parallel_compilation = false

[database.sqlite]
directory = "{SQLITE_DIRECTORY_PATH}"
pragma = {{ "cache_size" = "3000" }}

[log.file]
enabled = true
target = true
directory = "/var/log"
prefix = "obelisk.log"

[[outbound_http.allowed_host]]
pattern = "*://*:*"
methods = "*"

[[http_server]]
name = "{HEALTHCHECK_SERVER_NAME}"
listening_addr = "[::]:{HEALTHCHECK_INTERNAL_PORT}"

"#
    );

    let deployment_toml_template = format!(
        r#"
[[webhook_endpoint_wasm]]
name = "webhook_healthcheck"
location = "{webhook_healthcheck_location}"
http_server = "{HEALTHCHECK_SERVER_NAME}"
routes = [""]

"#
    );

    let server_table = server_toml_template
        .parse::<Table>()
        .map_err(|e| anyhow!("Failed to parse server TOML: {}", e))?;

    let mut deployment_table = deployment_toml_template
        .parse::<Table>()
        .map_err(|e| anyhow!("Failed to parse deployment TOML: {}", e))?;

    fn get_or_create_array_of_tables<'a>(
        table: &'a mut Table,
        key: &str,
    ) -> Result<&'a mut Vec<toml::Value>, anyhow::Error> {
        table
            .entry(key)
            .or_insert_with(|| toml::Value::Array(Vec::new()))
            .as_array_mut()
            .with_context(|| format!("Expected '{key}' to be an array of tables"))
    }

    // Add activity_wasm
    if let Some(activities) = &config.activity_wasm_list {
        let activity_array = get_or_create_array_of_tables(&mut deployment_table, "activity_wasm")?;
        for activity in activities {
            let mut activity_table = Table::new();
            activity_table.insert(
                "name".to_string(),
                toml::Value::String(activity.name.clone()),
            );

            activity_table.insert(
                "location".to_string(),
                toml::Value::String(activity.location_oci.clone()),
            );

            if let Some(env_vars) = &activity.env_vars {
                activity_table.insert(
                    "env_vars".to_string(),
                    toml::Value::Array(
                        env_vars
                            .iter()
                            .map(|v| toml::Value::String(v.clone()))
                            .collect(),
                    ),
                );
            }
            if let Some(lock_expiry) = activity.lock_expiry_seconds {
                let mut exec_table = Table::new();
                let mut lock_expiry_table = Table::new();
                lock_expiry_table.insert(
                    "seconds".to_string(),
                    toml::Value::Integer(lock_expiry as i64),
                );
                exec_table.insert(
                    "lock_expiry".to_string(),
                    toml::Value::Table(lock_expiry_table),
                );
                activity_table.insert("exec".to_string(), toml::Value::Table(exec_table));
            }
            if let Some(max_retries) = activity.max_retries {
                activity_table.insert(
                    "max_retries".to_string(),
                    toml::Value::Integer(max_retries.into()),
                );
            }

            // Add allowed_host
            let mut allowed_host_table = Table::new();
            allowed_host_table.insert(
                "pattern".to_string(),
                toml::Value::String("*://*:*".to_string()),
            );
            allowed_host_table.insert("methods".to_string(), toml::Value::String("*".to_string()));

            activity_table.insert(
                "allowed_host".to_string(),
                toml::Value::Array(vec![toml::Value::Table(allowed_host_table)]),
            );

            activity_array.push(toml::Value::Table(activity_table));
        }
    }

    // Add workflow_wasm
    if let Some(workflows) = &config.workflow_list {
        let workflow_array = get_or_create_array_of_tables(&mut deployment_table, "workflow_wasm")?;
        for workflow in workflows {
            let mut workflow_table = Table::new();
            workflow_table.insert(
                "name".to_string(),
                toml::Value::String(workflow.name.clone()),
            );
            workflow_table.insert(
                "location".to_string(),
                toml::Value::String(workflow.location_oci.clone()),
            );

            workflow_array.push(toml::Value::Table(workflow_table));
        }
    }

    // Add webhook_endpoint_wasm
    if let Some(webhooks) = &config.webhook_endpoint_list {
        let webhook_array =
            get_or_create_array_of_tables(&mut deployment_table, "webhook_endpoint_wasm")?;
        for webhook in webhooks {
            let mut webhook_table = Table::new();
            webhook_table.insert(
                "name".to_string(),
                toml::Value::String(webhook.name.clone()),
            );

            webhook_table.insert(
                "location".to_string(),
                toml::Value::String(webhook.location_oci.clone()),
            );

            // No http_server field — uses the default external server at 0.0.0.0:9090

            let routes_array: Vec<toml::Value> = webhook
                .routes
                .iter()
                .map(|route| {
                    let mut route_table = Table::new();
                    route_table.insert(
                        "methods".to_string(),
                        toml::Value::Array(
                            route
                                .methods
                                .iter()
                                .map(|m| toml::Value::String(m.clone()))
                                .collect(),
                        ),
                    );
                    route_table.insert(
                        "route".to_string(),
                        toml::Value::String(route.route.clone()),
                    );
                    toml::Value::Table(route_table)
                })
                .collect();
            webhook_table.insert("routes".to_string(), toml::Value::Array(routes_array));

            if let Some(env_vars) = &webhook.env_vars {
                webhook_table.insert(
                    "env_vars".to_string(),
                    toml::Value::Array(
                        env_vars
                            .iter()
                            .map(|v| toml::Value::String(v.clone()))
                            .collect(),
                    ),
                );
            }
            webhook_array.push(toml::Value::Table(webhook_table));
        }
    }

    Ok((
        toml::to_string_pretty(&toml::Value::Table(deployment_table))?,
        toml::to_string_pretty(&toml::Value::Table(server_table))?,
    ))
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use crate::{
        generated::obelisk_flyio::workflow::types::{
            ActivityWasm, ObeliskConfig, Route, WebhookEndpoint, Workflow,
        },
        toml::serialize_obelisk_toml,
    };

    #[test]
    fn serialize_obelisk_toml_should_produce_correct_config() {
        let config = ObeliskConfig {
            obelisk_version: "1.2.3".to_string(),
            activity_wasm_list: Some(vec![
                ActivityWasm {
                    name: "stargazers_activity_llm_chatgpt".to_string(),
                    location_oci: "oci://docker.io/getobelisk/demo_stargazers_activity_llm_openai:2025-09-28@sha256:4b10a66c80bec625a6b0a2e8a4b5192f8a2356eca19c0a6705335771a8b8b1e8".to_string(),
                    env_vars: Some(vec!["OPENAI_API_KEY".to_string()]),
                    lock_expiry_seconds: Some(10),
                    max_retries: None,
                },
                ActivityWasm {
                    name: "stargazers_activity_github_impl".to_string(),
                    location_oci: "oci://docker.io/getobelisk/demo_stargazers_activity_github_impl:2025-09-28@sha256:8f6fc9b1379b359e085998fa2fd7c966c450327d09770807dfba4b2f75731d72".to_string(),
                    env_vars: Some(vec!["GITHUB_TOKEN".to_string()]),
                    lock_expiry_seconds: Some(5),
                    max_retries: None,
                },
                ActivityWasm {
                    name: "stargazers_activity_db_turso".to_string(),
                    location_oci: "oci://docker.io/getobelisk/demo_stargazers_activity_db_turso:2025-09-28@sha256:26b08b3d0c6e430944d8187a00bd9817a83ab89e11ba72d15e7533a758addf33".to_string(),
                    env_vars: Some(vec!["TURSO_TOKEN".to_string(), "TURSO_LOCATION".to_string()]),
                    lock_expiry_seconds: Some(5),
                    max_retries: Some(9),
                },
            ]),
            workflow_list: Some(vec![
                Workflow {
                    name: "stargazers_workflow".to_string(),
                    location_oci: "oci://docker.io/getobelisk/demo_stargazers_workflow:2025-09-28@sha256:678d85e3e2f89d22794fd1ffc0217bf23510e1349ee150a54d5c82cc2ef75834".to_string(),
                },
            ]),
            webhook_endpoint_list: Some(vec![
                WebhookEndpoint {
                    name: "stargazers_webhook".to_string(),
                    location_oci: "oci://docker.io/getobelisk/demo_stargazers_webhook:2025-09-28@sha256:aa4dfa18d1ad7c1623163eeabb41a415ebad5296fca8f3b957987afcdb2a0f40".to_string(),
                    routes: vec![
                        Route {
                            methods: vec!["POST".to_string(), "GET".to_string()],
                            route: "".to_string(),
                        },
                    ],
                    env_vars: Some(vec!["GITHUB_WEBHOOK_SECRET".to_string()]),
                },
            ]),
        };

        let (deployment_toml, server_toml) = serialize_obelisk_toml(&config).unwrap();
        assert_snapshot!("deployment_toml", deployment_toml);
        assert_snapshot!("server_toml", server_toml);
    }
}
