# outline-api

This package implements [OutlineVPN](https://getoutline.org) Management API.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://github.com/rust-lang/rust)

## Installation

To use this package, you'll need to have Rust and Cargo installed on your system. Once you have them, you can add this package to your application by running the following command:

```bash
cargo add outline-api
```

## Usage

You can use this lib to build your own OutlineVPN management app like this:

### main.rs

```rust
extern crate toml;

use std::{fs, io, time::Duration};
use toml::Value;

fn main() {
    let toml_str = fs::read_to_string("config.toml").expect("Failed to read config file");

    let toml_value: Value = toml::from_str(&toml_str).expect("Failed to parse config.toml");

    let server_section = toml_value
        .get("server")
        .expect("Missing [server] section")
        .as_table()
        .expect("[server] section should be a table");

    let cert_sha256 = server_section
        .get("cert_sha256")
        .and_then(Value::as_str)
        .expect("Missing or invalid cert_sha256");

    let api_url = server_section
        .get("api_url")
        .and_then(Value::as_str)
        .expect("Missing or invalid api_url");

    let request_timeout = server_section
        .get("request_timeout_in_sec")
        .and_then(|timeout| timeout.as_integer())
        .map(|timeout_secs| Duration::from_secs(timeout_secs as u64))
        .expect("Missing or invalid request_timeout");

    let vpn = outline_api::new(cert_sha256, api_url, Some(request_timeout));

    match vpn.get_server_info() {
        Ok(info) => println!("Server info: {}", info),
        Err(err) => eprintln!("Error getting server info: {}", err),
    }

    let raw_id = get_user_input("Provide user ID:");
    match raw_id.parse::<u16>() {
        Ok(id) => match vpn.delete_access_key_by_id(&id) {
            Ok(info) => println!("response: {:?}\n", info),
            Err(err) => {
                eprintln!("Error delete_access_key_by_id: {}", err)
            }
        },
        Err(_) => {
            eprintln!("Failed to parse the ID input as number (u16)");
        }
    }

    fn get_user_input(prompt: &str) -> String {
        println!("{}", prompt);
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");
        input.trim().to_string()
    }
}
```

## config.toml

```toml
[server]
cert_sha256 = "CERT_SHA256" # like E2DE8...2A75D
api_url = "https://<IP_ADDR>:<PORT>/<SECRET>"
request_timeout_in_sec = 5
```

## Outline API

This is some important note about OutlineVPN API:

- The official version of the API (see [api.yml](/api.yml)) is not quite right.
- In fact, the `/access-keys/<ID>` endpoint is not available on the server.
