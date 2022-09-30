# hifi.rs

### A terminal-based (tui), high resolution audio player for the discerning listener.<sup>\*</sup>

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

Qobuz only officially supports Linux through the browser. There are ways of accessing Qobuz in Linux outside of the browser through third-party applications like Squeezelite and Roon. These apps are great<sup>\*</sup>, but I wanted something simpler that just focused on playing music from Qobuz that I could play in a terminal or as a background service.

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

<sup>\*</sup>Squeezelite is less refined, but free and has been well tested. Roon is more refined, but younger and requires you to pay to host the app on your own hardware or buy expensive hardware to host the server.

<sup>\*</sup>/s
