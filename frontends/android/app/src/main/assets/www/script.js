// ==================== CONFIG ====================
const { GbaWasm } = wasm_bindgen;

let bios_bin = null;
let rom_bin = null;
let save_bin = null;
let rom_name = null;
let gba = null;
let has_init = false;
let last_scheduled = null;
let ctx = null;
let keys = {};
let save_key_pressed_this_frame = false;

const fps_label = document.getElementById("fps_label");
let audio_ctx = null;
let audio_offset = null;

// ==================== LOCALSTORAGE HELPERS ====================
function arrayToBase64(uint8Array) {
    let binary = '';
    const len = uint8Array.byteLength;
    for (let i = 0; i < len; i++) binary += String.fromCharCode(uint8Array[i]);
    return btoa(binary);
}
function base64ToArray(base64) {
    const binary = atob(base64);
    const len = binary.length;
    const bytes = new Uint8Array(len);
    for (let i = 0; i < len; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
}
function saveToLocalStorage(rom_name, data) {
    if (!rom_name) return;
    try {
        const base64 = arrayToBase64(data);
        localStorage.setItem(`gba_save_${rom_name}`, base64);
        console.log(`Saved to localStorage for ${rom_name}`);
    } catch (e) { console.error('localStorage save failed', e); }
}
function loadFromLocalStorage(rom_name) {
    if (!rom_name) return null;
    const base64 = localStorage.getItem(`gba_save_${rom_name}`);
    if (base64) {
        try {
            console.log(`Loaded from localStorage for ${rom_name}`);
            return base64ToArray(base64);
        } catch (e) { console.error('localStorage load failed', e); }
    }
    return null;
}

// ==================== TOUCH INPUT ====================
function handleTouch(keyNum, pressed) {
    if (!keys) return;
    keys[keyNum] = ['touch', pressed];
}

document.querySelectorAll('[data-key]').forEach(btn => {
    const keyNum = parseInt(btn.getAttribute('data-key'), 10);
    btn.addEventListener('touchstart', (e) => {
        e.preventDefault();
        handleTouch(keyNum, true);
    });
    btn.addEventListener('touchend', (e) => {
        e.preventDefault();
        handleTouch(keyNum, false);
    });
    btn.addEventListener('touchcancel', (e) => {
        e.preventDefault();
        handleTouch(keyNum, false);
    });
    btn.addEventListener('mousedown', (e) => { e.preventDefault(); handleTouch(keyNum, true); });
    btn.addEventListener('mouseup', (e) => { e.preventDefault(); handleTouch(keyNum, false); });
    btn.addEventListener('mouseleave', (e) => { if (e.buttons) handleTouch(keyNum, false); });
});

// ==================== FILE INPUT HANDLING ====================
function configureFileInput(id, callback) {
    const node = document.getElementById(id);
    node.addEventListener('change', (e) => {
        const file = e.target.files[0];
        const reader = new FileReader();
        reader.onload = () => {
            const array = new Uint8Array(reader.result);
            callback(array);
        };
        reader.readAsArrayBuffer(file);
    });
}
function configureRomFileInput(id, callback) {
    const node = document.getElementById(id);
    node.addEventListener('change', (e) => {
        const file = e.target.files[0];
        rom_name = file.name.split('.')[0];
        const reader = new FileReader();
        reader.onload = () => {
            const array = new Uint8Array(reader.result);
            callback(array);
        };
        reader.readAsArrayBuffer(file);
    });
}

document.getElementById('load_bios_button').addEventListener('click', () => {
    document.getElementById('bios_input').click();
});
document.getElementById('load_rom_button').addEventListener('click', () => {
    document.getElementById('rom_input').click();
});
document.getElementById('load_save_button').addEventListener('click', () => {
    document.getElementById('save_state_input').click();
});

configureFileInput('bios_input', (data) => { bios_bin = data; console.log('bios loaded'); });
configureRomFileInput('rom_input', (data) => { rom_bin = data; console.log('rom loaded'); });
configureFileInput('save_state_input', (data) => {
    save_bin = data;
    console.log('save loaded');
    if (rom_name) saveToLocalStorage(rom_name, data);
});

// ==================== DOWNLOAD SAVE ====================
// Call this function with a Blob and desired filename
function downloadBlob(blob, filename) {
    var reader = new FileReader();
    reader.onload = function () {
        var base64DataUrl = reader.result; // e.g. "data:application/pdf;base64,JVBERi0..."
        Android.download(base64DataUrl, filename, blob.type);
    };
    reader.readAsDataURL(blob); // converts blob to base64 data URL
}

function downloadFile() {
    if (!rom_name || !gba || !has_init) return;
    const binaryData = gba.get_save_state();
    saveToLocalStorage(rom_name, binaryData);
    const blob = new Blob([binaryData], { type: 'application/octet-stream' });
    downloadBlob(blob, rom_name + `-${Date.now()}.rustsav`)
}
document.getElementById('download_save_button').addEventListener('click', downloadFile);

// ==================== RESET / INIT ====================
function getSaveSlot() {
    return parseInt(document.getElementById('save_slot').value, 10);
}
document.getElementById('reset_button').addEventListener('click', () => {
    if (bios_bin && rom_bin) {
        if (save_bin === null && rom_name) {
            save_bin = loadFromLocalStorage(rom_name);
        }
        if (last_scheduled) clearTimeout(last_scheduled);
        audio_ctx = new (window.AudioContext || window.webkitAudioContext)();
        gba = new GbaWasm(bios_bin, rom_bin, save_bin, getSaveSlot(), audio_ctx.sampleRate);
        has_init = false;
        spacePressed = false;
        scheduleGba(BigInt(0));
        console.log('GBA started');
    }
});

// ==================== CANVAS ====================
function initCanvas() {
    const canvas = document.getElementById('gba_rust_canvas');
    canvas.width = 480;
    canvas.height = 320;
    ctx = canvas.getContext('2d');
}
initCanvas();

// ==================== AUDIO ====================
function playAudio(audio_data) {
    if (!audio_data || audio_data.length === 0) return;
    const cnt = audio_data.length / 2;
    const buf = audio_ctx.createBuffer(2, cnt, audio_ctx.sampleRate);
    for (let ch = 0; ch < 2; ch++) {
        const channel = buf.getChannelData(ch);
        for (let i = 0; i < cnt; i++) {
            channel[i] = audio_data[i * 2 + ch];
        }
    }
    const src = audio_ctx.createBufferSource();
    src.buffer = buf;
    src.connect(audio_ctx.destination);
    src.start(audio_offset);
    audio_offset += cnt / audio_ctx.sampleRate;
    audio_offset = Math.max(audio_offset, audio_ctx.currentTime + 0.05);
}

// ==================== MAIN LOOP ====================
function scheduleGba(time_micros) {
    const closure = () => {
        if (!gba) return;
        const time = BigInt(Date.now() * 1000);
        if (!has_init) {
            audio_offset = audio_ctx.currentTime + 0.05;
            gba.init(time);
            has_init = true;
            keys = {};
            save_key_pressed_this_frame = false;
        }
        const micros = gba.process_frame(time);
        gba.display_picture(ctx);
        playAudio(gba.get_audio_buffer());
        modifyFpsLabel(gba.get_fps());
        gba.input_frame_preprocess();

        if (save_key_pressed_this_frame && rom_name) {
            const saveData = gba.get_save_state();
            saveToLocalStorage(rom_name, saveData);
            console.log('Auto‑saved to localStorage');
            save_key_pressed_this_frame = false;
        }

        for (const key in keys) {
            // MODIFIED: if a save key (1-5) is pressed, set the flag (ignoring releases)
            if (key >= 11 && key <= 15 && keys[key][1]) {
                save_key_pressed_this_frame = true;
            }
            gba.key_input(key, keys[key][1]);
        }


        keys = {};
        scheduleGba(micros);
    };
    last_scheduled = setTimeout(closure, Number(time_micros / BigInt(1000)));
}

function modifyFpsLabel(fps) {
    if (fps != null) fps_label.textContent = `FPS: ${fps}`;
}

// ==================== INIT ====================
wasm_bindgen().catch(console.error);

// Menu dialog handling
const menuButton = document.querySelector('.menu-button');
const menuDialog = document.querySelector('.menu-dialog');
const closeDialog = document.querySelector('.close-dialog');

menuButton.addEventListener('click', () => {
    menuDialog.classList.remove('hidden');
});

closeDialog.addEventListener('click', () => {
    menuDialog.classList.add('hidden');
});

// Close when clicking outside the dialog content
menuDialog.addEventListener('click', (e) => {
    if (e.target === menuDialog) {
        menuDialog.classList.add('hidden');
    }
});

// Commit save button: sends a quick press+release of the corresponding save slot key (11-15)
document.getElementById('commit_save_button').addEventListener('click', (e) => {
    e.preventDefault();

    const slot = getSaveSlot(); // 0-4
    // Map slot to key index: slot 0 -> key 11, slot 1 -> 12, etc.
    const keyIndex = 11 + slot;

    if (!keys) {
        console.warn('Keys object not ready');
        return;
    }

    // Simulate key down and up in the same frame (or consecutive frames)
    // We'll set down now, and schedule up after a short delay.
    keys[keyIndex] = ['menu', true]; // down

    // Schedule key up after ~50ms to mimic a button press
    setTimeout(() => {
        if (keys) {
            keys[keyIndex] = ['menu', false]; // up
        }
    }, 50);

    // Optionally close the menu dialog after commit
    menuDialog.classList.add('hidden');

    // Also trigger the internal save-to-localStorage (like original save key)
    // But the emulator will handle saving when it receives the key press.
    // However, the auto-save flag is only set for actual button presses.
    // We can manually set the flag or call saveToLocalStorage after a short delay.
    // For consistency, we'll let the emulator process the key and rely on the auto-save
    // that happens when a save key is pressed. But our synthetic press may not set the flag.
    // We'll manually set the flag to ensure auto-save occurs.
    save_key_pressed_this_frame = true;
});

// Space toggle button (key index 10)
const spaceToggle = document.getElementById('space-toggle');
let spacePressed = false;

spaceToggle.addEventListener('click', (e) => {
    e.preventDefault();
    if (!has_init) return;
    spacePressed = !spacePressed;
    // Send the current state to the emulator via keys object
    if (keys) {
        keys[10] = ['toggle', spacePressed];
    }
    // Visual feedback
    if (spacePressed) {
        spaceToggle.classList.add('active');
    } else {
        spaceToggle.classList.remove('active');
    }
});
