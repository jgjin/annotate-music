"""Convert albums in music home from .webm to .mp3 format."""

from os.path import basename, exists, isfile, join as path_join, splitext
import os

from pydub import AudioSegment


def list_dir(
        dir_,
):
    """List contents of dir_, including own path in path names."""
    return map(
        lambda fi: path_join(dir_, fi),
        os.listdir(dir_),
    )


def convert(
        album,
):
    """Convert album from .webm to .mp3 format."""
    for track in list_dir(album):
        ext = splitext(track)[1]
        if ext != ".mp3":
            new_track = track.replace(ext, ".mp3")
            if not exists(new_track):
                track_non_mp3 = AudioSegment.from_file(track, format=ext[1:])
                print(f"{track} -> {new_track}")
                track_non_mp3.export(new_track, format="mp3")
            os.remove(track)


def main(
):
    """Convert albums in music home from .webm to .mp3 format."""
    music_home = "/home/banana/music"
    for inode in list_dir(music_home):
        if basename(inode) in [
                "annotate",
                "metadata",
                "sped-up",
                "tracklists",
        ] or isfile(inode):
            continue
        convert(inode)


if __name__ == "__main__":
    main()
