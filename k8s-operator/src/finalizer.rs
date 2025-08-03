use kube::api::{Patch, PatchParams};
use kube::{Api, Client, Error, Resource};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Adds a finalizer to a Kubernetes resource of type `$resource_ty`.
///
/// This macro patches the specified resource in the given namespace to add
/// a finalizer with the format `{id}/finalizer`. If the finalizer already
/// exists, this operation has no effect.
///
/// # Arguments
/// - `client`: A `kube::Client` instance for communicating with the cluster.
/// - `resource_ty`: The Kubernetes resource type (e.g. `Echo`, `MyCustomResource`).
/// - `name`: The name of the resource to patch.
/// - `namespace`: The namespace where the resource lives.
/// - `id`: The identifier string used to generate the finalizer name.
///
/// # Returns
/// A `Result` containing the patched resource of type `$resource_ty` on success,
/// or a `kube::Error` if the patch operation fails.
///
/// # Example
/// ```ignore
/// let added = add_finalizer!(client, Echo, "my-echo", "default", "my-controller").await?;
/// ```
#[macro_export]
macro_rules! add_finalizer {
    ($client:expr, $resource_ty:ty, $name:expr, $namespace:expr, $id:expr) => {{
        use kube::{Api, api::{Patch, PatchParams}};
        use serde_json::{json, Value};

        let api: Api<$resource_ty> = Api::namespaced($client.clone(), $namespace);
        let finalizer: Value = json!({
            "metadata": {
                "finalizers": [format!("{}/finalizer", $id)]
            }
        });

        let patch: Patch<&Value> = Patch::Merge(&finalizer);
        api.patch($name, &PatchParams::default(), &patch).await
    }};
}

/// Removes *only* the finalizer `{id}/finalizer` from a Kubernetes resource of type `$resource_ty`.
///
/// This macro fetches the specified resource and removes the finalizer string
/// matching `{id}/finalizer` from its metadata. If the finalizer is not present,
/// the resource remains unchanged.
///
/// # Arguments
/// - `client`: A `kube::Client` instance for communicating with the cluster.
/// - `resource_ty`: The Kubernetes resource type (e.g. `Echo`, `MyCustomResource`).
/// - `name`: The name of the resource to patch.
/// - `namespace`: The namespace where the resource lives.
/// - `id`: The identifier string used to generate the finalizer name.
///
/// # Returns
/// A `Result` containing the patched resource of type `$resource_ty` on success,
/// or a `kube::Error` if the patch operation fails.
///
/// # Example
/// ```ignore
/// let deleted = delete_finalizer!(client, Echo, "my-echo", "default", "my-controller").await?;
/// ```
#[macro_export]
macro_rules! delete_finalizer {
    ($client:expr, $resource_ty:ty, $name:expr, $namespace:expr, $id:expr) => {{
        use kube::{Api, api::{Patch, PatchParams}};
        use serde_json::{json};

        let api: Api<$resource_ty> = Api::namespaced($client.clone(), $namespace);
        let resource = api.get($name).await?;
        let metadata = resource.meta().clone();
        let existing = metadata.finalizers.clone().unwrap_or_default();

        let my_finalizer = format!("{}/finalizer", $id);
        let updated: Vec<String> = existing.into_iter().filter(|f| f != &my_finalizer).collect();

        let patch = json!({
            "metadata": {
                "finalizers": if updated.is_empty() { serde_json::Value::Null } else { json!(updated) }
            }
        });

        api.patch($name, &PatchParams::default(), &Patch::Merge(&patch)).await
    }};
}
