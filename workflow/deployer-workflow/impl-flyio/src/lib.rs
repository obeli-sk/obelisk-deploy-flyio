mod generated {
    #![allow(clippy::empty_line_after_outer_attr)]
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
use const_format::formatcp;
use generated::{
    export,
    exports::obelisk_flyio::workflow::workflow::Guest,
    obelisk::{
        types::time::{Duration as SchedulingDuration, ScheduleAt},
        workflow::workflow_support,
    },
    obelisk_flyio::{
        activity_fly_http::{
            self,
            ips::{IpRequest, IpVariant, Ipv6Config},
            machines::{
                CpuKind, GuestConfig, InitConfig, MachineConfig, MachineRestart, MachineState,
                Mount, PortConfig, PortHandler, RestartPolicy, ServiceConfig, ServiceProtocol,
            },
            regions::Region,
            volumes::VolumeCreateRequest,
        },
        workflow::{
            types::{AppCleanupFailed, AppInitModifyError},
            workflow::{self as workflow_import, AppInitError, ObeliskConfig},
        },
    },
    testing::http::http_get,
};
use hashbrown::HashSet;

struct Component;
export!(Component with_types_in generated);

const VOLUME_NAME: &str = "db";
const VM_NAME_TEMP: &str = "temp";
const VM_NAME_FINAL: &str = "obelisk";
const SLEEP: &str = "/usr/bin/sleep";
const INFINITY: &str = "infinity";
const VOLUME_MOUNT_PATH: &str = "/volume";
const IMAGE: &str = "getobelisk/obelisk:0.25.1-ubuntu";
const OBELISK_TOML_PATH: &str = formatcp!("{VOLUME_MOUNT_PATH}/obelisk.toml");
const OBELISK_BIN_PATH: &str = "/obelisk/obelisk";
const REGION: Region = Region::Ams;
const WEBHOOK_INTERNAL_PORT: u16 = 9090;
const HEALTHCHECK_INTERNAL_PORT: u16 = 9091;
const HEALTHCHECK_EXTERNAL_PORT: u16 = 444;

fn allocate_ip(app_name: &str) -> Result<(), AppInitModifyError> {
    activity_fly_http::ips::allocate(
        app_name,
        IpRequest {
            config: IpVariant::Ipv6(Ipv6Config { region: None }),
        },
    )
    .map(|_ip| ())
    .map_err(AppInitModifyError::IpAllocateError)
}

fn setup_volume(app_name: &str, config: &ObeliskConfig) -> Result<(), AppInitModifyError> {
    // Create a volume
    activity_fly_http::volumes::create(
        app_name,
        &VolumeCreateRequest {
            name: VOLUME_NAME.to_string(),
            size_gb: 1,
            region: REGION,
            require_unique_zone: None,
        },
    )
    .map_err(AppInitModifyError::VolumeCreateError)?;

    // Launch a temporary VM
    let temp_vm = activity_fly_http::machines::create(
        app_name,
        VM_NAME_TEMP,
        &MachineConfig {
            image: IMAGE.to_string(),
            guest: Some(GuestConfig {
                cpu_kind: Some(CpuKind::Shared),
                cpus: Some(1),
                memory_mb: Some(256),
                kernel_args: None,
            }),
            auto_destroy: None, // Some(false) - was creating a stopped machine
            init: Some(InitConfig {
                cmd: Some(vec![INFINITY.to_string()]),
                entrypoint: Some(vec![SLEEP.to_string()]),
                exec: None,
                kernel_args: None,
                swap_size_mb: Some(256),
                tty: None,
            }),
            env: None,
            restart: Some(MachineRestart {
                max_retries: None,
                policy: RestartPolicy::No,
            }),
            stop_config: None,
            mounts: Some(vec![Mount {
                volume: VOLUME_NAME.to_string(),
                path: VOLUME_MOUNT_PATH.to_string(),
            }]),
            services: None,
        },
        Some(REGION),
    )
    .map_err(AppInitModifyError::TempVmError)?;

    // Wait until its state is "started"
    for _ in 0..10 {
        let machine = activity_fly_http::machines::get(app_name, &temp_vm)
            .map_err(AppInitModifyError::TempVmError)?;
        let state = machine
            .ok_or_else(|| {
                AppInitModifyError::TempVmError(
                    "cannot find temp VM that was created successfuly".to_string(),
                )
            })?
            .state;
        if state == MachineState::Started {
            break;
        }
        workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(1)));
    }

    // Write obelisk.toml
    let obelisk_toml = serialize_obelisk_toml(config);
    let exec_response = activity_fly_http::machines::exec(
        app_name,
        &temp_vm,
        &[
            "sh".to_string(),
            "-c".to_string(),
            format!("cat <<EOF > {OBELISK_TOML_PATH}\n{obelisk_toml}"),
        ],
    )
    .map_err(AppInitModifyError::VolumeWriteError)?;
    if exec_response.exit_code != Some(0) {
        return Err(AppInitModifyError::VolumeWriteError(format!(
            "cannot write obelisk.toml - {exec_response:?}"
        )));
    }
    // Download WASM Components, verify configuration.
    let exec_response = activity_fly_http::machines::exec(
        app_name,
        &temp_vm,
        &[
            OBELISK_BIN_PATH.to_string(),
            "server".to_string(),
            "verify".to_string(),
            "--ignore-missing-env-vars".to_string(),
            "--config".to_string(),
            OBELISK_TOML_PATH.to_string(),
        ],
    )
    .map_err(AppInitModifyError::VerifyError)?;
    if exec_response.exit_code != Some(0) {
        return Err(AppInitModifyError::VolumeWriteError(format!(
            "cannot verify config - {exec_response:?}"
        )));
    }
    // Attempt to shutdown the temp VM.
    // Ignore failure to shut down, temp VM will be deleted with force.
    let _ = activity_fly_http::machines::stop(app_name, &temp_vm);
    // Wait a bit for clean shutdown
    workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(5)));
    // Destroy the VM with force.
    activity_fly_http::machines::delete(app_name, &temp_vm, true)
        .map_err(AppInitModifyError::TempVmError)?;

    Ok(())
}

// Sleep until all requested secrets are stored in the app.
fn wait_for_secrets(
    app_name: &str,
    required_secrets: HashSet<String>,
    wait_for_secrets_sleep_between_retries_seconds: u32,
) -> Result<(), AppInitModifyError> {
    while !required_secrets.is_empty() {
        let actual_secrets = match activity_fly_http::secrets::list(app_name) {
            Ok(actual_secrets) => actual_secrets
                .into_iter()
                .map(|secret| secret.name)
                .collect(),
            Err(_) => {
                // has the app been deleted?
                match activity_fly_http::apps::get(app_name) {
                    Ok(None) => return Err(AppInitModifyError::AppDeleted),
                    Ok(Some(_)) | Err(_) => HashSet::new(), // app exists or unknown, keep looping.
                }
            }
        };
        if required_secrets.is_subset(&actual_secrets) {
            break;
        }
        workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(
            wait_for_secrets_sleep_between_retries_seconds as u64,
        )));
    }
    Ok(())
}

fn launch_final_vm(app_name: &str) -> Result<(), AppInitModifyError> {
    activity_fly_http::machines::create(
        app_name,
        VM_NAME_FINAL,
        &MachineConfig {
            image: IMAGE.to_string(),
            guest: Some(GuestConfig {
                cpu_kind: Some(CpuKind::Shared),
                cpus: Some(1),
                memory_mb: Some(256),
                kernel_args: None,
            }),
            auto_destroy: None,
            init: Some(InitConfig {
                cmd: Some(
                    vec!["server", "run", "--config", "/volume/obelisk.toml"]
                        .into_iter()
                        .map(ToString::to_string)
                        .collect(),
                ),
                entrypoint: None, // defaults to /obelisk/obelisk
                exec: None,
                kernel_args: None,
                swap_size_mb: Some(256),
                tty: None,
            }),
            env: None,
            restart: Some(MachineRestart {
                max_retries: None,
                policy: RestartPolicy::No,
            }),
            stop_config: None,
            mounts: Some(vec![Mount {
                volume: VOLUME_NAME.to_string(),
                path: VOLUME_MOUNT_PATH.to_string(),
            }]),
            services: Some(vec![
                // Expose health check server as https://[::]:HEALTHCHECK_EXTERNAL_PORT
                ServiceConfig {
                    internal_port: HEALTHCHECK_INTERNAL_PORT,
                    protocol: ServiceProtocol::Tcp,
                    ports: vec![PortConfig {
                        port: HEALTHCHECK_EXTERNAL_PORT,
                        handlers: vec![PortHandler::Tls],
                    }],
                },
                // expose webhook server as default https
                ServiceConfig {
                    internal_port: WEBHOOK_INTERNAL_PORT,
                    protocol: ServiceProtocol::Tcp,
                    ports: vec![PortConfig {
                        port: 443,
                        handlers: vec![PortHandler::Tls],
                    }],
                },
            ]),
        },
        Some(REGION),
    )
    .map(|_| ())
    .map_err(AppInitModifyError::FinalVmError)
}

fn check_health(app_name: &str, max_healthcheck_attempts: u32) -> Result<(), AppInitModifyError> {
    for _ in 0..max_healthcheck_attempts {
        match http_get::get_resp(&format!(
            "https://{app_name}.fly.dev:{HEALTHCHECK_EXTERNAL_PORT}"
        )) {
            Ok(http_get::Response {
                status_code,
                body: _,
            }) if (200..300).contains(&status_code) => {
                return Ok(());
            }
            _ => {}
        }
        workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(1)));
    }
    Err(AppInitModifyError::HealthCheckFailed)
}

fn cleanup(app_name: &str, modify_error: AppInitModifyError) -> AppInitError {
    if matches!(
        modify_error,
        AppInitModifyError::AppNameGetError
            | AppInitModifyError::AppNameConflict
            | AppInitModifyError::AppDeleted
    ) {
        return AppInitError::CleanupNotRequired;
    }
    // Delete the app with force.
    match activity_fly_http::apps::delete(app_name, true) {
        Ok(()) => AppInitError::CleanupOk,
        Err(cleanup_error) => AppInitError::CleanupFailed(AppCleanupFailed {
            modify_error,
            cleanup_error,
        }),
    }
}

fn app_create(org_slug: &str, app_name: &str) -> Result<(), AppInitModifyError> {
    // Create the app
    // If the app already exists, fail with AppNameConflict
    if activity_fly_http::apps::get(app_name)
        .map_err(|_| AppInitModifyError::AppNameGetError)?
        .is_some()
    {
        return Err(AppInitModifyError::AppNameConflict);
    }
    // Create the app
    activity_fly_http::apps::put(org_slug, app_name).map_err(AppInitModifyError::AppCreateError)?;
    Ok(())
}

impl Guest for Component {
    fn app_init_no_cleanup(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
        wait_for_secrets_sleep_between_retries_seconds: u32,
        max_healthcheck_attempts: u32,
    ) -> Result<(), AppInitModifyError> {
        app_create(&org_slug, &app_name)?;
        // Allocate an IPv6 address first.
        allocate_ip(&app_name)?;
        // Put `obelisk.toml`, downloaded WASM files and codegen cache on a new volume.
        setup_volume(&app_name, &config)?;
        // Sleep until all requested secrets are stored in the app.
        let required_secrets = get_secret_keys(config);
        wait_for_secrets(
            &app_name,
            required_secrets,
            wait_for_secrets_sleep_between_retries_seconds,
        )?;
        // All preparation is done, start the final VM.
        launch_final_vm(&app_name)?;
        // Make sure it is up.
        check_health(&app_name, max_healthcheck_attempts)?;
        Ok(())
    }

    fn app_init(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
        wait_for_secrets_sleep_between_retries_seconds: u32,
        max_healthcheck_attempts: u32,
    ) -> Result<(), AppInitError> {
        // Launch a child workflow by using import.
        // In case of any error including a trap (panic), delete the whole app.
        workflow_import::app_init_no_cleanup(
            &org_slug,
            &app_name,
            &config,
            wait_for_secrets_sleep_between_retries_seconds,
            max_healthcheck_attempts,
        )
        .map_err(|err| cleanup(&app_name, err))
    }
}

fn get_secret_keys(config: ObeliskConfig) -> HashSet<String> {
    let a_iter = config
        .activity_wasm_list
        .into_iter()
        .flatten()
        .flat_map(|component| component.env_vars)
        .flatten()
        .filter(|env_var| !env_var.contains("="));
    let w_iter = config
        .webhook_endpoint_list
        .into_iter()
        .flatten()
        .flat_map(|component| component.env_vars)
        .flatten()
        .filter(|env_var| !env_var.contains("="));
    a_iter.chain(w_iter).collect()
}

// FIXME: Insecure, use proper TOML serializer.
fn serialize_obelisk_toml(config: &ObeliskConfig) -> String {
    const WEBHOOK_SERVER_NAME: &str = "webhook_server";
    const HEALTHCHECK_SERVER_NAME: &str = "healthcheck_server";

    let mut toml_string = format!(
        r#"
sqlite.directory = "{VOLUME_MOUNT_PATH}/obelisk-sqlite"
wasm.cache_directory = "{VOLUME_MOUNT_PATH}/wasm"
wasm.codegen_cache.directory = "{VOLUME_MOUNT_PATH}olume/codegen"

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

    for activity in config.activity_wasm_list.iter().flatten() {
        toml_string.push_str("\n[[activity_wasm]]\n");
        toml_string.push_str(&format!("name = \"{}\"\n", activity.name));
        toml_string.push_str(&format!("location.oci = \"{}\"\n", activity.location_oci));
        if let Some(env_vars) = &activity.env_vars {
            let quoted_env_vars: Vec<String> =
                env_vars.iter().map(|var| format!("\"{}\"", var)).collect();
            toml_string.push_str(&format!("env_vars = [{}]\n", quoted_env_vars.join(", ")));
        }
        if let Some(lock_expiry) = activity.lock_expiry_seconds {
            toml_string.push_str(&format!("exec.lock_expiry.seconds = {}\n", lock_expiry));
        }
    }

    for workflow in config.workflow_list.iter().flatten() {
        toml_string.push_str("\n[[workflow]]\n");
        toml_string.push_str(&format!("name = \"{}\"\n", workflow.name));
        toml_string.push_str(&format!("location.oci = \"{}\"\n", workflow.location_oci));
    }

    for webhook in config.webhook_endpoint_list.iter().flatten() {
        toml_string.push_str("\n[[webhook_endpoint]]\n");
        toml_string.push_str(&format!("name = \"{}\"\n", webhook.name));
        toml_string.push_str(&format!("location.oci = \"{}\"\n", webhook.location_oci));
        toml_string.push_str("http_server = \"");
        toml_string.push_str(WEBHOOK_SERVER_NAME);
        toml_string.push_str("\"\n");

        let routes_str: Vec<String> = webhook
            .routes
            .iter()
            .map(|route| {
                let methods_str: Vec<String> = route
                    .methods
                    .iter()
                    .map(|method| format!("\"{method}\""))
                    .collect();
                format!(
                    "{{ methods = [{}], route = \"{}\" }}",
                    methods_str.join(", "),
                    route.path
                )
            })
            .collect();
        toml_string.push_str(&format!("routes = [{}]\n", routes_str.join(", ")));

        if let Some(env_vars) = &webhook.env_vars {
            let quoted_env_vars: Vec<String> =
                env_vars.iter().map(|var| format!("\"{}\"", var)).collect();
            toml_string.push_str(&format!("env_vars = [{}]\n", quoted_env_vars.join(", ")));
        }
    }

    toml_string
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use crate::{
        generated::obelisk_flyio::workflow::types::{
            ActivityWasm, ObeliskConfig, Route, WebhookEndpoint, Workflow,
        },
        serialize_obelisk_toml,
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

        let toml = serialize_obelisk_toml(&config);
        assert_snapshot!(toml);
    }
}
