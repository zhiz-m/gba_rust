import("../pkg/index.js").then((rust_wasm) => {
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
    
    configureFileInput("bios_input", (data) => {bios_bin = data; console.log("bios loaded")});
    configureRomFileInput("rom_input", (data) => {rom_bin = data; console.log("rom loaded")});
    configureFileInput("save_state_input", (data) => {save_bin = data; console.log("save loaded")});
    initCanvas();
    initResetButton();
    initKeyInput();
    initDownloadSaveButton();
    // initSelectSaveSlotDropdown();

    function downloadFile(){
        if (!rom_name || !gba || !has_init) return;

        let binaryData  = gba.get_save_state();
        
        // Create a Blob object from the binary data
        const blob = new Blob([binaryData], { type: 'application/octet-stream' });

        // Create a URL for the Blob
        const blobUrl = URL.createObjectURL(blob);

        // Create a link element
        const downloadLink = document.createElement('a');
        downloadLink.href = blobUrl;

        // Specify the filename for the downloaded file
        downloadLink.download = rom_name + `-${Date.now()}` + ".rustsav" ; // Change to your desired filename and extension

        // Trigger a click event on the link to initiate the download
        downloadLink.click();

        // Clean up by revoking the Blob URL (optional, but recommended)
        URL.revokeObjectURL(blobUrl);
    }

    function getSaveSlot(){
        let value = document.getElementById("save_slot").value;
        return parseInt(value);
    }

    function initDownloadSaveButton(){
        document.getElementById("download_save_button").addEventListener("click", () => {
            downloadFile();
        });;

    }

    function initKeyInput(){
        window.addEventListener("keydown", (e) => {
            handleKey(e.key, true);
            if(e.key == " " && e.target == document.body) {
                e.preventDefault();
            }
        });
        window.addEventListener("keyup", (e) => handleKey(e.key, false));
    }

    function handleKey(key, is_pressed){
        let num = null;
        if (key == "z") num = 0;
        else if (key == "x") num = 1;
        else if (key == "q") num = 2;
        else if (key == "w") num = 3;
        else if (key == "ArrowRight") num = 4;
        else if (key == "ArrowLeft") num = 5;
        else if (key == "ArrowUp") num = 6;
        else if (key == "ArrowDown") num = 7;
        else if (key == "s") num = 8;
        else if (key == "a") num = 9;
        else if (key == " ") num = 10;
        else if (key == "1") num = 11;
        else if (key == "2") num = 12;
        else if (key == "3") num = 13;
        else if (key == "4") num = 14;
        else if (key == "5") num = 15;
        else return;

        // console.log(`key press: ${key}, ${num}, ${is_pressed}`);
        
        if(keys) keys.push([num, is_pressed]);
    }

    function modifyFpsLabel(fps){
        if (fps != null) fps_label.innerHTML = `FPS: ${fps}`
    }

    function initResetButton(){
        document.getElementById("reset_button").addEventListener("click", () => {
            if (bios_bin != null && rom_bin != null){
                if (last_scheduled != null) clearTimeout(last_scheduled);
                
                audio_ctx = new (window.AudioContext || window.webkitAudioContext)();
                gba = new rust_wasm.GbaWasm(bios_bin, rom_bin, save_bin, getSaveSlot(), audio_ctx.sampleRate);
                has_init = false;

                scheduleGba(BigInt(0));
                console.log("GBA scheduled to start / restart");
            }
        });
    }

    function initCanvas(){
        let canvas = document.getElementById("gba_rust_canvas");
        canvas.width = 480;
        canvas.height = 320;
        ctx = canvas.getContext("2d");
    }

    function configureFileInput(id, callback){
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

    function configureRomFileInput(id, callback){
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
    function playAudio(audio_data){
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
                buffering[i] = audio_data[i*2 + channel];
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

    function scheduleGba(time_micros){
        let closure = () => {
            if (gba != null){
                let time = BigInt(Date.now() * 1000);
                if (!has_init){
                    audio_offset = audio_ctx.currentTime + 0.05;
                    gba.init(time);
                    has_init = true;
                    keys = []
                }
                let micros = gba.process_frame(time);

                // video
                gba.display_picture(ctx);

                // audio
                playAudio(gba.get_audio_buffer());

                // fps
                modifyFpsLabel(gba.get_fps());

                gba.input_frame_preprocess();

                for (let i = 0; i < keys.length; i++) {
                    // console.log(`key send ${keys[i][0]} ${keys[i][1]}`);
                    gba.key_input(keys[i][0], keys[i][1]);
                }

                keys = []

                scheduleGba(micros);
            }
        }

        last_scheduled = setTimeout(closure, Number(time_micros / BigInt(1000)));
    }
});


