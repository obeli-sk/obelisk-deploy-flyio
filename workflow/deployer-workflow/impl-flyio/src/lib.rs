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
const WEBHOOK_PORT: u16 = 9090;

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
    sleep_between_retries_seconds: u32,
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
            sleep_between_retries_seconds as u64,
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
            services: Some(vec![ServiceConfig {
                internal_port: WEBHOOK_PORT,
                protocol: ServiceProtocol::Tcp,
                ports: vec![PortConfig {
                    port: 443,
                    handlers: vec![PortHandler::Tls],
                }],
            }]),
        },
        Some(REGION),
    )
    .map(|_| ())
    .map_err(AppInitModifyError::FinalVmError)
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
        sleep_between_retries_seconds: u32,
    ) -> Result<(), AppInitModifyError> {
        app_create(&org_slug, &app_name)?;
        // Allocate an IPv6 address first.
        allocate_ip(&app_name)?;
        // Put `obelisk.toml`, downloaded WASM files and codegen cache on a new volume.
        setup_volume(&app_name, &config)?;
        // Sleep until all requested secrets are stored in the app.
        let required_secrets = get_secret_keys(config);
        wait_for_secrets(&app_name, required_secrets, sleep_between_retries_seconds)?;
        // All preparation is done, start the final VM.
        launch_final_vm(&app_name)?;
        // TODO Add a healthcheck to the exposed server and loop until success is reached, with configurable max retries. Cleanup on failure.
        Ok(())
    }

    fn app_init(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
        sleep_between_retries_seconds: u32,
    ) -> Result<(), AppInitError> {
        // Launch a child workflow by using import.
        // In case of any error including a trap (panic), delete the whole app.
        workflow_import::app_init_no_cleanup(
            &org_slug,
            &app_name,
            &config,
            sleep_between_retries_seconds,
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
name = "{WEBHOOK_SERVER_NAME}"
listening_addr = "0.0.0.0:{WEBHOOK_PORT}"

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
