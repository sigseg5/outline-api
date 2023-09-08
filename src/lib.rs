//! This package implements [OutlineVPN](https://getoutline.org) Management API.

use std::time::Duration;

use log::debug;
use reqwest::blocking::{Client, Response};
use reqwest::header::HeaderMap;

extern crate serde_json;

// API reference v1.0
// See api.yml at project github or
// https://github.com/Jigsaw-Code/outline-server/blob/1ac9f238132d5917b42d4b6615727e477aa7bbc0/src/shadowbox/server/api.yml

// API documentation is, hmm... discussable
// List of actually unavailable methods:
//   get_access_key_by_id

/// Configures the logging system based on the build mode.
///
/// You should call this function if you are interested in debug printing, and you should also build your project
/// with debug mode (e.g. `cargo build`, not `cargo build --release`).
///
/// - In debug mode, this function sets up the logging system to print debug messages.
/// - In release mode or when debug assertions are disabled, it configures logging to do nothing.
pub fn configure_logging() {
    if cfg!(debug_assertions) {
        // In debug mode, configure logging to print debug messages.
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        // In release mode or when debug assertions are disabled, configure logging to do nothing.
        env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .init();
    }
}

#[derive(Debug)]
enum APIError {
    UnknownServerError,
    InternalError,
    InvalidHostname,
    InvalidPort,
    PortConflict,
    InvalidDataLimit,
    AccessKeyInexistent,
    InvalidName,
    InvalidRequest,
    UnknownError,
}

impl APIError {
    fn to_string(&self) -> String {
        match self {
            APIError::UnknownServerError => "An unknown server error occurred.".to_string(),
            APIError::InternalError => "An internal error occurred.".to_string(),
            APIError::InvalidHostname => "An invalid hostname or IP address was provided.".to_string(),
            APIError::InvalidPort => "The requested port wasn't an integer from 1 through 65535, or the request had no port parameter.".to_string(),
            APIError::PortConflict => "The requested port was already in use by another service.".to_string(),
            APIError::InvalidDataLimit => "Invalid data limit.".to_string(),
            APIError::AccessKeyInexistent => "Access key inexistent.".to_string(),
            APIError::InvalidName => "Invalid name.".to_string(),
            APIError::InvalidRequest => "Invalid request.".to_string(),
            APIError::UnknownError => "An unknown error occurred.".to_string(),
        }
    }
}

/// Handles API responses and returns a result with either a JSON value or an error message.
///
/// This function processes the response from an API request and checks the status code to determine
/// the outcome. If the response status code is `200 OK`, it attempts to deserialize the response body
/// as JSON and returns the JSON value. If the status code is `500 Internal Server Error`, it returns
/// an error indicating an internal server error. For all other status codes, it returns an unknown
/// error message.
///
/// # Arguments
///
/// - `response`: The `Response` object received from the API request.
///
/// # Returns
///
/// Returns a `Result` where:
/// - `Ok(json_value)` contains the deserialized JSON value if the response status code is `200 OK`.
/// - `Err(error_message)` contains an error message if the response status code is not `200 OK`.
///
/// # Errors
///
/// This function can return the following errors:
///
/// - `APIError::InternalError`: If the response status code is `500 Internal Server Error`, indicating
///   an internal server error.
/// - `APIError::UnknownError`: If the response status code is not `200 OK` or `500 Internal Server Error`,
///   indicating an unknown error occurred.
fn handle_json_api_error(response: Response) -> Result<serde_json::Value, String> {
    match response.status() {
        reqwest::StatusCode::OK => {
            let response_body = response
                .text()
                .map_err(|_| "Error reading response body".to_string())?;
            let json_value: serde_json::Value = serde_json::from_str(&response_body)
                .map_err(|_| "Error deserializing JSON".to_string())?;
            Ok(json_value)
        }
        reqwest::StatusCode::INTERNAL_SERVER_ERROR => Err(APIError::InternalError.to_string()),
        _ => Err(APIError::UnknownError.to_string()),
    }
}

/// Represents a client for interacting with the Outline VPN Server API.
///
/// The `OutlineVPN` struct provides methods to perform various operations on the Outline VPN server
/// such as retrieving server information, changing settings, creating access keys, and more.
///
/// # Fields
///
/// - `api_url`: A reference to a string representing the URL (including `secret`) of the Outline VPN server API.
/// - `session`: A reqwest HTTP client used to make API requests.
/// - `request_timeout`: The time to set the timeout for API requests.
pub struct OutlineVPN<'a> {
    api_url: &'a str,
    session: Client,
    request_timeout: Duration,
}

// Endpoints
const SERVER_ENDPOINT: &str = "/server";
const HOSTNAME_ENDPOINT: &str = "/server/hostname-for-access-keys";
const CHANGE_PORT_ENDPOINT: &str = "/server/port-for-new-access-keys";
const KEY_DATA_LIMIT_ENDPOINT: &str = "/server/access-key-data-limit";

const REQUEST_TIMEOUT_IN_SEC: u64 = 5;

impl OutlineVPN<'_> {
    fn call_api(
        &self,
        api_path: &str,
        request_method: reqwest::Method,
        request_body: String,
    ) -> Result<Response, reqwest::Error> {
        let url = format!("{}{}", self.api_url, api_path);
        debug!("URL: {}", url);
        debug!("Method: {:?}", request_method);
        debug!("Request Body: {}", request_body);
        let response = self
            .session
            .request(request_method, &url)
            .timeout(self.request_timeout)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request_body)
            .send()?;

        Ok(response)
    }

    /// Get server information.
    ///
    /// Responses:
    ///
    /// - `200` – Server information.
    pub fn get_server_info(&self) -> Result<serde_json::Value, String> {
        let response = match self.call_api(&SERVER_ENDPOINT, reqwest::Method::GET, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_json_api_error(response)
    }

    /// Change hostname for access keys.
    ///
    /// Responses:
    ///
    ///  - `204` – The hostname was successfully changed.
    ///  - `400` – An invalid hostname or IP address was provided.
    ///  - `500` – An internal error occurred.  This could be thrown if there were network errors while validating the hostname.
    pub fn change_hostname_for_access_keys(&self, hostname: &str) -> Result<(), String> {
        let body = format!(r#"{{ "hostname": "{}" }}"#, hostname);
        let response = match self.call_api(HOSTNAME_ENDPOINT, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => Err(APIError::InvalidHostname.to_string()),
            reqwest::StatusCode::INTERNAL_SERVER_ERROR => Err(APIError::InternalError.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Change default port for newly created access keys.
    ///
    /// Responses:
    ///
    /// - `204` – The default port was successfully changed.
    /// - `400` – The requested port wasn't an integer from 1 through 65535, or the request had no port parameter.
    /// - `409` – The requested port was already in use by another service.
    pub fn change_default_port_for_newly_created_access(&self, port: &str) -> Result<(), String> {
        let body = format!(r#"{{ "port": {} }}"#, port);
        let response = match self.call_api(&CHANGE_PORT_ENDPOINT, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => Err(APIError::InvalidPort.to_string()),
            reqwest::StatusCode::CONFLICT => Err(APIError::PortConflict.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Set data transfer limit (in bytes) for all access keys.
    ///
    /// Responses:
    ///
    /// - `204` – Access key data limit set successfully.
    /// - `400` – Invalid data limit.
    pub fn set_data_transfer_limit_for_all_access_keys(&self, byte: &u64) -> Result<(), String> {
        let body = format!(r#"{{ "limit": {{ "bytes": {} }} }}"#, byte);
        let response = match self.call_api(&KEY_DATA_LIMIT_ENDPOINT, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => Err(APIError::InvalidDataLimit.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Remove data transfer limit for all access keys.
    ///
    /// Responses:
    ///
    /// - `204` – Access key limit deleted successfully.
    pub fn remove_data_limit_for_all_access_keys(&self) -> Result<(), String> {
        let response = match self.call_api(
            &KEY_DATA_LIMIT_ENDPOINT,
            reqwest::Method::DELETE,
            String::new(),
        ) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Rename server.
    ///
    /// Responses:
    ///
    /// - `204` – Server renamed successfully.
    /// - `400` – Invalid name.
    pub fn rename_server(&self, name: &str) -> Result<(), String> {
        let body = format!(r#"{{ "name": "{}" }}"#, name);
        let response = match self.call_api("/name", reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => Err(APIError::InvalidName.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Create new access key.
    ///
    /// Responses:
    ///
    /// - `201` – The newly created access key.
    pub fn create_access_key(&self) -> Result<serde_json::Value, String> {
        let response = match self.call_api("/access-keys", reqwest::Method::POST, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_json_api_error(response)
    }

    /// Display complete list of the access keys.
    ///
    /// Responses:
    ///
    /// - `200` – List of access keys.
    pub fn list_access_keys(&self) -> Result<serde_json::Value, String> {
        let response = match self.call_api("/access-keys", reqwest::Method::GET, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_json_api_error(response)
    }

    // /// Incorrect API specification, this method is defined in the API, but is not actually supported by the server!!!
    // /// Get access key by ID.
    // ///
    // /// Responses:
    // ///
    // /// - 200 – The access key.
    // /// - 404 – Access key inexistent.
    // pub fn get_access_key_by_id(&self, id: &u16) -> Result<serde_json::Value, String> {
    //     let api_path = format!("/access-keys/{}", id);
    //     let response = match self.call_api(&api_path, reqwest::Method::GET, String::new()) {
    //         Ok(response) => response,
    //         Err(_) => return Err(APIError::UnknownServerError.to_string()),
    //     };
    //
    //     handle_api_error(response)
    // }

    /// Delete access key by ID.
    ///
    /// Responses:
    ///
    /// - `204` – Access key deleted successfully.
    /// - `404` – Access key inexistent.
    pub fn delete_access_key_by_id(&self, id: &u16) -> Result<(), String> {
        let api_path = format!("/access-keys/{}", id);
        let response = match self.call_api(&api_path, reqwest::Method::DELETE, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::NOT_FOUND => Err(APIError::AccessKeyInexistent.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Change name for access key (by ID).
    ///
    /// Responses:
    ///
    /// - `204` – Access key renamed successfully.
    /// - `404` – Access key inexistent.
    pub fn change_name_for_access_key(&self, id: &u16, username: &str) -> Result<(), String> {
        let body = format!(r#"{{ "name": "{}" }}"#, username);
        let api_path = format!("/access-keys/{}/name", id);
        let response = match self.call_api(&api_path, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::NOT_FOUND => Err(APIError::AccessKeyInexistent.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Set data transfer limit by ID.
    ///
    /// Responses:
    ///
    /// - `204` – Access key limit set successfully.
    /// - `400` – Invalid data limit.
    /// - `404` –  Access key inexistent.
    pub fn set_data_transfer_limit_by_id(&self, id: &u16, byte: &u64) -> Result<(), String> {
        let body = format!(r#"{{ "limit": {{ "bytes": {} }} }}"#, byte);
        let api_path = format!("/access-keys/{}/data-limit", id);
        let response = match self.call_api(&api_path, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => Err(APIError::InvalidDataLimit.to_string()),
            reqwest::StatusCode::NOT_FOUND => Err(APIError::AccessKeyInexistent.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Remove data transfer limit by ID.
    ///
    /// Responses:
    ///
    /// - `204` – Access key limit deleted successfully.
    /// - `404` – Access key inexistent.
    pub fn del_data_transfer_limit_by_id(&self, id: &u16) -> Result<(), String> {
        let api_path = format!("/access-keys/{}/data-limit", id);
        let response = match self.call_api(&api_path, reqwest::Method::DELETE, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::NOT_FOUND => Err(APIError::AccessKeyInexistent.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Get data transfer stats for each access key in bytes.
    ///
    /// Responses:
    ///
    /// - `200` – The data transferred by each access key.
    pub fn get_each_access_key_data_transferred(&self) -> Result<serde_json::Value, String> {
        let response = match self.call_api("/metrics/transfer", reqwest::Method::GET, String::new())
        {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::OK => {
                let response_body = response
                    .text()
                    .map_err(|_| "Error reading response body".to_string())?;
                let json_value: serde_json::Value = serde_json::from_str(&response_body)
                    .map_err(|_| "Error deserializing JSON".to_string())?;
                Ok(json_value)
            }
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Get 'Share anonymous metrics' status.
    ///
    /// Responses:
    ///
    /// - `200` – The metrics enabled setting.
    pub fn get_whether_metrics_is_being_shared(&self) -> Result<serde_json::Value, String> {
        let response = match self.call_api("/metrics/enabled", reqwest::Method::GET, String::new())
        {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::OK => {
                let response_body = response
                    .text()
                    .map_err(|_| "Error reading response body".to_string())?;
                let json_value: serde_json::Value = serde_json::from_str(&response_body)
                    .map_err(|_| "Error deserializing JSON".to_string())?;
                Ok(json_value)
            }
            _ => Err(APIError::UnknownError.to_string()),
        }
    }

    /// Enable or disable 'Share anonymous metrics' setting.
    ///
    /// Responses:
    ///
    /// - `204` – Setting successful.
    /// - `400` – Invalid request.
    pub fn enable_or_disable_sharing_metrics(&self, metrics_enabled: bool) -> Result<(), String> {
        let body = format!(r#"{{ "metricsEnabled": {} }}"#, metrics_enabled);

        let response = match self.call_api("/metrics/enabled", reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        match response.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => Err(APIError::InvalidRequest.to_string()),
            _ => Err(APIError::UnknownError.to_string()),
        }
    }
}

/// Creates a new `OutlineVPN` client to interact with the Outline VPN Server management API.
///
/// This function initializes and configures an `OutlineVPN` client with the provided parameters.
///
/// # Arguments
///
/// - `cert_sha256`: A reference to a string representing the SHA-256 hash of the server's certificate.
/// - `api_url`: A reference to a string representing the URL of the Outline VPN server API.
/// - `request_timeout`: An optional `Duration` specifying the timeout for API requests. If `None` is
///   provided, a default timeout of 5 seconds is used.
///
/// # Returns
///
/// Returns an `OutlineVPN` client configured with the specified parameters.
///
/// # Examples:
///
/// ## Creating an OutlineVPM API client:
///
/// ```rust
/// use std::time::Duration;
///
/// // Reading from the `config.rs` is preferred way, see README.md at github repo
/// // https://github.com/sigseg5/outline-api/blob/master/README.md
/// let api_url = "https://example.com/secret";
/// let cert_sha256 = "cert_sha256_hash";
/// let request_timeout = Some(Duration::from_secs(10));
///
/// let outline_vpn = outline_api::new(api_url, cert_sha256, request_timeout);
///
/// // Performing operations using the lient:
///
/// match outline_vpn.get_server_info() {
///     Ok(server_info) => {
///         println!("Server information: {}", server_info);
///     },
///     Err(err) => {
///         eprintln!("Error: {}", err);
///     }
/// }
/// ```
pub fn new<'a>(
    cert_sha256: &'a str,
    api_url: &'a str,
    request_timeout: Option<Duration>,
) -> OutlineVPN<'a> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Certificate-SHA256",
        reqwest::header::HeaderValue::from_str(cert_sha256).unwrap(),
    );

    // .danger_accept_invalid_certs(true) use is safe because it uses a self-issued encryption certificate when the server is created
    let session = Client::builder()
        .danger_accept_invalid_certs(true)
        .default_headers(headers)
        .build()
        .unwrap();

    let default_request_timeout = Duration::from_secs(REQUEST_TIMEOUT_IN_SEC);
    let safe_request_timeout = if let Some(timeout) = request_timeout {
        timeout
    } else {
        default_request_timeout
    };

    OutlineVPN {
        api_url: &api_url,
        session,
        request_timeout: safe_request_timeout,
    }
}
