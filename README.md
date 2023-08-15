# hifi.rs

### A terminal-based (tui), high resolution audio player backed by Qobuz

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

Qobuz only supports Linux through the browser and has no officially supported API. There are ways of accessing Qobuz in Linux outside of the browser through third-party applications like Squeezelite and Roon. These apps are great, but I wanted something simpler that just focused on being able to quickly find and play an album inside the console.

<img width="50%" src="hifi-rs.png" alt="screenshot" />

## Player Features

- Low resource usage
- [GStreamer](https://gstreamer.freedesktop.org/)-backed player, [SQLite](https://www.sqlite.org/index.html) database
- High resolution audio: Supports up to 24bit/192Khz (max quality Qobuz offers)
- MPRIS support (control via [playerctl](https://github.com/altdesktop/playerctl) or other D-Bus client)
- Gapless playback
- Resume last session
- TUI can be disabled to use as a headless player, controlled via MPRIS

In addition to the player, there is a Spotify to Qobuz playlist sync tool and an incomplete Rust library for the Qobuz API.

## Installation

Download tar from releases page, extract the tar and execute it or copy it to the your $PATH.

## Requirements

- [GStreamer v1.20+](https://gstreamer.freedesktop.org/documentation/installing/index.html) (should come with most/all current Linux and MacOS versions)

## Get started

Run `hifi-rs --help` or `hifi-rs <subcommand> --help` to see all available options.

To get started:

```shell
hifi-rs config username # enter username at prompt
hifi-rs config password # enter password at prompt
hifi-rs config default-quality # enter quality at prompt (mp3, cd, hifi96 or hifi192)

# play from the command line
hifi-rs play --url <Qobuz Album, Playlist or Track URL>

# open player
hifi-rs open
```

The TUI has full mouse support.

## Keyboard Shortcuts

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
| Move up in list     | <kbd>up arrow</kbd>                           |
| Move down in list   | <kbd>down arrow</kbd>                           |
| Select item in list | <kbd>enter</kbd>                       |
| Dismiss popup       | <kbd>esc</kbd>                         |

## Known Issues

- UI will freeze during loading of long lists and then works fine. The issue is there is no feedback alerting the user that something is happening in the background and signifying it is normal behavior. Probably best solved when switching to Cursive.

## Todo

- Sortable lists
