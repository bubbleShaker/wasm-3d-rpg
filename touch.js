// スマホ・タブレット用のタッチ操作。
// 画面の上に仮想スティックとボタンを重ね、入力を window.__touchTake() でゲーム (input.rs) に渡す。
// ゲーム側は body の data-mode (title / play / dialog / menu) と data-near で今の場面を知らせてくる。
(function () {
  const q = new URLSearchParams(location.search);
  const coarse = window.matchMedia && window.matchMedia("(pointer: coarse)").matches;
  const on = q.get("touch") === "1" || (q.get("touch") !== "0" && coarse && navigator.maxTouchPoints > 0);
  window.__touchUI = on;
  if (!on) return;

  // ボタンの番号は input.rs の TOUCH_* と揃える
  const B = { attack: 0, roll: 1, jump: 2, talk: 3, potion: 4, lock: 5, bag: 6, menu: 7 };
  let pressed = 0; // 前回の take 以降に押されたボタン
  let stick = null; // { id, ox, oy, x, y }
  let look = [0, 0];
  let cam = null; // { id, x, y }
  let taps = []; // 前回の take 以降のタップ [x, y]
  const down = new Map(); // 画面上のタップ判定用 pointerId -> {x, y, t}

  const css = `
    #touch { position: fixed; inset: 0; z-index: 5; pointer-events: none; user-select: none; -webkit-user-select: none;
      font-family: "Hiragino Sans", "Noto Sans JP", system-ui, sans-serif; touch-action: none; }
    body:not([data-mode="play"]) #touch .play { display: none; }
    #lzone { position: absolute; left: 0; top: 18%; bottom: 0; width: 42%; pointer-events: auto; }
    #base { position: absolute; width: 124px; height: 124px; margin: -62px 0 0 -62px; border-radius: 50%;
      background: rgba(10, 20, 40, .28); border: 2px solid rgba(255, 255, 255, .35); }
    #knob { position: absolute; width: 56px; height: 56px; margin: -28px 0 0 -28px; border-radius: 50%;
      background: rgba(255, 255, 255, .55); box-shadow: 0 2px 10px rgba(0, 0, 0, .35); }
    .tb { position: absolute; display: flex; align-items: center; justify-content: center; border-radius: 50%;
      pointer-events: auto; color: #fff; font-weight: 700; text-shadow: 0 1px 3px rgba(0, 0, 0, .8);
      background: rgba(15, 25, 50, .42); border: 2px solid rgba(255, 255, 255, .5); box-sizing: border-box; }
    .tb.on { background: rgba(247, 213, 138, .55); border-color: #fff3c8; transform: scale(.94); }
    #b-attack { width: 84px; height: 84px; font-size: 17px; background: rgba(150, 40, 40, .45); }
    .tb.m { width: 60px; height: 60px; font-size: 13px; }
    .tb.s { width: 48px; height: 48px; font-size: 11px; }
    .tb.pill { border-radius: 14px; width: auto; height: 34px; padding: 0 12px; font-size: 12px; }
    body[data-near="1"] #b-talk { background: rgba(80, 160, 255, .6); border-color: #cfe6ff; animation: near 1s infinite; }
    @keyframes near { 50% { box-shadow: 0 0 16px rgba(120, 190, 255, .9); } }
    #topbar { position: absolute; top: calc(6px + env(safe-area-inset-top)); left: 50%; transform: translateX(-50%);
      display: flex; gap: 8px; }
    #topbar .tb { position: static; }
    body:not([data-mode="play"]):not([data-mode="dialog"]) #topbar { top: auto; left: auto; transform: none;
      bottom: calc(8px + env(safe-area-inset-bottom)); right: calc(10px + env(safe-area-inset-right)); }
    #rotate { position: fixed; inset: 0; z-index: 20; display: none; align-items: center; justify-content: center;
      flex-direction: column; gap: 14px; background: #070a14; color: #f3e6c4; font: 700 18px system-ui, sans-serif; }
    #rotate .ph { width: 46px; height: 78px; border: 4px solid #f3e6c4; border-radius: 9px; animation: rot 2s infinite; }
    @keyframes rot { 0%, 30% { transform: rotate(0); } 60%, 100% { transform: rotate(-90deg); } }
    @media (orientation: portrait) { #rotate { display: flex; } }
  `;
  const R = "env(safe-area-inset-right)";
  const L = "env(safe-area-inset-left)";
  const html = `
    <div id="lzone" class="play"><div id="base"></div><div id="knob"></div></div>
    <div class="tb play" id="b-attack" data-b="attack" style="right:calc(26px + ${R});bottom:30px">攻撃</div>
    <div class="tb m play" data-b="roll" style="right:calc(122px + ${R});bottom:18px">回避</div>
    <div class="tb m play" data-b="jump" style="right:calc(40px + ${R});bottom:128px">ジャンプ</div>
    <div class="tb m play" id="b-talk" data-b="talk" style="right:calc(118px + ${R});bottom:100px">話す</div>
    <div class="tb s play" data-b="potion" style="right:calc(196px + ${R});bottom:22px">回復</div>
    <div class="tb s play" data-b="lock" style="right:calc(196px + ${R});bottom:96px">ロック</div>
    <div id="topbar">
      <div class="tb pill play" data-b="bag">持ち物</div>
      <div class="tb pill play" data-b="menu">メニュー</div>
      <div class="tb pill" id="b-fs">全画面</div>
    </div>`;
  const rotate = `<div id="rotate"><div class="ph"></div><div>横向きにして遊んでください</div></div>`;

  function setup() {
    const st = document.createElement("style");
    st.textContent = css;
    document.head.appendChild(st);
    const root = document.createElement("div");
    root.id = "touch";
    root.innerHTML = html;
    document.body.appendChild(root);
    document.body.insertAdjacentHTML("beforeend", rotate);
    if (!document.body.dataset.mode) document.body.dataset.mode = "title";

    const base = root.querySelector("#base");
    const knob = root.querySelector("#knob");
    const lzone = root.querySelector("#lzone");
    const R0 = 52; // スティックが倒れきる距離
    const restStick = () => {
      const r = lzone.getBoundingClientRect();
      const x = Math.max(110, r.width * 0.4), y = r.height - 118;
      base.style.left = knob.style.left = x + "px";
      base.style.top = knob.style.top = y + "px";
    };
    restStick();
    window.addEventListener("resize", restStick);
    const local = (e) => { const r = lzone.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };

    lzone.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      if (stick) return;
      const [x, y] = local(e);
      stick = { id: e.pointerId, ox: x, oy: y, x, y };
      lzone.setPointerCapture(e.pointerId);
      base.style.left = knob.style.left = x + "px";
      base.style.top = knob.style.top = y + "px";
    });
    lzone.addEventListener("pointermove", (e) => {
      if (!stick || e.pointerId !== stick.id) return;
      e.preventDefault();
      const [x, y] = local(e);
      let dx = x - stick.ox, dy = y - stick.oy;
      const d = Math.hypot(dx, dy);
      if (d > R0) { dx *= R0 / d; dy *= R0 / d; }
      stick.x = stick.ox + dx; stick.y = stick.oy + dy;
      knob.style.left = stick.x + "px";
      knob.style.top = stick.y + "px";
    });
    const endStick = (e) => {
      if (!stick || e.pointerId !== stick.id) return;
      stick = null;
      restStick();
    };
    lzone.addEventListener("pointerup", endStick);
    lzone.addEventListener("pointercancel", endStick);

    for (const el of root.querySelectorAll("[data-b]")) {
      const bit = 1 << B[el.dataset.b];
      el.addEventListener("pointerdown", (e) => {
        e.preventDefault();
        e.stopPropagation();
        pressed |= bit;
        el.classList.add("on");
      });
      const off = () => el.classList.remove("on");
      el.addEventListener("pointerup", off);
      el.addEventListener("pointercancel", off);
      el.addEventListener("pointerleave", off);
    }
    root.querySelector("#b-fs").addEventListener("pointerdown", (e) => {
      e.preventDefault();
      const d = document.documentElement;
      if (document.fullscreenElement) document.exitFullscreen?.();
      else (d.requestFullscreen || d.webkitRequestFullscreen)?.call(d)?.then?.(() => screen.orientation?.lock?.("landscape").catch(() => {}));
    });

    // ゲーム画面 (canvas) を触ったとき: プレイ中はドラッグでカメラ、どの場面でも短いタップを記録する
    const canvas = document.getElementById("game");
    canvas.addEventListener("pointerdown", (e) => {
      if (e.pointerType === "mouse") return;
      down.set(e.pointerId, { x: e.clientX, y: e.clientY, t: performance.now(), moved: false });
      if (document.body.dataset.mode === "play" && !cam) cam = { id: e.pointerId, x: e.clientX, y: e.clientY };
    });
    window.addEventListener("pointermove", (e) => {
      const d = down.get(e.pointerId);
      if (d && Math.hypot(e.clientX - d.x, e.clientY - d.y) > 12) d.moved = true;
      if (cam && e.pointerId === cam.id) {
        look[0] += e.clientX - cam.x;
        look[1] += e.clientY - cam.y;
        cam.x = e.clientX; cam.y = e.clientY;
      }
    });
    const up = (e) => {
      const d = down.get(e.pointerId);
      if (d) {
        if (!d.moved && performance.now() - d.t < 350 && e.type === "pointerup") taps.push([d.x, d.y]);
        down.delete(e.pointerId);
      }
      if (cam && e.pointerId === cam.id) cam = null;
    };
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }

  // ゲームが毎フレーム呼ぶ: [スティックx, スティックy(上が+), カメラdx, カメラdy, 押したボタン, タップ数, タップx, タップy]
  window.__touchTake = function () {
    let sx = 0, sy = 0;
    if (stick) { sx = (stick.x - stick.ox) / 52; sy = -(stick.y - stick.oy) / 52; }
    const t = taps.length ? taps[taps.length - 1] : [0, 0];
    const out = [sx, sy, look[0], look[1], pressed, taps.length, t[0], t[1]];
    pressed = 0; look = [0, 0]; taps = [];
    return out;
  };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", setup);
  else setup();
})();
