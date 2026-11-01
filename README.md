<div align="center">

<img src="logo.svg" alt="brazier" width="440">

<img src="demo.gif" alt="brazier flaring up as the CPU gets busy">

[Читать на русском](README.ru.md)

</div>

Every core gets its own patch of the fire and the flames grow with its load.
An idle machine just smolders, a compile makes it roar. The color follows
the CPU temperature, so a cold chip burns amber and a hot one goes white.

## Install

It's a single static binary, nothing to compile:

```sh
curl -fsSL https://github.com/Frad70/brazier/releases/latest/download/brazier-$(uname -m)-linux.tar.gz \
  | sudo tar -xz --no-same-owner -C /usr/local/bin brazier
```

There are builds for x86_64, aarch64 and armv7, and they run on any distro.
To uninstall, delete `/usr/local/bin/brazier`. Without root, unpack it into
`~/.local/bin` instead and make sure that directory is on your `PATH`.

If you have Rust, you can build it from source:

```sh
cargo install --git https://github.com/Frad70/brazier
```

## Usage

```
brazier [--total] [--fps N] [--info]
```

| Flag | Effect |
|---|---|
| `--total` | one fire for the whole CPU instead of one per core |
| `--fps N` | frame rate from 1 to 120, 30 by default |
| `--info` | start with the load and temperature shown in the corner |

While it burns, `i` toggles that readout, and `q`, Esc or Ctrl-C puts the
fire out.

Some places to keep it:

- a narrow tmux pane next to a long build, so you see it working
- the spare monitor of a home server
- a laptop with `--total --fps 15`, which is easier on the battery

## How it works

It's the fire from the PlayStation port of Doom. The bottom row is kept hot,
and every frame each pixel takes the heat of the one below it, a little
cooler and shifted sideways at random. That's the whole trick, and it
already looks like fire.

brazier only changes how fast the heat fades. Each column cools at a rate
set by the load of its core, so a busy core's flame climbs almost to the top
while an idle one stays a few rows high. The temperature decides how hot the
bottom row is, which is why a cool CPU never reaches white.

Load comes from `/proc/stat` and temperature from hwmon.

## Requirements

Linux and a terminal with truecolor. Cold pixels are left on the terminal's
own background, so a transparent terminal stays transparent.

For the temperature brazier takes the first sensor it finds in this order:
k10temp, zenpower, coretemp, cpu_thermal, acpitz. Without any of them the
fire still burns, just in one fixed color.

## Credits

The algorithm and the palette come from Fabien Sanglard's
[How DOOM fire was done](https://fabiensanglard.net/doom_fire_psx/).

## License

MIT
