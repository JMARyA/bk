use facet::Facet;
use reqwest::blocking::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::error::Error;

/// Sends a POST request to the specified API endpoint with a given body.
fn post_api(url: &str, body: &str, auth: Option<(String, String)>) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut request = client.post(url).body(body.to_string());

    if let Some((username, password)) = auth {
        request = request.basic_auth(username, Some(password));
    }

    let response = request.send()?;

    log::info!("NTFY POST {} => {}", url, response.status());
    Ok(())
}

/// Sends a message to an `ntfy.sh` topic.
pub fn ntfy(
    host: &str,
    topic: &str,
    auth: Option<(String, String)>,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{host}/{topic}");
    post_api(&url, message, auth)
}

// Notification

/// Ntfy configuration
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct NtfyTarget {
    pub ntfy: Option<NtfyConfiguration>,
}

impl NtfyTarget {
    pub fn send_notification(&self, msg: &str) {
        if let Some(ntfy_conf) = &self.ntfy {
            ntfy(
                &ntfy_conf.host,
                &ntfy_conf.topic,
                ntfy_conf.auth.clone().map(|x| x.auth()),
                msg,
            )
            .unwrap();
        }
    }
}

/// Ntfy configuration
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct NtfyConfiguration {
    pub host: String,
    pub topic: String,
    pub auth: Option<NtfyAuth>,
}

/// Ntfy configuration
#[derive(Facet, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[facet(skip_all_unless_truthy)]
pub struct NtfyAuth {
    pub user: String,
    #[facet(sensitive)]
    pub pass: Option<String>,
    pub pass_file: Option<String>,
}

impl NtfyAuth {
    pub fn auth(&self) -> (String, String) {
        let pass = if let Some(pass) = &self.pass {
            Some(pass.clone())
        } else if let Some(pass) = &self.pass_file {
            Some(std::fs::read_to_string(pass).expect("unable to read ntfy passfile"))
        } else {
            None
        };

        (
            self.user.clone(),
            pass.expect("neither pass nor passfile provided"),
        )
    }
}
