use k8s_openapi::api::core::v1::KeyToPath;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::api::core::v1::SecretVolumeSource;
use k8s_openapi::api::core::v1::Volume;
use k8s_openapi::api::core::v1::VolumeMount;
use kube::api::DeleteParams;
use kube::api::PostParams;
use kube::{Api, client::Client};
use std::collections::BTreeMap;
use std::collections::HashMap;

use crate::crd::SecretKeyRef;

/// Get the value of a `SecretKeyRef` within namespace `ns`
pub async fn get_secret(client: Client, ns: &str, reference: SecretKeyRef) -> String {
    let secrets: Api<Secret> = Api::namespaced(client, &ns);
    let secret = secrets.get(&reference.secretName).await.unwrap();

    let secret_string_data = secret.data.unwrap();
    let value = secret_string_data.get(&reference.secretKey).unwrap();

    let value = String::from_utf8(value.0.clone()).unwrap();
    value
}

/// Generate the `Volume` and `VolumeMount` to mount a secret
pub fn mount_secret_file(
    name: String,
    secret_name: String,
    secret_key: String,
    path: String,
) -> (Volume, VolumeMount) {
    (
        Volume {
            name: name.clone(),
            secret: SecretVolumeSource {
                secret_name: Some(secret_name),
                items: vec![KeyToPath {
                    key: secret_key.clone(),
                    path: secret_key,
                    mode: Some(0o600),
                    ..Default::default()
                }]
                .into(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        },
        VolumeMount {
            name: name,
            mount_path: path,
            read_only: true.into(),
            ..Default::default()
        },
    )
}

pub async fn create_secret(
    client: Client,
    ns: &str,
    data: HashMap<String, String>,
    name: String,
) -> Result<Secret, kube::Error> {
    let secrets: Api<Secret> = Api::namespaced(client, &ns);

    let mut btree_data: BTreeMap<String, String> = BTreeMap::new();

    for (key, val) in data {
        btree_data.insert(key, val).unwrap();
    }

    let secret = Secret {
        metadata: kube::api::ObjectMeta {
            name: Some(name),
            ..Default::default()
        },
        string_data: Some(
            btree_data
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
        type_: Some("Opaque".to_string()),
        ..Default::default()
    };

    secrets.create(&PostParams::default(), &secret).await
}

pub async fn delete_secret(client: Client, ns: &str, name: &str) -> Result<(), kube::Error> {
    let secrets: Api<Secret> = Api::namespaced(client.clone(), ns);
    if let Err(e) = secrets.delete(&name, &DeleteParams::default()).await {
        return Err(e);
    }

    Ok(())
}
