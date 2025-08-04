use k8s_openapi::api::apps::v1::Deployment;
use kube::{client::Client, runtime::controller::Action};
use std::sync::Arc;
use tokio::time::Duration;

mod backupcron;
pub mod crd;
mod deployment;
mod finalizer;
mod nodebackup;
mod secrets;

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }

    env_logger::init();

    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG.");

    let context: Arc<ContextData> = Arc::new(ContextData::new(client.clone()));

    let deployment_controller = deployment::init_controller(client.clone(), context.clone());
    let nodebackup_controller = nodebackup::init_controller(client.clone(), context.clone());

    tokio::join!(deployment_controller, nodebackup_controller);
}

pub struct ContextData {
    client: Client,
}

impl ContextData {
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }
}

enum ResourceAction {
    Create,
    Delete,
    NoOp,
}

/// Resources arrives into reconciliation queue in a certain state. This function looks at
/// the state of given `Echo` resource and decides which actions needs to be performed.
/// The finite set of possible actions is represented by the `EchoAction` enum.
///
/// # Arguments
/// - `echo`: A reference to `Echo` being reconciled to decide next action upon.
fn determine_action<T: kube::Resource>(echo: &T) -> ResourceAction {
    if echo.meta().deletion_timestamp.is_some() {
        ResourceAction::Delete
    } else if echo
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        ResourceAction::Create
    } else {
        ResourceAction::NoOp
    }
}

/// All errors possible to occur during reconciliation
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Any error originating from the `kube-rs` crate
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    /// Error in user input or Echo resource definition, typically missing fields.
    #[error("Invalid Echo CRD: {0}")]
    UserInputError(String),
}
