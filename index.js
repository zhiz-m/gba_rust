!function(e){function t(t){for(var n,o,i=t[0],l=t[1],u=0,a=[];u<i.length;u++)o=i[u],Object.prototype.hasOwnProperty.call(r,o)&&r[o]&&a.push(r[o][0]),r[o]=0;for(n in l)Object.prototype.hasOwnProperty.call(l,n)&&(e[n]=l[n]);for(f&&f(t);a.length;)a.shift()()}var n={},r={0:0};var o={};var i={4:function(){return{"./index_bg.js":{__wbg_length_72e2208bbc0efc61:function(e){return n[2].exports.c(e)},__wbindgen_memory:function(){return n[2].exports.l()},__wbg_buffer_085ec1f694018c4f:function(e){return n[2].exports.b(e)},__wbg_new_8125e318e6245eed:function(e){return n[2].exports.d(e)},__wbindgen_object_drop_ref:function(e){return n[2].exports.m(e)},__wbg_set_5cf90238115182c3:function(e,t,r){return n[2].exports.j(e,t,r)},__wbindgen_string_new:function(e,t){return n[2].exports.n(e,t)},__wbg_newwithu8clampedarrayandsh_e2b3fce567acd708:function(e,t,r,o){return n[2].exports.h(e,t,r,o)},__wbg_putImageData_f157d52a70a206d5:function(e,t,r,o){return n[2].exports.i(e,t,r,o)},__wbg_newwithbyteoffsetandlength_69193e31c844b792:function(e,t,r){return n[2].exports.f(e,t,r)},__wbg_new_d086a66d1c264b3f:function(e){return n[2].exports.e(e)},__wbg_newwithbyteoffsetandlength_6da8e527659b86aa:function(e,t,r){return n[2].exports.g(e,t,r)},__wbindgen_throw:function(e,t){return n[2].exports.o(e,t)}}}}};function l(t){if(n[t])return n[t].exports;var r=n[t]={i:t,l:!1,exports:{}};return e[t].call(r.exports,r,r.exports,l),r.l=!0,r.exports}l.e=function(e){var t=[],n=r[e];if(0!==n)if(n)t.push(n[2]);else{var u=new Promise((function(t,o){n=r[e]=[t,o]}));t.push(n[2]=u);var a,s=document.createElement("script");s.charset="utf-8",s.timeout=120,l.nc&&s.setAttribute("nonce",l.nc),s.src=function(e){return l.p+""+({}[e]||e)+".js"}(e);var f=new Error;a=function(t){s.onerror=s.onload=null,clearTimeout(c);var n=r[e];if(0!==n){if(n){var o=t&&("load"===t.type?"missing":t.type),i=t&&t.target&&t.target.src;f.message="Loading chunk "+e+" failed.\n("+o+": "+i+")",f.name="ChunkLoadError",f.type=o,f.request=i,n[1](f)}r[e]=void 0}};var c=setTimeout((function(){a({type:"timeout",target:s})}),12e4);s.onerror=s.onload=a,document.head.appendChild(s)}return({1:[4]}[e]||[]).forEach((function(e){var n=o[e];if(n)t.push(n);else{var r,u=i[e](),a=fetch(l.p+""+{4:"0e75c44361fee0787a3d"}[e]+".module.wasm");if(u instanceof Promise&&"function"==typeof WebAssembly.compileStreaming)r=Promise.all([WebAssembly.compileStreaming(a),u]).then((function(e){return WebAssembly.instantiate(e[0],e[1])}));else if("function"==typeof WebAssembly.instantiateStreaming)r=WebAssembly.instantiateStreaming(a,u);else{r=a.then((function(e){return e.arrayBuffer()})).then((function(e){return WebAssembly.instantiate(e,u)}))}t.push(o[e]=r.then((function(t){return l.w[e]=(t.instance||t).exports})))}})),Promise.all(t)},l.m=e,l.c=n,l.d=function(e,t,n){l.o(e,t)||Object.defineProperty(e,t,{enumerable:!0,get:n})},l.r=function(e){"undefined"!=typeof Symbol&&Symbol.toStringTag&&Object.defineProperty(e,Symbol.toStringTag,{value:"Module"}),Object.defineProperty(e,"__esModule",{value:!0})},l.t=function(e,t){if(1&t&&(e=l(e)),8&t)return e;if(4&t&&"object"==typeof e&&e&&e.__esModule)return e;var n=Object.create(null);if(l.r(n),Object.defineProperty(n,"default",{enumerable:!0,value:e}),2&t&&"string"!=typeof e)for(var r in e)l.d(n,r,function(t){return e[t]}.bind(null,r));return n},l.n=function(e){var t=e&&e.__esModule?function(){return e.default}:function(){return e};return l.d(t,"a",t),t},l.o=function(e,t){return Object.prototype.hasOwnProperty.call(e,t)},l.p="",l.oe=function(e){throw console.error(e),e},l.w={};var u=window.webpackJsonp=window.webpackJsonp||[],a=u.push.bind(u);u.push=t,u=u.slice();for(var s=0;s<u.length;s++)t(u[s]);var f=a;l(l.s=0)}([function(e,t,n){n.e(1).then(n.bind(null,1)).then(e=>{let t=null,n=null,r=null,o=null,i=null,l=!1,u=null,a=null,s=document.getElementById("fps_label"),f=null,c=null,d=null;var p,_;function b(e,t){let n=null;if("z"==e)n=0;else if("x"==e)n=1;else if("q"==e)n=2;else if("w"==e)n=3;else if("ArrowRight"==e)n=4;else if("ArrowLeft"==e)n=5;else if("ArrowUp"==e)n=6;else if("ArrowDown"==e)n=7;else if("s"==e)n=8;else if("a"==e)n=9;else if(" "==e)n=10;else if("1"==e)n=11;else if("2"==e)n=12;else if("3"==e)n=13;else if("4"==e)n=14;else{if("5"!=e)return;n=15}d&&d.push([n,t])}function m(e,t){document.getElementById(e).addEventListener("change",e=>{let n=e.target.files[0];var r=new FileReader;r.onload=e=>{var n=r.result,o=new Uint8Array(n);t(o)},r.readAsArrayBuffer(n)})}m("bios_input",e=>{t=e,console.log("bios loaded")}),p="rom_input",_=e=>{n=e,console.log("rom loaded")},document.getElementById(p).addEventListener("change",e=>{let t=e.target.files[0];o=e.target.files[0].name.split(".")[0];var n=new FileReader;n.onload=e=>{var t=n.result,r=new Uint8Array(t);_(r)},n.readAsArrayBuffer(t)}),m("save_state_input",e=>{r=e,console.log("save loaded")}),function(){let e=document.getElementById("gba_rust_canvas");e.width=480,e.height=320,a=e.getContext("2d")}(),document.getElementById("reset_button").addEventListener("click",()=>{null!=t&&null!=n&&(null!=u&&clearTimeout(u),f=new(window.AudioContext||window.webkitAudioContext),i=new e.GbaWasm(t,n,r,function(){let e=document.getElementById("save_slot").value;return parseInt(e)}(),f.sampleRate),l=!1,function e(t){u=setTimeout(()=>{if(null!=i){let n=BigInt(1e3*Date.now());l||(c=f.currentTime+.05,i.init(n),l=!0,d=[]);let r=i.process_frame(n);i.display_picture(a),function(e){if(null==e||0==e.length)return;let t=e.length/2;const n=f.createBuffer(2,t,f.sampleRate);for(let r=0;r<2;r++){let o=n.getChannelData(r);for(let n=0;n<t;n++)o[n]=e[2*n+r]}let r=f.createBufferSource();r.buffer=n,r.connect(f.destination),r.start(c),c+=e.length/2/f.sampleRate,c=Math.max(c,f.currentTime+.05)}(i.get_audio_buffer()),null!=(t=i.get_fps())&&(s.innerHTML="FPS: "+t),i.input_frame_preprocess();for(let e=0;e<d.length;e++)i.key_input(d[e][0],d[e][1]);d=[],e(r)}var t},Number(t/BigInt(1e3)))}(BigInt(0)),console.log("GBA scheduled to start / restart"))}),window.addEventListener("keydown",e=>{b(e.key,!0)," "==e.key&&e.target==document.body&&e.preventDefault()}),window.addEventListener("keyup",e=>b(e.key,!1)),document.getElementById("download_save_button").addEventListener("click",()=>{!function(){if(!o||!i||!l)return;let e=i.get_save_state();const t=new Blob([e],{type:"application/octet-stream"}),n=URL.createObjectURL(t),r=document.createElement("a");r.href=n,r.download=o+"-"+Date.now()+".rustsav",r.click(),URL.revokeObjectURL(n)}()})})}]);