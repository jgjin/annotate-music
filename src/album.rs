use crate::{
    types::{
        Album,
        AlbumFull,
        ClientWithToken,
        Paging,
        SimpleError,
    },
    utils::{
        get_with_retry,
        search,
    },
};

fn search_album(
    query: &str,
    client_with_token: &ClientWithToken,
) -> Result<Paging<Album>, SimpleError> {
    let result = search::<serde_json::Value>(
        query,
        "album",
        client_with_token,
    ).map(|value| {
        serde_json::from_value(
            value.get("albums")
                .expect("error in album::search_album format")
                .to_owned()
        ).expect("error in album::search_album format")
    });

    result.and_then(|album_results: Paging<Album>| {
        if album_results.items.is_empty() {
            return Err(SimpleError {
                msg: format!("no results for {}", query),
            });
        }
        Ok(album_results)
    })
}

fn get_album_ids(
    albums: &Vec<String>,
    client_with_token: &ClientWithToken,
) -> Result<Vec<String>, SimpleError> {
    albums.into_iter().map(|album_query| {
        let album_query = album_query
            .replace("u-s-girls", "u.s. girls")
            .replace("m-a-a-d", "m.a.a.d.")
            .replace("-m-", "'m ")
            .replace("-re-", "'re ")
            .replace("-s-", "'s ")
            .replace("-t-", "'t ")
            .replace("-", " ")
            .replace("_", " ");
        search_album(
            &album_query[..],
            client_with_token,
        )
    }).collect::<Result<Vec<Paging<Album>>, SimpleError>>()
        .and_then(|albums_results| {
            albums_results.into_iter().map(|album_result| {
                album_result.items.into_iter().next().map(|result| {
                    result.id
                }).ok_or(SimpleError {
                    msg: "no results".to_string(),
                })
            }).collect::<Result<Vec<String>, SimpleError>>()
        })
}

fn get_album_full(
    album_id: &str,
    client_with_token: &ClientWithToken,
) -> Result<AlbumFull, SimpleError> {
    get_with_retry(
        &format!("https://api.spotify.com/v1/albums/{}/", album_id)[..],
        client_with_token,
    )
}

pub fn get_albums_full(
    albums: &Vec<String>,
    client_with_token: &ClientWithToken,
) -> Result<Vec<AlbumFull>, SimpleError> {
    get_album_ids(albums, client_with_token).and_then(|ids| {
        ids.into_iter().map(|id| {
            get_album_full(&id[..], &client_with_token)
        }).collect()
    })
}
