extern crate chrono;
extern crate id3;
extern crate mp3_duration;
extern crate regex;
extern crate reqwest;

use std::{
    fs::{
        read_dir,
        rename,
    },
    io::{
        Read,
    },
    path::{
        Path,
        PathBuf,
    },
    time::{
        Duration,
    }
};

use chrono::{
    Datelike,
    format::{
        ParseResult,
    },
    NaiveDate,
};
use id3::{
    frame::{
        Picture,
        PictureType,
    },
    Tag,
    Timestamp,
    Version,
};
use regex::{
    Regex,
};

use crate::{
    types::{
        AlbumFull,
        ClientWithToken,
        SimpleError,
        Track,
    },
    utils::{
        get_with_retry,
    },
    whitelist::{
        add_whitelist,
    },
};

#[derive(Debug)]
pub struct TrackData {
    album_name: String,
    album_artists: String,
    release_date: Option<Timestamp>,
    image_url: Option<String>,

    track_name: String,
    track_number: i32,
    track_artists: Option<String>,
    expected_duration_ms: i32,
}

impl TrackData {
    fn release_date_from(
        album_full: &AlbumFull,
    ) -> ParseResult<Timestamp> {
        let mut year = -1;
        let mut month = None;
        let mut day = None;

        if album_full.release_date_precision == "year" {
            let date = NaiveDate::parse_from_str(
                &album_full.release_date[..],
                "%Y",
            )?;
            year = date.year();
        }
        if album_full.release_date_precision == "month" {
            let date = NaiveDate::parse_from_str(
                &album_full.release_date[..],
                "%Y-%m",
            )?;
            year = date.year();
            month = Some(date.month() as u8);
        }
        else if album_full.release_date_precision == "day" {
            let date = NaiveDate::parse_from_str(
                &album_full.release_date[..],
                "%Y-%m-%d",
            ).expect("wat");
            year = date.year();
            month = Some(date.month() as u8);
            day = Some(date.day() as u8);
        }

        Ok(Timestamp {
            year: year,
            month: month,
            day: day,
            hour: None,
            minute: None,
            second: None,
        })
    }
    
    pub fn from(
        track: Track,
        album_full: &AlbumFull,
    ) -> Self {
        let album_artists = album_full.artists.iter().map(|artist| {
            artist.name.clone()
        }).collect::<Vec<String>>().join(", ");
        let track_artists = track.artists.iter().map(|artist| {
            artist.name.clone()
        }).collect::<Vec<String>>().join(", ");
        Self {
            album_name: album_full.name.clone(),
            album_artists: album_artists.clone(),
            release_date: Self::release_date_from(album_full).ok(),
            image_url: album_full.images.iter().next().map(|image| {
                image.url.clone()
            }),

            track_name: track.name,
            track_number: track.track_number,
            track_artists: Some(track_artists).filter(|artists| {
                // need clone?
                artists != &album_artists
            }),
            expected_duration_ms: track.duration_ms,
        }
    }
}

fn get_tracks_files(
    abs_path: &Path,
) -> Result<Vec<PathBuf>, SimpleError> {
    read_dir(abs_path).map_err(SimpleError::from).and_then(|dir_iter| {
        dir_iter.map(|entry| {
            entry.map(|entry_ok| {
                entry_ok.path()
            }).map_err(SimpleError::from)
        }).collect::<Result<Vec<PathBuf>, SimpleError>>()
    }).map(|mut paths| {
        paths.sort();
        paths.into_iter().filter(|path| {
            path.is_file()
        }).collect()
    })
}

pub fn get_tracks_data(
    album_full: &AlbumFull,
    client_with_token: &ClientWithToken,
) -> Result<Vec<TrackData>, SimpleError> {
    let mut tracks = Vec::new();
    let mut paging = album_full.tracks.clone();
    while let Some(next_url) = paging.next {
        tracks.append(&mut paging.items);
        paging = get_with_retry(
            &next_url[..],
            client_with_token,
        )?;
    }
    tracks.append(&mut paging.items);

    Ok(tracks.into_iter().map(|track| {
        TrackData::from(track, album_full)
    }).collect())
}

fn norm_track_number(
    track_number: i32,
) -> String {
    if track_number < 10 {
        return format!("0{}", track_number);
    }

    track_number.to_string()
}

fn expected_time(
    file: &PathBuf,
    track_data: &TrackData,
) -> bool {
    let actual_duration = mp3_duration::from_path(file.as_path()).expect(
        &format!("error measuring {}", file.display())[..],
    );
    let expected_duration = Duration::from_millis(
        track_data.expected_duration_ms as u64,
    );

    actual_duration.checked_sub(expected_duration).or(
        expected_duration.checked_sub(actual_duration)
    ).and_then(|res| {
        res.checked_sub(Duration::from_secs(5))
    }).is_none()
}

fn add_image(
    tags: &mut Tag,
    image_url: &str,
) -> Result<(), SimpleError> {
    reqwest::get(image_url).map_err(SimpleError::from).and_then(|response| {
        response.bytes().map(|byte_res| {
            byte_res.map_err(SimpleError::from)
        }).collect::<Result<Vec<u8>, SimpleError>>().map(|data| {
            tags.add_picture(Picture {
                mime_type: "image/jpeg".to_string(),
                picture_type: PictureType::CoverFront,
                description: format!(
                    "Cover for {} by {}",
                    tags.album().expect("error in writing tags"),
                    tags.artist().expect("error in writing tags"),
                ),
                data: data,
            });
        })
    })
}

fn annotate_tags(
    tags: &mut Tag,
    file: &PathBuf,
    track_data: TrackData,
) -> String {
    lazy_static! {
        static ref INVALID_FILE_CHRS: Regex = Regex::new(r"[\W\s]+").unwrap();
    }
    
    let mut new_name = format!(
        "{} {}.mp3",
        norm_track_number(track_data.track_number),
        track_data.track_name,
    );

    if !expected_time(file, &track_data) {
        new_name = format!(
            "{} {} (unexpected duration).mp3",
            norm_track_number(track_data.track_number),
            track_data.track_name,
        );
    }

    tags.set_album(track_data.album_name);
    let album_artists = track_data.album_artists.clone();
    track_data.track_artists.map(|artists| {
        tags.set_album_artist(album_artists.clone());
        tags.set_artist(artists);
    }).unwrap_or_else(|| {
        tags.set_artist(album_artists);
    });
    track_data.release_date.map(|date| {
        tags.set_date_released(date);
    });
    tags.set_title(track_data.track_name);
    tags.set_track(track_data.track_number as u32);
    tags.set_duration(mp3_duration::from_path(file.as_path()).expect(
        &format!("error measuring {}", file.display())[..],
    ).as_millis() as u32);
    println!("{:?}", tags.duration());

    track_data.image_url.ok_or(SimpleError {
        msg: format!("no image for {}", file.display()),
    }).and_then(|url| {
        add_image(tags, &url[..])
    }).unwrap_or_else(|err| {
        println!("error getting image for {}: {}", file.display(), err.msg);
    });
        
    INVALID_FILE_CHRS.replace_all(&new_name[..], "_").to_string()
}

fn annotate_file(
    file: &PathBuf,
    track_data: TrackData,
) -> Result<(), SimpleError> {
    let mut tags = Tag::new();
    let new_name = annotate_tags(&mut tags, file, track_data); // annotate tags

    tags.write_to_path(file, Version::Id3v24).map_err(SimpleError::from)
        .and_then(|_| { // rename file
            file.as_path().file_name().ok_or(SimpleError {
                msg: format!("{} not file?", file.display()),
            }).and_then(|file_name| {
                if new_name != file_name.to_string_lossy() {
                    return rename(
                        file,
                        file.with_file_name(new_name),
                    ).map_err(SimpleError::from);
                }
                Ok(())
            })
        })
}

pub fn annotate(
    dir: &PathBuf,
    album_full: &AlbumFull,
    client_with_token: &ClientWithToken,
) -> Result<(), SimpleError> {
    let abs_path = Path::new("/home/banana/music/").join(&dir.as_path());

    let files = get_tracks_files(&abs_path)?;
    let data = get_tracks_data(album_full, client_with_token)?;
    if files.len() != data.len() {
        println!(
            "number of files in {} should be {}",
            dir.display(),
            data.len(),
        );
    }

    files.iter().zip(
        data.into_iter()
    ).map(|(track_file, track_data)| {
        annotate_file(track_file, track_data)
            .and_then(|_| { // add to whitelist
                add_whitelist(dir.to_string_lossy().to_string())
            })
    }).collect()
}
