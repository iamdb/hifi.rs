# hifi.rs

### A terminal-based (tui), high resolution audio player backed by Qobuz

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

Qobuz only officially supports Linux through the browser and has no officially supported API. There are ways of accessing Qobuz in Linux outside of the browser through third-party applications like Squeezelite and Roon. These apps are great, but I wanted something simpler that just focused on being able to quickly find and play an album inside the console.

## Player Features

- [GStreamer](https://gstreamer.freedesktop.org/)-backed player
- High resolution audio: Supports up to 24bit/192Khz (max quality Qobuz offers)
- MPRIS support (control via [playerctl](https://github.com/altdesktop/playerctl) or other D-Bus client)
- Resume previous session
- TUI can be disabled to use as a headless player controlled via MPRIS

In addition to the player, there is a Spotify to Qobuz playlist sync tool and an incomplete Rust library for the Qobuz API.

I've spent [![n hours](https://wakatime.com/badge/github/iamdb/hifi.rs.svg)](https://wakatime.com/badge/github/iamdb/hifi.rs) building this.

### Known Issues

- If left paused for a while, the player will crash when attempting to play again.
- When resuming and seeking to the spot in the track, pressing play will cause mpris and the player to go out of sync.
- UI will freeze during loading of long lists.
