"""効果音と BGM を標準ライブラリだけで合成して assets/audio/*.wav に書き出す。

使い方: python3 tools/gen_audio.py
"""
import math
import pathlib
import random
import struct
import wave

SR = 22050
OUT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "audio"
TAU = math.tau
rnd = random.Random(7)


def buf(sec):
    return [0.0] * int(sec * SR)


def write(name, data, gain=0.9):
    peak = max(1e-9, max(abs(x) for x in data))
    k = gain / peak
    OUT.mkdir(parents=True, exist_ok=True)
    with wave.open(str(OUT / f"{name}.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1, min(1, x * k)) * 32767)) for x in data))


def env_adsr(i, n, a=0.005, d=0.05, s=0.6, r=0.1):
    t = i / SR
    total = n / SR
    if t < a:
        return t / a
    if t < a + d:
        return 1 - (1 - s) * (t - a) / d
    if t > total - r:
        return s * max(0.0, (total - t) / r)
    return s


def lowpass(data, cutoff):
    rc = 1 / (TAU * cutoff)
    a = (1 / SR) / (rc + 1 / SR)
    y = 0.0
    out = []
    for x in data:
        y += a * (x - y)
        out.append(y)
    return out


def highpass(data, cutoff):
    lp = lowpass(data, cutoff)
    return [x - l for x, l in zip(data, lp)]


def noise(n):
    return [rnd.uniform(-1, 1) for _ in range(n)]


def mix(dst, src, at=0, gain=1.0):
    for i, x in enumerate(src):
        j = at + i
        if j >= len(dst):
            break
        dst[j] += x * gain


def tone(freq, sec, shape="sine", decay=None, vib=0.0, vib_rate=5.0, sweep=None, a=0.005, r=0.05, s=1.0, d=0.0):
    n = int(sec * SR)
    out = []
    ph = 0.0
    for i in range(n):
        t = i / SR
        f = freq
        if sweep:
            f = freq + (sweep - freq) * (t / sec)
        f *= 1 + vib * math.sin(TAU * vib_rate * t)
        ph += f / SR
        p = ph % 1.0
        if shape == "sine":
            v = math.sin(TAU * p)
        elif shape == "tri":
            v = 4 * abs(p - 0.5) - 1
        elif shape == "saw":
            v = 2 * p - 1
        elif shape == "square":
            v = 1.0 if p < 0.5 else -1.0
        else:
            v = math.sin(TAU * p) * 0.6 + math.sin(TAU * p * 2) * 0.25 + math.sin(TAU * p * 3) * 0.15
        e = env_adsr(i, n, a=a, d=d if d else 0.001, s=s, r=r)
        if decay:
            e *= math.exp(-t / decay)
        out.append(v * e)
    return out


def pluck(freq, sec, damp=0.996, bright=0.5):
    period = max(2, int(SR / freq))
    ring = [rnd.uniform(-1, 1) for _ in range(period)]
    ring = lowpass(ring, 2000 + 6000 * bright)
    out = []
    idx = 0
    for _ in range(int(sec * SR)):
        a = ring[idx]
        b = ring[(idx + 1) % period]
        ring[idx] = (a + b) * 0.5 * damp
        out.append(a)
        idx = (idx + 1) % period
    return out


def reverb(data, wet=0.3, room=0.8):
    delays = [1557, 1617, 1491, 1422]
    out = list(data)
    for dl in delays:
        b = [0.0] * dl
        k = 0
        fb = 0.72 * room
        for i, x in enumerate(data):
            y = b[k]
            b[k] = x + y * fb
            k = (k + 1) % dl
            out[i] += y * wet / len(delays)
    return out


def note(n):
    """MIDI ノート番号 → 周波数"""
    return 440.0 * 2 ** ((n - 69) / 12)


# ------------------------------------------------------------------ 効果音


def sfx():
    # 素振り
    n = noise(int(0.2 * SR))
    w = [x * math.sin(math.pi * i / len(n)) ** 2 for i, x in enumerate(n)]
    w = highpass(lowpass(w, 2500), 400)
    write("swing", w, 0.6)
    n = noise(int(0.32 * SR))
    w = [x * math.sin(math.pi * i / len(n)) ** 1.5 for i, x in enumerate(n)]
    w = lowpass(w, 1300)
    mix(w, tone(160, 0.32, "sine", sweep=70, r=0.1), gain=0.4)
    write("swing_heavy", w, 0.75)

    # 打撃
    h = buf(0.2)
    mix(h, [x * math.exp(-i / (0.015 * SR)) for i, x in enumerate(noise(int(0.12 * SR)))], gain=0.8)
    mix(h, tone(140, 0.18, "sine", decay=0.05, sweep=60), gain=1.0)
    mix(h, tone(900, 0.06, "square", decay=0.01), gain=0.15)
    write("hit", lowpass(h, 5000), 0.9)
    c = buf(0.45)
    mix(c, h, gain=1.0)
    for f in (1800, 2430, 3150):
        mix(c, tone(f, 0.45, "sine", decay=0.12), gain=0.22)
    write("crit", c, 0.9)

    # 敵の消滅
    e = buf(0.6)
    mix(e, tone(700, 0.5, "square", sweep=120, decay=0.2), gain=0.25)
    mix(e, lowpass([x * math.exp(-i / (0.12 * SR)) for i, x in enumerate(noise(int(0.6 * SR)))], 1800), gain=0.7)
    for k, f in enumerate((1200, 1500, 1900)):
        mix(e, tone(f, 0.2, "sine", decay=0.06), at=int((0.1 + k * 0.07) * SR), gain=0.2)
    write("enemy_die", e, 0.8)

    # 被弾
    u = buf(0.3)
    mix(u, tone(200, 0.25, "saw", sweep=110, decay=0.1), gain=0.5)
    mix(u, tone(90, 0.2, "sine", decay=0.06), gain=0.8)
    mix(u, [x * math.exp(-i / (0.02 * SR)) for i, x in enumerate(noise(int(0.1 * SR)))], gain=0.4)
    write("hurt", lowpass(u, 2400), 0.85)

    d = buf(1.6)
    for k, f in enumerate((392, 349, 311, 262)):
        mix(d, tone(f, 0.5, "tri", decay=0.3, vib=0.01), at=int(k * 0.28 * SR), gain=0.5)
    write("player_die", reverb(d, 0.5), 0.8)

    write("jump", tone(260, 0.14, "sine", sweep=520, r=0.05), 0.5)
    ld = buf(0.15)
    mix(ld, tone(90, 0.12, "sine", decay=0.04), gain=1)
    mix(ld, lowpass([x * math.exp(-i / (0.02 * SR)) for i, x in enumerate(noise(int(0.12 * SR)))], 900), gain=0.6)
    write("land", ld, 0.6)
    n = noise(int(0.35 * SR))
    r = lowpass([x * math.sin(math.pi * i / len(n)) for i, x in enumerate(n)], 900)
    write("roll", r, 0.6)
    st = lowpass([x * math.exp(-i / (0.012 * SR)) for i, x in enumerate(noise(int(0.06 * SR)))], 1200)
    write("step", st, 0.5)

    # 回復薬
    p = buf(0.6)
    for k in range(7):
        f = rnd.uniform(500, 1100)
        mix(p, tone(f, 0.06, "sine", sweep=f * 1.6, decay=0.03), at=int(k * 0.07 * SR), gain=0.5)
    write("potion", p, 0.6)
    hl = buf(1.0)
    for k, m in enumerate((72, 76, 79, 84, 88)):
        mix(hl, tone(note(m), 0.5, "bell", decay=0.25), at=int(k * 0.08 * SR), gain=0.4)
    write("heal", reverb(hl, 0.5), 0.75)

    lv = buf(1.4)
    for k, m in enumerate((67, 71, 74, 79)):
        mix(lv, tone(note(m), 0.25, "square", decay=0.2), at=int(k * 0.09 * SR), gain=0.18)
        mix(lv, tone(note(m), 0.3, "tri", decay=0.2), at=int(k * 0.09 * SR), gain=0.3)
    for m in (79, 83, 86):
        mix(lv, tone(note(m), 0.9, "tri", decay=0.5, vib=0.004), at=int(0.38 * SR), gain=0.28)
    write("levelup", reverb(lowpass(lv, 6000), 0.4), 0.8)

    cn = buf(0.3)
    mix(cn, tone(note(83), 0.08, "square", decay=0.04), gain=0.3)
    mix(cn, tone(note(88), 0.25, "square", decay=0.08), at=int(0.06 * SR), gain=0.3)
    write("coin", lowpass(cn, 7000), 0.5)
    write("pickup", tone(520, 0.18, "tri", sweep=1100, decay=0.1), 0.55)

    ch = buf(1.1)
    mix(ch, lowpass(tone(110, 0.35, "saw", vib=0.08, vib_rate=30, r=0.1), 800), gain=0.5)
    for k, m in enumerate((76, 79, 83, 88, 91)):
        mix(ch, tone(note(m), 0.5, "bell", decay=0.25), at=int((0.35 + k * 0.07) * SR), gain=0.35)
    write("chest", reverb(ch, 0.4), 0.8)

    po = buf(1.4)
    for det in (1.0, 1.01, 0.99):
        mix(po, tone(200 * det, 1.3, "sine", sweep=900 * det, vib=0.03, vib_rate=7, a=0.2, r=0.4), gain=0.3)
    nn = noise(int(1.3 * SR))
    mix(po, lowpass([x * math.sin(math.pi * i / len(nn)) for i, x in enumerate(nn)], 1500), gain=0.4)
    write("portal", reverb(po, 0.5), 0.75)

    write("talk", tone(880, 0.05, "tri", decay=0.02), 0.3)
    write("select", tone(1320, 0.05, "square", decay=0.015), 0.3)
    dn = buf(0.3)
    mix(dn, tone(180, 0.12, "square"), gain=0.3)
    mix(dn, tone(140, 0.14, "square"), at=int(0.13 * SR), gain=0.3)
    write("deny", lowpass(dn, 1800), 0.5)
    q = buf(1.4)
    for m in (62, 66, 69, 74):
        mix(q, tone(note(m), 1.1, "tri", decay=0.6, vib=0.004, a=0.02), gain=0.25)
    for k, m in enumerate((74, 78, 81, 86)):
        mix(q, tone(note(m), 0.3, "bell", decay=0.15), at=int(k * 0.08 * SR), gain=0.3)
    write("quest", reverb(q, 0.45), 0.8)

    g = buf(0.7)
    mix(g, lowpass(tone(85, 0.65, "saw", vib=0.05, vib_rate=18, a=0.05, r=0.2), 500), gain=0.8)
    mix(g, lowpass([x * math.sin(math.pi * i / (0.65 * SR)) for i, x in enumerate(noise(int(0.65 * SR)))], 700), gain=0.4)
    write("growl", g, 0.7)
    write("slime", lowpass(tone(300, 0.25, "sine", sweep=140, vib=0.15, vib_rate=25, r=0.08), 1500), 0.55)

    sl = buf(1.0)
    mix(sl, tone(70, 0.9, "sine", sweep=35, decay=0.3), gain=1.0)
    mix(sl, lowpass([x * math.exp(-i / (0.18 * SR)) for i, x in enumerate(noise(int(0.9 * SR)))], 900), gain=0.9)
    mix(sl, [x * math.exp(-i / (0.01 * SR)) for i, x in enumerate(noise(int(0.05 * SR)))], gain=0.6)
    write("slam", sl, 0.95)

    ro = buf(1.6)
    for det in (1.0, 1.5, 2.02):
        mix(ro, tone(70 * det, 1.5, "saw", vib=0.04, vib_rate=11, a=0.15, r=0.5, sweep=55 * det), gain=0.3)
    nn = noise(int(1.5 * SR))
    mix(ro, lowpass([x * math.sin(math.pi * i / len(nn)) for i, x in enumerate(nn)], 1100), gain=0.5)
    write("roar", reverb(lowpass(ro, 1400), 0.35), 0.9)
    gr = buf(0.35)
    mix(gr, lowpass(tone(110, 0.3, "saw", sweep=80, a=0.02, r=0.1), 700), gain=0.8)
    write("grunt", gr, 0.6)
    mg = buf(0.7)
    for k in range(8):
        mix(mg, tone(rnd.uniform(900, 2600), 0.2, "sine", decay=0.06), at=int(k * 0.05 * SR), gain=0.25)
    mix(mg, tone(300, 0.6, "sine", sweep=1200, a=0.05, r=0.2), gain=0.3)
    write("magic", reverb(mg, 0.4), 0.7)


# ------------------------------------------------------------------ BGM


def render_loop(sec, fn, verb=0.0):
    """ループ再生用: 末尾からはみ出た残響を先頭に足し込む。"""
    total = int(sec * SR)
    b = [0.0] * (total + SR * 3)
    fn(b)
    if verb:
        b = reverb(b, verb)
    for i in range(total, len(b)):
        b[i - total] += b[i]
    return b[:total]


def bgm_field():
    bpm = 88
    beat = 60 / bpm
    bars = 16
    # D - A - Bm - G / G - A - F#m - Bm / D - A - Bm - G / Em - G - A - A
    prog = [
        (62, "M"), (57, "M"), (59, "m"), (55, "M"),
        (55, "M"), (57, "M"), (54, "m"), (59, "m"),
        (62, "M"), (57, "M"), (59, "m"), (55, "M"),
        (52, "m"), (55, "M"), (57, "M"), (57, "M"),
    ]
    melody = [
        # (拍, ノート, 長さ拍)
        (0, 74, 1.5), (1.5, 76, 0.5), (2, 78, 1), (3, 76, 1),
        (4, 73, 2), (6, 69, 1), (7, 71, 1),
        (8, 74, 1.5), (9.5, 73, 0.5), (10, 71, 2),
        (12, 67, 1), (13, 69, 1), (14, 71, 1), (15, 74, 1),
        (16, 74, 1.5), (17.5, 76, 0.5), (18, 78, 1), (19, 81, 1),
        (20, 79, 2), (22, 76, 1), (23, 73, 1),
        (24, 74, 1), (25, 73, 1), (26, 71, 1), (27, 69, 1),
        (28, 71, 3), (31, 69, 1),
        (32, 78, 1.5), (33.5, 79, 0.5), (34, 81, 1), (35, 78, 1),
        (36, 76, 2), (38, 73, 1), (39, 76, 1),
        (40, 74, 1.5), (41.5, 76, 0.5), (42, 78, 1), (43, 74, 1),
        (44, 71, 2), (46, 74, 1), (47, 79, 1),
        (48, 79, 1.5), (49.5, 78, 0.5), (50, 76, 1), (51, 74, 1),
        (52, 74, 1), (53, 76, 1), (54, 78, 1), (55, 79, 1),
        (56, 81, 2), (58, 78, 1), (59, 76, 1),
        (60, 76, 3), (63, 73, 1),
    ]
    sec = bars * 4 * beat

    def fn(b):
        for bar, (root, q) in enumerate(prog):
            t0 = bar * 4 * beat
            third = 4 if q == "M" else 3
            chord = [root, root + third, root + 7]
            # パッド
            for n in chord:
                mix(b, lowpass(tone(note(n), 4 * beat + 0.3, "tri", vib=0.003, a=0.4, r=0.6, s=0.8), 1600), at=int(t0 * SR), gain=0.08)
            # ベース
            for k in (0, 2):
                mix(b, tone(note(root - 12), 1.8 * beat, "tri", a=0.01, r=0.2, s=0.7), at=int((t0 + k * beat) * SR), gain=0.22)
            # アルペジオ (撥弦)
            arp = [chord[0] + 12, chord[1] + 12, chord[2] + 12, chord[1] + 12]
            for k in range(8):
                n = arp[k % 4]
                mix(b, pluck(note(n), 1.2, damp=0.994, bright=0.35), at=int((t0 + k * beat / 2) * SR), gain=0.09)
            # シェイカー
            for k in range(8):
                hh = highpass([x * math.exp(-i / (0.02 * SR)) for i, x in enumerate(noise(int(0.08 * SR)))], 5000)
                mix(b, hh, at=int((t0 + k * beat / 2) * SR), gain=0.035 if k % 2 else 0.02)
        for bt, n, ln in melody:
            mix(b, tone(note(n), ln * beat + 0.1, "bell", vib=0.006, vib_rate=5.5, a=0.03, r=0.15, s=0.8), at=int(bt * beat * SR), gain=0.12)

    write("bgm_field", render_loop(sec, fn, 0.35), 0.8)


def bgm_dungeon():
    bpm = 66
    beat = 60 / bpm
    bars = 12
    prog = [50, 50, 46, 46, 48, 48, 45, 45, 50, 53, 46, 45]
    bells = [(0, 74), (3, 72), (6, 69), (8, 70), (11, 69), (14, 65), (16, 72), (19, 70), (22, 69),
             (24, 74), (26, 77), (28, 76), (31, 72), (34, 69), (36, 70), (38, 69), (40, 65), (44, 69), (46, 73)]
    sec = bars * 4 * beat

    def fn(b):
        mix(b, lowpass(tone(note(38), sec, "saw", vib=0.002, a=1.0, r=1.0, s=1.0), 220), gain=0.12)
        for bar, root in enumerate(prog):
            t0 = bar * 4 * beat
            minor = [root, root + 3, root + 7]
            for n in minor:
                mix(b, lowpass(tone(note(n), 4 * beat + 0.5, "saw", vib=0.004, vib_rate=0.3, a=1.2, r=1.2, s=0.8), 700), at=int(t0 * SR), gain=0.05)
            # 低い太鼓
            k = lowpass(tone(55, 0.8, "sine", sweep=40, decay=0.25), 300)
            mix(b, k, at=int(t0 * SR), gain=0.35)
            mix(b, k, at=int((t0 + 2.5 * beat) * SR), gain=0.2)
        for bt, n in bells:
            mix(b, tone(note(n), 3.0, "bell", decay=1.0, a=0.005), at=int(bt * beat * SR), gain=0.09)
        # 水滴
        for _ in range(14):
            t = rnd.uniform(0, sec - 1)
            f = rnd.uniform(1500, 2600)
            mix(b, tone(f, 0.12, "sine", sweep=f * 1.5, decay=0.03), at=int(t * SR), gain=0.04)

    write("bgm_dungeon", render_loop(sec, fn, 0.7), 0.75)


def bgm_boss():
    bpm = 150
    beat = 60 / bpm
    bars = 16
    prog = [52, 52, 48, 50, 52, 52, 55, 54, 48, 50, 52, 52, 48, 50, 47, 47]
    lead = [
        (0, 76, 1.5), (1.5, 79, 0.5), (2, 78, 1), (3, 76, 1), (4, 74, 2), (6, 71, 2),
        (8, 72, 1.5), (9.5, 74, 0.5), (10, 76, 2), (12, 74, 1), (13, 72, 1), (14, 71, 2),
        (16, 76, 1.5), (17.5, 79, 0.5), (18, 83, 2), (20, 81, 1), (21, 79, 1), (22, 78, 2),
        (24, 79, 1), (25, 81, 1), (26, 83, 1), (27, 86, 1), (28, 83, 4),
        (32, 84, 2), (34, 83, 1), (35, 81, 1), (36, 79, 2), (38, 76, 2),
        (40, 74, 1), (41, 76, 1), (42, 78, 2), (44, 79, 1), (45, 78, 1), (46, 76, 2),
        (48, 84, 2), (50, 83, 2), (52, 81, 2), (54, 79, 2),
        (56, 78, 2), (58, 75, 2), (60, 71, 4),
    ]
    sec = bars * 4 * beat

    def fn(b):
        kick = tone(60, 0.25, "sine", sweep=35, decay=0.08)
        snare = [x * math.exp(-i / (0.06 * SR)) for i, x in enumerate(highpass(noise(int(0.25 * SR)), 900))]
        mix(snare, tone(190, 0.15, "tri", decay=0.04), gain=0.5)
        hat = highpass([x * math.exp(-i / (0.015 * SR)) for i, x in enumerate(noise(int(0.06 * SR)))], 6000)
        for bar, root in enumerate(prog):
            t0 = bar * 4 * beat
            for k in range(8):
                t = t0 + k * beat / 2
                # 8 分のベース
                mix(b, lowpass(tone(note(root - 12), beat / 2 * 0.9, "saw", a=0.003, r=0.03, s=0.9), 900), at=int(t * SR), gain=0.2)
                mix(b, hat, at=int(t * SR), gain=0.08)
            for k in (0, 1.5, 2, 3.5):
                mix(b, kick, at=int((t0 + k * beat) * SR), gain=0.6)
            for k in (1, 3):
                mix(b, snare, at=int((t0 + k * beat) * SR), gain=0.35)
            # パワーコード
            for n in (root, root + 7, root + 12):
                pc = lowpass([max(-0.6, min(0.6, x * 2.2)) for x in tone(note(n), 4 * beat, "saw", a=0.01, r=0.1, s=0.8)], 1800)
                mix(b, pc, at=int(t0 * SR), gain=0.05)
            # 弦のパッド
            for n in (root + 12, root + 15, root + 19):
                mix(b, lowpass(tone(note(n), 4 * beat + 0.1, "saw", vib=0.005, a=0.1, r=0.2, s=0.7), 2500), at=int(t0 * SR), gain=0.035)
        for bt, n, ln in lead:
            mix(b, lowpass(tone(note(n), ln * beat, "square", vib=0.008, vib_rate=6, a=0.01, r=0.08, s=0.8), 3200), at=int(bt * beat * SR), gain=0.07)
            mix(b, tone(note(n - 12), ln * beat, "tri", a=0.01, r=0.08, s=0.8), at=int(bt * beat * SR), gain=0.06)

    write("bgm_boss", render_loop(sec, fn, 0.2), 0.85)


def bgm_victory():
    b = buf(7.0)
    beat = 60 / 110
    seq = [(0, 67, 0.33), (0.33, 67, 0.33), (0.66, 67, 0.33), (1, 72, 1.5), (2.5, 70, 0.5), (3, 72, 0.5), (3.5, 74, 0.5), (4, 76, 3)]
    for bt, n, ln in seq:
        mix(b, tone(note(n), ln * beat + 0.1, "square", a=0.01, r=0.1, s=0.8), at=int(bt * beat * SR), gain=0.1)
        mix(b, tone(note(n), ln * beat + 0.2, "tri", a=0.01, r=0.2, s=0.8), at=int(bt * beat * SR), gain=0.2)
    for n in (60, 64, 67, 72):
        mix(b, tone(note(n), 3.5, "tri", a=0.05, r=1.5, s=0.7, vib=0.004), at=int(4 * beat * SR), gain=0.12)
    mix(b, tone(note(48), 3.5, "tri", a=0.01, r=1.5, s=0.7), at=int(4 * beat * SR), gain=0.25)
    write("bgm_victory", reverb(b, 0.4), 0.8)


if __name__ == "__main__":
    sfx()
    print("sfx done")
    bgm_victory()
    bgm_dungeon()
    print("dungeon done")
    bgm_boss()
    print("boss done")
    bgm_field()
    print("field done")
