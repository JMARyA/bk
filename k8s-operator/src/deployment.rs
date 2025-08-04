// This file contains the reconcile definitions for the Deployment kind.

use futures::stream::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::CronJob;
use kube::Resource;
use kube::ResourceExt;
use kube::api::DeleteParams;
use kube::api::PostParams;
use kube::runtime::reflector::Lookup;
use kube::runtime::watcher::Config;
use kube::{Api, client::Client, runtime::Controller, runtime::controller::Action};
use std::sync::Arc;
use tokio::time::Duration;

use crate::add_finalizer;
use crate::backupcron::BackupCronJob;
use crate::backupcron::BkOptions;
use crate::delete_finalizer;
use crate::determine_action;
use crate::secrets::delete_secret;

use crate::ContextData;
use crate::Error;
use crate::ResourceAction;

pub fn init_controller(
    client: Client,
    context: Arc<ContextData>,
) -> impl std::future::Future<Output = ()> {
    let deployments: Api<Deployment> = Api::all(client.clone());

    Controller::new(deployments.clone(), Config::default())
        .run(reconcile, crate::on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(echo_resource) => {
                    println!("Reconciliation successful. Resource: {:?}", echo_resource);
                }
                Err(reconciliation_err) => {
                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
}

async fn reconcile(echo: Arc<Deployment>, context: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = context.client.clone(); // The `Client` is shared -> a clone from the reference is obtained

    let namespace: String = match Lookup::namespace(&*echo) {
        None => {
            // If there is no namespace to deploy to defined, reconciliation ends with an error immediately.
            return Err(Error::UserInputError(
                "Expected BackupDefinition resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }
        // If namespace is known, proceed. In a more advanced version of the operator, perhaps
        // the namespace could be checked for existence first.
        Some(namespace) => namespace.to_string(),
    };
    let name = echo.name_any(); // Name of the Echo resource is used to name the subresources as well.

    // Performs action as decided by the `determine_action` function.
    match determine_action(&*echo) {
        ResourceAction::Create => {
            let ns = echo.metadata.namespace.as_ref().unwrap();
            println!(
                "Found {} in {}",
                echo.metadata.name.as_ref().unwrap(),
                echo.metadata.namespace.as_ref().unwrap()
            );

            // Skip system namespaces
            if ns == "kube-system" {
                return Ok(Action::await_change());
            }

            if let Some(options) = BkOptions::parse(echo.metadata.annotations.as_ref().unwrap()) {
                let cjob = BackupCronJob::new(client.clone(), options, &echo).await;
                println!(
                    "Creating Backup Cron for {}",
                    echo.metadata.name.as_ref().unwrap()
                );

                add_finalizer!(
                    client,
                    Deployment,
                    &echo.name().unwrap(),
                    ns,
                    "bk.jmarya.me"
                );

                let cronjobs: Api<CronJob> = Api::namespaced(client.clone(), ns);
                match cronjobs.create(&PostParams::default(), &cjob).await {
                    Ok(_) => {
                        println!("Created CronJob for {}", name);
                    }
                    Err(kube::Error::Api(e)) if e.code == 409 => {
                        // Already exists, do an update instead
                        let current = cronjobs.get(&cjob.name().unwrap()).await?;
                        let mut updated = current.clone();

                        // You decide how much to update — possibly update .spec only
                        updated.spec = cjob.spec.clone();

                        println!("Updating CronJob {}", cjob.name().unwrap());
                        cronjobs
                            .replace(&cjob.name().unwrap(), &PostParams::default(), &updated)
                            .await?;
                    }
                    Err(e) => return Err(e.into()), // Other errors are real problems
                }
            }

            Ok(Action::requeue(Duration::from_secs(60)))
        }
        ResourceAction::Delete => {
            let ns = echo.metadata.namespace.as_ref().unwrap();
            println!(
                "Deleting Backup Cron for {}",
                echo.metadata.name.as_ref().unwrap()
            );

            let cronjobs: Api<CronJob> = Api::namespaced(client.clone(), ns);
            cronjobs
                .delete(
                    &BackupCronJob::cronjob_name(&name),
                    &DeleteParams::default(),
                )
                .await
                .unwrap();

            delete_secret(
                client.clone(),
                ns,
                &BackupCronJob::cronjob_secret_name(&name),
            )
            .await
            .unwrap();

            delete_finalizer!(
                client,
                Deployment,
                &echo.name().unwrap(),
                ns,
                "bk.jmarya.me"
            )
            .unwrap();

            Ok(Action::await_change())
        }
        // The resource is already in desired state, do nothing and re-check after 10 seconds
        ResourceAction::NoOp => Ok(Action::requeue(Duration::from_secs(60))),
    }
}
