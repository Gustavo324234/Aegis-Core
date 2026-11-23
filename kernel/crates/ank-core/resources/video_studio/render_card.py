import json
import os
import sys
import textwrap
from PIL import Image, ImageDraw, ImageFilter, ImageFont

def load_font(size: int, bold: bool = False, mono: bool = False):
    candidates = []
    if os.name == "nt":
        windir = os.environ.get("WINDIR", "C:\\Windows")
        fonts = os.path.join(windir, "Fonts")
        if mono:
            candidates += [
                os.path.join(fonts, "consolab.ttf" if bold else "consola.ttf"),
                os.path.join(fonts, "lucon.ttf"),
            ]
        elif bold:
            candidates += [
                os.path.join(fonts, "segoeuib.ttf"),
                os.path.join(fonts, "arialbd.ttf"),
                os.path.join(fonts, "calibrib.ttf"),
            ]
        else:
            candidates += [
                os.path.join(fonts, "segoeui.ttf"),
                os.path.join(fonts, "arial.ttf"),
                os.path.join(fonts, "calibri.ttf"),
            ]
    else:
        if mono:
            candidates += [
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf" if bold else "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ]
        elif bold:
            candidates += [
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            ]
        else:
            candidates += [
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            ]
    for path in candidates:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, size)
            except Exception:
                pass
    return ImageFont.load_default()

def detect_theme(spec):
    blob = " ".join([
        str(spec.get("title") or ""),
        str(spec.get("text") or ""),
        str(spec.get("visual_prompt") or ""),
    ]).lower()
    idx = int(spec.get("scene_idx") or 0)
    total = max(1, int(spec.get("total_scenes") or 1))

    if any(k in blob for k in ("enclave", "cifrad", "privac", "segur", "aes", "sqlcipher", "shield", "candado")):
        return "privacy", (16, 185, 129), (6, 182, 212)
    if any(k in blob for k in ("módulo", "modulo", "program", "creador", "negocio", "stock", "bento", "herramienta")):
        return "modules", (6, 182, 212), (99, 102, 241)
    if any(k in blob for k in ("app", "dispers", "caos", "basta", "problema", "hook", "dejá", "deja")):
        return "chaos", (244, 63, 94), (245, 158, 11)
    if any(k in blob for k in ("cta", "probá", "proba", "descarg", "cierre", "empez", "unite")) or (total >= 2 and idx == total - 1):
        return "cta", (168, 85, 247), (34, 211, 238)
    palettes = [
        ("chaos", (236, 72, 153), (99, 102, 241)),
        ("modules", (6, 182, 212), (99, 102, 241)),
        ("privacy", (16, 185, 129), (34, 211, 238)),
        ("cta", (168, 85, 247), (236, 72, 153)),
    ]
    return palettes[idx % len(palettes)]

def wrap_fit(draw, text: str, max_width: int, start_size: int, min_size: int, max_lines: int):
    clean = " ".join((text or "").split()).rstrip(".")
    for sz in range(start_size, min_size - 1, -4):
        fnt = load_font(sz, bold=True)
        # Estimate chars per line using average uppercase/lowercase char width
        bbox = draw.textbbox((0, 0), "AgostoMódulo", font=fnt)
        avg_w = max(8.0, (bbox[2] - bbox[0]) / 12.0)
        est_chars = max(10, int(max_width / avg_w))
        lines = textwrap.wrap(clean, width=est_chars, break_long_words=False)
        if len(lines) <= max_lines:
            widest = max((draw.textbbox((0, 0), ln, font=fnt)[2] for ln in lines), default=0)
            if widest <= max_width:
                return fnt, lines, sz
    fnt = load_font(min_size, bold=True)
    est_chars = max(12, int(max_width / max(8.0, min_size * 0.56)))
    lines = textwrap.wrap(clean, width=est_chars, break_long_words=True)[:max_lines]
    return fnt, lines, min_size

def draw_glow_orb(base_img: Image.Image, cx: int, cy: int, radius: int, rgb: tuple, alpha: int = 95):
    layer = Image.new("RGBA", base_img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.ellipse([cx - radius, cy - radius, cx + radius, cy + radius], fill=(rgb[0], rgb[1], rgb[2], alpha))
    layer = layer.filter(ImageFilter.GaussianBlur(radius=max(16, int(radius * 0.45))))
    return Image.alpha_composite(base_img, layer)

def draw_hero_graphic(canvas: Image.Image, theme: str, w: int, h: int, c1: tuple, c2: tuple):
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    sc = w / 1080.0
    cy_mid = int(h * 0.57)
    cx_mid = w // 2
    box_w = int(w * 0.84)
    left = (w - box_w) // 2
    right = left + box_w
    top = int(h * 0.45)
    bot = int(h * 0.71)

    if theme == "chaos":
        chips = [
            ("Chat Suelto", left + int(24 * sc), top + int(20 * sc), (244, 63, 94)),
            ("Planilla Excel", right - int(320 * sc), top + int(32 * sc), (245, 158, 11)),
            ("App de Notas", left + int(40 * sc), bot - int(155 * sc), (148, 163, 184)),
            ("Editor Aparte", right - int(310 * sc), bot - int(145 * sc), (236, 72, 153)),
        ]
        f_chip = load_font(max(18, int(24 * sc)), bold=True)
        for lbl, x, y, col in chips:
            cw, ch = int(285 * sc), int(84 * sc)
            d.rounded_rectangle([x, y, x + cw, y + ch], radius=18, fill=(22, 26, 40, 225), outline=(col[0], col[1], col[2], 150), width=2)
            d.text((x + int(24 * sc), y + int(24 * sc)), lbl, font=f_chip, fill=(203, 213, 225, 220))
            d.line([(x + 18, y + ch // 2 + 2), (x + cw - 18, y + ch // 2 + 2)], fill=(244, 63, 94, 195), width=3)
        # Central converging Aegis ring
        r = int(96 * sc)
        d.ellipse([cx_mid - r, cy_mid - r, cx_mid + r, cy_mid + r], fill=(12, 18, 34, 245), outline=(34, 211, 238, 255), width=5)
        f_core = load_font(max(24, int(34 * sc)), bold=True)
        tw = d.textbbox((0, 0), "AEGIS", font=f_core)[2]
        d.text((cx_mid - tw // 2, cy_mid - int(20 * sc)), "AEGIS", font=f_core, fill=(255, 255, 255, 255))

    elif theme == "modules":
        cards = [
            ("DEV STUDIO", "Agentes & Código", (6, 182, 212), 0.78),
            ("CREATOR + VIDEO", "Guiones & MP4 IA", (168, 85, 247), 0.92),
            ("NEGOCIO POS", "Stock & Ventas", (16, 185, 129), 0.68),
        ]
        f_h = load_font(max(20, int(28 * sc)), bold=True)
        f_s = load_font(max(16, int(22 * sc)), bold=False)
        card_h = int(112 * sc)
        gap = int(24 * sc)
        start_y = top + int(24 * sc)
        for i, (t1, t2, col, pct) in enumerate(cards):
            y0 = start_y + i * (card_h + gap)
            d.rounded_rectangle([left, y0, right, y0 + card_h], radius=24, fill=(17, 24, 39, 230), outline=(col[0], col[1], col[2], 190), width=3)
            d.ellipse([left + int(28 * sc), y0 + int(38 * sc), left + int(56 * sc), y0 + int(66 * sc)], fill=(col[0], col[1], col[2], 255))
            d.text((left + int(78 * sc), y0 + int(20 * sc)), t1, font=f_h, fill=(248, 250, 252, 255))
            d.text((left + int(78 * sc), y0 + int(60 * sc)), t2, font=f_s, fill=(148, 163, 184, 245))
            bar_w = int(170 * sc)
            bx = right - bar_w - int(28 * sc)
            by = y0 + int(48 * sc)
            d.rounded_rectangle([bx, by, bx + bar_w, by + int(14 * sc)], radius=7, fill=(30, 41, 59, 255))
            d.rounded_rectangle([bx, by, bx + int(bar_w * pct), by + int(14 * sc)], radius=7, fill=(col[0], col[1], col[2], 255))

    elif theme == "privacy":
        for mul, al in [(1.45, 75), (1.12, 130), (0.82, 210)]:
            rr = int(115 * sc * mul)
            d.ellipse([cx_mid - rr, cy_mid - rr - int(18 * sc), cx_mid + rr, cy_mid + rr - int(18 * sc)], outline=(c1[0], c1[1], c1[2], al), width=3)
        sw = int(118 * sc)
        sh = int(138 * sc)
        yc = cy_mid - int(22 * sc)
        pts = [
            (cx_mid, yc - sh // 2),
            (cx_mid + sw // 2, yc - int(sh * 0.28)),
            (cx_mid + int(sw * 0.42), yc + int(sh * 0.25)),
            (cx_mid, yc + sh // 2),
            (cx_mid - int(sw * 0.42), yc + int(sh * 0.25)),
            (cx_mid - sw // 2, yc - int(sh * 0.28)),
        ]
        d.polygon(pts, fill=(12, 42, 42, 235), outline=(16, 185, 129, 255), width=5)
        f_tag = load_font(max(18, int(24 * sc)), bold=True, mono=True)
        tag = "AES-256  ·  SQLCIPHER  ·  ENCLAVE LOCAL"
        tw = d.textbbox((0, 0), tag, font=f_tag)[2]
        py = bot - int(72 * sc)
        d.rounded_rectangle([cx_mid - tw // 2 - 28, py, cx_mid + tw // 2 + 28, py + int(50 * sc)], radius=25, fill=(10, 30, 32, 230), outline=(16, 185, 129, 190), width=2)
        d.text((cx_mid - tw // 2, py + int(11 * sc)), tag, font=f_tag, fill=(110, 231, 183, 255))

    elif theme == "cta":
        card_top = top + int(20 * sc)
        d.rounded_rectangle([left, card_top, right, bot - int(20 * sc)], radius=32, fill=(18, 20, 38, 235), outline=(c1[0], c1[1], c1[2], 210), width=3)
        f_sub = load_font(max(22, int(30 * sc)), bold=True)
        f_btn = load_font(max(24, int(34 * sc)), bold=True)
        s1 = "Tu sistema IA modular y privado"
        w1 = d.textbbox((0, 0), s1, font=f_sub)[2]
        d.text((cx_mid - w1 // 2, card_top + int(56 * sc)), s1, font=f_sub, fill=(226, 232, 240, 255))
        btn_txt = "DESCUBRÍ AEGIS OS"
        bw = max(int(520 * sc), d.textbbox((0, 0), btn_txt, font=f_btn)[2] + 90)
        bx0 = cx_mid - bw // 2
        by0 = card_top + int(135 * sc)
        bh = int(88 * sc)
        d.rounded_rectangle([bx0, by0, bx0 + bw, by0 + bh], radius=44, fill=(34, 211, 238, 255))
        btw = d.textbbox((0, 0), btn_txt, font=f_btn)[2]
        d.text((cx_mid - btw // 2, by0 + int(22 * sc)), btn_txt, font=f_btn, fill=(8, 12, 22, 255))

    else:
        d.rounded_rectangle([left, top + 20, right, bot - 20], radius=28, fill=(15, 23, 42, 225), outline=(c1[0], c1[1], c1[2], 185), width=3)
        bars = [0.35, 0.72, 0.55, 0.92, 0.64, 0.84, 0.48, 0.76, 0.90, 0.58]
        bw = int((box_w - 140) / (len(bars) * 1.45))
        gap = int(bw * 0.45)
        x = left + 70
        base_y = bot - int(65 * sc)
        max_bh = int(175 * sc)
        for i, ratio in enumerate(bars):
            bh = int(max_bh * ratio)
            col = c1 if i % 2 == 0 else c2
            d.rounded_rectangle([x, base_y - bh, x + bw, base_y], radius=8, fill=(col[0], col[1], col[2], 225))
            x += bw + gap

    return Image.alpha_composite(canvas, layer)

def main():
    if len(sys.argv) < 3 or sys.argv[1] != "--spec":
        raise SystemExit("Usage: render_card.py --spec <spec.json>")
    with open(sys.argv[2], "r", encoding="utf-8") as f:
        spec = json.load(f)

    w = int(spec.get("width", 1080))
    h = int(spec.get("height", 1920))
    mode = (spec.get("mode") or "full").strip().lower()
    title = (spec.get("title") or "").strip()
    headline = (spec.get("text") or "").strip() or title or "AEGIS STUDIO"
    badge = (spec.get("badge") or "AEGIS STUDIO").strip()
    idx = int(spec.get("scene_idx") or 0)
    total = max(1, int(spec.get("total_scenes") or 1))
    out_path = spec["out"]
    sc = w / 1080.0

    theme, c1, c2 = detect_theme(spec)

    if mode == "overlay":
        img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        # Subtle top and bottom scrims for crisp readability over AI/stock visuals
        top_h = int(h * 0.29)
        for y in range(top_h):
            al = int(185 * (1.0 - y / max(1, top_h)))
            d.line([(0, y), (w, y)], fill=(6, 9, 18, al))
        bot_start = int(h * 0.65)
        for y in range(bot_start, h):
            t = (y - bot_start) / max(1, h - bot_start)
            al = int(215 * (t ** 1.1))
            d.line([(0, y), (w, y)], fill=(6, 9, 18, al))

        pad = int(w * 0.075)
        f_badge = load_font(max(18, int(24 * sc)), bold=True, mono=True)
        pill_txt = f"{badge.upper()}"
        pb = d.textbbox((0, 0), pill_txt, font=f_badge)
        pw = (pb[2] - pb[0]) + int(42 * sc)
        ph = int(46 * sc)
        py = int(h * 0.055)
        d.rounded_rectangle([pad, py, pad + pw, py + ph], radius=ph // 2, fill=(10, 15, 28, 215), outline=(c1[0], c1[1], c1[2], 235), width=2)
        d.text((pad + int(21 * sc), py + int(10 * sc)), pill_txt, font=f_badge, fill=(c1[0], c1[1], c1[2], 255))

        f_head, lines, sz = wrap_fit(d, headline, w - pad * 2, max(36, int(58 * sc)), max(26, int(36 * sc)), 3)
        y_cur = py + ph + int(26 * sc)
        for ln in lines:
            d.text((pad + 3, y_cur + 3), ln, font=f_head, fill=(0, 0, 0, 210))
            d.text((pad, y_cur), ln, font=f_head, fill=(255, 255, 255, 255))
            y_cur += int(sz * 1.24)
        img.save(out_path, "PNG")
        return

    # Full Motion Graphics 2.0 Canvas
    img = Image.new("RGBA", (w, h), (7, 10, 19, 255))
    bg_draw = ImageDraw.Draw(img)
    for y in range(h):
        t = y / max(1, h - 1)
        r = int(8 + 10 * (1 - t) + c2[0] * 0.07 * t)
        g = int(11 + 8 * (1 - t) + c1[1] * 0.06 * (1 - t))
        b = int(24 + 20 * t)
        bg_draw.line([(0, y), (w, y)], fill=(min(255, r), min(255, g), min(255, b), 255))

    # Soft blurred neon light orbs
    img = draw_glow_orb(img, int(w * 0.18), int(h * 0.18), int(w * 0.38), c1, 88)
    img = draw_glow_orb(img, int(w * 0.85), int(h * 0.56), int(w * 0.42), c2, 76)

    # Subtle blueprint dot matrix background
    grid = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    gd = ImageDraw.Draw(grid)
    step = max(36, int(56 * sc))
    for gx in range(step // 2, w, step):
        for gy in range(step // 2, h, step):
            gd.rectangle([gx, gy, gx + 2, gy + 2], fill=(148, 163, 184, 34))
    img = Image.alpha_composite(img, grid)

    # Hero scene-specific illustration
    img = draw_hero_graphic(img, theme, w, h, c1, c2)

    # Bottom gradient scrim for live wave & subtitles
    scrim = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    sd = ImageDraw.Draw(scrim)
    bot_start = int(h * 0.71)
    for y in range(bot_start, h):
        t = (y - bot_start) / max(1, h - bot_start)
        al = int(215 * (t ** 1.15))
        sd.line([(0, y), (w, y)], fill=(5, 8, 16, al))
    img = Image.alpha_composite(img, scrim)

    d = ImageDraw.Draw(img)
    pad = int(w * 0.08)

    # Top segmented progress indicator
    seg_w = int((w - pad * 2 - max(0, total - 1) * 12) / max(1, total))
    seg_y = int(h * 0.052)
    for si in range(total):
        sx = pad + si * (seg_w + 12)
        col = (c1[0], c1[1], c1[2], 245) if si <= idx else (255, 255, 255, 45)
        d.rounded_rectangle([sx, seg_y, sx + seg_w, seg_y + int(7 * sc)], radius=4, fill=col)

    # Neon pill badge
    f_badge = load_font(max(18, int(24 * sc)), bold=True, mono=True)
    pill_txt = badge.upper()
    pb = d.textbbox((0, 0), pill_txt, font=f_badge)
    pw = (pb[2] - pb[0]) + int(42 * sc)
    ph = int(48 * sc)
    py = seg_y + int(28 * sc)
    d.rounded_rectangle([pad, py, pad + pw, py + ph], radius=ph // 2, fill=(14, 21, 38, 235), outline=(c1[0], c1[1], c1[2], 235), width=2)
    d.text((pad + int(21 * sc), py + int(11 * sc)), pill_txt, font=f_badge, fill=(c1[0], c1[1], c1[2], 255))

    # Optional crisp kicker title above hero headline when distinct
    y_cur = py + ph + int(34 * sc)
    if title and title.lower() not in headline.lower() and len(title) <= 32:
        f_kicker = load_font(max(20, int(28 * sc)), bold=True)
        d.text((pad, y_cur), title.upper(), font=f_kicker, fill=(148, 163, 184, 255))
        y_cur += int(42 * sc)

    # Big non-truncated editorial headline
    f_head, lines, sz = wrap_fit(d, headline, w - pad * 2, max(40, int(68 * sc)), max(28, int(38 * sc)), 4)
    for ln in lines:
        d.text((pad + 3, y_cur + 4), ln, font=f_head, fill=(0, 0, 0, 195))
        d.text((pad, y_cur), ln, font=f_head, fill=(255, 255, 255, 255))
        y_cur += int(sz * 1.24)

    # Accent underline bar beneath headline
    d.rounded_rectangle([pad, y_cur + int(24 * sc), pad + int(130 * sc), y_cur + int(32 * sc)], radius=4, fill=(c1[0], c1[1], c1[2], 255))

    img.convert("RGB").save(out_path, "PNG")

if __name__ == "__main__":
    main()
