import { derived, writable } from 'svelte/store';

export const currentStatus = writable('Stopped');
export const connected = writable(false);
export const isBuffering = writable(false);
export const isLoading = writable(false);
export const searchResults = writable({
  albums: [],
  artists: [],
  playlists: [],
  tracks: [],
});
export const userPlaylists = writable([])

const position = writable(0);
const duration = writable(0);
const currentTrackList = writable(null);
export const currentTrack = derived(currentTrackList, (list) => {
  return list?.queue.find((l) => l.status === "Playing")
});

export const queue = derived(currentTrackList, (v) => {
  return v?.queue || []
})

export const numOfTracks = derived(queue, (q) => {
  return q.length
})

export const listType = derived(currentTrackList, (v) => {
  return v?.list_type
})

export const coverImage = derived([currentTrackList, currentTrack], ([tl, c]) => {
  if (tl) {
    switch (tl.list_type) {
      case "Album":
        return tl?.album?.coverArt;
      case "Playlist":
        return tl?.playlist?.coverArt;
      case "Track":
        return c?.album?.coverArt
    }
  }

  return []
})


export const entityTitle = derived([currentTrackList, currentTrack], ([tl, c]) => {
  if (tl) {
    switch (tl.list_type) {
      case "Album":
        return tl?.album?.title
      case "Playlist":
        return tl?.playlist?.title;
      case "Track":
        return c?.album?.title;
    }
  }
})

export const secsToTimecode = (secs) => {
  const minutes = Math.floor(secs / 60);
  const seconds = Math.floor(secs - (minutes * 60));

  return `${minutes.toString(10).padStart(2, 0)}:${seconds.toString(10).padStart(2, 0)}`
}

export const positionString = derived(position, (p) => {
  const positionMinutes = Math.floor(p / 1000 / 1000 / 1000 / 60);
  const positionSeconds = Math.floor(p / 1000 / 1000 / 1000) - positionMinutes * 60;

  return `${positionMinutes.toString(10).padStart(2, 0)}:${positionSeconds.toString(10).padStart(2, 0)}`
})

export const durationString = derived(currentTrack, (d) => {
  const durationMinutes = Math.floor(d.durationSeconds / 60);
  const durationSeconds = d.durationSeconds - durationMinutes * 60;

  return `${durationMinutes.toString(10).padStart(2, 0)}:${durationSeconds.toString(10).padStart(2, 0)}`
})

export const artistAlbums = writable({ "id": null, albums: [] });
export const playlistTracks = writable({ "id": null, tracks: [] });
export const playlistTitle = writable('');

export class WS {
  constructor(dev) {
    this.dev = dev;
    this.host = dev ? 'localhost:9888' : window.location.host;

    this.playPause.bind(this)
    this.next.bind(this)
    this.previous.bind(this)
    this.close.bind(this)

    this.connect();
  }

  connect() {
    this.ws = new WebSocket(`ws://${this.host}/ws`);
    this.ws.onopen = () => {
      connected.set(true);
      this.fetchUserPlaylists()
    };

    this.ws.onclose = () => {
      connected.set(false);

      setTimeout(() => {
        this.connect(this.dev)
      }, 1000);
    };

    this.ws.onmessage = (message) => {
      const json = JSON.parse(message.data);

      if (Object.hasOwn(json, 'buffering')) {
        isBuffering.set(json.buffering.is_buffering);
      } else if (Object.hasOwn(json, 'loading')) {
        isLoading.set(json.loading.isLoading);
      } else if (Object.hasOwn(json, 'position')) {
        position.set(json.position.clock);
      } else if (Object.hasOwn(json, 'duration')) {
        duration.set(json.duration.clock);
      } else if (Object.hasOwn(json, 'status')) {
        currentStatus.set(json.status.status);
      } else if (Object.hasOwn(json, 'currentTrackList')) {
        currentTrackList.set(json.currentTrackList?.list);
      } else if (Object.hasOwn(json, 'searchResults')) {
        searchResults.set(json.searchResults.results);
      } else if (Object.hasOwn(json, 'artistAlbums')) {
        artistAlbums.set(json.artistAlbums);
      } else if (Object.hasOwn(json, 'playlistTracks')) {
        playlistTracks.set(json.playlistTracks);
      } else if (Object.hasOwn(json, 'userPlaylists')) {
        userPlaylists.set(json.userPlaylists)
      } else if (Object.hasOwn(json, 'audioQuality')) {
        currentTrack.update((track) => {
          if (track) {
            track.bitDepth = json.audioQuality.bitdepth
            track.samplingRate = json.audioQuality.sampling_rate / 1000
          }
          return track
        })
      }
    };

    this.ws.onerror = () => {
      this.ws.close();
    }
  }

  playPause() {
    this.ws.send(JSON.stringify({ playPause: null }));
  }

  next() {
    this.ws.send(JSON.stringify({ next: null }));
  }

  previous() {
    this.ws.send(JSON.stringify({ previous: null }));
  }

  close() {
    this.ws.close()
  }

  skipTo(num) {
    this.ws.send(JSON.stringify({ skipTo: { num } }))
  }

  playAlbum(album_id) {
    this.ws.send(JSON.stringify({ playAlbum: { album_id } }))
  }

  playTrack(track_id) {
    this.ws.send(JSON.stringify({ playTrack: { track_id } }))
  }

  playPlaylist(playlist_id) {
    this.ws.send(JSON.stringify({ playPlaylist: { playlist_id } }))
  }

  search(query) {
    this.ws.send(JSON.stringify({ search: { query } }))
  }

  fetchArtistAlbums(artist_id) {
    this.ws.send(JSON.stringify({ fetchArtistAlbums: { artist_id } }))
  }

  fetchPlaylistTracks(playlist_id) {
    this.ws.send(JSON.stringify({ fetchPlaylistTracks: { playlist_id } }))
  }

  fetchUserPlaylists() {
    this.ws.send(JSON.stringify({ fetchUserPlaylists: null }))
  }
}
