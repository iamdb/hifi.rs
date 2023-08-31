import { derived, writable } from 'svelte/store';

export const currentStatus = writable('Stopped');
export const connected = writable(false);
export const currentTrack = writable(null);
export const isBuffering = writable(false);
export const searchResults = writable({
  albums: [],
  artists: [],
  playlists: [],
  tracks: [],
});

const position = writable(0);
const duration = writable(0);
const currentTrackList = writable(null);

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

export const positionString = derived(position, (p) => {
  const positionMinutes = Math.floor(p / 1000 / 1000 / 1000 / 60);
  const positionSeconds = Math.floor(p / 1000 / 1000 / 1000) - positionMinutes * 60;

  return `${positionMinutes.toString(10).padStart(2, 0)}:${positionSeconds.toString(10).padStart(2, 0)}`
})

export const durationString = derived(duration, (d) => {
  const durationMinutes = Math.floor(d / 1000 / 1000 / 1000 / 60);
  const durationSeconds = Math.floor(d / 1000 / 1000 / 1000) - durationMinutes * 60;

  return `${durationMinutes.toString(10).padStart(2, 0)}:${durationSeconds.toString(10).padStart(2, 0)}`
})

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
    };

    this.ws.onclose = () => {
      connected.set(false);

      setTimeout(() => {
        this.connect(this.dev)
      }, 1000);
    };

    this.ws.onmessage = (message) => {
      const json = JSON.parse(message.data);
      console.log(json)

      if (Object.hasOwn(json, 'buffering')) {
        isBuffering.set(json.buffering.is_buffering);
      } else if (Object.hasOwn(json, 'position')) {
        position.set(json.position.clock);
      } else if (Object.hasOwn(json, 'duration')) {
        duration.set(json.duration.clock);
      } else if (Object.hasOwn(json, 'status')) {
        currentStatus.set(json.status.status);
      } else if (Object.hasOwn(json, 'currentTrack')) {
        currentTrack.set(json.currentTrack.track);
      } else if (Object.hasOwn(json, 'currentTrackList')) {
        currentTrackList.set(json.currentTrackList?.list);
      } else if (Object.hasOwn(json, 'searchResults')) {
        searchResults.set(json.searchResults?.results);
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
}
