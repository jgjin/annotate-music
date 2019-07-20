extern crate reqwest;
extern crate serde;
extern crate serde_json;

use reqwest::{
    Client,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::{
    Map,
    Value,
};

pub struct ClientWithToken {
    client: Client,
    token: String,
}

impl ClientWithToken {
    pub fn new(
        token: String,
    ) -> Self {
        Self {
            client: Client::new(),
            token: token,
        }
    }

    pub fn get(
        &self,
    ) -> (&Client, &str) {
        (&self.client, &self.token[..])
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Paging<Item: Clone> {
    pub href: String,
    pub items: Vec<Item>,
    pub limit: i32,
    pub next: Option<String>,
    pub offset: i32,
    pub previous: Option<String>,
    pub total: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artist {
    pub external_urls: Map<String, Value>,
    pub href: String,
    pub id: String,
    pub name: String,
    pub uri: String,
    #[serde(rename = "type")] 
    pub object_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Image {
    pub height: i32,
    pub url: String,
    pub width: i32,
}

macro_rules! with_album_core_fields {
    (pub struct $name:ident { $( pub $field:ident: $ty:ty ),* $(,)* }) => {
        #[derive(Clone, Debug, Deserialize, Serialize)]
        pub struct $name {
            pub album_group: Option<String>,
            pub album_type: String,
            pub artists: Vec<Artist>,
            pub available_markets: Option<Vec<String>>,
            pub external_urls: Map<String, Value>,
            pub href: String,
            pub id: String,
            pub images: Vec<Image>,
            pub name: String,
            pub release_date: String,
            pub release_date_precision: String,
            pub restrictions: Option<Map<String, Value>>,
            pub uri: String,
            #[serde(rename = "type")] 
            pub object_type: String,
            $( pub $field: $ty ),*
        }
    };
}

with_album_core_fields!(pub struct Album {});

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Copyright {
    pub text: String,
    #[serde(rename = "type")]
    pub object_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrackLink {
    external_urls: Map<String, Value>,
    href: String,
    id: String,
    uri: String,
    #[serde(rename = "type")]
    pub object_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Track {
    pub artists: Vec<Artist>,
    pub available_markets: Option<Vec<String>>,
    pub disc_number: i32,
    pub duration_ms: i32,
    pub explicit: bool,
    pub external_urls: Map<String, Value>,
    pub href: String,
    pub id: String,
    pub is_playable: Option<bool>,
    pub linked_from: Option<TrackLink>,
    pub name: String,
    pub preview_url: Option<String>,
    pub track_number: i32,
    pub uri: String,
    #[serde(rename = "type")]
    pub object_type: String,
}


with_album_core_fields!(pub struct AlbumFull {
    pub copyrights: Vec<Copyright>,
    pub external_ids: Map<String, Value>,
    pub genres: Vec<String>,
    pub label: Option<String>,
    pub popularity: i32,
    pub tracks: Paging<Track>,
});

#[derive(Debug)]
pub struct SimpleError {
    pub msg: String,
}

impl SimpleError {
    pub fn from<E: std::error::Error + std::fmt::Display>(
        error: E,
    ) -> Self {
        Self {
            msg: format!("{}", error)
        }
    }
}
