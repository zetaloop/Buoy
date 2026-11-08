# Buoy

Buoy shows live gateway status on a 128×64 SSD130x OLED display: upload, download and daily traffic on the default-route interface, a mirrored rate chart, and CPU, memory, temperature and fan readings.

The display needs the kernel `ssd130x` driver, which exposes it as a framebuffer named `ssd130xdrmfb`. Buoy draws on that framebuffer and switches `tty1` to graphics mode while it runs; the console comes back when it exits. Fan duty is read from [argononed](https://gitlab.com/DarkElvenAngel/argononed) and shows `-` when unavailable.

## Installation

Download the x86_64 or aarch64 Linux archive from [Releases](https://github.com/zetaloop/Buoy/releases) and install the binary:

```sh
sudo install -m755 buoy /usr/local/bin/buoy
```

Run it as root after `getty@tty1.service` so the login prompt does not reset the display.

## Building

```sh
cargo build --release
```
