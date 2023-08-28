import { writable } from 'svelte/store';

export const isBuffering = writable(false);
export const position = writable(0);
export const duration = writable(0);
export const currentStatus = writable('Stopped');
export const connected = writable(false);
export const currentTrack = writable(null);
export const currentTrackList = writable([]);

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

  connected() {
    this.ws.readyState == this.ws.OPEN
  }

  connecting() {
    this.ws.readyState == this.ws.CONNECTING
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
        currentTrackList.set(json.currentTrackList.list.queue);
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
}
