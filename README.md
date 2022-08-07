# Hifi.rs

## A terminal-based (tui), high resolution audio player for the discerning listener.

Powered by [Qobuz](https://www.qobuz.com). Requires a paid subscription. This does not allow you to listen for free.

As a linux user with a subscription to Qobuz, I was frustrated that there isn't more support for the OS. You can use Squeezelite or Roon to stream music, but both require separate interfaces which make it less than ideal to search and play music quickly while working.

### Features

- GStreamer-backed player
- High resolution audio: Supports up to 24bit/192Khz
- MPRIS support (control via [playerctl](https://github.com/altdesktop/playerctl) or other D-Bus client)
- Resume previous session
- TUI can be disabled to use as a headless player
- Search results are output to a pretty-printed table of pre-selected fields by default
- Results can be output in a few formats for your own scripting
  - `tabs`: tab-separated list of pre-selected fields, ideal for piping to `awk`, `cut`, etc.
  - `json`: entire json payload, ideal for `jq`, etc.

I've spent [![n hours](https://wakatime.com/badge/user/a20519d3-9690-4af7-ad04-136d21595be5/project/aaa28014-a87d-40db-961a-0d1ab3b0f3ea.svg)](https://wakatime.com/badge/user/a20519d3-9690-4af7-ad04-136d21595be5/project/aaa28014-a87d-40db-961a-0d1ab3b0f3ea) building this.
