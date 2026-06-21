const actions = [
  { name: "Veneration", cp: 18, dur: 0, kind: "buff", set: "ven", turns: 4 },
  { name: "Innovation", cp: 18, dur: 0, kind: "buff", set: "inn", turns: 4 },
  { name: "WasteNot", cp: 56, dur: 0, kind: "buff", set: "wn", turns: 4 },
  { name: "Synth", cp: 0, dur: 10, kind: "synth", p: 18 },
  { name: "CarefulSynth", cp: 7, dur: 10, kind: "synth", p: 24 },
  { name: "Touch", cp: 18, dur: 10, kind: "touch", q: 14 },
  { name: "PreciseTouch", cp: 24, dur: 10, kind: "touch", q: 20 },
  { name: "Mend", cp: 88, dur: -30, kind: "mend" },
];

const targetP = 160;
const targetQ = 130;
const maxSteps = 15;
const init = { p: 0, q: 0, cp: 300, dur: 70, ven: 0, inn: 0, wn: 0 };

function tick(v) {
  return v > 0 ? v - 1 : 0;
}

function apply(s, a) {
  if (s.cp < a.cp) return null;
  let durCost = a.dur > 0 ? (s.wn > 0 ? Math.ceil(a.dur / 2) : a.dur) : a.dur;
  if (durCost > 0 && s.dur < durCost) return null;
  const ns = { ...s, cp: s.cp - a.cp };
  if (a.kind === "synth") ns.p = Math.min(targetP, ns.p + Math.floor(a.p * (s.ven > 0 ? 1.5 : 1)));
  else if (a.kind === "touch") ns.q = Math.min(targetQ, ns.q + Math.floor(a.q * (s.inn > 0 ? 1.5 : 1)));
  if (a.kind === "buff") ns[a.set] = a.turns;
  ns.dur = Math.min(70, ns.dur - durCost);
  if (ns.dur < 0) return null;
  if (a.kind !== "buff" || a.set !== "ven") ns.ven = tick(ns.ven);
  if (a.kind !== "buff" || a.set !== "inn") ns.inn = tick(ns.inn);
  if (a.kind !== "buff" || a.set !== "wn") ns.wn = tick(ns.wn);
  return ns;
}

function goal(s) {
  return s.p >= targetP && s.q >= targetQ;
}

function stateKey(s) {
  return `${s.p},${s.q},${s.ven},${s.inn},${s.wn}`;
}

function insertBucket(map, s) {
  const key = stateKey(s);
  const bucket = map.get(key);
  if (!bucket) {
    map.set(key, [s]);
    return;
  }
  for (let i = 0; i < bucket.length; i++) {
    const o = bucket[i];
    if (o.cp >= s.cp && o.dur >= s.dur) return;
    if (s.cp >= o.cp && s.dur >= o.dur) {
      bucket.splice(i, 1);
      i--;
    }
  }
  bucket.push(s);
}

function compact(states) {
  const map = new Map();
  for (const s of states) insertBucket(map, s);
  return [...map.values()].flat();
}

function suffixTables() {
  const rel = [
    { p: 27, q: 0, cp: 0, dur: 5 },
    { p: 36, q: 0, cp: 7, dur: 5 },
    { p: 0, q: 21, cp: 18, dur: 5 },
    { p: 0, q: 30, cp: 24, dur: 5 },
    { p: 0, q: 0, cp: 88, dur: -30 },
  ];
  const tables = [];
  tables[0] = [{ p: 0, q: 0, cp: 0, dur: 0 }];
  const key = (l) => `${l.p},${l.q},${l.cp},${l.dur}`;
  const labelDom = (a, b) => a.p >= b.p && a.q >= b.q && a.cp <= b.cp && a.dur <= b.dur;
  const prune = (labels) => {
    const seen = new Map();
    for (const l0 of labels) {
      const l = {
        p: Math.min(targetP, l0.p),
        q: Math.min(targetQ, l0.q),
        cp: l0.cp,
        dur: Math.max(-70, Math.min(70, l0.dur)),
      };
      const k = key(l);
      if (!seen.has(k)) seen.set(k, l);
    }
    const arr = [...seen.values()];
    const out = [];
    outer: for (const l of arr) {
      for (let i = 0; i < out.length; i++) {
        if (labelDom(out[i], l)) continue outer;
        if (labelDom(l, out[i])) {
          out.splice(i, 1);
          i--;
        }
      }
      out.push(l);
    }
    return out;
  };
  for (let r = 1; r <= maxSteps; r++) {
    let next = [...tables[r - 1]];
    for (const l of tables[r - 1]) {
      for (const a of rel) {
        next.push({ p: l.p + a.p, q: l.q + a.q, cp: l.cp + a.cp, dur: l.dur + a.dur });
      }
    }
    tables[r] = prune(next);
  }
  return tables;
}

const suffix = suffixTables();

function canCompleteRelaxed(s, remaining) {
  const needP = targetP - s.p;
  const needQ = targetQ - s.q;
  if (needP <= 0 && needQ <= 0) return true;
  return suffix[remaining].some((l) => l.p >= needP && l.q >= needQ && l.cp <= s.cp && l.dur <= s.dur);
}

function search(useBound) {
  let layer = [init];
  let expanded = 0;
  let generated = 0;
  let prunedBound = 0;
  const sizes = [];
  for (let step = 0; step <= maxSteps; step++) {
    sizes.push(layer.length);
    if (layer.some(goal)) return { found: step, expanded, generated, prunedBound, sizes };
    if (step === maxSteps) break;
    const next = [];
    for (const s of layer) {
      expanded++;
      for (const a of actions) {
        const ns = apply(s, a);
        if (!ns) continue;
        generated++;
        if (useBound && !canCompleteRelaxed(ns, maxSteps - step - 1)) {
          prunedBound++;
          continue;
        }
        next.push(ns);
      }
    }
    layer = compact(next);
  }
  return { found: null, expanded, generated, prunedBound, sizes };
}

console.log(JSON.stringify({
  suffixSizes: suffix.map((x) => x.length),
  forward: search(false),
  bounded: search(true),
}, null, 2));
