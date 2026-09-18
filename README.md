# herdr-process

[herdr](https://herdr.dev)-owned windows for long-lived processes. A profile names a program, and
the plugin keeps one instance of it alive, attaches a herdr pane to it on demand, and lets that pane
be closed, split or floated without the program noticing. The process outlives the window.

Each profile gets its own set of actions (`<profile>:toggle-float`, `<profile>:split-right`,
`<profile>:split-below`, `<profile>:kill`), bound with `type = "plugin_action"` keybindings.

## Install

**This plugin ships no `herdr-plugin.toml`, so `herdr plugin install` cannot install it.** The
actions are one set per declared profile, so the manifest is a function of your configuration rather
than a file that can be committed once for everybody. It is built and linked instead:

```bash
git clone https://github.com/webdavis/herdr-process.git ~/.local/share/herdr-process
cd ~/.local/share/herdr-process
cargo build --release --locked
./target/release/herdr-process generate --output .   # renders herdr-plugin.toml from your config
herdr plugin link ~/.local/share/herdr-process
```

`generate` reads `~/.config/herdr/processes.toml` for the profiles and `~/.config/herdr/config.toml`
for the chords. Re-run it, and re-link, whenever either changes; the manifest is build output, not
source, and is deliberately not committed.

Whether a generated manifest can ever be a `herdr plugin install` is an open design question here:
it needs either a committed manifest that herdr can rewrite, or runtime action registration, which
herdr plugin v1 does not have.

## License

MIT
