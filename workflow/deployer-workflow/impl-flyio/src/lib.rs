mod toml;
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
use std::time::Duration;
use toml::serialize_obelisk_toml;

struct Component;
export!(Component with_types_in generated);

const VOLUME_NAME: &str = "db";
const VM_NAME_TEMP: &str = "temp";
const VM_NAME_FINAL: &str = "obelisk";
const VOLUME_MOUNT_PATH: &str = "/volume";
const IMAGE: &str = "getobelisk/obelisk:0.25.1-ubuntu";
const OBELISK_TOML_PATH: &str = formatcp!("{VOLUME_MOUNT_PATH}/obelisk.toml");
const OBELISK_BIN_PATH: &str = "/obelisk/obelisk";
const REGION: Region = Region::Ams;
const WEBHOOK_INTERNAL_PORT: u16 = 9090;
const HEALTHCHECK_INTERNAL_PORT: u16 = 9091;
const HEALTHCHECK_EXTERNAL_PORT: u16 = 444;
const SLEEP_BETWEEN_RETRIES: Duration = Duration::from_secs(10);
const SLEEP_AFTER_TEMP_VM_SHUTDOWN: Duration = Duration::from_secs(5);

fn allocate_ip(app_name: &str) -> Result<(), AppInitModifyError> {
    activity_fly_http::ips::allocate_unsafe(
        app_name,
        IpRequest {
            config: IpVariant::Ipv6(Ipv6Config { region: None }),
        },
    )
    .map(|_ip| ())
    .map_err(AppInitModifyError::IpAllocateError)?;
    // Since this API is not idempotent, make sure just one IP has been allocated.
    let ips =
        activity_fly_http::ips::list(app_name).map_err(AppInitModifyError::IpAllocateError)?;
    for ip_detail in ips.into_iter().skip(1) {
        activity_fly_http::ips::release(app_name, &ip_detail.ip)
            .map_err(AppInitModifyError::IpAllocateError)?;
    }
    Ok(())
}

fn setup_volume(app_name: &str, obelisk_toml: &str) -> Result<(), AppInitModifyError> {
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
                entrypoint: Some(vec!["/usr/bin/sleep".to_string()]),
                cmd: Some(vec!["infinity".to_string()]),
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
        workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(
            SLEEP_BETWEEN_RETRIES.as_secs(),
        )));
    }

    // Write obelisk.toml
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
    workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(
        SLEEP_AFTER_TEMP_VM_SHUTDOWN.as_secs(),
    )));
    // Destroy the VM with force.
    activity_fly_http::machines::delete(app_name, &temp_vm, true)
        .map_err(AppInitModifyError::TempVmError)?;

    Ok(())
}

fn bail_on_app_deletion(app_name: &str) -> Result<(), AppInitModifyError> {
    match activity_fly_http::apps::get(app_name) {
        Ok(None) => Err(AppInitModifyError::AppDeleted),
        _ => Ok(()),
    }
}

// Sleep until all requested secrets are stored in the app or the app is deleted.
fn wait_for_secrets(
    app_name: &str,
    required_secrets: HashSet<String>,
) -> Result<(), AppInitModifyError> {
    while !required_secrets.is_empty() {
        let actual_secrets = match activity_fly_http::secrets::list(app_name) {
            Ok(actual_secrets) => actual_secrets
                .into_iter()
                .map(|secret| secret.name)
                .collect(),
            Err(_) => {
                bail_on_app_deletion(app_name)?;
                HashSet::default()
            }
        };
        if required_secrets.is_subset(&actual_secrets) {
            break;
        }
        workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(
            SLEEP_BETWEEN_RETRIES.as_secs(),
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

/// Sleep until the health check passes, observing the deadline, or the app is deleted.
fn check_health(app_name: &str, health_check_deadline_secs: u16) -> Result<(), AppInitModifyError> {
    let start_secs = workflow_support::sleep(ScheduleAt::Now).seconds;
    let url = format!("https://{app_name}.fly.dev:{HEALTHCHECK_EXTERNAL_PORT}");
    loop {
        if let Ok(http_get::Response {
            status_code,
            body: _,
        }) = http_get::get_resp(&url)
            && (200..300).contains(&status_code)
        {
            return Ok(());
        }
        bail_on_app_deletion(app_name)?;
        let current_secs = workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(
            SLEEP_BETWEEN_RETRIES.as_secs(),
        )))
        .seconds;
        if current_secs - start_secs > health_check_deadline_secs as u64 {
            return Err(AppInitModifyError::HealthCheckFailed);
        }
    }
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
    fn prepare(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
    ) -> Result<(), AppInitModifyError> {
        // Check that we can serialize the configuration first.
        // A panic is translated to `app-init-modify-error::execution-failed`
        let obelisk_toml = serialize_obelisk_toml(&config).unwrap();
        app_create(&org_slug, &app_name)?;
        // Allocate an IPv6 address first.
        allocate_ip(&app_name)?;
        // Put `obelisk.toml`, downloaded WASM files and codegen cache on a new volume.
        setup_volume(&app_name, &obelisk_toml)?;
        Ok(())
    }

    fn wait_for_secrets(app_name: String, config: ObeliskConfig) -> Result<(), AppInitModifyError> {
        let required_secrets = get_secret_keys(config);
        wait_for_secrets(&app_name, required_secrets)?;
        Ok(())
    }

    fn start_final_vm(app_name: String) -> Result<(), AppInitModifyError> {
        launch_final_vm(&app_name)?;
        Ok(())
    }

    fn wait_for_health_check(
        app_name: String,
        health_check_deadline_secs: u16,
    ) -> Result<(), AppInitModifyError> {
        check_health(&app_name, health_check_deadline_secs)?;
        Ok(())
    }

    fn app_init(
        org_slug: String,
        app_name: String,
        config: ObeliskConfig,
        health_check_deadline_secs: u16,
    ) -> Result<(), AppInitError> {
        // Launch sub-workflows by using import.
        // In case of any error including a trap (panic), delete the whole app.
        workflow_import::prepare(&org_slug, &app_name, &config)
            .map_err(|err| cleanup(&app_name, err))?;

        workflow_import::wait_for_secrets(&app_name, &config)
            .map_err(|err| cleanup(&app_name, err))?;

        workflow_import::start_final_vm(&app_name).map_err(|err| cleanup(&app_name, err))?;

        workflow_import::wait_for_health_check(&app_name, health_check_deadline_secs)
            .map_err(|err| cleanup(&app_name, err))?;

        Ok(())
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
