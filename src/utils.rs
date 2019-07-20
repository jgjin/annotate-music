extern crate percent_encoding;
extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::{
    fs::{
        read_dir,
    },
    io::{
        self,
    },
    path::{
        PathBuf,
    },
    thread::{
        sleep,
    },
    time::{
        Duration,
    },
};

use percent_encoding::{
    utf8_percent_encode,
    DEFAULT_ENCODE_SET,
};
use reqwest::{
    header::{
        RETRY_AFTER,
    },
    StatusCode,
};
use serde::{
    de::{
        DeserializeOwned,
    },
};

use crate::{
    types::{
        ClientWithToken,
        SimpleError,
    },
    WHITELIST,
};

pub fn get_albums(
) -> io::Result<Vec<PathBuf>> {
    read_dir("/home/banana/music")?.map(|entry| {
        entry.map(|entry_ok| {
            entry_ok.path()
        })
    }).collect::<io::Result<Vec<PathBuf>>>().map(|paths| {
        paths.into_iter().filter(|path| {
            path.is_dir()
        }).filter(|dir| {
            let dir = dir.to_string_lossy().to_string();
            !WHITELIST.lock().expect("error checking whitelist").contains(&dir) &&
                dir.contains("jeff-rosenstock_post")
        }).collect()
    })
}

pub fn search<D: DeserializeOwned>(
    query: &str,
    query_type: &str,
    client_with_token: &ClientWithToken,
) -> Result<D, SimpleError> {
    get_with_retry::<D>(
        &format!(
            "https://api.spotify.com/v1/search/?q={}&type={}",
            utf8_percent_encode(query, DEFAULT_ENCODE_SET),
            query_type,
        )[..],
        client_with_token,
    )
}

pub fn get_with_retry<D: DeserializeOwned>(
    url: &str,
    client_with_token: &ClientWithToken,
) -> Result<D, SimpleError> {
    let (client, token) = client_with_token.get();

    let mut response = client.get(url)
        .bearer_auth(token)
        .send().map_err(SimpleError::from)?;

    match response.status() {
        StatusCode::OK => response.json::<D>().map_err(SimpleError::from),
        StatusCode::TOO_MANY_REQUESTS => {
            match response.headers().get(RETRY_AFTER) {
                Some(retry_after_value) => {
                    sleep(Duration::from_secs(
                        retry_after_value.to_str()
                            .expect("unexpected format in retry-after header")
                            .parse::<u64>()
                            .expect("unexpected format in retry-after header"),
                    ));
                    get_with_retry(url, client_with_token)
                },
                None => Err(SimpleError {
                    msg: "missing retry-after header".to_string(),
                }),
            }
        },
        status_code => Err(SimpleError {
            msg: format!("unexpected error code: {}", status_code),
        }),
    }
}
