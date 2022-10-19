# gba_rust

## Pre-requisites:
This emulator requires a copy of the GBA bios. It shouldn't be hard to find online, but for legal reasons it can't be included in this repo. Create an environment variable named `GBA_RUST_BIOS_PATH` with the path to the bios file. 

## How to run:

`$ cargo run --release -- --help`

Eg, to run a game on my local setup:

`cargo run --release -- -b 0 -o ..\..\Games\GBA\Pokemon_emerald.gba`

## Screenshots
![plot](./img/img1.png)

![plot](./img/img2.png)

![plot](./img/img3.png)

## Acknowledgements:

### Reference materials

#### General reference materials
- http://www.problemkaputt.de/gbatek.htm
- https://www.coranac.com/tonc/text/toc.htm

#### ARM7TDMI CPU
- https://www.dwedit.org/files/ARM7TDMI.pdf
- https://www.intel.com/content/dam/www/programmable/us/en/pdfs/literature/third-party/archives/ddi0100e_arm_arm.pdf

#### Audio processing unit
- http://belogic.com/gba/

### Test roms
- https://github.com/jsmolka/gba-tests/tree/3fc2dc019f91180585c7f71d1d68c271baa331fe
- https://github.com/shonumi/Emu-Docs/tree/master/GameBoy%20Advance/test_roms/arm_wrestler

## Notes

The code quality could be improved, but the emulation is fully functional and great effort has been made to optimize performance- hold down your spacebar to run the emulator at an uncapped framerate. Also, there is a WIP WebAssembly frontend in this repo which is not intended for general use just yet. 