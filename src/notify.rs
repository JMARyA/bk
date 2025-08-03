use reqwest::blocking::Client;
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
