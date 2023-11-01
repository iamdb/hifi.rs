# hifi.rs

### A terminal-based (tui), high resolution audio player backed by Qobuz

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

Qobuz only supports Linux through the browser and has no officially supported API. There are ways of accessing Qobuz in Linux outside of the browser through third-party applications like Squeezelite and Roon. These apps are great, but I wanted something simpler that just focused on being able to quickly find and play an album inside the console.

![TUI Screenshot](/hifi-rs.png?raw=true)

## Player Features

- Low resource usage
- [GStreamer](https://gstreamer.freedesktop.org/)-backed player, [SQLite](https://www.sqlite.org/index.html) database
- High resolution audio: Supports up to 24bit/192Khz (max quality Qobuz offers)
- MPRIS support (control via [playerctl](https://github.com/altdesktop/playerctl) or other D-Bus client)
- Gapless playback
- Resume last session
- Optional Web UI with WebSocket API

In addition to the player, there is a Spotify to Qobuz playlist sync tool and an incomplete Rust library for the Qobuz API.

## Requirements

- [GStreamer v1.18+](https://gstreamer.freedesktop.org/documentation/installing/index.html) (comes with most/all current Linux and MacOS versions)

## Installation

### Download Release

Download the tar.gz file for your OS from the [releases page](https://github.com/iamdb/hifi.rs/releases), extract the file and execute `hifi-rs` or copy it to the your `$PATH`.

### Build from source

To make building from source easier, there is a `Dockerfile` and `Dockerfile.arm64` to compile the project for Linux x86 and arm64 into a container.

Run `build_linux.sh` or `build_arm64.sh` to automatically build the app in Docker and output the file.

## Get started

Run `hifi-rs --help` or `hifi-rs <subcommand> --help` to see all available options.

To get started:

```shell
hifi-rs config username # enter username at prompt
hifi-rs config password # enter password at prompt
hifi-rs config default-quality <quality> # mp3, cd, hifi96 or hifi192

# play from the command line
hifi-rs play --url <Qobuz Album, Playlist or Track URL>

# open player
hifi-rs open

# open player with web ui
hifi-rs --web open
```

## TUI Controls

The TUI has full mouse support.

### Keyboard Shortcuts

| Command             | Key(s)                                 |
| ------------------- | -------------------------------------- |
| Now Playing         | <kbd>1</kbd>                           |
| My Playlists        | <kbd>2</kbd>                           |
| Search              | <kbd>3</kbd>                           |
| Enter URL           | <kbd>3</kbd>                           |
| Cycle elements      | <kbd>tab</kbd>                         |
| Play/Pause          | <kbd>space</kbd>                       |
| Next track          | <kbd>N</kbd>                           |
| Previous track      | <kbd>P</kbd>                           |
| Jump forward        | <kbd>l</kbd>                           |
| Jump backward       | <kbd>h</kbd>                           |
| Quit                | <kbd>ctrl</kbd> + <kbd>c</kbd>         |
| Move up in list     | <kbd>up arrow</kbd>                    |
| Move down in list   | <kbd>down arrow</kbd>                  |
| Select item in list | <kbd>enter</kbd>                       |
| Dismiss popup       | <kbd>esc</kbd>                         |

## Web UI and WebSocket API

!["WebUI Desktop Screenshot"](/hifi-rs-webui-desktop.png?raw=true)

The player can start an embedded web interface along with a websocket API. As this is a potential attack vector, the
server is disabled by default and must be started with the `--web` argument. It also listens on `0.0.0.0:9888` by default,
but an inteface can be specified with the `--interface` argument.

Go to `http://<ip>:9888` to view the UI. The WebSocket API can be found at `ws://<ip>:9888/ws`.

There is no security on the WebSocket API, however it will reject any messages that cannot be parsed into a player
action and it only interacts with the player. There is no reading or writing to the file system by the serve. All files are served from
within the binary.

For any new clients, the server will send a stream of messages that bootstrap the active state of the player.

### API Controls

To control the player through the WebSocket API, send it a message with the required action.

Example payloads:

Play:
```json
{ "play": null }
```
Pause:
```json
{ "pause": null }
```
Skip To Track:
```json
{ "skipTo": { "num": "<track index>"} }
```
For more options, see the [`Action`](hifirs/src/player/controls.rs#L7) enum.
