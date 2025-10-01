use crate::generated::obelisk_flyio::workflow::types::ObeliskConfig;
use crate::{HEALTHCHECK_INTERNAL_PORT, VOLUME_MOUNT_PATH, WEBHOOK_INTERNAL_PORT};
use anyhow::anyhow;
use toml::Table; // Explicitly import Table
pub(crate) fn serialize_obelisk_toml(config: &ObeliskConfig) -> Result<String, anyhow::Error> {
    const HEALTHCHECK_SERVER_NAME: &str = "healthcheck_server";
    const WEBHOOK_SERVER_NAME: &str = "webhook_server";

    let initial_toml_template = format!(
        r#"
sqlite.directory = "{VOLUME_MOUNT_PATH}/obelisk-sqlite"
wasm.cache_directory = "{VOLUME_MOUNT_PATH}/wasm"
wasm.codegen_cache.directory = "{VOLUME_MOUNT_PATH}/codegen"

wasm.parallel_compilation = false
wasm.backtrace.persist = false # Speed up execution

api.listening_addr = "[::]:5005"
webui.listening_addr = "[::]:8080"

sqlite.pragma = {{ "cache_size" = "3000" }}

[log.stdout]
enabled = true
level = "WARN,obelisk=info"

[[http_server]]
name = "{HEALTHCHECK_SERVER_NAME}"
listening_addr = "0.0.0.0:{HEALTHCHECK_INTERNAL_PORT}"

[[webhook_endpoint]]
name = "webhook_healthcheck"
location.oci = "docker.io/getobelisk/components_flyio_webhook_healthcheck:2025-10-01@sha256:6fbc11b80b441ae6e642327b1ec0ceba85b2868d85dbce2d99d0d7b14a525c8c"
http_server = "{HEALTHCHECK_SERVER_NAME}"
routes = [""]

[[http_server]]
name = "{WEBHOOK_SERVER_NAME}"
listening_addr = "0.0.0.0:{WEBHOOK_INTERNAL_PORT}"

"#
    );

    let mut root_table = initial_toml_template
        .parse::<Table>()
        .map_err(|e| anyhow!("Failed to parse static TOML: {}", e))?;

    fn get_or_create_array_of_tables<'a>(
        table: &'a mut Table,
        key: &str,
    ) -> &'a mut Vec<toml::Value> {
        table
            .entry(key)
            .or_insert_with(|| toml::Value::Array(Vec::new()))
            .as_array_mut()
            .expect(&format!("Expected '{}' to be an array of tables", key))
    }

    // Add activity_wasm
    if let Some(activities) = &config.activity_wasm_list {
        let activity_array = get_or_create_array_of_tables(&mut root_table, "activity_wasm");
        for activity in activities {
            let mut activity_table = Table::new();
            activity_table.insert(
                "name".to_string(),
                toml::Value::String(activity.name.clone()),
            );

            let mut location_table = Table::new();
            location_table.insert(
                "oci".to_string(),
                toml::Value::String(activity.location_oci.clone()),
            );
            activity_table.insert("location".to_string(), toml::Value::Table(location_table));

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
            activity_array.push(toml::Value::Table(activity_table));
        }
    }

    // Add workflow
    if let Some(workflows) = &config.workflow_list {
        let workflow_array = get_or_create_array_of_tables(&mut root_table, "workflow");
        for workflow in workflows {
            let mut workflow_table = Table::new();
            workflow_table.insert(
                "name".to_string(),
                toml::Value::String(workflow.name.clone()),
            );

            let mut location_table = Table::new();
            location_table.insert(
                "oci".to_string(),
                toml::Value::String(workflow.location_oci.clone()),
            );
            workflow_table.insert("location".to_string(), toml::Value::Table(location_table));

            workflow_array.push(toml::Value::Table(workflow_table));
        }
    }

    // Add webhook_endpoint
    if let Some(webhooks) = &config.webhook_endpoint_list {
        let webhook_array = get_or_create_array_of_tables(&mut root_table, "webhook_endpoint");
        for webhook in webhooks {
            let mut webhook_table = Table::new();
            webhook_table.insert(
                "name".to_string(),
                toml::Value::String(webhook.name.clone()),
            );

            let mut location_table = Table::new();
            location_table.insert(
                "oci".to_string(),
                toml::Value::String(webhook.location_oci.clone()),
            );
            webhook_table.insert("location".to_string(), toml::Value::Table(location_table));

            webhook_table.insert(
                "http_server".to_string(),
                toml::Value::String(WEBHOOK_SERVER_NAME.to_string()),
            );

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
                    route_table
                        .insert("route".to_string(), toml::Value::String(route.path.clone()));
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

    Ok(toml::to_string_pretty(&toml::Value::Table(root_table))?)
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
            activity_wasm_list: Some(vec![
                ActivityWasm {
                    name: "stargazers_activity_llm_chatgpt".to_string(),
                    location_oci: "docker.io/getobelisk/demo_stargazers_activity_llm_openai:2025-09-28@sha256:4b10a66c80bec625a6b0a2e8a4b5192f8a2356eca19c0a6705335771a8b8b1e8".to_string(),
                    env_vars: Some(vec!["OPENAI_API_KEY".to_string()]),
                    lock_expiry_seconds: Some(10),
                },
                ActivityWasm {
                    name: "stargazers_activity_github_impl".to_string(),
                    location_oci: "docker.io/getobelisk/demo_stargazers_activity_github_impl:2025-09-28@sha256:8f6fc9b1379b359e085998fa2fd7c966c450327d09770807dfba4b2f75731d72".to_string(),
                    env_vars: Some(vec!["GITHUB_TOKEN".to_string()]),
                    lock_expiry_seconds: Some(5),
                },
                ActivityWasm {
                    name: "stargazers_activity_db_turso".to_string(),
                    location_oci: "docker.io/getobelisk/demo_stargazers_activity_db_turso:2025-09-28@sha256:26b08b3d0c6e430944d8187a00bd9817a83ab89e11ba72d15e7533a758addf33".to_string(),
                    env_vars: Some(vec!["TURSO_TOKEN".to_string(), "TURSO_LOCATION".to_string()]),
                    lock_expiry_seconds: Some(5),
                },
            ]),
            workflow_list: Some(vec![
                Workflow {
                    name: "stargazers_workflow".to_string(),
                    location_oci: "docker.io/getobelisk/demo_stargazers_workflow:2025-09-28@sha256:678d85e3e2f89d22794fd1ffc0217bf23510e1349ee150a54d5c82cc2ef75834".to_string(),
                },
            ]),
            webhook_endpoint_list: Some(vec![
                WebhookEndpoint {
                    name: "stargazers_webhook".to_string(),
                    location_oci: "docker.io/getobelisk/demo_stargazers_webhook:2025-09-28@sha256:aa4dfa18d1ad7c1623163eeabb41a415ebad5296fca8f3b957987afcdb2a0f40".to_string(),
                    routes: vec![
                        Route {
                            methods: vec!["POST".to_string(), "GET".to_string()],
                            path: "".to_string(),
                        },
                    ],
                    env_vars: Some(vec!["GITHUB_WEBHOOK_SECRET".to_string()]),
                },
            ]),
        };

        let toml = serialize_obelisk_toml(&config).unwrap();
        assert_snapshot!(toml);
    }
}
