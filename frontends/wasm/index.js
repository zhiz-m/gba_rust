wasm_bindgen().catch(console.error)
const { GbaWasm } = wasm_bindgen;
let bios_bin = null;
let rom_bin = null;
let save_bin = null;
let rom_name = null;
let gba = null;
let has_init = false;
let last_scheduled = null;
let ctx = null;

let fps_label = document.getElementById("fps_label");

let audio_ctx = null;
let audio_offset = null;

let keys = null;

configureFileInput("bios_input", (data) => { bios_bin = data; console.log("bios loaded") });
configureRomFileInput("rom_input", (data) => { rom_bin = data; console.log("rom loaded") });
configureFileInput("save_state_input", (data) => { save_bin = data; console.log("save loaded") });
initCanvas();
initResetButton();
initKeyInput();
initDownloadSaveButton();
// initSelectSaveSlotDropdown();

function downloadFile() {
    if (!rom_name || !gba || !has_init) return;

    let binaryData = gba.get_save_state();

    // Create a Blob object from the binary data
    const blob = new Blob([binaryData], { type: 'application/octet-stream' });

    // Create a URL for the Blob
    const blobUrl = URL.createObjectURL(blob);

    // Create a link element
    const downloadLink = document.createElement('a');
    downloadLink.href = blobUrl;

    // Specify the filename for the downloaded file
    downloadLink.download = rom_name + `-${Date.now()}` + ".rustsav"; // Change to your desired filename and extension

    // Trigger a click event on the link to initiate the download
    downloadLink.click();

    // Clean up by revoking the Blob URL (optional, but recommended)
    URL.revokeObjectURL(blobUrl);
}

function getSaveSlot() {
    let value = document.getElementById("save_slot").value;
    return parseInt(value);
}

function initDownloadSaveButton() {
    document.getElementById("download_save_button").addEventListener("click", () => {
        downloadFile();
    });;

}

function initKeyInput() {
    // Initialize keyboard input
    window.addEventListener("keydown", (e) => {
        handleKey(e.key, true, "keyboard");
        if (e.key == " " && e.target == document.body) {
            e.preventDefault();
        }
    });
    window.addEventListener("keyup", (e) => handleKey(e.key, false, "keyboard"));
}

// let keyState = {}

// todo: add input source disambiguation
function handleKey(key, is_pressed, source) {
    // console.log("handlekey", key, is_pressed);
    let num = mapKeyToNum(key);
    if (num === null) return;
    // if (keyState[key] !== null && keyState[key] === is_pressed) return;
    // if (keys && keyState[key] !== true) {

    // keys[num] == true means that we've set it to true during this frame. attempting to unset it is 
    // probably due to double input from both a controller and keyboard, so we ignore it.
    if (keys) {
        if (!keys[num] || (keys[num][0] === "keyboard" || keys[num][1] !== true)) {
            // keyState[key] = is_pressed;
            keys[num] = [source, is_pressed];
            // console.log(key, source, is_pressed, keys[num])
        }
    }
}

function mapKeyToNum(key) {
    if (key == "z") return 0;
    if (key == "x") return 1;
    if (key == "q") return 2;
    if (key == "w") return 3;
    if (key == "ArrowRight") return 4;
    if (key == "ArrowLeft") return 5;
    if (key == "ArrowUp") return 6;
    if (key == "ArrowDown") return 7;
    if (key == "s") return 8;
    if (key == "a") return 9;
    if (key == " ") return 10;
    if (key == "1") return 11;
    if (key == "2") return 12;
    if (key == "3") return 13;
    if (key == "4") return 14;
    if (key == "5") return 15;
    return null;
}


// Map gamepad buttons to key equivalents
function mapGamepadButtonToKey(buttonIndex) {
    const mapping = {
        0: "z",        // Cross
        1: "x",        // Circle
        2: "q",        // Square
        3: "w",        // Triangle
        // todo: fix d pad
        14: "ArrowLeft", // D-pad Left
        15: "ArrowRight", // D-pad Right
        12: "ArrowUp", // D-pad Up
        13: "ArrowDown", // D-pad Down
        6: " ",        // L2
        4: "a",        // L1
        7: " ",        // R2
        5: "s",        // R1
        8: "w",        // Share
        9: "q",        // Options
    };
    return mapping[buttonIndex] || null;
}

let gamepadLoop = null;

function pollGamepad() {
    const handle = (a, b) => handleKey(a, b, "controller")
    const gamepads = navigator.getGamepads();
    for (let i = 0; i < gamepads.length; i++) {
        const gamepad = gamepads[i];
        if (!gamepad) continue;

        gamepad.buttons.forEach((button, index) => {
            const key = mapGamepadButtonToKey(index);
            if (key) {
                handle(key, button.pressed);
            }
        });

        gamepad.axes.forEach((axis, index) => {
            if (index === 0) {
                // Left stick horizontal
                if (axis < -0.5) handle("ArrowLeft", true);
                else handle("ArrowLeft", false);
                if (axis > 0.5) handle("ArrowRight", true);
                else {
                    handle("ArrowRight", false);
                }
            } else if (index === 1) {
                // Left stick vertical
                if (axis < -0.5) handle("ArrowUp", true);
                else handle("ArrowUp", false);
                if (axis > 0.5) handle("ArrowDown", true);
                else {
                    handle("ArrowDown", false);
                }
            }
        });
    }
}

function modifyFpsLabel(fps) {
    if (fps != null) fps_label.innerHTML = `FPS: ${fps}`
}

function initResetButton() {
    document.getElementById("reset_button").addEventListener("click", () => {
        if (bios_bin != null && rom_bin != null) {
            if (last_scheduled != null) clearTimeout(last_scheduled);

            audio_ctx = new (window.AudioContext || window.webkitAudioContext)();
            gba = new GbaWasm(bios_bin, rom_bin, save_bin, getSaveSlot(), audio_ctx.sampleRate);
            has_init = false;

            scheduleGba(BigInt(0));
            console.log("GBA scheduled to start / restart");
        }
    });
}

function initCanvas() {
    let canvas = document.getElementById("gba_rust_canvas");
    canvas.width = 480;
    canvas.height = 320;
    ctx = canvas.getContext("2d");
}

function configureFileInput(id, callback) {
    let node = document.getElementById(id);
    node.addEventListener("change", (e) => {
        let file = e.target.files[0];
        var reader = new FileReader();
        reader.onload = (e) => {
            var data = reader.result;
            var array = new Uint8Array(data);
            callback(array);
        };
        reader.readAsArrayBuffer(file);
    });
}

function configureRomFileInput(id, callback) {
    let node = document.getElementById(id);
    node.addEventListener("change", (e) => {
        let file = e.target.files[0];
        rom_name = e.target.files[0].name.split(".")[0];
        var reader = new FileReader();
        reader.onload = (e) => {
            var data = reader.result;
            var array = new Uint8Array(data);
            callback(array);
        };
        reader.readAsArrayBuffer(file);
    });
}

// NOTE: a lot of the logic for audio playing in JavaScript came from https://github.com/michelhe/rustboyadvance-ng/blob/master/platform/rustboyadvance-wasm/app/index.js
function playAudio(audio_data) {
    if (audio_data == null || audio_data.length == 0) return;

    let cnt = audio_data.length / 2;
    const buf = audio_ctx.createBuffer(
        2,
        cnt,
        audio_ctx.sampleRate
    );

    for (let channel = 0; channel < 2; channel++) {
        let buffering = buf.getChannelData(channel);
        for (let i = 0; i < cnt; i++) {
            // audio data frames are interleaved
            buffering[i] = audio_data[i * 2 + channel];
        }
    }

    // const newaudioBuffer = (src && src.buffer)
    //     ? appendBuffer(source.buffer, audioBufferChunk, audioContext)
    //     : audioBufferChunk;

    let src = audio_ctx.createBufferSource();



    src.buffer = buf;

    src.connect(audio_ctx.destination);
    src.start(audio_offset);
    audio_offset += audio_data.length / 2 / audio_ctx.sampleRate;

    audio_offset = Math.max(audio_offset, audio_ctx.currentTime + 0.05);
    // console.log(`audiocontext time: ${audio_ctx.currentTime}\n`);
    // console.log(`offset: ${audio_offset}\n`);
}

function scheduleGba(time_micros) {
    let closure = () => {
        if (gba != null) {
            let time = BigInt(Date.now() * 1000);
            if (!has_init) {
                audio_offset = audio_ctx.currentTime + 0.05;
                gba.init(time);
                has_init = true;
                keys = {}
            }
            let micros = gba.process_frame(time);

            // video
            gba.display_picture(ctx);

            // audio
            playAudio(gba.get_audio_buffer());

            // fps
            modifyFpsLabel(gba.get_fps());

            gba.input_frame_preprocess();

            pollGamepad();
            for (const key in keys) {
                // console.log(`key send ${keys[i][0]} ${keys[i][1]}`);
                gba.key_input(key, keys[key][1]);
            }

            keys = {}

            scheduleGba(micros);
        }
    }

    last_scheduled = setTimeout(closure, Number(time_micros / BigInt(1000)));
}


