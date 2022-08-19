# hifi.rs

### A terminal-based (tui), high resolution audio player for the discerning listener.<super>\*</super>

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

As a linux user who enjoys the Qobuz music service, I was frustrated that there isn't more support for the OS. You can use [Squeezelite](https://github.com/ralph-irving/squeezelite), [Roon](https://roonlabs.com/) or similar apps to stream music from the service, but they typically either require a separate interface, have an ineffective interface, or require you to run a server which make it less than ideal to search and play music quickly while working.

## Features

- [GStreamer](https://gstreamer.freedesktop.org/)-backed player
- High resolution audio: Supports up to 24bit/192Khz (max quality Qobuz offers)
- MPRIS support (control via [playerctl](https://github.com/altdesktop/playerctl) or other D-Bus client)
- Resume previous session
- TUI can be disabled to use as a headless player controlled via MPRIS
- Search results are output to a pretty-printed table of pre-selected fields by default
- Results can be output in a few formats for your own scripting
  - `tabs`: tab-separated list of pre-selected fields, ideal for piping to `awk`, `cut`, etc.
  - `json`: entire json payload, ideal for `jq`, etc.

I've spent [![n hours](https://wakatime.com/badge/github/iamdb/hifi.rs.svg)](https://wakatime.com/badge/github/iamdb/hifi.rs) building this.

<super>\*</super>/s
