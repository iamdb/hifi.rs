pub mod controls;

use crate::service::{Album, Playlist, Track, TrackStatus};
use serde::{Deserialize, Serialize, Serializer};
use std::{collections::BTreeMap, fmt::Display};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrackListType {
    Album,
    Playlist,
    Track,
    #[default]
    Unknown,
}

impl Display for TrackListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackListType::Album => f.write_fmt(format_args!("album")),
            TrackListType::Playlist => f.write_fmt(format_args!("playlist")),
            TrackListType::Track => f.write_fmt(format_args!("track")),
            TrackListType::Unknown => f.write_fmt(format_args!("unknown")),
        }
    }
}

impl From<&str> for TrackListType {
    fn from(tracklist_type: &str) -> Self {
        match tracklist_type {
            "album" => TrackListType::Album,
            "playlist" => TrackListType::Playlist,
            "track" => TrackListType::Track,
            _ => TrackListType::Unknown,
        }
    }
}

fn serialize_btree<S>(queue: &BTreeMap<u32, Track>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let vec_values: Vec<_> = queue.values().collect();
    vec_values.serialize(s)
}

/// A tracklist is a list of tracks.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackListValue {
    #[serde(serialize_with = "serialize_btree")]
    pub queue: BTreeMap<u32, Track>,
    album: Option<Album>,
    playlist: Option<Playlist>,
    list_type: TrackListType,
}

impl TrackListValue {
    #[instrument]
    pub fn new(queue: Option<BTreeMap<u32, Track>>) -> TrackListValue {
        let queue = if let Some(q) = queue {
            q
        } else {
            BTreeMap::new()
        };

        TrackListValue {
            queue,
            album: None,
            playlist: None,
            list_type: TrackListType::Unknown,
        }
    }

    pub fn total(&self) -> u32 {
        if let Some(album) = &self.album {
            album.total_tracks
        } else if let Some(list) = &self.playlist {
            list.tracks_count
        } else {
            self.queue.len() as u32
        }
    }

    #[instrument(skip(self))]
    pub fn clear(&mut self) {
        self.list_type = TrackListType::Unknown;
        self.album = None;
        self.playlist = None;
        self.queue.clear();
    }

    #[instrument(skip(self, album), fields(album_id = album.id))]
    pub fn set_album(&mut self, album: Album) {
        debug!("setting tracklist album");
        self.album = Some(album);
        debug!("setting tracklist list type");
        self.list_type = TrackListType::Album;
    }

    #[instrument(skip(self))]
    pub fn get_album(&self) -> Option<&Album> {
        self.album.as_ref()
    }

    #[instrument(skip(self))]
    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist);
        self.list_type = TrackListType::Playlist;
    }

    #[instrument(skip(self))]
    pub fn get_playlist(&self) -> Option<&Playlist> {
        self.playlist.as_ref()
    }

    #[instrument(skip(self))]
    pub fn set_list_type(&mut self, list_type: TrackListType) {
        self.list_type = list_type;
    }

    #[instrument(skip(self))]
    pub fn list_type(&self) -> &TrackListType {
        &self.list_type
    }

    #[instrument(skip(self))]
    pub fn find_track_by_index(&self, index: u32) -> Option<&Track> {
        self.queue.get(&index)
    }

    #[instrument(skip(self))]
    pub fn set_track_status(&mut self, position: u32, status: TrackStatus) {
        self.queue.entry(position).and_modify(|e| e.status = status);
    }

    #[instrument(skip(self))]
    pub fn unplayed_tracks(&self) -> Vec<&Track> {
        self.queue
            .values()
            .filter(|t| t.status == TrackStatus::Unplayed)
            .collect::<Vec<&Track>>()
    }

    #[instrument(skip(self))]
    pub fn played_tracks(&self) -> Vec<&Track> {
        self.queue
            .values()
            .filter(|t| t.status == TrackStatus::Played)
            .collect::<Vec<&Track>>()
    }

    #[instrument(skip(self))]
    pub fn track_index(&self, track_id: u32) -> Option<u32> {
        let mut index: Option<u32> = None;

        self.queue.iter().for_each(|(i, t)| {
            if t.id == track_id {
                index = Some(*i);
            }
        });

        index
    }

    pub fn current_track(&self) -> Option<Track> {
        for track in self.queue.values() {
            if track.status == TrackStatus::Playing {
                return Some(track.clone());
            }
        }

        None
    }

    pub fn cursive_list(&self) -> Vec<(String, i32)> {
        self.queue
            .values()
            .map(|i| (i.title.clone(), i.id as i32))
            .collect::<Vec<(String, i32)>>()
    }
}
