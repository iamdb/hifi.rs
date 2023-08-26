import { writable } from 'svelte/store';

export const isBuffering = writable(false);
export const position = writable(0);
export const duration = writable(0);
export const currentStatus = writable('Stopped');
export const connected = writable(false);
export const currentTrack = writable(null);
export const currentTrackList = writable([]);

export const init = (dev) => {
  const host = dev ? 'localhost:3000' : window.location.host;
  const ws = new WebSocket(`ws://${host}/ws`);

  ws.onopen = () => {
    connected.set(true);
  };

  let retryInterval;

  ws.onclose = () => {
    retryInterval = setInterval(() => {
      ws.connected;
    });
  };

  ws.onmessage = (message) => {
    const json = JSON.parse(message.data);
    console.log(json);

    if (Object.hasOwn(json, 'buffering')) {
      isBuffering.set(json.buffering.is_buffering);
    } else if (Object.hasOwn(json, 'position')) {
      position.set(json.position.clock);
    } else if (Object.hasOwn(json, 'duration')) {
      position.set(json.duration.clock);
    } else if (Object.hasOwn(json, 'status')) {
      currentStatus.set(json.status.status);
    } else if (Object.hasOwn(json, 'currentTrack')) {
      currentTrack.set(json.currentTrack.track);
    } else if (Object.hasOwn(json, 'currentTrackList')) {
      currentTrackList.set(json.currentTrackList.list.queue);
    }
  };


  const playPause = () => {
    ws.send(JSON.stringify({ playPause: null }));
  };

  const next = () => {
    ws.send(JSON.stringify({ next: null }));
  }

  const previous = () => {
    ws.send(JSON.stringify({ previous: null }));
  }

  const close = ws.close;

  return { playPause, next, previous, close }
}
