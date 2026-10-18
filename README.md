# herdr-process

A [herdr](https://herdr.dev) plugin that keeps one long-lived process per profile and shows it in a
herdr pane on demand. Closing, splitting or floating the pane leaves the process running.

## Install

`herdr plugin install` cannot install this plugin. Build and link it:

```bash
git clone https://github.com/webdavis/herdr-process.git ~/.local/share/herdr-process
cd ~/.local/share/herdr-process
cargo build --release --locked
./target/release/herdr-process generate --output .
herdr plugin link ~/.local/share/herdr-process
```

`generate` writes `herdr-plugin.toml` from your profiles and your herdr config. Run it and link
again after either changes.

## Profiles

Each `[windows.<profile>]` table in `~/.config/herdr/processes.toml` is one profile:

```toml
[windows.editor]
program = "nvim"
args = ["--clean"]
cwd = "~/projects"
width = "80%"
height = "70%"
ctrl_c = "hide"
```

`args` is optional. `cwd` is absolute or starts with `~/`. `width` and `height` size the float,
from 1% to 100%. `ctrl_c` is `hide` to hide the float or `quit` to kill the process.

## Bindings

Each profile has four actions: `split-right`, `split-below`, `toggle-float` and `kill`. Bind them in
`~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+e"
type = "plugin_action"
command = "herdr-process.editor:toggle-float"
```

## License

MIT
