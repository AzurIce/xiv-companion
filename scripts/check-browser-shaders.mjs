#!/usr/bin/env node
// Validate shipped WGSL with Chromium/Tint, which can reject shaders accepted
// by native wgpu/Naga. Uses an isolated headless profile and an existing origin;
// starts no HTTP server. Shader compilation is not a GPU performance test.
// Usage: node scripts/check-browser-shaders.mjs <generated.wgsl> [...]
// Optional: --reject <old.wgsl> proves the pre-fix shader is actually rejected.
import {spawn} from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const shaders = [];
const args = process.argv.slice(2);
for (let i = 0; i < args.length; i++) {
  const reject = args[i] === '--reject';
  const file = reject ? args[++i] : args[i];
  if (!file) throw new Error('Missing shader path');
  shaders.push({file, reject, code: await fs.readFile(file, 'utf8')});
}
if (!shaders.length) throw new Error('Pass at least one compiled WGSL file');
const profile = await fs.mkdtemp(path.join(os.tmpdir(), 'xiv-shader-check-'));
const browser = spawn(process.env.CHROMIUM || 'chromium', [
  '--headless=new', '--no-first-run', '--no-default-browser-check',
  '--no-sandbox', '--disable-gpu-sandbox', '--ozone-platform=headless',
  '--disable-background-networking', '--disable-dev-shm-usage',
  '--enable-unsafe-webgpu', '--use-angle=vulkan',
  '--enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan', '--disable-vulkan-surface',
  '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank',
], {stdio: ['ignore', 'ignore', 'pipe']});
let browserLog = '';
browser.stderr.on('data', chunk => { browserLog = (browserLog + chunk).slice(-4000); });
let socket;
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
try {
  let port;
  for (let attempt = 0; attempt < 100; attempt++) {
    try { port = (await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; } catch {}
    if (port) break;
    if (browser.exitCode !== null) throw new Error(`Chromium exited: ${browserLog}`);
    await pause(100);
  }
  if (!port) throw new Error(`Chromium did not expose DevTools: ${browserLog}`);
  const endpoint = `http://127.0.0.1:${port}`;
  const target = await (await fetch(`${endpoint}/json/new?about:blank`, {method: 'PUT'})).json();
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let id = 0;
  const pending = new Map();
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    if (!message.id || !pending.has(message.id)) return;
    const {resolve, reject, timer} = pending.get(message.id);
    clearTimeout(timer);
    pending.delete(message.id);
    if (message.error) reject(new Error(JSON.stringify(message.error)));
    else resolve(message.result);
  };
  const rpc = (method, params = {}) => new Promise((resolve, reject) => {
    const requestId = ++id;
    const timer = setTimeout(() => { pending.delete(requestId); reject(new Error(`${method} timed out`)); }, 30000);
    pending.set(requestId, {resolve, reject, timer});
    socket.send(JSON.stringify({id: requestId, method, params}));
  });
  const evaluate = async expression => {
    const result = await rpc('Runtime.evaluate', {expression, awaitPromise: true, returnByValue: true});
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  // about:blank is not a secure WebGPU origin; use an existing local application.
  const origin = process.env.XIV_SHADER_CHECK_ORIGIN || 'http://127.0.0.1:8080';
  await rpc('Page.navigate', {url: origin});
  let ready = false;
  for (let attempt = 0; attempt < 100; attempt++) {
    ready = await evaluate(`location.origin === ${JSON.stringify(new URL(origin).origin)} && document.readyState === "complete" && !!navigator.gpu`);
    if (ready) break;
    await pause(100);
  }
  if (!ready) throw new Error('Existing origin did not provide WebGPU');
  const adapter = await evaluate(`(async () => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) return null;
    window.shaderCheckDevice = await adapter.requestDevice();
    return {...adapter.info.toJSON?.(), vendor: adapter.info.vendor, architecture: adapter.info.architecture, device: adapter.info.device, description: adapter.info.description};
  })()`);
  if (!adapter) throw new Error(`No browser WebGPU adapter: ${browserLog}`);
  const results = [];
  for (const shader of shaders) {
    const messages = await evaluate(`(async () => {
      const module = window.shaderCheckDevice.createShaderModule({code: ${JSON.stringify(shader.code)}});
      const info = await module.getCompilationInfo();
      return [...info.messages].map(m => ({type:m.type, line:m.lineNum, column:m.linePos, message:m.message}));
    })()`);
    const errors = messages.filter(message => message.type === 'error');
    results.push({file: shader.file, expectedRejected: shader.reject, messages});
    if (shader.reject ? !errors.length : errors.length) {
      throw new Error(`Unexpected compilation result for ${shader.file}: ${JSON.stringify(messages)}`);
    }
  }
  const version = await rpc('Browser.getVersion');
  console.log(JSON.stringify({browser: version.product, adapter, compilationOnly: true, results}, null, 2));
  await evaluate('window.shaderCheckDevice.destroy()');
} finally {
  socket?.close();
  const running = () => browser.exitCode === null && browser.signalCode === null;
  browser.kill('SIGTERM');
  if (running()) await Promise.race([
    new Promise(resolve => browser.once('exit', resolve)), pause(3000),
  ]);
  if (running()) {
    browser.kill('SIGKILL');
    await new Promise(resolve => browser.once('exit', resolve));
  }
  await fs.rm(profile, {recursive: true, force: true});
}
