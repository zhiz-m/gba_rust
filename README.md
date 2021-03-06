# gba_rust

## Pre-requisites:
This emulator requires a copy of the GBA bios. It shouldn't be hard to find online, but for legal reasons it can't be included in this repo. Place the BIOS file at `extern/GBA/gba_bios.bin`.

## How to run:

`$ cargo run --release -- --help`

## Acknowledgements:

### Reference materials
- http://www.problemkaputt.de/gbatek.htm
- https://www.dwedit.org/files/ARM7TDMI.pdf
- https://www.intel.com/content/dam/www/programmable/us/en/pdfs/literature/third-party/archives/ddi0100e_arm_arm.pdf
- https://www.coranac.com/tonc/text/toc.htm
- http://belogic.com/gba/

### Test roms
- https://github.com/jsmolka/gba-tests/tree/3fc2dc019f91180585c7f71d1d68c271baa331fe
- https://github.com/shonumi/Emu-Docs/tree/master/GameBoy%20Advance/test_roms/arm_wrestler