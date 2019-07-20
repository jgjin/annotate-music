extern crate reqwest;
extern crate serde;

use std::{
    collections::{
        HashMap,
    },
};

use reqwest::{
    Client,
};
use serde::{
    Deserialize,
};

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u32,
}

pub fn retrieve_access_token(
) -> reqwest::Result<TokenResponse> {
    let client_id = "cb1596c4292a414bba1593704d9508d6";
    let client_secret = "78336321ef08402b87671fad84a74c83";
    let mut form_data = HashMap::new();
    form_data.insert("grant_type", "client_credentials");

    Client::new().post("https://accounts.spotify.com/api/token")
        .basic_auth::<String, String>(
            format!("{}:{}", client_id, client_secret),
            None,
        )
        .form(&form_data)
        .send().and_then(|mut response| {
            response.json()
        })
}
