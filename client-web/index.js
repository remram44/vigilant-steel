/*
 * WebGL setup
 */

var vertex_shader = `
// Position and scale of the camera
uniform vec4 uCamera;
// Position and orientation of the object
uniform vec2 uPosition;
uniform vec2 uBase;
// Vertex coordinate
attribute vec2 aPosition;
// Color
attribute vec4 aColor;
varying highp vec4 vColor;

void main() {
  vColor = aColor;
  vec2 pos = aPosition;
  pos = vec2(
    pos.x * uBase.x - pos.y * uBase.y,
    pos.x * uBase.y + pos.y * uBase.x
  );
  pos += uPosition;
  pos = vec2((pos.x - uCamera.x) * uCamera.z, (pos.y - uCamera.y) * uCamera.w);
  gl_Position = vec4(pos, 0.0, 1.0);
}
`;
var fragment_shader = `
#ifdef GL_ES
precision highp float;
#endif

uniform vec4 uColor;
varying vec4 vColor;

void main() {
  gl_FragColor = uColor * vColor;
}
`;

var cv = document.getElementById('canvas');
var gl = cv.getContext('webgl', { alpha: false });

function compileShader(source, type) {
  var shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if(!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error("Error linking shaders\n" + gl.getShaderInfoLog(shader));
  }
  return shader;
}

var shaderProgram = gl.createProgram();
gl.attachShader(shaderProgram, compileShader(vertex_shader, gl.VERTEX_SHADER));
gl.attachShader(shaderProgram, compileShader(fragment_shader, gl.FRAGMENT_SHADER));
gl.linkProgram(shaderProgram);
if(!gl.getProgramParameter(shaderProgram, gl.LINK_STATUS)) {
  throw new Error("Error linking shaders\n" + gl.getProgramInfoLog(shaderProgram));
}

var uCamera = gl.getUniformLocation(shaderProgram, 'uCamera');
var uPosition = gl.getUniformLocation(shaderProgram, 'uPosition');
var uBase = gl.getUniformLocation(shaderProgram, 'uBase');
var uColor = gl.getUniformLocation(shaderProgram, 'uColor');
var aPosition = gl.getAttribLocation(shaderProgram, 'aPosition');
var aColor = gl.getAttribLocation(shaderProgram, 'aColor');

var buffers = {};


/*
 * Sound setup
 */
function Sound(playFunc) {
  this.playFunc = playFunc;
}
Sound.prototype.start = function() {
  if(this.sound) {
    this.sound.stop();
  }
  this.sound = this.playFunc();
}
Sound.prototype.stop = function() {
  if(this.sound) {
    this.sound.stop();
  }
  this.sound = undefined;
}

var audio = new AudioContext();
var sndNoise = (function() {
  var bufferSize = audio.sampleRate * 0.2;
  var buffer = audio.createBuffer(1, bufferSize, audio.sampleRate);
  var data = buffer.getChannelData(0);
  for(var i = 0; i < bufferSize; ++i) {
    data[i] = Math.random() * 2.0 - 1.0;
  }

  return new Sound(function() {
    var sound = audio.createBufferSource();
    sound.buffer = buffer;
    sound.loop = true;

    var filter = audio.createBiquadFilter();
    filter.type = 'bandpass';
    filter.frequency.value = 100.0;

    sound.connect(filter).connect(audio.destination);
    sound.start();
    return sound;
  });
})();

function loadSound(name) {
  return fetch('assets/' + name).then(function(response) {
    if(response.status !== 200) {
      throw new Error("HTTP status ", response.status);
    }
    return response.arrayBuffer();
  }).then(function(buffer) {
    return audio.decodeAudioData(buffer);
  }).then(function(buffer) {
    console.log("Loaded sound ", name);
    return new Sound(function() {
      var sound = audio.createBufferSource();
      sound.buffer = buffer;
      sound.loop = false;

      sound.connect(audio.destination);
      sound.start();
      return sound;
    });
  }).catch(function(err) {
    console.error("Error loading sound ", name, ": ", err);
  });
}

var sndLaser = sndNoise;
loadSound('laser.wav').then(function(snd) {
  sndLaser = snd;
});

/*
 * Input
 */
var input = { x: 0.0, y: 0.0, r: 0.0, fire: false, mouse: [100, 100] };
function kbInput(evt, down) {
  if(down && evt.repeat) {
    return;
  }
  if(evt.code === 'KeyS') {
    input.x = down ? -1.0 : 0.0;
  } else if(evt.code === 'KeyW') {
    input.x = down ? 1.0 : 0.0;
  } else if(evt.code === 'KeyQ') {
    input.y = down ? 1.0 : 0.0;
  } else if(evt.code === 'KeyE') {
    input.y = down ? -1.0 : 0.0;
  } else if(evt.code === 'KeyA') {
    input.r = down ? 1.0 : 0.0;
  } else if(evt.code === 'KeyD') {
    input.r = down ? -1.0 : 0.0;
  } else if(evt.code === 'Space') {
    input.fire = down;
  }
}
document.addEventListener('keydown', function(e) { kbInput(e, true); });
document.addEventListener('keyup', function(e) { kbInput(e, false); });
document.addEventListener('mousemove', function(evt) {
  input.mouse = [
    evt.clientX,
    evt.clientY,
  ];
});


/*
 * Game loop
 */
var lastFrame = undefined;

var fpsTime = 0.0;
var fpsFrames = 0;
var fpsCounter = document.getElementById('fps');
var buffers_set = 0;

function render(now) {
  // Compute delta and FPS
  fpsFrames += 1;
  if(now - fpsTime > 1000.0) {
    fpsCounter.innerText = fpsFrames;
    fpsTime = now;
    fpsFrames = 0;
    console.log(
      "set ", buffers_set, " buffers last second, ",
      Object.getOwnPropertyNames(buffers).length, " buffers exist",
    );
    buffers_set = 0;
  }
  if(lastFrame === undefined) {
    delta = 0.02;
    lastFrame = now;
  } else {
    delta = (now - lastFrame) / 1000.0;
    lastFrame = now;
  }

  // Set up rendering
  cv.width = cv.clientWidth;
  cv.height = cv.clientHeight;
  gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
  gl.colorMask(true, true, true, true);
  gl.clearColor(0.0, 0.0, 0.0, 1.0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  gl.useProgram(shaderProgram);

  // Call WebAssembly
  _wasm_instance.update(
    delta, gl.drawingBufferWidth, gl.drawingBufferHeight,
    input.x, input.y, input.r, input.fire,
    input.mouse[0], input.mouse[1],
  );

  // Reset alpha
  gl.colorMask(false, false, false, true);
  gl.clearColor(0, 0, 0, 1);
  gl.clear(gl.COLOR_BUFFER_BIT);

  // Call again
  requestAnimationFrame(render);
}


/*
 * WebAssembly setup
 */

// Keep a reference for callbacks
var _wasm_instance;

// Log a string
function log_str(s) {
  console.log(s);
}

// Set camera
function set_camera(pos_x, pos_y, scale_x, scale_y) {
  gl.uniform4fv(uCamera, [pos_x, pos_y, scale_x, scale_y]);
}

// Set buffers from WebAssembly
function set_buffer(id, vertex_array, color_array, mode) {
  id = '' + id;
  var buffer;
  if(id in buffers) {
    buffer = buffers[id];
  } else {
    buffer = buffers[id] = { vertex: gl.createBuffer(), color: gl.createBuffer() };
  }
  if(mode === 0) {
    mode = gl.STATIC_DRAW;
  } else if (mode === 1) {
    mode = gl.DYNAMIC_DRAW;
  } else {
    mode = gl.STREAM_DRAW;
  }
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer.vertex);
  gl.bufferData(gl.ARRAY_BUFFER, vertex_array, mode);
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer.color);
  gl.bufferData(gl.ARRAY_BUFFER, color_array, mode);
  if(vertex_array.length * 4 != color_array.length * 2) {
    console.warn("Mismatched vertex/color buffer lengths: ", vertex_array.length, " ", color_array.length);
  }
  buffer.length = vertex_array.length / 2;
  buffers_set += 1;
}
function del_buffer(id) {
  id = '' + id;
  if(id in buffers) {
    var buffer = buffers[id];
    gl.deleteBuffer(buffer.vertex);
    gl.deleteBuffer(buffer.color);
    delete buffers[id];
    buffers_set += 1;
  }
}

// Draw from WebAssembly
function draw(position_x, position_y, angle, scale, color, buffer_id) {
  gl.uniform2fv(uPosition, [position_x, position_y]);
  gl.uniform2fv(uBase, [scale * Math.cos(angle), scale * Math.sin(angle)]);
  gl.uniform4fv(uColor, color);
  var buffer = buffers['' + buffer_id];
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer.vertex);
  gl.enableVertexAttribArray(aPosition);
  gl.vertexAttribPointer(aPosition, 2, gl.FLOAT, false, 0, 0);
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer.color);
  gl.enableVertexAttribArray(aColor);
  gl.vertexAttribPointer(aColor, 4, gl.FLOAT, false, 0, 0);
  gl.drawArrays(gl.TRIANGLES, 0, buffer.length);
}

// Play a sound from WebAssembly
function play_sound(sound) {
  if(sound === 0) {
    sndNoise.start();
  } else if(sound === 1) {
    sndLaser.start();
  } else {
    console.error("Unknown sound ", sound);
    sndNoise.start();
  }
}

// Load module
client_web('client_web_bg.wasm')
.then(function(obj) {
  _wasm_instance = obj;

  requestAnimationFrame(render);
}, console.error);
