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
    iter::{
        repeat_with,
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
    pub fn release_date_from(
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

fn get_image(
    image_url: &str,
)  -> Result<Vec<u8>, SimpleError> {
    reqwest::get(image_url).map_err(SimpleError::from).and_then(|response| {
        response.bytes().map(|byte_res| {
            byte_res.map_err(SimpleError::from)
        }).collect()
    })
}

fn add_image(
    tags: &mut Tag,
    image: &Vec<u8>,
) {
    tags.add_picture(Picture {
        mime_type: "image/jpeg".to_string(),
        picture_type: PictureType::CoverFront,
        description: format!(
            "Cover for {} by {}",
            tags.album().expect("error in writing tags"),
            tags.artist().expect("error in writing tags"),
        ),
        data: image.clone(),
    });
}

fn annotate_tags(
    tags: &mut Tag,
    file: &PathBuf,
    track_data: TrackData,
    album_image: &Vec<u8>,
) -> String {
    lazy_static! {
        static ref INVALID_FILE_CHRS: Regex = Regex::new(r"[^\w\s.\(\)]+").unwrap();
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

    if !album_image.is_empty() {
        add_image(tags, album_image)
    }
    
    INVALID_FILE_CHRS.replace_all(&new_name[..], "_").to_string()
}

fn annotate_file(
    file: &PathBuf,
    track_data: TrackData,
    album_image: &Vec<u8>,
    rename_file: bool,
) -> Result<(), SimpleError> {
    let mut tags = Tag::new();
    let new_name = annotate_tags(&mut tags, file, track_data, album_image);

    tags.write_to_path(file, Version::Id3v24).map_err(SimpleError::from)
        .and_then(|_| {
            if rename_file {
                return file.as_path().file_name().ok_or(SimpleError {
                    msg: format!("{} not file?", file.display()),
                }).and_then(|file_name| {
                    if new_name != file_name.to_string_lossy() {
                        return rename(
                            file,
                            file.with_file_name(new_name),
                        ).map_err(SimpleError::from);
                    }
                    Ok(())
                });
            }
            return Ok(());
        })
}

pub fn annotate(
    dir: &PathBuf,
    album_full: &AlbumFull,
    client_with_token: &ClientWithToken,
) -> Result<(), SimpleError> {
    let abs_path = Path::new("/home/banana/music/").join(&dir.as_path());

    let mut rename_files = true;
    let files = get_tracks_files(&abs_path)?;
    let mut data = get_tracks_data(album_full, client_with_token)?;
    if files.len() != data.len() {
        println!(
            "number of files in {} should be {}, not renaming",
            dir.display(),
            data.len(),
        );
        rename_files = false;
    }

    let album_image = album_full.images.iter().next().map(|image| {
        image.url.clone()
    }).map(|url| {
        get_image(&url[..]).unwrap_or_else(|err| {
            println!("error getting image for {}: {}", album_full.name, err.msg);
            vec![]
        })
    }).unwrap_or_else(|| {
        println!("no image for {}", album_full.name);
        vec![]
    });
    let mut track_counter = data.len() as i32;
    data.extend(repeat_with(|| {
        let track_data = TrackData {
            album_name: album_full.name.clone(),
            album_artists: album_full.artists.iter().map(|artist| {
                artist.name.clone()
            }).collect::<Vec<String>>().join(", "),
            release_date: TrackData::release_date_from(album_full).ok(),
            image_url: album_full.images.iter().next().map(|image| {
                image.url.clone()
            }),

            track_name: "unknown track name".to_string(),
            track_number: track_counter,
            track_artists: None,
            expected_duration_ms: 0,
        };
        track_counter += 1;
        track_data
    }).take(files.len()));
    files.iter().zip(
        data.into_iter(),
    ).map(|(track_file, track_data)| {
        annotate_file(track_file, track_data, &album_image, rename_files)
            .and_then(|_| {
                add_whitelist(dir.to_string_lossy().to_string())
            })
    }).collect()
}

pub fn test_run(
    dir: &PathBuf,
) -> Result<(), SimpleError> {
    let abs_path = Path::new("/home/banana/music/").join(&dir.as_path());

    let files = get_tracks_files(&abs_path)?;

    files.iter().map(|track_file| {
        mp3_duration::from_path(track_file.as_path()).map(|_| {
            ()
        }).unwrap_or_else(|err| {
            println!("error measuring {}: {}", track_file.display(), err);
        });
        Ok(())
    }).collect()
}
