# nes-emulator

## Building

```
$ rustc --version
rustc 1.32.0 (9fda7c223 2019-01-16)
$ cargo --version
cargo 1.32.0 (8610973aa 2019-01-02)

$ cargo build --release
$ cargo run --release --bin nes-emulator
```

The emulator loads a ROM in iNES format located at the hardcoded path `roms/mario.nes`.

## Inputs

The emulator has been tested with an Xbox 360 controller, but should work with any controller the SDL library recognizes.

Additionally, these keyboard keys control the emulator:
* Escape: Exits the emulator
* Pause: (Developer use) Breaks a command-line debugger
* F5: Saves a savestate
* F6: Loads the most recent savestate
* F7: Restart the current ROM and playback a video of recorded inputs
