use std::{
    fs::{
        File,
        OpenOptions,
    },
    io::{
        BufRead,
        BufReader,
        Write,
    },
};

use crate::{
    types::{
        SimpleError,
    },
    WHITELIST,
};

pub fn read_whitelist(
) -> Result<(), SimpleError> {
    BufReader::new(
        File::open("whitelist.txt").map_err(SimpleError::from)?,
    ).lines().map(|line| {
        line.map_err(SimpleError::from).and_then(|line_ok| {
            WHITELIST.lock().map_err(SimpleError::from).map(|mut lock| {
                lock.insert(line_ok);
            })
        })
    }).collect()
}

pub fn add_whitelist(
    entry: String,
) -> Result<(), SimpleError> {
    WHITELIST.lock().map_err(SimpleError::from).map(|mut lock| {
        lock.insert(entry);
    })
}

pub fn write_whitelist(
) -> Result<(), SimpleError> {
    let mut output = OpenOptions::new()
        .write(true)
        .open("whitelist.txt")
        .map_err(SimpleError::from)?;

    WHITELIST.lock().map_err(SimpleError::from).and_then(|lock| {
        lock.iter().map(|entry| {
            output.write(format!("{}\n", entry).as_bytes()).map(|_| {
                ()
            }).map_err(SimpleError::from)
        }).collect::<Result<(), SimpleError>>()
    })
}
