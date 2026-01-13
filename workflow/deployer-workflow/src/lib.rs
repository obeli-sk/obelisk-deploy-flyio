mod testing;
mod toml;
mod generated {
    #![allow(clippy::empty_line_after_outer_attr)]
    include!(concat!(env!("OUT_DIR"), "/any.rs"));
}
use assert_matches::assert_matches;
use base64::{Engine as _, engine::general_purpose};
use const_format::formatcp;
use generated::{
    export,
    exports::obelisk_flyio::workflow::workflow::Guest,
    obelisk::{
        types::time::{Duration as SchedulingDuration, ScheduleAt},
        workflow::workflow_support,
    },
    obelisk_components::generic_http::http,
    obelisk_flyio::{
        activity_fly_http::{
            self,
            ips::{IpVariant, Ipv6Config},
            machines::{
                CpuKind, GuestConfig, InitConfig, MachineConfig, MachineRestart, MachineState,
                Mount, PortConfig, PortHandler, RestartPolicy, ServiceConfig, ServiceProtocol,
            },
            regions::Region,
            volumes::VolumeCreateRequest,
        },
        workflow::{
            types::{AppCleanupFailed, AppInitConfig, AppInitModifyError},
            workflow::{self as workflow_import, AppInitError, ObeliskConfig},
        },
    },
};
use hashbrown::{HashMap, HashSet};
use std::time::Duration;
use toml::serialize_obelisk_toml;

use crate::generated::{
    exports::obelisk_flyio::workflow::workflow::MachineId,
    obelisk::{
        log::log::warn,
        types::join_set::{JoinSet, ResponseId},
    },
    obelisk_flyio::activity_fly_http::machines::{ExecConfig, FileConfig},
};

struct Component;
export!(Component with_types_in generated);

const VOLUME_NAME: &str = "db";
const VM_NAME_TEMP: &str = "temp";
const TEMP_VM_MEMORY_MB: u64 = 256;
const TEMP_VM_SWAP_MB: u64 = 256;

const MAX_VM_FAILURE_RETRIES: u32 = 5;

const MINIO_VM_NAME: &str = "minio";
const MINIO_IMAGE: &str = "minio/minio:RELEASE.2025-09-07T16-13-09Z-cpuv1";
const MINIO_BUCKET_NAME: &str = "litestream-bucket";
// TODO: Move to obelisk.toml
const MINIO_ACCESS_KEY_ID: &str = "minioadmin";
const MINIO_SECRET_ACCESS_KEY: &str = "minioadmin";
const MINIO_PORT: u16 = 9000;
const MINIO_VM_MEMORY_MB: u64 = 256;

const VM_NAME_FINAL: &str = "obelisk";
const FINAL_VM_MEMORY_MB: u64 = 256;
const FINAL_VM_SWAP_MB: u64 = 256;
const VOLUME_MOUNT_PATH: &str = "/volume";
const OBELISK_TOML_PATH: &str = "/etc/obelisk/obelisk.toml";
const OBELISK_BIN_PATH: &str = "/obelisk/obelisk";
const LITESTREAM_CONFIG_PATH: &str = "/etc/litestream.yml";
const SQLITE_DIRECTORY_PATH: &str = formatcp!("{VOLUME_MOUNT_PATH}/obelisk-sqlite");
const SQLITE_FILE_PATH: &str = formatcp!("{SQLITE_DIRECTORY_PATH}/obelisk.sqlite");
const REGION: Region = Region::Ams;
const WEBHOOK_INTERNAL_PORT: u16 = 9090;
const API_INTERNAL_PORT: u16 = 5005;
const HEALTHCHECK_INTERNAL_PORT: u16 = 9091;
const HEALTHCHECK_EXTERNAL_PORT: u16 = 444;
const SLEEP_BETWEEN_RETRIES: Duration = Duration::from_secs(10);
const SLEEP_AFTER_TEMP_VM_SHUTDOWN: Duration = Duration::from_secs(5);
const LITESTREAM_ENTRYPOINT_PATH: &str = "/usr/local/bin/litestream-entrypoint.sh";

fn obelisk_image(obelisk_version: &str) -> String {
    format!("getobelisk/obelisk:{obelisk_version}-ubuntu-litestream")
}

fn allocate_ip(app_name: &str) -> Result<(), AppInitModifyError> {
    activity_fly_http::ips::allocate(
        app_name,
        IpVariant::Ipv6(Ipv6Config { region: None }),
        &[], // Newly created App, thus no pre-existing IPs
    )
    .map(|_ip| ())
    .map_err(AppInitModifyError::IpAllocateError)?;
    Ok(())
}

fn wait_until_started(
    app_name: &str,
    machine_id: &str,
    vm_error: fn(String) -> AppInitModifyError,
    vm_startup_deadline_secs: u16,
) -> Result<(), AppInitModifyError> {
    let start_secs = workflow_support::sleep(ScheduleAt::Now)
        .map_err(|()| AppInitModifyError::Cancelled)?
        .seconds;
    let mut join_sets = HashMap::new();
    loop {
        let machine = activity_fly_http::machines::get(app_name, machine_id).map_err(vm_error)?;
        let state = machine
            .ok_or_else(|| {
                vm_error("cannot find VM that was just created successfuly".to_string())
            })?
            .state;
        if state == MachineState::Started {
            return Ok(());
        }

        wait_or_fail(
            start_secs,
            vm_startup_deadline_secs,
            &format!("{machine_id}/{state:?}"),
            || vm_error("timed out waiting for 'started' state".to_string()),
            &mut join_sets,
        )?;
    }
}

// Put `obelisk.toml`, downloaded WASM files and codegen cache on a new volume.
// If minio is enabled, configure litestream.yml
fn setup_volume(
    app_name: &str,
    obelisk_version: &str,
    obelisk_toml: &str,
    temp_vm_startup_deadline_secs: u16,
) -> Result<(), AppInitModifyError> {
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
    let temp_vm_id = activity_fly_http::machines::create(
        app_name,
        VM_NAME_TEMP,
        &MachineConfig {
            image: obelisk_image(obelisk_version),
            guest: Some(GuestConfig {
                cpu_kind: Some(CpuKind::Shared),
                cpus: Some(1),
                memory_mb: Some(TEMP_VM_MEMORY_MB),
                kernel_args: None,
            }),
            auto_destroy: None, // Some(false) - was creating a stopped machine
            init: Some(InitConfig {
                entrypoint: Some(vec!["/usr/bin/sleep".to_string()]),
                cmd: Some(vec!["infinity".to_string()]),
                exec: None,
                kernel_args: None,
                swap_size_mb: Some(TEMP_VM_SWAP_MB),
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
            files: Some(vec![FileConfig {
                guest_path: OBELISK_TOML_PATH.to_string(),
                raw_value: Some(general_purpose::STANDARD.encode(obelisk_toml)),
                image_config: None,
                mode: None,
                secret_name: None,
            }]),
        },
        Some(REGION),
    )
    .map_err(AppInitModifyError::TempVmError)?;

    wait_until_started(
        app_name,
        &temp_vm_id,
        AppInitModifyError::TempVmError,
        temp_vm_startup_deadline_secs,
    )?;
    // Download WASM Components, verify configuration.
    activity_fly_http::machines::exec_check_success(
        app_name,
        &temp_vm_id,
        &[
            OBELISK_BIN_PATH.to_string(),
            "server".to_string(),
            "verify".to_string(),
            "--ignore-missing-env-vars".to_string(),
            "--config".to_string(),
            OBELISK_TOML_PATH.to_string(),
        ],
        &ExecConfig {
            timeout_secs: Some(30),
            stdin: None,
        },
    )
    .map_err(AppInitModifyError::VerifyError)?;

    // Attempt to shutdown the temp VM.
    // Ignore failure to shut down, temp VM will be deleted with force.
    let _ = activity_fly_http::machines::stop(app_name, &temp_vm_id);
    // Wait a bit for clean shutdown
    workflow_support::sleep(ScheduleAt::In(SchedulingDuration::Seconds(
        SLEEP_AFTER_TEMP_VM_SHUTDOWN.as_secs(),
    )))
    .map_err(|()| AppInitModifyError::Cancelled)?;
    // Destroy the VM with force.
    activity_fly_http::machines::delete(app_name, &temp_vm_id, true)
        .map_err(AppInitModifyError::TempVmError)?;

    Ok(())
}

fn litestream_entrypoint_contents() -> String {
    format!(
        r#"
#!/usr/bin/env bash

set -euo pipefail

litestream restore -if-replica-exists --config {LITESTREAM_CONFIG_PATH} {SQLITE_FILE_PATH}
exec litestream replicate --config {LITESTREAM_CONFIG_PATH} --exec 'obelisk server run --config {OBELISK_TOML_PATH}'
        "#
    )
}

fn litestream_config_contents(app_name: &str, minio_machine_id: &str) -> String {
    format!(
        r#"
dbs:
  - path: "{SQLITE_FILE_PATH}"
    replica:
      url: "s3://{MINIO_BUCKET_NAME}.{minio_machine_id}.vm.{app_name}.internal:{MINIO_PORT}/litestream/obelisk"
      access-key-id: "{MINIO_ACCESS_KEY_ID}"
      secret-access-key: "{MINIO_SECRET_ACCESS_KEY}"
"#
    )
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
    secrets_deadline_secs: u16,
) -> Result<(), AppInitModifyError> {
    if required_secrets.is_empty() {
        return Ok(());
    }
    let start_secs = workflow_support::sleep(ScheduleAt::Now)
        .map_err(|()| AppInitModifyError::Cancelled)?
        .seconds;
    let mut join_sets = HashMap::new();
    loop {
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
        let mut missing_secrets = required_secrets.difference(&actual_secrets);
        let Some(first_missing) = missing_secrets.next() else {
            return Ok(());
        };
        wait_or_fail(
            start_secs,
            secrets_deadline_secs,
            first_missing,
            || AppInitModifyError::WaitingForSecretsTimedOut,
            &mut join_sets,
        )?;
    }
}

// Testing instance of MinIO
fn minio_start(app_name: &str) -> Result<String, AppInitModifyError> {
    let machine_id = activity_fly_http::machines::create(
        app_name,
        MINIO_VM_NAME,
        &MachineConfig {
            image: MINIO_IMAGE.to_string(),
            guest: Some(GuestConfig {
                cpu_kind: Some(CpuKind::Shared),
                cpus: Some(1),
                memory_mb: Some(MINIO_VM_MEMORY_MB),
                kernel_args: None,
            }),
            auto_destroy: None,
            init: Some(InitConfig {
                cmd: Some(
                    "server /data --console-address :9001"
                        .split(' ')
                        .map(ToString::to_string)
                        .collect(),
                ),
                entrypoint: None,
                exec: None,
                kernel_args: None,
                swap_size_mb: None,
                tty: None,
            }),
            env: None,
            restart: Some(MachineRestart {
                max_retries: Some(MAX_VM_FAILURE_RETRIES),
                policy: RestartPolicy::OnFailure,
            }),
            stop_config: None,
            mounts: None,
            services: None,
            files: None,
        },
        Some(REGION),
    )
    .map_err(AppInitModifyError::MinioVmError)?;
    Ok(machine_id)
}

fn minio_configure(app_name: &str, machine_id: &str) -> Result<(), AppInitModifyError> {
    let exec = |command: &str| {
        activity_fly_http::machines::exec_check_success(
            app_name,
            machine_id,
            &command
                .split(' ')
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            &ExecConfig {
                timeout_secs: None,
                stdin: None,
            },
        )
        .map_err(AppInitModifyError::MinioVmError)
    };

    exec("mc alias set myminio http://127.0.0.1:9000 minioadmin minioadmin")?;
    exec(&format!("mc mb myminio/{MINIO_BUCKET_NAME}"))?;
    exec("mc ls myminio --json")?;
    Ok(())
}

fn start_final_vm(
    app_name: &str,
    obelisk_config: ObeliskConfig,
    litestream_minio_machine_id: Option<MachineId>,
    vm_startup_deadline_secs: u16,
    expose_api_server: Option<u16>,
) -> Result<MachineId, AppInitModifyError> {
    let obelisk_toml = serialize_obelisk_toml(&obelisk_config).unwrap();
    let entrypoint = if litestream_minio_machine_id.is_some() {
        Some(vec![
            "/usr/bin/env".to_string(),
            "bash".to_string(),
            LITESTREAM_ENTRYPOINT_PATH.to_string(),
        ])
    } else {
        None
    };
    let cmd = if litestream_minio_machine_id.is_some() {
        None // Same as `Some(vec![])`, $@ will be empty.
    } else {
        Some(
            vec!["server", "run", "--config", OBELISK_TOML_PATH]
                .into_iter()
                .map(ToString::to_string)
                .collect(),
        )
    };
    let mut files = vec![FileConfig {
        guest_path: OBELISK_TOML_PATH.to_string(),
        raw_value: Some(general_purpose::STANDARD.encode(obelisk_toml)),
        image_config: None,
        mode: None,
        secret_name: None,
    }];
    if let Some(minio_machine_id) = litestream_minio_machine_id {
        files.push(FileConfig {
            guest_path: LITESTREAM_ENTRYPOINT_PATH.to_string(),
            raw_value: Some(general_purpose::STANDARD.encode(litestream_entrypoint_contents())),
            mode: None,
            image_config: None,
            secret_name: None,
        });
        files.push(FileConfig {
            guest_path: LITESTREAM_CONFIG_PATH.to_string(),
            raw_value: Some(
                general_purpose::STANDARD
                    .encode(litestream_config_contents(app_name, &minio_machine_id)),
            ),
            mode: None,
            image_config: None,
            secret_name: None,
        });
    }
    let mut services = vec![
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
    ];
    if let Some(exposed_api_port) = expose_api_server {
        services.push(ServiceConfig {
            internal_port: API_INTERNAL_PORT,
            protocol: ServiceProtocol::Tcp,
            ports: vec![PortConfig {
                port: exposed_api_port,
                handlers: vec![PortHandler::Tls],
            }],
        });
    }

    let machine_id = activity_fly_http::machines::create(
        app_name,
        VM_NAME_FINAL,
        &MachineConfig {
            image: obelisk_image(&obelisk_config.obelisk_version),
            guest: Some(GuestConfig {
                cpu_kind: Some(CpuKind::Shared),
                cpus: Some(1),
                memory_mb: Some(FINAL_VM_MEMORY_MB),
                kernel_args: None,
            }),
            auto_destroy: None,
            init: Some(InitConfig {
                cmd,
                entrypoint,
                exec: None,
                kernel_args: None,
                swap_size_mb: Some(FINAL_VM_SWAP_MB),
                tty: None,
            }),
            env: None,
            restart: Some(MachineRestart {
                max_retries: Some(MAX_VM_FAILURE_RETRIES),
                policy: RestartPolicy::OnFailure,
            }),
            stop_config: None,
            mounts: Some(vec![Mount {
                volume: VOLUME_NAME.to_string(),
                path: VOLUME_MOUNT_PATH.to_string(),
            }]),
            services: Some(services),
            files: Some(files),
        },
        Some(REGION),
    )
    .map_err(AppInitModifyError::FinalVmError)?;
    wait_until_started(
        app_name,
        &machine_id,
        AppInitModifyError::FinalVmError,
        vm_startup_deadline_secs,
    )?;
    Ok(machine_id)
}

fn wait_or_fail(
    start_secs: u64,
    deadline_secs: u16,
    name: &str,
    err: impl Fn() -> AppInitModifyError,
    join_sets: &mut HashMap<String, JoinSet>,
) -> Result<(), AppInitModifyError> {
    // Obtain current time
    let current_secs = workflow_support::sleep(ScheduleAt::Now)
        .map_err(|()| AppInitModifyError::Cancelled)?
        .seconds;

    if current_secs + SLEEP_BETWEEN_RETRIES.as_secs() - start_secs > deadline_secs as u64 {
        // bail out even if the timeout would be reached after the sleep.
        return Err(err());
    }
    let name = sanitize_join_set_name(name);
    let join_set = join_sets.entry(name).or_insert_with_key(|name| {
        workflow_support::join_set_create_named(name).unwrap_or_else(|err| {
            warn(&format!("Cannot create `{name}` - {err}"));
            workflow_support::join_set_create()
        })
    });
    let delay_id = join_set.submit_delay(ScheduleAt::In(SchedulingDuration::Seconds(
        SLEEP_BETWEEN_RETRIES.as_secs(),
    )));
    let (response_id, res) = join_set.join_next().expect("cannot return all-processed");
    assert_matches!(response_id, ResponseId::DelayId(awaited) if awaited.id == delay_id.id);
    res.map_err(|()| AppInitModifyError::Cancelled)?;
    Ok(())
}

pub fn sanitize_join_set_name(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '/' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn url(app_name: &str, port: u16, suffix: &str) -> String {
    format!("https://{app_name}.fly.dev:{port}{suffix}")
}

/// Sleep until the health check passes, observing the deadline, or the app is deleted.
fn check_health(app_name: &str, health_check_deadline_secs: u16) -> Result<(), AppInitModifyError> {
    let start_secs = workflow_support::sleep(ScheduleAt::Now)
        .map_err(|()| AppInitModifyError::Cancelled)?
        .seconds;
    let url = url(app_name, HEALTHCHECK_EXTERNAL_PORT, "");

    let mut join_sets = HashMap::new();
    loop {
        let resp = http::request(http::Method::Get, &url, &[], None).map(|resp| resp.status_code);
        let reason = match resp {
            Ok(200..300) => return Ok(()),
            Ok(other) => format!("wrong status code: {other}"),
            Err(err) => format!("cannot connect: {err}"),
        };
        bail_on_app_deletion(app_name)?;
        wait_or_fail(
            start_secs,
            health_check_deadline_secs,
            &reason,
            || AppInitModifyError::HealthCheckFailed,
            &mut join_sets,
        )?;
    }
}

fn cleanup(
    app_name: &str,
    modify_error: AppInitModifyError,
    skip_cleanup_on_error: bool,
) -> AppInitError {
    if skip_cleanup_on_error
        || matches!(
            modify_error,
            AppInitModifyError::AppNameGetError
                | AppInitModifyError::AppNameConflict
                | AppInitModifyError::AppDeleted
        )
    {
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
        minio: bool,
        minio_vm_startup_deadline_secs: u16,
    ) -> Result<Option<MachineId>, AppInitModifyError> {
        app_create(&org_slug, &app_name)?;
        // Allocate an IPv6 address first.
        allocate_ip(&app_name)?;
        let minio_machine_id = if minio {
            let minio_machine_id = minio_start(&app_name)?;
            // TODO: MinIO configuration can be executed in parallel with `setup_volume`
            wait_until_started(
                &app_name,
                &minio_machine_id,
                AppInitModifyError::MinioVmError,
                minio_vm_startup_deadline_secs,
            )?;
            minio_configure(&app_name, &minio_machine_id)?;
            Some(minio_machine_id)
        } else {
            None
        };
        Ok(minio_machine_id)
    }

    fn set_up_volume(
        app_name: String,
        config: ObeliskConfig,
        temp_vm_startup_deadline_secs: u16,
    ) -> Result<(), AppInitModifyError> {
        let obelisk_toml = serialize_obelisk_toml(&config).unwrap();
        setup_volume(
            &app_name,
            &config.obelisk_version,
            &obelisk_toml,
            temp_vm_startup_deadline_secs,
        )
    }

    fn wait_for_secrets(
        app_name: String,
        config: ObeliskConfig,
        secrets_deadline_secs: u16,
    ) -> Result<(), AppInitModifyError> {
        let required_secrets = get_secret_keys(config);
        wait_for_secrets(&app_name, required_secrets, secrets_deadline_secs)?;
        Ok(())
    }

    fn start_final_vm(
        app_name: String,
        obelisk_config: ObeliskConfig,
        litestream_minio_machine_id: Option<MachineId>,
        vm_startup_deadline_secs: u16,
        expose_api_server: Option<u16>,
    ) -> Result<MachineId, AppInitModifyError> {
        start_final_vm(
            &app_name,
            obelisk_config,
            litestream_minio_machine_id,
            vm_startup_deadline_secs,
            expose_api_server,
        )
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
        obelisk_config: ObeliskConfig,
        init_config: AppInitConfig,
    ) -> Result<MachineId, AppInitError> {
        // Launch sub-workflows by using import.
        // In case of any error including a trap (panic), delete the whole app.

        let minio_id = workflow_import::prepare(
            &org_slug,
            &app_name,
            init_config.minio,
            init_config.vm_startup_deadline_secs,
        )
        .map_err(|err| cleanup(&app_name, err, init_config.skip_cleanup_on_error))?;

        workflow_import::set_up_volume(
            &app_name,
            &obelisk_config,
            init_config.vm_startup_deadline_secs,
        )
        .map_err(|err| cleanup(&app_name, err, init_config.skip_cleanup_on_error))?;

        workflow_import::wait_for_secrets(
            &app_name,
            &obelisk_config,
            init_config.secrets_deadline_secs,
        )
        .map_err(|err| cleanup(&app_name, err, init_config.skip_cleanup_on_error))?;

        let machine_id = workflow_import::start_final_vm(
            &app_name,
            &obelisk_config,
            minio_id.as_deref(),
            init_config.vm_startup_deadline_secs,
            init_config.expose_api_server,
        )
        .map_err(|err| cleanup(&app_name, err, init_config.skip_cleanup_on_error))?;

        workflow_import::wait_for_health_check(&app_name, init_config.health_check_deadline_secs)
            .map_err(|err| cleanup(&app_name, err, init_config.skip_cleanup_on_error))?;

        Ok(machine_id)
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
