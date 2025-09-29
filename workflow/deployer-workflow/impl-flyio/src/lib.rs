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
            machines::{
                CpuKind, GuestConfig, InitConfig, MachineConfig, MachineRestart, MachineState,
                Mount, RestartPolicy,
            },
            regions::Region,
            volumes::VolumeCreateRequest,
        },
        workflow::{
            types::{AppCleanupFailed, AppInitModifyError},
            workflow::{
                self as workflow_import, AppInitError, AppInitNoCleanupError, ObeliskConfig,
                SecretKey, ServeError,
            },
        },
    },
};

struct Component;
export!(Component with_types_in generated);

const VOLUME_NAME: &str = "db";
const TEMP_VM_NAME: &str = "temp";
const SLEEP: &str = "/usr/bin/sleep";
const INFINITY: &str = "infinity";
const VOLUME_MOUNT_PATH: &str = "/volume";
const IMAGE: &str = "getobelisk/obelisk:0.25.1-ubuntu";
const OBELISK_TOML_PATH: &str = formatcp!("{VOLUME_MOUNT_PATH}/obelisk.toml");
const OBELISK_BIN_PATH: &str = "/obelisk/obelisk";
const REGION: Region = Region::Ams; // TODO: Move to env var

fn app_modify_without_cleanup(
    app_name: &str,
    config: ObeliskConfig,
) -> Result<Vec<SecretKey>, AppInitModifyError> {
    // Create a volume
    let volume = activity_fly_http::volumes::create(
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
        TEMP_VM_NAME,
        &MachineConfig {
            image: IMAGE.to_string(),
            guest: Some(GuestConfig {
                cpu_kind: Some(CpuKind::Shared),
                cpus: Some(1),
                memory_mb: Some(512),
                kernel_args: None,
            }),
            auto_destroy: None, // Some(false) - was creating a stopped machine
            init: Some(InitConfig {
                cmd: Some(vec![INFINITY.to_string()]),
                entrypoint: Some(vec![SLEEP.to_string()]),
                exec: None,
                kernel_args: None,
                swap_size_mb: None,
                tty: None,
            }),
            env: None,
            restart: Some(MachineRestart {
                max_retries: None,
                policy: RestartPolicy::No,
            }),
            stop_config: None,
            mounts: Some(vec![Mount {
                volume: volume.id.clone(),
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
    let obelisk_toml = serialize_obelisk_toml(&config);
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

    // All OK, return secrets that are needed by the configuration.
    Ok(get_secret_keys(&config))
}

fn cleanup(app_name: &str, modify_error: Option<AppInitModifyError>) -> AppInitError {
    // Delete the app with force.
    match activity_fly_http::apps::delete(app_name, true) {
        Ok(()) => AppInitError::CleanupOk,
        Err(cleanup_error) => AppInitError::CleanupFailed(AppCleanupFailed {
            modify_error,
            cleanup_error,
        }),
    }
}

impl Guest for Component {
    fn app_init_no_cleanup_on_error(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
    ) -> Result<Vec<SecretKey>, AppInitNoCleanupError> {
        // If the app already exists, fail with AppNameConflict
        if activity_fly_http::apps::get(&app_name)
            .map_err(AppInitNoCleanupError::AppCreateError)?
            .is_some()
        {
            return Err(AppInitNoCleanupError::AppNameConflict);
        }
        // Create the app
        activity_fly_http::apps::put(&org_slug, &app_name)
            .map_err(AppInitNoCleanupError::AppCreateError)?;
        app_modify_without_cleanup(&app_name, config)
            .map_err(AppInitNoCleanupError::AppInitModifyError)
    }

    fn app_init(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
    ) -> Result<Vec<SecretKey>, AppInitError> {
        // Launch a child workflow by using import
        workflow_import::app_init_no_cleanup_on_error(&org_slug, &app_name, &config).map_err(
            |err| match err {
                AppInitNoCleanupError::AppCreateError(err) => {
                    // No cleanup needed, app creation failed.
                    AppInitError::AppCreateError(err)
                }
                AppInitNoCleanupError::AppNameConflict => {
                    // No cleanup needed, app creation failed on name conflict.
                    AppInitError::AppNameConflict
                }
                AppInitNoCleanupError::AppInitModifyError(err) => cleanup(&app_name, Some(err)),
                AppInitNoCleanupError::ExecutionFailed => cleanup(&app_name, None),
            },
        )
    }

    fn serve(_app_name: String) -> Result<(), ServeError> {
        todo!()
    }
}

fn get_secret_keys(config: &ObeliskConfig) -> Vec<SecretKey> {
    let a_iter = config
        .activity_wasm_list
        .iter()
        .flatten()
        .flat_map(|component| &component.env_vars)
        .flatten()
        .filter(|env_var| !env_var.contains("="));
    let w_iter = config
        .webhook_endpoint_list
        .iter()
        .flatten()
        .flat_map(|component| &component.env_vars)
        .flatten()
        .filter(|env_var| !env_var.contains("="));
    let unique_keys: hashbrown::HashSet<_> = a_iter.chain(w_iter).collect();
    unique_keys
        .into_iter()
        .map(|key| SecretKey {
            name: key.to_string(),
            present: false,
        })
        .collect()
}

// FIXME: Insecure, use proper TOML serializer.
fn serialize_obelisk_toml(config: &ObeliskConfig) -> String {
    const WEBHOOK_SERVER_NAME: &str = "webhook_server";
    let mut toml_string = format!(
        r#"
sqlite.directory = "{VOLUME_MOUNT_PATH}/obelisk-sqlite"
wasm.cache_directory = "{VOLUME_MOUNT_PATH}/wasm"
wasm.codegen_cache.directory = "{VOLUME_MOUNT_PATH}olume/codegen"

# wasm.parallel_compilation = false
wasm.backtrace.persist = false # Speed up execution

api.listening_addr = "[::]:5005"
webui.listening_addr = "[::]:8080"

sqlite.pragma = {{ "cache_size" = "3000" }}

[log.stdout]
enabled = true
level = "WARN,obelisk=info"

[[http_server]]
name = "{WEBHOOK_SERVER_NAME}"
listening_addr = "0.0.0.0:9090"

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
