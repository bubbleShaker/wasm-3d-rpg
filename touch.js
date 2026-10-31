// スマホ・タブレット用のタッチ操作。
// 画面の上に仮想スティックとボタンを重ね、入力を window.__touchTake() でゲーム (input.rs) に渡す。
// ゲーム側は body の data-mode (title / play / dialog / menu) と data-near で今の場面を知らせてくる。
(function () {
  if (!window.__touchUI) return; // 判定は index.html

  // ボタンの番号は input.rs の TOUCH_* と揃える
  const B = { attack: 0, roll: 1, jump: 2, talk: 3, potion: 4, lock: 5, bag: 6, menu: 7 };
  let pressed = 0; // 前回の take 以降に押されたボタン
  let stick = null; // { id, ox, oy, x, y }
  let look = [0, 0];
  let cam = null; // { id, x, y }
  let taps = []; // 前回の take 以降のタップ [x, y]
  const down = new Map(); // 画面上のタップ判定用 pointerId -> {x, y, t}

  // 寸法は横向きのスマホ (高さ 390px 前後) を基準に、画面の高さに合わせて拡大する
  const u = (n) => `calc(${n}px * var(--u, 1))`;
  const fit = () => {
    const z = Math.min(2.4, Math.max(1, Math.min(innerWidth, innerHeight) / 430));
    document.documentElement.style.setProperty("--u", z.toFixed(3));
    return z;
  };
  let zoom = fit();
  const css = `
    html, body { user-select: none; -webkit-user-select: none; -webkit-tap-highlight-color: transparent; }
    #touch, #touch * { touch-action: none; }
    #touch { position: fixed; inset: 0; z-index: 5; pointer-events: none;
      font-family: "Hiragino Sans", "Noto Sans JP", system-ui, sans-serif; touch-action: none; }
    body:not([data-mode="play"]) #touch .play { display: none; }
    #lzone { position: absolute; left: 0; top: 18%; bottom: 0; width: 42%; pointer-events: auto; }
    #base { position: absolute; width: ${u(124)}; height: ${u(124)}; margin: ${u(-62)} 0 0 ${u(-62)}; border-radius: 50%;
      background: rgba(10, 20, 40, .28); border: ${u(2)} solid rgba(255, 255, 255, .35); }
    #knob { position: absolute; width: ${u(56)}; height: ${u(56)}; margin: ${u(-28)} 0 0 ${u(-28)}; border-radius: 50%;
      background: rgba(255, 255, 255, .55); box-shadow: 0 ${u(2)} ${u(10)} rgba(0, 0, 0, .35); }
    .tb { position: absolute; display: flex; align-items: center; justify-content: center; border-radius: 50%;
      pointer-events: auto; color: #fff; font-weight: 700; text-shadow: 0 ${u(1)} ${u(3)} rgba(0, 0, 0, .8);
      background: rgba(15, 25, 50, .42); border: ${u(2)} solid rgba(255, 255, 255, .5); box-sizing: border-box; }
    .tb.on { background: rgba(247, 213, 138, .55); border-color: #fff3c8; transform: scale(.94); }
    #b-attack { width: ${u(84)}; height: ${u(84)}; font-size: ${u(17)}; background: rgba(150, 40, 40, .45); }
    .tb.m { width: ${u(60)}; height: ${u(60)}; font-size: ${u(13)}; }
    .tb.s { width: ${u(48)}; height: ${u(48)}; font-size: ${u(11)}; }
    .tb.pill { border-radius: ${u(14)}; width: auto; height: ${u(34)}; padding: 0 ${u(12)}; font-size: ${u(12)}; }
    body[data-near="1"] #b-talk { background: rgba(80, 160, 255, .6); border-color: #cfe6ff; animation: near 1s infinite; }
    @keyframes near { 50% { box-shadow: 0 0 ${u(16)} rgba(120, 190, 255, .9); } }
    #topbar { position: absolute; top: calc(${u(6)} + env(safe-area-inset-top)); left: 50%; transform: translateX(-50%);
      display: flex; gap: ${u(8)}; }
    #topbar .tb { position: static; }
    body:not([data-mode="play"]):not([data-mode="dialog"]) #topbar { top: auto; left: auto; transform: none;
      bottom: calc(${u(8)} + env(safe-area-inset-bottom)); right: calc(${u(10)} + env(safe-area-inset-right)); }
    #rotate { position: fixed; inset: 0; z-index: 20; display: none; align-items: center; justify-content: center;
      flex-direction: column; gap: ${u(14)}; background: #070a14; color: #f3e6c4; font: 700 ${u(18)} system-ui, sans-serif; }
    #rotate .ph { width: ${u(46)}; height: ${u(78)}; border: ${u(4)} solid #f3e6c4; border-radius: ${u(9)}; animation: rot 2s infinite; }
    @keyframes rot { 0%, 30% { transform: rotate(0); } 60%, 100% { transform: rotate(-90deg); } }
    @media (orientation: portrait) { #rotate { display: flex; } }
    #fsmsg { position: fixed; z-index: 15; left: 50%; top: calc(${u(48)} + env(safe-area-inset-top)); transform: translateX(-50%);
      width: max-content; max-width: 80vw; padding: ${u(8)} ${u(14)}; border-radius: ${u(10)}; background: rgba(7, 10, 20, .88);
      color: #f3e6c4; font: 700 ${u(13)}/1.5 system-ui, sans-serif; text-align: center; pointer-events: none; display: none; }
  `;
  const R = "env(safe-area-inset-right)";
  const html = `
    <div id="lzone" class="play"><div id="base"></div><div id="knob"></div></div>
    <div class="tb play" id="b-attack" data-b="attack" style="right:calc(${u(26)} + ${R});bottom:${u(30)}">攻撃</div>
    <div class="tb m play" data-b="roll" style="right:calc(${u(122)} + ${R});bottom:${u(18)}">回避</div>
    <div class="tb m play" data-b="jump" style="right:calc(${u(40)} + ${R});bottom:${u(128)}">ジャンプ</div>
    <div class="tb m play" id="b-talk" data-b="talk" style="right:calc(${u(118)} + ${R});bottom:${u(100)}">話す</div>
    <div class="tb s play" data-b="potion" style="right:calc(${u(196)} + ${R});bottom:${u(22)}">回復</div>
    <div class="tb s play" data-b="lock" style="right:calc(${u(196)} + ${R});bottom:${u(96)}">ロック</div>
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
    const R0 = () => 52 * zoom; // スティックが倒れきる距離
    const restStick = () => {
      zoom = fit();
      const r = lzone.getBoundingClientRect();
      const x = Math.max(110 * zoom, r.width * 0.4), y = r.height - 118 * zoom;
      base.style.left = knob.style.left = x + "px";
      base.style.top = knob.style.top = y + "px";
    };
    restStick();
    window.addEventListener("resize", restStick);
    // タイトル中は非表示で高さが 0 なので、プレイが始まって表示されたときに置き直す
    new ResizeObserver(() => { if (!stick) restStick(); }).observe(lzone);
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
      if (d > R0()) { dx *= R0() / d; dy *= R0() / d; }
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
    // 全画面: ブラウザが操作として認めるのは指を離したときなので、pointerup で呼ぶ
    const fsBtn = root.querySelector("#b-fs");
    if (window.matchMedia("(display-mode: standalone)").matches || navigator.standalone) fsBtn.style.display = "none"; // ホーム画面から開いていて、すでに全画面
    const fsMsg = document.createElement("div");
    fsMsg.id = "fsmsg";
    document.body.appendChild(fsMsg);
    let fsTimer = 0;
    const fsFail = () => {
      fsMsg.textContent = "このブラウザは全画面に対応していません。共有メニューの「ホーム画面に追加」から開くと全画面で遊べます";
      fsMsg.style.display = "block";
      clearTimeout(fsTimer);
      fsTimer = setTimeout(() => { fsMsg.style.display = "none"; }, 6000);
    };
    fsBtn.addEventListener("pointerdown", (e) => e.preventDefault());
    fsBtn.addEventListener("pointerup", (e) => {
      e.preventDefault();
      const d = document.documentElement;
      if (document.fullscreenElement || document.webkitFullscreenElement) {
        (document.exitFullscreen || document.webkitExitFullscreen)?.call(document);
        return;
      }
      const req = d.requestFullscreen || d.webkitRequestFullscreen;
      if (!req) return fsFail();
      // 古い webkit は Promise を返さない
      Promise.resolve(req.call(d)).then(() => screen.orientation?.lock?.("landscape")).catch(() => {
        if (!document.fullscreenElement && !document.webkitFullscreenElement) fsFail();
      });
    });

    // iPhone の Safari は user-scalable=no も touch-action: none も効かず、連打やピンチで拡大してしまう。
    // このゲームにスクロールやリンクはないので、ブラウザのタッチ操作は全部止める (pointer イベントは届く)
    const stop = (e) => { if (e.cancelable) e.preventDefault(); };
    for (const t of ["touchend", "touchmove"]) document.addEventListener(t, stop, { passive: false });
    for (const t of ["gesturestart", "gesturechange", "gestureend", "dblclick"]) document.addEventListener(t, stop, { passive: false });

    // ゲーム画面 (canvas) を触ったとき: プレイ中はドラッグでカメラ、どの場面でも短いタップを記録する
    const canvas = document.getElementById("game");
    canvas.addEventListener("pointerdown", (e) => {
      if (e.pointerType === "mouse") return;
      down.set(e.pointerId, { x: e.clientX, y: e.clientY, t: performance.now(), moved: false });
      if (document.body.dataset.mode === "play" && !cam) cam = { id: e.pointerId, x: e.clientX, y: e.clientY };
    });
    window.addEventListener("pointermove", (e) => {
      const d = down.get(e.pointerId);
      if (d && Math.hypot(e.clientX - d.x, e.clientY - d.y) > 12 * zoom) d.moved = true;
      if (cam && e.pointerId === cam.id) {
        // 大きい画面でも同じ量なぞれば同じだけ回るよう、拡大率で割る
        look[0] += (e.clientX - cam.x) / zoom;
        look[1] += (e.clientY - cam.y) / zoom;
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
    if (stick) { sx = (stick.x - stick.ox) / (52 * zoom); sy = -(stick.y - stick.oy) / (52 * zoom); }
    const t = taps.length ? taps[taps.length - 1] : [0, 0];
    const out = [sx, sy, look[0], look[1], pressed, taps.length, t[0], t[1]];
    pressed = 0; look = [0, 0]; taps = [];
    return out;
  };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", setup);
  else setup();
})();
