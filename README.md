# hifi.rs

### A terminal-based (tui), high resolution audio player for the discerning listener.<sup>\*</sup>

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

Qobuz only officially supports Linux through the browser. There are ways of accessing Qobuz in Linux outside of the browser through third-party applications like Squeezelite and Roon. These apps are great, but I wanted something simpler that just focused on playing music from Qobuz that I could play in a terminal or as a background service.

## Features

- [GStreamer](https://gstreamer.freedesktop.org/)-backed player
- High resolution audio: Supports up to 24bit/192Khz (max quality Qobuz offers)
- MPRIS support (control via [playerctl](https://github.com/altdesktop/playerctl) or other D-Bus client)
- Resume previous session
- TUI can be disabled to use as a headless player controlled via MPRIS

I've spent [![n hours](https://wakatime.com/badge/github/iamdb/hifi.rs.svg)](https://wakatime.com/badge/github/iamdb/hifi.rs) building this.
