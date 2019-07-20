#![feature(custom_attribute)]

#[macro_use] extern crate lazy_static;

mod album;
mod annotate;
mod token;
mod types;
mod utils;
mod whitelist;

use crate::{
    types::{
        ClientWithToken,
    },
};

use std::{
    collections::{
        HashSet,
    },
    sync::{
        Mutex,
    },
};

lazy_static! {
    static ref WHITELIST: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}


fn main() {
    whitelist::read_whitelist().expect("error reading whitelist");

    let token = token::retrieve_access_token()
        .expect("error fetching token")
        .access_token;
    let client_with_token = ClientWithToken::new(token);

    let albums = utils::get_albums().expect("error reading albums");
    let albums_full = album::get_albums_full(
        &albums.iter().map(|dir| {
            dir.parent().and_then(|parent| {
                dir.strip_prefix(parent).map(|path| {
                    path.to_string_lossy().to_string()
                }).ok()
            }).unwrap_or(dir.to_string_lossy().to_string())
        }).collect(),
        &client_with_token,
    ).expect("error fetching albums");

    albums.into_iter().zip(albums_full.into_iter()).map(|(dir, album)| {
        annotate::annotate(&dir, &album, &client_with_token)
    }).collect::<Result<(), types::SimpleError>>()
        .expect("error in annotating");

    whitelist::write_whitelist().expect("error writing whitelist");
}
