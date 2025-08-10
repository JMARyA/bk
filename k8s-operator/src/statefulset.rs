// This file contains the reconcile definitions for the StatefulSet kind.

use futures::stream::StreamExt;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::batch::v1::CronJob;
use kube::Resource;
use kube::ResourceExt;
use kube::api::DeleteParams;
use kube::api::PostParams;
use kube::runtime::reflector::Lookup;
use kube::runtime::watcher::Config;
use kube::{Api, client::Client, runtime::Controller, runtime::controller::Action};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Duration;

use crate::add_finalizer;
use crate::backupcron::BackupCronJob;
use crate::backupcron::BkOptions;
use crate::delete_finalizer;
use crate::determine_action;
use crate::secrets::create_or_update_secret;
use crate::secrets::delete_secret;

use crate::ContextData;
use crate::Error;
use crate::ResourceAction;
use crate::secrets::mount_secret_file;

pub fn init_controller(
    client: Client,
    context: Arc<ContextData>,
) -> impl std::future::Future<Output = ()> {
    let StatefulSets: Api<StatefulSet> = Api::all(client.clone());

    Controller::new(StatefulSets.clone(), Config::default())
        .run(reconcile, on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(echo_resource) => {
                    log::info!("Reconciliation successful. Resource: {:?}", echo_resource);
                }
                Err(reconciliation_err) => {
                    log::error!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
}

async fn reconcile(
    StatefulSet: Arc<StatefulSet>,
    context: Arc<ContextData>,
) -> Result<Action, Error> {
    let client: Client = context.client.clone();

    let namespace: String = match Lookup::namespace(&*StatefulSet) {
        None => {
            return Err(Error::UserInputError(
                "Expected BackupDefinition resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }
        Some(namespace) => namespace.to_string(),
    };
    let name = StatefulSet.name_any();

    // Performs action as decided by the `determine_action` function.
    match determine_action(&*StatefulSet) {
        ResourceAction::Create => {
            // Skip system namespaces
            if namespace.ends_with("system") {
                return Ok(Action::await_change());
            }

            log::info!("Found StatefulSet {name} in {namespace}");

            // Handle if bk options are set on the StatefulSet
            if let Some(options) =
                BkOptions::parse(StatefulSet.metadata.annotations.as_ref().unwrap())
            {
                log::info!("Creating Backup Cron for StatefulSet {name}");

                add_finalizer!(
                    client,
                    StatefulSet,
                    &StatefulSet.name().unwrap(),
                    &namespace,
                    "bk.jmarya.me"
                );

                let (mut volumes, mut volume_mounts) =
                    BackupCronJob::get_vols_of_statefulset(&StatefulSet);

                let targets = BackupCronJob::get_remote_config(
                    client.clone(),
                    &namespace,
                    &options.repo,
                    &mut volumes,
                    &mut volume_mounts,
                )
                .await?;

                let conf = BackupCronJob::build_bk_conf(
                    volume_mounts.clone(),
                    targets,
                    options.repo.clone(),
                    name.clone(),
                );

                create_or_update_secret(
                    client.clone(),
                    &namespace,
                    {
                        let mut h = HashMap::new();
                        h.insert("bk.toml".to_string(), toml::to_string(&conf).unwrap());
                        h
                    },
                    BackupCronJob::cronjob_secret_name(&name),
                )
                .await?;

                // add config secret to volumes
                let (vol, mount) = mount_secret_file(
                    "bk-config".to_string(),
                    BackupCronJob::cronjob_secret_name(&name),
                    "bk.toml".to_string(),
                    "/etc/bk-config".to_string(),
                );
                volumes.push(vol);
                volume_mounts.push(mount);

                let cjob = BackupCronJob::create_cronjob(
                    BackupCronJob::cronjob_name(&name),
                    &name,
                    &namespace,
                    volume_mounts,
                    volumes,
                    options,
                );

                create_or_update_cron(client.clone(), &namespace, &name, cjob).await?;
            }

            Ok(Action::requeue(Duration::from_secs(60)))
        }
        ResourceAction::Delete => {
            log::info!("Deleting Backup Cron for StatefulSet {name}");

            let cronjobs: Api<CronJob> = Api::namespaced(client.clone(), &namespace);
            cronjobs
                .delete(
                    &BackupCronJob::cronjob_name(&name),
                    &DeleteParams::default(),
                )
                .await;

            delete_secret(
                client.clone(),
                &namespace,
                &BackupCronJob::cronjob_secret_name(&name),
            )
            .await
            .unwrap();

            delete_finalizer!(
                client,
                StatefulSet,
                &StatefulSet.name().unwrap(),
                &namespace,
                "bk.jmarya.me"
            )
            .unwrap();

            Ok(Action::await_change())
        }
        // The resource is already in desired state, do nothing and re-check after 10 seconds
        ResourceAction::NoOp => Ok(Action::requeue(Duration::from_secs(60))),
    }
}

/// Actions to be taken when a reconciliation fails - for whatever reason.
/// Prints out the error to `stderr` and requeues the resource for another reconciliation after
/// five seconds.
///
/// # Arguments
/// - `echo`: The erroneous resource.
/// - `error`: A reference to the `kube::Error` that occurred during reconciliation.
/// - `_context`: Unused argument. Context Data "injected" automatically by kube-rs.
fn on_error(echo: Arc<StatefulSet>, error: &Error, _context: Arc<ContextData>) -> Action {
    log::error!("Reconciliation error:\n{:?}.\n{:?}", error, echo);
    Action::requeue(Duration::from_secs(5))
}

pub async fn create_or_update_cron(
    client: Client,
    ns: &str,
    name: &str,
    cjob: CronJob,
) -> Result<(), kube::Error> {
    let cronjobs: Api<CronJob> = Api::namespaced(client.clone(), ns);

    match cronjobs.create(&PostParams::default(), &cjob).await {
        Ok(_) => {
            log::info!("Created CronJob for {name}",);
        }
        Err(kube::Error::Api(e)) if e.code == 409 => {
            // Already exists, do an update instead
            let current = cronjobs.get(&cjob.name().unwrap()).await?;
            let mut updated = current.clone();

            // You decide how much to update — possibly update .spec only
            updated.spec = cjob.spec.clone();

            log::info!("Updating CronJob {}", cjob.name().unwrap());
            cronjobs
                .replace(&cjob.name().unwrap(), &PostParams::default(), &updated)
                .await?;
        }
        Err(e) => return Err(e.into()), // Other errors are real problems
    }

    Ok(())
}
