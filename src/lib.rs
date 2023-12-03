//! This package implements [OutlineVPN](https://getoutline.org) Management API.

use log::debug;
use reqwest::blocking::{Client, Response};
use reqwest::header::HeaderMap;
use std::time::Duration;

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

// Endpoints
const NAME_ENDPOINT: &str = "/name";
const SERVER_ENDPOINT: &str = "/server";
const HOSTNAME_ENDPOINT: &str = "/server/hostname-for-access-keys";
const CHANGE_PORT_ENDPOINT: &str = "/server/port-for-new-access-keys";
const KEY_DATA_LIMIT_ENDPOINT: &str = "/server/access-key-data-limit";
const METRICS_ENDPOINT: &str = "/metrics";
const ACCESS_KEYS_ENDPOINT: &str = "/access-keys";

impl std::fmt::Display for APIError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            APIError::UnknownServerError => write!(f, "An unknown server error occurred."),
            APIError::InternalError => write!(f, "An internal error occurred."),
            APIError::InvalidHostname => write!(f, "An invalid hostname or IP address was provided."),
            APIError::InvalidPort => write!(f, "The requested port wasn't an integer from 1 through 65535, or the request had no port parameter."),
            APIError::PortConflict => write!(f, "The requested port was already in use by another service."),
            APIError::InvalidDataLimit => write!(f, "Invalid data limit."),
            APIError::AccessKeyInexistent => write!(f, "Access key inexistent."),
            APIError::InvalidName => write!(f, "Invalid name."),
            APIError::InvalidRequest => write!(f, "Invalid request."),
            APIError::UnknownError => write!(f, "An unknown error occurred."),
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

/// Handles the HTTP response status for various API requests.
///
/// This function checks the HTTP response status code from a `reqwest::Response` object
/// and maps it to an appropriate `Result` type. It handles various standard HTTP status
/// codes, such as OK, NO_CONTENT, BAD_REQUEST, CONFLICT, NOT_FOUND, and INTERNAL_SERVER_ERROR.
/// Depending on the API endpoint and the specific error, it returns either an `Ok(())` for
/// successful responses or an `Err(String)` for errors, with a descriptive error message.
///
/// # Arguments
///
/// * `response` – A reference to the `reqwest::Response` object obtained from an API call.
/// * `api_path` – A string slice that holds the API endpoint path.
///
/// # Returns
///
/// This function returns a `Result<(), String>`. On successful response status, it returns `Ok(())`.
/// On error, it returns `Err(String)` with an appropriate error message based on the API endpoint
/// and the response status code.
///
/// # Error Handling
///
/// This function handles the following `reqwest::StatusCode` variants:
/// - `OK`: Indicates a successful request.
/// - `NO_CONTENT`: Indicates a successful request with no content to return.
/// - `BAD_REQUEST`: Maps to specific API errors based on the `api_path`.
/// - `CONFLICT`: Indicates a port conflict error.
/// - `NOT_FOUND`: Indicates an invalid access key error.
/// - `INTERNAL_SERVER_ERROR`: Indicates an internal server error.
/// - Any other status codes are mapped to an unknown error.
///
/// Specific errors are derived from the `APIError` enum, translating enum variants to strings.
fn handle_response_status(response: &Response, api_path: &str) -> Result<(), String> {
    match response.status() {
        reqwest::StatusCode::OK => Ok(()),
        reqwest::StatusCode::NO_CONTENT => Ok(()),
        reqwest::StatusCode::BAD_REQUEST => match api_path {
            NAME_ENDPOINT => Err(APIError::InvalidName.to_string()),
            HOSTNAME_ENDPOINT => Err(APIError::InvalidHostname.to_string()),
            CHANGE_PORT_ENDPOINT => Err(APIError::InvalidPort.to_string()),
            KEY_DATA_LIMIT_ENDPOINT | ACCESS_KEYS_ENDPOINT => {
                Err(APIError::InvalidDataLimit.to_string())
            }
            _ => Err(APIError::InvalidRequest.to_string()),
        },
        reqwest::StatusCode::CONFLICT => Err(APIError::PortConflict.to_string()),
        reqwest::StatusCode::NOT_FOUND => Err(APIError::AccessKeyInexistent.to_string()),
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
/// - `request_timeout_in_sec`: The time to set the timeout for API requests.
pub struct OutlineVPN<'a> {
    api_url: &'a str,
    session: Client,
    request_timeout_in_sec: Duration,
}

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
            .timeout(self.request_timeout_in_sec)
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
        let response = match self.call_api(SERVER_ENDPOINT, reqwest::Method::GET, String::new()) {
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

        handle_response_status(&response, HOSTNAME_ENDPOINT)
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
        let response = match self.call_api(CHANGE_PORT_ENDPOINT, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, CHANGE_PORT_ENDPOINT)
    }

    /// Set data transfer limit (in bytes) for all access keys.
    ///
    /// Responses:
    ///
    /// - `204` – Access key data limit set successfully.
    /// - `400` – Invalid data limit.
    pub fn set_data_transfer_limit_for_all_access_keys(&self, byte: &u64) -> Result<(), String> {
        let body = format!(r#"{{ "limit": {{ "bytes": {} }} }}"#, byte);
        let response = match self.call_api(KEY_DATA_LIMIT_ENDPOINT, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, KEY_DATA_LIMIT_ENDPOINT)
    }

    /// Remove data transfer limit for all access keys.
    ///
    /// Responses:
    ///
    /// - `204` – Access key limit deleted successfully.
    pub fn remove_data_limit_for_all_access_keys(&self) -> Result<(), String> {
        let response = match self.call_api(
            KEY_DATA_LIMIT_ENDPOINT,
            reqwest::Method::DELETE,
            String::new(),
        ) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, KEY_DATA_LIMIT_ENDPOINT)
    }

    /// Rename server.
    ///
    /// Responses:
    ///
    /// - `204` – Server renamed successfully.
    /// - `400` – Invalid name.
    pub fn rename_server(&self, name: &str) -> Result<(), String> {
        let body = format!(r#"{{ "name": "{}" }}"#, name);
        let response = match self.call_api(NAME_ENDPOINT, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, NAME_ENDPOINT)
    }

    /// Create new access key.
    ///
    /// Responses:
    ///
    /// - `201` – The newly created access key.
    pub fn create_access_key(&self) -> Result<serde_json::Value, String> {
        let response =
            match self.call_api(ACCESS_KEYS_ENDPOINT, reqwest::Method::POST, String::new()) {
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
        let response =
            match self.call_api(ACCESS_KEYS_ENDPOINT, reqwest::Method::GET, String::new()) {
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
    //     let api_path = format!("{}/{}", ACCESS_KEYS_ENDPOINT, id);
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
        let api_path = format!("{}/{}", ACCESS_KEYS_ENDPOINT, id);
        let response = match self.call_api(&api_path, reqwest::Method::DELETE, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, ACCESS_KEYS_ENDPOINT)
    }

    /// Change name for access key (by ID).
    ///
    /// Responses:
    ///
    /// - `204` – Access key renamed successfully.
    /// - `404` – Access key inexistent.
    pub fn change_name_for_access_key(&self, id: &u16, username: &str) -> Result<(), String> {
        let body = format!(r#"{{ "name": "{}" }}"#, username);
        let api_path = format!("{}/{}/name", ACCESS_KEYS_ENDPOINT, id);
        let response = match self.call_api(&api_path, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, ACCESS_KEYS_ENDPOINT)
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
        let api_path = format!("{}/{}/data-limit", ACCESS_KEYS_ENDPOINT, id);
        let response = match self.call_api(&api_path, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, ACCESS_KEYS_ENDPOINT)
    }

    /// Remove data transfer limit by ID.
    ///
    /// Responses:
    ///
    /// - `204` – Access key limit deleted successfully.
    /// - `404` – Access key inexistent.
    pub fn del_data_transfer_limit_by_id(&self, id: &u16) -> Result<(), String> {
        let api_path = format!("{}/{}/data-limit", ACCESS_KEYS_ENDPOINT, id);
        let response = match self.call_api(&api_path, reqwest::Method::DELETE, String::new()) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, ACCESS_KEYS_ENDPOINT)
    }

    /// Get data transfer stats for each access key in bytes.
    ///
    /// Responses:
    ///
    /// - `200` – The data transferred by each access key.
    pub fn get_each_access_key_data_transferred(&self) -> Result<serde_json::Value, String> {
        let api_path = format!("{}/transfer", METRICS_ENDPOINT);
        let response = match self.call_api(&api_path, reqwest::Method::GET, String::new()) {
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
        let api_path = format!("{}/enabled", METRICS_ENDPOINT);
        let response = match self.call_api(&api_path, reqwest::Method::GET, String::new()) {
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
        let api_path = format!("{}/enabled", METRICS_ENDPOINT);
        let response = match self.call_api(&api_path, reqwest::Method::PUT, body) {
            Ok(response) => response,
            Err(_) => return Err(APIError::UnknownServerError.to_string()),
        };

        handle_response_status(&response, ACCESS_KEYS_ENDPOINT)
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
/// - `request_timeout_in_sec`: `Duration` specifying the timeout for API requests.
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
/// let request_timeout = Duration::from_secs(10);
///
/// let outline_vpn = outline_api::new(api_url, cert_sha256, request_timeout);
///
/// // Performing operations using the Client:
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
    request_timeout: Duration,
) -> OutlineVPN<'a> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Certificate-SHA256",
        reqwest::header::HeaderValue::from_str(cert_sha256).unwrap(),
    );

    // .danger_accept_invalid_certs(true) is safe to use because it uses a self-issued encryption certificate when the server is created
    let session = Client::builder()
        .danger_accept_invalid_certs(true)
        .default_headers(headers)
        .build()
        .unwrap();

    OutlineVPN {
        api_url,
        session,
        request_timeout_in_sec: request_timeout,
    }
}
