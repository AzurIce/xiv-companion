#!/usr/bin/env node
"use strict";

// Exercise the page's actual loader, validation and rendering without a server.
// The DOM is simulated; this does not test browser file permissions or caching.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const docs = path.resolve(__dirname, "../docs");
const html = fs.readFileSync(path.join(docs, "vfx-progress.html"), "utf8");
const source = fs.readFileSync(path.join(docs, "vfx-progress-data.js"), "utf8");
const page = html.match(/<script>([\s\S]*?)<\/script>/)[1];

function openPage(initialSource = source, protocol = "file:") {
  const nodes = new Map();
  const errors = [];
  const timers = new Map();
  let nextTimer = 0;
  let nextSource = initialSource;
  let context;
  let reload;

  class Element {
    constructor(tag) {
      this.tag = tag;
      this.dataset = {};
      this.children = [];
      this.value = "";
    }
    get textContent() { return (this.text ?? "") + this.children.map(child => child.textContent ?? "").join(""); }
    set textContent(value) { this.text = value; this.children = []; }
    append(...children) {
      this.children.push(...children);
      if (this.tag !== "head") return;
      for (const script of children) {
        if (protocol === "file:") assert.equal(script.src, "vfx-progress-data.js");
        else assert.match(script.src, /^vfx-progress-data\.js\?read=\d+$/);
        if (nextSource === null) script.onerror();
        else if (nextSource !== undefined) {
          vm.runInContext(nextSource, context);
          script.onload();
        }
      }
    }
    replaceChildren(...children) { this.children = children; }
    addEventListener() {}
    setAttribute() {}
    remove() {}
  }

  const document = {
    hidden: false,
    head: new Element("head"),
    getElementById(id) {
      if (!nodes.has(id)) nodes.set(id, new Element(id));
      return nodes.get(id);
    },
    createElement(tag) { return new Element(tag); },
    querySelector() { return { value: "all" }; },
    addEventListener() {},
  };
  context = vm.createContext({
    window: { location: { protocol } }, document, Intl, Date, Set, Object, JSON, Number,
    console: { error(error) { errors.push(error.message); } },
    setTimeout(fn) { const id = ++nextTimer; timers.set(id, fn); return id; },
    clearTimeout(id) { timers.delete(id); },
    setInterval(fn) { reload = fn; },
  });
  vm.runInContext(page, context);
  return {
    nodes, errors,
    data: () => context.window.VFX_PROGRESS,
    retry(value) { nextSource = value; reload(); },
    timeout() { for (const fn of [...timers.values()]) fn(); },
  };
}

function expectLoaded(page, count) {
  assert.equal(page.nodes.get("sync").dataset.state, "ok");
  assert.equal(page.nodes.get("content").hidden, false);
  assert.equal(page.nodes.get("initial-state").hidden, true);
  assert.equal(page.nodes.get("task-list").children.length, count);
}

const loaded = openPage();
assert.deepEqual(loaded.errors, []);
const taskCount = loaded.data().tasks.length;
assert.ok(taskCount > 0);
expectLoaded(loaded, taskCount);
// Capability rows must expose delivery and acceptance separately from proofs.
const firstTask = loaded.data().tasks[0];
const firstRow = loaded.nodes.get("task-list").children[0].textContent;
for (const [key, label] of [["integration", "预览接入"], ["proof", "验证范围"], ["acceptance", "验收缺口"]]) {
  if (firstTask[key]) assert.ok(firstRow.includes(`${label}：${firstTask[key]}`));
}

// Reproduce the original failure: a record uses an unsupported state name.
const invalid = JSON.parse(JSON.stringify(loaded.data()));
invalid.tasks[0].state = "partial";
const invalidSource = `window.VFX_PROGRESS = ${JSON.stringify(invalid)};`;
const rejected = openPage(invalidSource);
assert.equal(rejected.nodes.get("sync").dataset.state, "error");
assert.match(rejected.nodes.get("initial-state").textContent, /Invalid task state:.*partial/);
rejected.retry(source);
expectLoaded(rejected, taskCount);

const invalidStage = JSON.parse(JSON.stringify(loaded.data()));
invalidStage.focus.steps[0].state = "partial";
const rejectedStage = openPage(`window.VFX_PROGRESS = ${JSON.stringify(invalidStage)};`);
assert.equal(rejectedStage.nodes.get("sync").dataset.state, "error");
assert.match(rejectedStage.nodes.get("initial-state").textContent, /Invalid stage state:.*partial/);

// A failed refresh must retain the displayed record and allow a later retry.
const oldRows = loaded.nodes.get("task-list").children;
loaded.retry(invalidSource);
assert.equal(loaded.nodes.get("sync").dataset.state, "error");
assert.match(loaded.nodes.get("sync").textContent, /保留上次记录/);
assert.equal(loaded.nodes.get("task-list").children, oldRows);
loaded.retry(source);
expectLoaded(loaded, taskCount);

const missing = openPage(null);
assert.match(missing.nodes.get("initial-state").textContent, /脚本加载失败/);
assert.equal(missing.nodes.get("initial-state").children[1].href, "vfx-progress-data.js");
missing.retry(source);
expectLoaded(missing, taskCount);

// HTTP pages still bypass the cache; local file pages use the exact adjacent path.
const httpPage = openPage(source, "https:");
expectLoaded(httpPage, taskCount);
httpPage.retry(null);
assert.match(httpPage.nodes.get("sync").textContent, /保留上次记录.*脚本加载失败/);
httpPage.retry(source);
expectLoaded(httpPage, taskCount);

const stalled = openPage();
stalled.retry(undefined);
stalled.timeout();
assert.equal(stalled.nodes.get("sync").dataset.state, "error");
assert.match(stalled.nodes.get("sync").textContent, /保留上次记录/);
stalled.retry(source);
expectLoaded(stalled, taskCount);

console.log(`PASS: ${taskCount} tasks rendered under file:// and HTTPS; invalid state, failed refresh, missing script and timeout recover with diagnostics. DOM simulated; no browser, server or GPU.`);
