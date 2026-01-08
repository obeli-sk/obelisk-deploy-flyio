use crate::{
    Component, VM_NAME_FINAL,
    generated::{
        exports::obelisk_flyio::workflow::testing as exp_testing,
        obelisk_client::api_http,
        obelisk_flyio::{activity_fly_http, workflow::workflow as imp_workflow},
    },
    url,
};
use anyhow::{Context, anyhow, ensure};
use hashbrown::HashSet;
use serde_json::json;

fn restart_should_persist_state(
    app_name: &str,
    obelisk_machine_id: Option<String>,
    obelisk_config: exp_testing::ObeliskConfig,
    init_config: exp_testing::AppInitConfig,
) -> Result<(), anyhow::Error> {
    let api_port = init_config
        .expose_api_server
        .expect("checked by the caller");

    let obelisk_machine_id = if let Some(obelisk_machine_id) = obelisk_machine_id {
        obelisk_machine_id
    } else {
        let machines = activity_fly_http::machines::list(app_name).anyhow()?;
        machines
            .into_iter()
            .find(|m| m.name == VM_NAME_FINAL)
            .context("obelisk vm not found")?
            .id
    };

    // There should be no executions. Create a dummy one.
    let execution_id = api_http::executions::generate().expect("no external service involved");
    let endpoint_url = url(app_name, api_port, "");
    api_http::executions::submit(
        &endpoint_url,
        &execution_id,
        "obelisk-client:api-http/executions@1.0.0-beta.list",
        &json!([&endpoint_url]).to_string(),
    )
    .anyhow()?;
    let old_executions: HashSet<String> = HashSet::from_iter(
        api_http::executions::list(&endpoint_url)
            .anyhow()?
            .into_iter()
            .map(|exe_with_state| exe_with_state.execution_id),
    );
    ensure!(old_executions.contains(&execution_id));

    // Delete the VM
    activity_fly_http::machines::delete(app_name, &obelisk_machine_id, true).anyhow()?;
    // Delete the single volume
    let volume_id = {
        let volumes: Vec<_> = activity_fly_http::volumes::list(app_name)
            .anyhow()?
            .into_iter()
            .filter(|volume| volume.state == "created")
            .collect();

        ensure!(volumes.len() == 1, "one volume expected, got {:?}", volumes);
        volumes.into_iter().next().unwrap().id
    };
    activity_fly_http::volumes::delete(app_name, &volume_id).anyhow()?;
    // Recreate the volume
    let minio_machine_id = {
        let machines = activity_fly_http::machines::list(app_name).anyhow()?;
        ensure!(
            machines.len() == 1,
            "one machine expected, got {:?}",
            machines
        );
        machines.into_iter().next().unwrap().id
    };
    imp_workflow::set_up_volume(
        app_name,
        &obelisk_config,
        Some(&minio_machine_id),
        init_config.vm_startup_deadline_secs,
    )?;
    // Create and start the final VM.
    let _machine_id = imp_workflow::start_final_vm(
        app_name,
        &obelisk_config.obelisk_version,
        init_config.minio,
        init_config.vm_startup_deadline_secs,
        init_config.expose_api_server,
    )?;
    imp_workflow::wait_for_health_check(app_name, init_config.health_check_deadline_secs)?;

    // Make sure the backup worked
    let new_executions: HashSet<String> = HashSet::from_iter(
        api_http::executions::list(&endpoint_url)
            .anyhow()?
            .into_iter()
            .map(|exe_with_state| exe_with_state.execution_id),
    );
    ensure!(
        new_executions.is_superset(&old_executions),
        "old: {old_executions:?}, new: {new_executions:?}"
    );
    Ok(())
}

impl exp_testing::Guest for Component {
    fn restart_should_persist_state(
        app_name: String,
        obelisk_config: exp_testing::ObeliskConfig,
        init_config: exp_testing::AppInitConfig,
    ) -> Result<(), String> {
        restart_should_persist_state(&app_name, None, obelisk_config, init_config)
            .map_err(|err| err.to_string())
    }

    fn start_restart_should_persist_state(
        org_slug: String,
        app_name: String,
        obelisk_config: exp_testing::ObeliskConfig,
        init_config: exp_testing::AppInitConfig,
    ) -> Result<(), String> {
        let obelisk_machine_id = (|| {
            ensure!(init_config.minio, "MinIO must be enabled");
            ensure!(
                init_config.expose_api_server.is_some(),
                "API port must be exposed"
            );

            // create an app with MinIO and an Obelisk VM.

            Ok(imp_workflow::app_init(
                &org_slug,
                &app_name,
                &obelisk_config,
                init_config,
            )?)
        })()
        .map_err(|err| err.to_string())?;

        restart_should_persist_state(
            &app_name,
            Some(obelisk_machine_id),
            obelisk_config,
            init_config,
        )
        .map_err(|err| err.to_string())
    }
}

trait ResultExt<T> {
    fn anyhow(self) -> Result<T, anyhow::Error>;
}
impl<T> ResultExt<T> for Result<T, String> {
    fn anyhow(self) -> Result<T, anyhow::Error> {
        self.map_err(|err| anyhow!("{err}"))
    }
}
