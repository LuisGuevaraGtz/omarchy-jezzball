#!/usr/bin/env python3
"""
Generador de niveles para Omarchy-Jezzball.

Uso:
    python3 tools/gen_levels.py

Genera (y REGENERA, sobrescribiendo) los dos ficheros de datos de niveles:
    assets/levels/original.ron   (10 niveles, modo Original)
    assets/levels/enhanced.ron   (60 niveles, 6 mundos x 10)

Las reglas de generacion viven en docs/LEVEL_SCHEMA.md (normativo). Este
script aplica TODAS las validaciones obligatorias y aborta con `exit 1`
(on issues) si algo no pasa. Es deterministico: para la misma version de
Python genera exactamente los mismos ficheros (todo RNG va sembrado con la
seed del nivel).

ArenaShape::Maze { density }
    Se genera un laberinto determinista recursive-backtracker sobre la
    rejilla entera (corredores de 1 celda, muros de 1 celda, borde exterior
    solido), sembrado con la seed del nivel. `density` (0..=100) abre
    muros adicionales de forma aleatoria sembrada (crea bucles y corredores
    mas anchos). Cualquier spawn de bola se coloca SIEMPRE en el centro de
    una celda abierta DEL LABERINTO DE ESTE SCRIPT. Nota: jezzball-core NO
    reproduce este laberinto: materializa el Maze con su propio PRNG
    (Rng64) y el resultado solo es ORIENTATIVO. El motor defiende el
    invariante de que toda bola nace en una celda Open: si un spawn cae
    donde no corresponde, lo reubica a la celda abierta mas cercana (o
    descarta la bola si la arena no tiene ninguna). Por eso generar aqui un
    laberinto distinto nunca produce bolas atrapadas en muros.

Sin dependencias externas: solo stdlib de Python 3.
"""

import math
import random
import sys

# ---------------------------------------------------------------------------
# Formato RON / f32
# ---------------------------------------------------------------------------


def fmt(v):
    """Formatea un f32 como RON valido: SIEMPRE con punto decimal."""
    v = round(float(v), 2)
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    if "." not in s:
        s += ".0"
    return s


def render_shape(sp):
    s = sp["shape"]
    if s == "Rect":
        return "Rect"
    if s == "Wide":
        return "Wide"
    if s == "Tall":
        return "Tall"
    if s == "Circle":
        return "Circle"
    if s == "Irregular":
        return f"Irregular(notch: {sp['notch']})"
    if s == "Maze":
        return f"Maze(density: {sp['maze_density']})"
    raise ValueError(s)


def render_obstacle(o):
    if o["type"] == "Block":
        return f"Block(x: {o['x']}, y: {o['y']}, w: {o['w']}, h: {o['h']})"
    if o["type"] == "Mover":
        return (f"Mover(x: {fmt(o['x'])}, y: {fmt(o['y'])}, w: {o['w']}, "
                f"h: {o['h']}, vx: {fmt(o['vx'])}, vy: {fmt(o['vy'])})")
    if o["type"] == "NoSplit":
        return f"NoSplit(x: {o['x']}, y: {o['y']}, w: {o['w']}, h: {o['h']})"
    raise ValueError(o)


def render_objective(o):
    name, val = o
    if val is None:
        return name
    if name in ("ClearRatio", "UnderTime"):
        return f"{name}({fmt(val)})"
    if name in ("MinScore", "KeepCombo"):
        return f"{name}({int(val)})"
    raise ValueError(o)


def render_spec(sp):
    lines = ["    ("]
    lines.append(f'        id: {sp["id"]},')
    lines.append(f'        name: "{sp["name"]}",')
    lines.append(f'        world: {sp["world"]},')
    lines.append(f'        kind: {sp["kind"]},')
    lines.append("        arena: (")
    lines.append(f'            w: {sp["w"]},')
    lines.append(f'            h: {sp["h"]},')
    lines.append(f"            shape: {render_shape(sp)},")
    obs = ", ".join(render_obstacle(o) for o in sp["obstacles"])
    lines.append(f"            obstacles: [ {obs} ],")
    lines.append("        ),")
    lines.append("        balls: [")
    for b in sp["balls"]:
        bkind = "Heavy" if b["kind"] == "Boss" else b["kind"]
        lines.append(
            f"            ( x: {fmt(b['x'])}, y: {fmt(b['y'])}, "
            f"vx: {fmt(b['vx'])}, vy: {fmt(b['vy'])}, "
            f"kind: {bkind}, radius_mul: {fmt(b['rm'])} ),"
        )
    lines.append("        ],")
    lines.append(f'        target_ratio: {fmt(sp["target"])},')
    lines.append(f'        lives: {sp["lives"]},')
    lines.append(f'        wall_speed: {fmt(sp["wspeed"])},')
    lines.append(f'        time_limit: {sp["time"]},')
    lines.append(f'        powerups_enabled: {str(sp["powerups"]).lower()},')
    objs = ", ".join(render_objective(o) for o in sp["objectives"])
    lines.append(f"        objectives: [ {objs} ],")
    lines.append(f'        purist: {str(sp["purist"]).lower()},')
    lines.append(f'        seed: {sp["seed"]},')
    lines.append("    ),")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Laberinto determinista (ArenaShape::Maze)
# ---------------------------------------------------------------------------

def maze_open_cells(w, h, seed, density):
    """Celda (x, y) -> bool. Recursive backtracker + apertura por density."""
    open_cells = set()
    if w < 3 or h < 3:
        return open_cells
    xs = [x for x in range(1, w - 1, 2)]
    ys = [y for y in range(1, h - 1, 2)]
    if not xs or not ys:
        return open_cells
    rng = random.Random(seed)
    start = (xs[0], ys[0])
    stack = [start]
    open_cells.add(start)
    while stack:
        cx, cy = stack[-1]
        nbrs = []
        for dx, dy in ((2, 0), (-2, 0), (0, 2), (0, -2)):
            nx, ny = cx + dx, cy + dy
            if (0 < nx < w - 1 and 0 < ny < h - 1
                    and nx in xs and ny in ys and (nx, ny) not in open_cells):
                nbrs.append((nx, ny))
        if nbrs:
            rng.shuffle(nbrs)
            nx, ny = nbrs[0]
            mid = ((cx + nx) // 2, (cy + ny) // 2)
            open_cells.add(mid)
            open_cells.add((nx, ny))
            stack.append((nx, ny))
        else:
            stack.pop()
    for y in range(1, h - 1):
        for x in range(1, w - 1):
            if (x, y) not in open_cells and rng.random() < density / 100.0:
                open_cells.add((x, y))
    return open_cells


# ---------------------------------------------------------------------------
# Obstaculos
# ---------------------------------------------------------------------------

def rect_overlaps(obstacles, x, y, bw, bh, ext=2):
    for o in obstacles:
        if (x < o["x"] + o["w"] + ext and x + bw + ext > o["x"] and
                y < o["y"] + o["h"] + ext and y + bh + ext > o["y"]):
            return True
    return False


def rect_inside_shape(w, h, shape, notch, x, y, bw, bh):
    if shape == "Circle":
        cx, cy = w / 2.0, h / 2.0
        for px, py in ((x, y), (x + bw, y), (x, y + bh), (x + bw, y + bh)):
            if ((px - cx) / (w / 2.0)) ** 2 + ((py - cy) / (h / 2.0)) ** 2 >= 0.9:
                return False
        return True
    if shape == "Irregular":
        n = notch
        for px, py in ((x, y), (x + bw, y), (x, y + bh), (x + bw, y + bh)):
            if ((px < n and py < n) or (px > w - n and py < n) or
                    (px < n and py > h - n) or (px > w - n and py > h - n)):
                return False
        return True
    return True


def gen_obstacles(n_blocks, n_movers, n_nosplit, w, h, shape, notch, seed):
    rng = random.Random(seed ^ 0xA55A0B51)
    obstacles = []
    for _ in range(n_blocks):
        for _t in range(120):
            bw = rng.randint(2, 4)
            bh = rng.randint(2, 4)
            x = rng.randint(2, max(2, w - 2 - bw))
            y = rng.randint(2, max(2, h - 2 - bh))
            if rect_inside_shape(w, h, shape, notch, x, y, bw, bh) and \
                    not rect_overlaps(obstacles, x, y, bw, bh):
                obstacles.append({"type": "Block", "x": x, "y": y, "w": bw, "h": bh})
                break
    for _ in range(n_movers):
        for _t in range(120):
            bw = rng.randint(1, 2)
            bh = rng.randint(1, 2)
            if shape == "Circle":
                ang = rng.uniform(0.0, 2.0 * math.pi)
                rad = rng.uniform(0.0, 0.70)
                cx = w / 2.0 + (w / 2.0) * rad * math.cos(ang)
                cy = h / 2.0 + (h / 2.0) * rad * math.sin(ang)
            else:
                cx = rng.uniform(4.0, w - 4.0)
                cy = rng.uniform(4.0, h - 4.0)
            mag = rng.uniform(2.0, 4.0)
            base = rng.uniform(0.55, math.pi - 0.55)
            quad = rng.randint(0, 3)
            theta = base + quad * math.pi / 2.0
            vx = round(mag * math.cos(theta), 2)
            vy = round(mag * math.sin(theta), 2)
            if abs(vx) < 0.5 or abs(vy) < 0.5:
                continue
            if any(cx >= o["x"] - 2 and cx <= o["x"] + o["w"] + 2 and
                   cy >= o["y"] - 2 and cy <= o["y"] + o["h"] + 2 for o in obstacles):
                continue
            obstacles.append({"type": "Mover", "x": round(cx, 2), "y": round(cy, 2),
                              "w": bw, "h": bh, "vx": vx, "vy": vy})
            break
    for _ in range(n_nosplit):
        for _t in range(120):
            bw = rng.randint(2, 3)
            bh = rng.randint(2, 3)
            x = rng.randint(3, max(3, w - 3 - bw))
            y = rng.randint(3, max(3, h - 3 - bh))
            if rect_inside_shape(w, h, shape, notch, x, y, bw, bh) and \
                    not rect_overlaps(obstacles, x, y, bw, bh):
                obstacles.append({"type": "NoSplit", "x": x, "y": y, "w": bw, "h": bh})
                break
    return obstacles


# ---------------------------------------------------------------------------
# Spawns de bolas
# ---------------------------------------------------------------------------

BASE_RADIUS = {"Normal": 0.45, "Fast": 0.45, "Erratic": 0.45,
               "Splitter": 0.45, "Heavy": 1.2, "Boss": 1.2}


def candidate_cells(w, h, shape, notch, maze_den, maze_seed, obstacles,
                    clearance, margin=4):
    maze_open = None
    if shape == "Maze":
        maze_open = maze_open_cells(w, h, maze_seed, maze_den)
    cl = int(math.ceil(clearance))
    out = []
    for y in range(margin, h - margin):
        for x in range(margin, w - margin):
            if shape == "Maze":
                if (x, y) not in maze_open:
                    continue
            elif shape == "Circle":
                cx, cy = x + 0.5, y + 0.5
                ex, ey = w / 2.0, h / 2.0
                if ((cx - ex) / (w / 2.0)) ** 2 + ((cy - ey) / (h / 2.0)) ** 2 >= 0.7:
                    continue
            elif shape == "Irregular":
                n = notch
                cx, cy = x + 0.5, y + 0.5
                if ((cx < n and cy < n) or (cx > w - n and cy < n) or
                        (cx < n and cy > h - n) or (cx > w - n and cy > h - n)):
                    continue
            blocked = False
            for o in obstacles:
                if (o["x"] - cl <= x < o["x"] + o["w"] + cl and
                        o["y"] - cl <= y < o["y"] + o["h"] + cl):
                    blocked = True
                    break
            if not blocked:
                out.append((x + 0.5, y + 0.5))
    return out


def _es_simetrica(p, w, h):
    """True si el spawn cae en una simetria que produce una orbita cerrada.

    Con velocidades en diagonal pura (45 grados) y paredes ortogonales, el
    rebote devuelve siempre otra diagonal. Si ademas el punto de partida es
    simetrico respecto al centro o a la diagonal principal de la arena, la
    trayectoria se cierra sobre si misma: la bola recorre eternamente la
    misma linea y el nivel se vuelve degenerado (el jugador la ve 'ciclada').
    Desplazamos esos spawns un poco para romper la resonancia.
    """
    x, y = p
    return abs(x - w / 2.0) < 0.01 or abs(y - h / 2.0) < 0.01 or abs(x - y) < 0.01


def place_spawns(count, w, h, shape, notch, maze_den, maze_seed, obstacles,
                 clearance, seed, min_dist=5.0):
    cells = candidate_cells(w, h, shape, notch, maze_den, maze_seed,
                            obstacles, clearance)
    rng = random.Random(seed ^ 0xC0FFEE)
    for _attempt in range(40):
        rng.shuffle(cells)
        picked = []
        for p in cells:
            if len(picked) >= count:
                break
            # Evita posiciones que generan orbitas cerradas (ver _es_simetrica).
            if _es_simetrica(p, w, h):
                continue
            if all(math.hypot(p[0] - q[0], p[1] - q[1]) >= min_dist for q in picked):
                picked.append(p)
        if len(picked) == count:
            return picked
    # Segunda vuelta sin el filtro de simetria: mas vale un nivel con una
    # orbita fea que un nivel imposible de generar.
    for _attempt in range(40):
        rng.shuffle(cells)
        picked = []
        for p in cells:
            if len(picked) >= count:
                break
            if all(math.hypot(p[0] - q[0], p[1] - q[1]) >= min_dist for q in picked):
                picked.append(p)
        if len(picked) == count:
            return picked
    raise RuntimeError(f"no caben {count} bolas en arena {w}x{h} shape={shape}")


KMULT = {"Normal": 1.0, "Fast": 1.6, "Erratic": 1.0, "Splitter": 1.0,
         "Heavy": 0.5, "Boss": 0.4}


def gen_velocities(compo, speed, seed, bump=1.0):
    """Velocidades iniciales en DIAGONAL PURA (45 grados).

    En el JezzBall original todas las bolas se mueven a 45 grados: |vx| == |vy|.
    Eso no es un detalle estetico, es lo que hace el juego legible y justo —
    el jugador puede predecir la trayectoria y el rebote de un vistazo, y con
    la resolucion por ejes separados el rebote devuelve siempre otra diagonal.
    Angulos arbitrarios producian trayectorias erraticas imposibles de anticipar.

    Lo unico que varia por bola es el CUADRANTE (los cuatro signos) y la
    rapidez segun su tipo. Se reparten los cuadrantes para que dos bolas del
    mismo nivel no salgan siempre en la misma direccion.
    """
    rng = random.Random(seed ^ 0x9E37)
    # Los cuatro cuadrantes diagonales, barajados de forma determinista.
    quadrants = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
    rng.shuffle(quadrants)
    out = []
    for i, kind in enumerate(compo):
        base = speed * KMULT[kind] * bump
        # Componente de una diagonal pura: base / sqrt(2) en cada eje, de modo
        # que el MODULO de la velocidad sigue siendo `base`.
        comp = round(base / math.sqrt(2.0), 2)
        # Nunca cero: una componente nula degenera en movimiento recto.
        if comp == 0.0:
            comp = 0.4
        sx, sy = quadrants[i % len(quadrants)]
        out.append((kind, round(comp * sx, 2), round(comp * sy, 2)))
    return out


# ---------------------------------------------------------------------------
# Datos de niveles: nombres
# ---------------------------------------------------------------------------

ORIG_NAMES = [
    "Salida de Consola",
    "CMOS Borrado",
    "Cortocircuito",
    "Bucle Cerrado",
    "Margen de Error",
    "Latencia Alta",
    "Sector Bloqueado",
    "Puerto 8080",
    "Compilacion Cruzada",
    "Pantalla Completa",
]

W1_NAMES = [
    "Clasico Reinicia", "Ventana Limpia", "Terminal Dormida",
    "Espacio en Blanco", "Modo Turbo", "Salto de Linea",
    "Prompt Abierto", "Cursiva Rota", "Cursor Parpadea",
    "Boss del Shell",
]

W2_NAMES = [
    "Cuadro por Segundo", "Carrera de Rebind", "Reloj Interno",
    "Latencia Cero", "Velocidad Critica", "Degradado",
    "Pixel DMA", "Memoria Cache", "Overclock",
    "Boss Tactil",
]

W3_NAMES = [
    "Rigidez del Nucleo", "Pared Maestra", "Hereda Bloque",
    "Suma de Verificacion", "Movimiento Recto", "Doble Empuje",
    "Zona Muerta", "Solid Stack", "Bloqueo Mutuo",
    "Caos Compilado",
]

W4_NAMES = [
    "Trayectoria Incierta", "Deriva Aleatoria", "Mitosis",
    "Doble Nucleo", "Peso Muerto", "Core Inestable",
    "Colision Multiple", "Escopeta de Particulas", "Bola de Nieve",
    "Boss Singular",
]

W5_NAMES = [
    "Botón de Escape", "Tecla de Funcion", "Acento Visible",
    "Escudo Termico", "Desfragmentar", "Conmutador",
    "Voltaje Inestable", "Fase Cero", "Sobreimpulso",
    "Caos Neon",
]

W6_NAMES = [
    "Consolidacion", "Minotauro ASCII", "Bandas Anchas",
    "Orbita Terminal", "Contrarreloj Final", "Esquina Viva",
    "Pixel Colgado", "Dedalo de Celdas", "Boss Absoluto",
    "Kernel Panic",
]


# ---------------------------------------------------------------------------
# Niveles Original (10)
# ---------------------------------------------------------------------------

def build_original():
    levels = []
    for lv in range(1, 11):
        nballs = 1 + lv
        speed = min(8.0 * 1.04 ** (lv - 1), 11.5)
        lives = 3 if lv <= 3 else (4 if lv <= 7 else 5)
        compo = ["Normal"] * nballs
        sp = _assemble(
            id=lv, name=ORIG_NAMES[lv - 1], world=0, kind="Standard",
            time="None", w=64, h=40, shape="Rect", notch=0, maze_den=0,
            obstacles=[], compo=compo, speed=speed, target=0.75, lives=lives,
            wspeed=22.0, powerups=False, objectives=[], purist=True,
            seed=1000 + lv, bump=1.0, chaos_small=False,
        )
        levels.append(sp)
    return levels


# ---------------------------------------------------------------------------
# Niveles Enhanced (60)
# ---------------------------------------------------------------------------

SHAPES = {
    1: [("Rect", 64, 40, None)] * 10,
    2: [("Rect", 64, 40, None), ("Wide", 80, 32, None), ("Tall", 48, 52, None),
        ("Rect", 64, 40, None), ("Wide", 80, 32, None), ("Tall", 48, 52, None),
        ("Rect", 64, 40, None), ("Wide", 80, 32, None), ("Tall", 48, 52, None),
        ("Rect", 72, 44, None)],
    3: [("Rect", 64, 40, None), ("Rect", 64, 40, None), ("Wide", 80, 32, None),
        ("Irregular", 64, 40, 6), ("Irregular", 64, 40, 6),
        ("Irregular", 72, 44, 8), ("Irregular", 72, 44, 8),
        ("Irregular", 64, 40, 6), ("Irregular", 64, 40, 6),
        ("Irregular", 72, 44, 8)],
    4: [("Rect", 64, 40, None), ("Circle", 64, 40, None),
        ("Circle", 64, 40, None), ("Rect", 64, 40, None),
        ("Rect", 64, 40, None), ("Circle", 64, 40, None),
        ("Wide", 72, 36, None), ("Rect", 64, 40, None),
        ("Circle", 64, 40, None), ("Circle", 72, 44, None)],
    5: [("Rect", 64, 40, None), ("Wide", 80, 32, None), ("Tall", 48, 52, None),
        ("Rect", 64, 40, None), ("Wide", 80, 32, None), ("Circle", 64, 40, None),
        ("Irregular", 64, 40, 6), ("Wide", 84, 32, None),
        ("Tall", 52, 54, None), ("Irregular", 72, 44, 8)],
    6: [("Rect", 64, 40, None), ("Maze", 64, 40, 25), ("Wide", 80, 32, None),
        ("Circle", 64, 40, None), ("Maze", 64, 40, 20),
        ("Irregular", 64, 40, 6), ("Tall", 48, 52, None),
        ("Maze", 72, 44, 30), ("Rect", 72, 44, None), ("Rect", 72, 44, None)],
}

OBSTACLES = {
    3: {1: (2, 0, 0), 2: (3, 0, 0), 3: (3, 0, 0), 4: (4, 0, 0),
        5: (4, 1, 0), 6: (5, 2, 0), 7: (5, 2, 1), 8: (6, 2, 1),
        9: (6, 1, 1), 10: (6, 2, 1)},
    4: dict((l, (min(l - 1, 3), 0, 0)) for l in range(1, 11)),
    5: {1: (0, 0, 0), 2: (0, 0, 0), 3: (1, 0, 0), 4: (0, 0, 0), 5: (0, 0, 0),
        6: (1, 0, 0), 7: (1, 0, 0), 8: (2, 0, 0), 9: (2, 0, 0), 10: (2, 0, 0)},
    6: {1: (3, 1, 0), 2: (0, 0, 0), 3: (2, 1, 0), 4: (2, 0, 1), 5: (0, 0, 0),
        6: (3, 1, 1), 7: (4, 2, 0), 8: (0, 0, 0), 9: (4, 2, 1), 10: (6, 3, 1)},
}

# world -> lista por mundo de (id: [composicion])
COMPOS = {
    1: {1: ["N", "N"], 2: ["N", "N", "N"], 3: ["N", "N", "N"],
        4: ["N", "N", "N", "N"], 5: ["N", "N", "N", "N"],
        6: ["N", "N", "N", "N", "N"], 7: ["N", "N", "N", "N", "N"],
        8: ["N", "N", "N", "N", "N", "N"],
        9: ["N", "N", "N", "N", "N", "N"],
        10: ["Boss", "N", "N", "N", "N", "N"]},
    2: {1: ["N", "N", "N"], 2: ["N", "N", "N"], 3: ["N", "N", "N", "N"],
        4: ["F", "N", "N", "N"], 5: ["F", "F", "N", "N", "N"],
        6: ["F", "F", "N", "N", "N"], 7: ["F", "F", "N", "N", "N", "N"],
        8: ["F", "F", "N", "N", "N", "N"],
        9: ["F", "F", "F", "N", "N", "N", "N"],
        10: ["Boss", "F", "F", "F", "N", "N", "N"]},
    3: {1: ["N", "N", "N"], 2: ["N", "N", "N"], 3: ["N", "N", "N", "N"],
        4: ["N", "N", "N", "N"], 5: ["N", "N", "N", "N"],
        6: ["N", "N", "N", "N", "N"], 7: ["N", "N", "N", "N", "N"],
        8: ["N", "N", "N", "N", "N"], 9: ["F", "N", "N", "N", "N", "N"],
        10: ["F", "F", "E", "N", "N", "N"]},
    4: {1: ["E", "N", "N"], 2: ["E", "E", "N"], 3: ["S", "N", "N", "N"],
        4: ["S", "S", "N", "N"], 5: ["H", "N", "N", "N", "N"],
        6: ["H", "H", "N", "N", "N"], 7: ["E", "E", "S", "N", "N", "N"],
        8: ["S", "S", "E", "N", "N", "N"],
        9: ["S", "H", "E", "N", "N", "N", "N"],
        10: ["Boss", "H", "S", "E", "N", "N", "N"]},
    5: {1: ["N", "N", "N", "N"], 2: ["N", "N", "N", "F"],
        3: ["F", "F", "N", "N", "N"], 4: ["F", "F", "N", "N", "N"],
        5: ["F", "F", "E", "N", "N", "N"], 6: ["F", "F", "E", "E", "N", "N"],
        7: ["E", "E", "F", "F", "N", "N", "N"],
        8: ["F", "F", "E", "E", "N", "N", "N"],
        9: ["E", "E", "F", "F", "N", "N", "N", "N"],
        10: ["F", "F", "F", "E", "E", "N", "N", "N"]},
    6: {1: ["N", "N", "N", "N"], 2: ["N", "N", "N", "F"],
        3: ["F", "F", "E", "N", "N"], 4: ["E", "E", "F", "F", "N"],
        5: ["F", "F", "E", "E", "N", "N"], 6: ["E", "E", "F", "F", "N", "N"],
        7: ["F", "F", "F", "E", "N", "N", "N"],
        8: ["E", "E", "F", "F", "F", "N", "N"],
        9: ["H", "F", "F", "E", "E", "N", "N", "N"],
        10: ["F", "F", "F", "E", "E", "H", "H", "N"]},
}

KIND_FULL = {"N": "Normal", "F": "Fast", "E": "Erratic", "S": "Splitter",
             "H": "Heavy", "Boss": "Boss"}

TARGETS = {
    1: 0.75, 2: 0.75, 4: 0.75, 5: 0.75,
    3: [0.75, 0.75, 0.75, 0.73, 0.72, 0.72, 0.70, 0.70, 0.72, 0.70],
    6: [0.75, 0.78, 0.76, 0.75, 0.78, 0.74, 0.76, 0.80, 0.78, 0.82],
}

LIVES = {
    1: [3] * 10, 2: [3, 3, 3, 3, 3, 3, 3, 3, 4, 4], 3: [3] * 10,
    4: [3] * 9 + [4], 5: [3] * 10, 6: [3, 3, 3, 3, 3, 2, 2, 2, 2, 2],
}

WSPEED = {
    1: [21.0, 21.5, 22.0, 22.0, 22.5, 22.5, 23.0, 23.0, 23.5, 23.5],
    2: [24.0, 24.5, 25.0, 25.0, 25.5, 26.0, 26.5, 27.0, 27.5, 28.0],
    3: [22.0, 22.5, 23.0, 23.5, 24.0, 24.0, 24.5, 25.0, 25.5, 26.0],
    4: [22.0, 22.5, 23.0, 23.0, 23.5, 24.0, 24.5, 25.0, 25.5, 26.0],
    5: [24.0, 24.5, 25.0, 25.5, 26.0, 26.5, 27.0, 27.5, 28.0, 28.5],
    6: [26.0, 26.5, 27.0, 27.0, 27.5, 28.0, 28.5, 29.0, 29.5, 30.0],
}

SPEEDS = {
    1: lambda l: 8.0 + 0.25 * (l - 1),
    2: lambda l: 12.0 + 0.6 * (l - 1),
    3: lambda l: 9.0 + 0.3 * (l - 1),
    4: lambda l: 10.0 + 0.3 * (l - 1),
    5: lambda l: 12.0 + 0.4 * (l - 1),
    6: lambda l: 14.0 + 0.5 * (l - 1),
}

TIME_LIMITS = {
    1: {5: 90.0},
    2: {2: 75.0, 4: 80.0, 5: 85.0, 6: 85.0, 8: 90.0},
    3: {5: 90.0},
    4: {5: 80.0},
    5: {5: 70.0},
    6: {5: 60.0},
}

OBJECTIVES = {
    # M1 Clasico
    1: [("ClearRatio", 0.80)],
    2: [("ClearRatio", 0.80)],
    3: [("MinScore", 1500)],
    4: [("UnderTime", 80.0), ("MinScore", 2000)],
    5: [("UnderTime", 75.0), ("MinScore", 2500)],
    6: [("KeepCombo", 3), ("ClearRatio", 0.82)],
    7: [("MinScore", 2000)],
    8: [("ClearRatio", 0.82), ("UnderTime", 70.0)],
    9: [("NoLivesLost", None), ("MinScore", 2500)],
    10: [("NoLivesLost", None), ("MinScore", 3000)],
    # M2 Velocidad
    11: [("ClearRatio", 0.80), ("UnderTime", 90.0)],
    12: [("UnderTime", 65.0), ("MinScore", 2500)],
    13: [("ClearRatio", 0.82), ("NoLivesLost", None)],
    14: [("UnderTime", 70.0), ("MinScore", 3000)],
    15: [("UnderTime", 80.0), ("ClearRatio", 0.80), ("MinScore", 3000)],
    16: [("KeepCombo", 3), ("MinScore", 3500)],
    17: [("ClearRatio", 0.82), ("UnderTime", 80.0)],
    18: [("MinScore", 4000), ("NoLivesLost", None)],
    19: [("ClearRatio", 0.82), ("UnderTime", 85.0)],
    20: [("UnderTime", 85.0), ("MinScore", 4000), ("ClearRatio", 0.80)],
    # M3 Obstaculos
    21: [("ClearRatio", 0.80), ("MinScore", 2500)],
    22: [("ClearRatio", 0.80), ("UnderTime", 90.0)],
    23: [("MinScore", 3000)],
    24: [("NoLivesLost", None), ("ClearRatio", 0.80)],
    25: [("UnderTime", 80.0), ("MinScore", 3000)],
    26: [("KeepCombo", 3), ("ClearRatio", 0.78)],
    27: [("UnderTime", 85.0), ("NoLivesLost", None)],
    28: [("MinScore", 3500), ("ClearRatio", 0.78)],
    29: [("UnderTime", 80.0), ("MinScore", 3500)],
    30: [("NoLivesLost", None), ("ClearRatio", 0.75), ("UnderTime", 75.0)],
    # M4 Bolas especiales
    31: [("ClearRatio", 0.82), ("MinScore", 2500)],
    32: [("UnderTime", 85.0), ("NoLivesLost", None)],
    33: [("ClearRatio", 0.80), ("MinScore", 3000)],
    34: [("UnderTime", 80.0), ("KeepCombo", 3)],
    35: [("UnderTime", 70.0), ("MinScore", 3500)],
    36: [("NoLivesLost", None), ("ClearRatio", 0.82)],
    37: [("ClearRatio", 0.80), ("UnderTime", 75.0)],
    38: [("KeepCombo", 4), ("MinScore", 3500)],
    39: [("NoLivesLost", None), ("UnderTime", 85.0)],
    40: [("UnderTime", 80.0), ("MinScore", 4000), ("ClearRatio", 0.78)],
    # M5 Power-ups
    41: [("MinScore", 3000), ("UnderTime", 80.0)],
    42: [("ClearRatio", 0.82), ("KeepCombo", 3)],
    43: [("NoPowerUps", None), ("MinScore", 3000)],
    44: [("UnderTime", 70.0), ("ClearRatio", 0.80)],
    45: [("UnderTime", 60.0), ("MinScore", 3500)],
    46: [("NoPowerUps", None), ("ClearRatio", 0.82)],
    47: [("KeepCombo", 4), ("UnderTime", 70.0)],
    48: [("MinScore", 4000), ("ClearRatio", 0.80)],
    49: [("NoPowerUps", None), ("UnderTime", 65.0)],
    50: [("MinScore", 4500), ("ClearRatio", 0.78), ("NoLivesLost", None)],
    # M6 Retos avanzados
    51: [("UnderTime", 70.0), ("MinScore", 4000), ("ClearRatio", 0.82)],
    52: [("NoLivesLost", None), ("UnderTime", 65.0), ("MinScore", 4000)],
    53: [("ClearRatio", 0.85), ("KeepCombo", 4), ("MinScore", 4500)],
    54: [("UnderTime", 70.0), ("NoLivesLost", None), ("MinScore", 4000)],
    55: [("UnderTime", 55.0), ("ClearRatio", 0.85), ("MinScore", 4500)],
    56: [("KeepCombo", 4), ("UnderTime", 65.0), ("MinScore", 4500)],
    57: [("ClearRatio", 0.82), ("UnderTime", 70.0), ("NoLivesLost", None)],
    58: [("NoLivesLost", None), ("MinScore", 5000), ("UnderTime", 60.0)],
    59: [("ClearRatio", 0.85), ("UnderTime", 60.0), ("KeepCombo", 4)],
    60: [("UnderTime", 60.0), ("MinScore", 6000), ("ClearRatio", 0.88)],
}


def _assemble(id, name, world, kind, time, w, h, shape, notch, maze_den,
              obstacles, compo, speed, target, lives, wspeed, powerups,
              objectives, purist, seed, bump, chaos_small):
    full = [_full(k) for k in compo]
    radius_mul = {}
    collapse = {}
    for k in full:
        if chaos_small and k in ("Fast", "Erratic", "Normal"):
            radius_mul[k] = 0.6 if k != "Normal" else 0.75
        elif k == "Boss":
            radius_mul[k] = 1.6
        else:
            radius_mul[k] = 1.0
        collapse[k] = 0.45 if k == "Boss" else BASE_RADIUS[k] * radius_mul[k]
    clearance = max(collapse.values()) + 0.4
    positions = place_spawns(len(full), w, h, shape, notch, maze_den, seed,
                             obstacles, clearance, seed)
    velos = gen_velocities(full, speed, seed, bump)
    balls = []
    for (bkind, vx, vy), (x, y) in zip(velos, positions):
        balls.append({"x": x, "y": y, "vx": vx, "vy": vy,
                      "kind": bkind, "rm": radius_mul[bkind],
                      "radius": collapse[bkind]})
    return {
        "id": id, "name": name, "world": world, "kind": kind, "time": time,
        "w": w, "h": h, "shape": shape, "notch": notch,
        "maze_density": maze_den, "obstacles": obstacles, "balls": balls,
        "target": target, "lives": lives, "wspeed": wspeed,
        "powerups": powerups, "objectives": objectives, "purist": purist,
        "seed": seed,
    }


def _full(code):
    return KIND_FULL.get(code, code)


def build_enhanced():
    levels = []
    for world in range(1, 7):
        for local in range(1, 11):
            lvid = (world - 1) * 10 + local
            shape_name, w, h, extra = SHAPES[world][local - 1]
            notch = extra if shape_name == "Irregular" else 0
            maze_den = extra if shape_name == "Maze" else 0
            if local == 5:
                kind = "SpeedRun"
            elif local == 10:
                kind = "Boss" if world in (1, 2, 4) else "Chaos"
            else:
                kind = "Standard"
            tl = TIME_LIMITS[world].get(local)
            time = "None" if tl is None else f"Some({fmt(tl)})"
            nb, nm, ns = OBSTACLES.get(world, {}).get(local, (0, 0, 0))
            obstacles = gen_obstacles(nb, nm, ns, w, h, shape_name, notch,
                                      2000 + lvid) if (nb or nm or ns) else []
            target = TARGETS[world][local - 1] if isinstance(TARGETS[world], list) \
                else TARGETS[world]
            lives = LIVES[world][local - 1]
            wspeed = WSPEED[world][local - 1]
            powerups = world in (5, 6)
            compo = COMPOS[world][local]
            bump = 1.15 if kind == "Chaos" else 1.0
            chaos_small = kind == "Chaos"
            seed = 2000 + lvid
            name = {
                1: W1_NAMES, 2: W2_NAMES, 3: W3_NAMES, 4: W4_NAMES,
                5: W5_NAMES, 6: W6_NAMES,
            }[world][local - 1]
            levels.append(_assemble(
                id=lvid, name=name, world=world, kind=kind, time=time,
                w=w, h=h, shape=shape_name, notch=notch, maze_den=maze_den,
                obstacles=obstacles, compo=compo, speed=SPEEDS[world](local),
                target=target, lives=lives, wspeed=wspeed, powerups=powerups,
                objectives=OBJECTIVES[lvid], purist=False, seed=seed,
                bump=bump, chaos_small=chaos_small,
            ))
    return levels


# ---------------------------------------------------------------------------
# Validacion
# ---------------------------------------------------------------------------

def dist_to_rect(px, py, rx, ry, rw, rh):
    dx = max(0.0, rx - px, px - (rx + rw))
    dy = max(0.0, ry - py, py - (ry + rh))
    return math.hypot(dx, dy)


def ball_radius(b):
    if b["kind"] == "Heavy":
        return 1.2 * b["rm"]
    if b["kind"] == "Boss":
        return 1.2 * b["rm"]
    return 0.45 * b["rm"]


def validate(levels, tag):
    issues = []
    seen = set()
    prev = 0
    kinds_ok = {"Standard", "Boss", "SpeedRun", "Chaos"}
    names_by_mundo = {}
    for sp in levels:
        lid = sp["id"]
        if lid in seen:
            issues.append(f"id {lid} duplicado")
        seen.add(lid)
        if lid != prev + 1:
            issues.append(f"{tag}: ids no consecutivos (esperaba {prev + 1}, hay {lid})")
        prev = lid
        if len(sp["name"]) > 22:
            issues.append(f"{tag}/{lid}: nombre '{sp['name']}' >22 chars")
        if sp["kind"] not in kinds_ok:
            issues.append(f"{tag}/{lid}: kind invalido {sp['kind']}")
        if sp["purist"] and (sp["objectives"] or sp["powerups"]):
            issues.append(f"{tag}/{lid}: purist con objectives/powerups")
        if sp["purist"] and sp["kind"] != "Standard":
            issues.append(f"{tag}/{lid}: purist pero kind {sp['kind']}")
        if sp["target"] < 0.60 or sp["target"] > 0.85:
            issues.append(f"{tag}/{lid}: target_ratio {sp['target']} fuera de 0.60..0.85")
        if not (1 <= sp["lives"] <= 5):
            issues.append(f"{tag}/{lid}: lives {sp['lives']} fuera de 1..5")
        if sp["wspeed"] < 18.0 or sp["wspeed"] > 30.0:
            issues.append(f"{tag}/{lid}: wall_speed {sp['wspeed']} fuera de 18..30")
        if len(sp["obstacles"]) > 0 and sp["shape"] == "Rect" and sp["purist"]:
            issues.append(f"{tag}/{lid}: purist con obstaculos")
        objs = sp["objectives"]
        if len(objs) > 3:
            issues.append(f"{tag}/{lid}: {len(objs)} objetivos (>3)")
        for o in objs:
            oname, oval = o
            if oname == "NoPowerUps" and not sp["powerups"]:
                issues.append(f"{tag}/{lid}: NoPowerUps con powerups_enabled=false")
            if oname == "ClearRatio" and oval <= sp["target"]:
                issues.append(f"{tag}/{lid}: ClearRatio({oval}) <= target {sp['target']}")
            if oname == "UnderTime":
                if sp["time"] != "None":
                    limit = float(sp["time"][5:-1])
                    if oval >= limit:
                        issues.append(f"{tag}/{lid}: UnderTime({oval}) >= time_limit {limit}")
                if not (20.0 <= oval <= 150.0):
                    issues.append(f"{tag}/{lid}: UnderTime {oval} no alcanzable")
        if sp["kind"] == "SpeedRun" and sp["time"] == "None":
            issues.append(f"{tag}/{lid}: SpeedRun sin time_limit")
        if sp["time"] != "None" and sp["purist"]:
            issues.append(f"{tag}/{lid}: purist con time_limit")
        # bolas
        if len(sp["balls"]) == 0:
            issues.append(f"{tag}/{lid}: sin bolas")
        maze_open = None
        if sp["shape"] == "Maze":
            maze_open = maze_open_cells(sp["w"], sp["h"], sp["seed"], sp["maze_density"])
        for bi, b in enumerate(sp["balls"]):
            bx, by = b["x"], b["y"]
            if b["vx"] == 0.0 or b["vy"] == 0.0:
                issues.append(f"{tag}/{lid}/ball{bi}: vx o vy == 0")
            if bx < 3.0 or by < 3.0 or bx > sp["w"] - 3.0 or by > sp["h"] - 3.0:
                issues.append(f"{tag}/{lid}/ball{bi}: spawn fuera de margen 3")
            if sp["shape"] == "Circle":
                cx, cy = sp["w"] / 2.0, sp["h"] / 2.0
                r = ((bx - cx) / (sp["w"] / 2.0)) ** 2 + \
                    ((by - cy) / (sp["h"] / 2.0)) ** 2
                if r >= 0.7:
                    issues.append(f"{tag}/{lid}/ball{bi}: fuera de elipse ({r:.3f})")
            if sp["shape"] == "Irregular":
                n = sp["notch"]
                if ((bx < n and by < n) or (bx > sp["w"] - n and by < n) or
                        (bx < n and by > sp["h"] - n) or
                        (bx > sp["w"] - n and by > sp["h"] - n)):
                    issues.append(f"{tag}/{lid}/ball{bi}: en esquina recortada")
            if sp["shape"] == "Maze":
                cell = (int(bx), int(by))
                if cell not in maze_open:
                    issues.append(f"{tag}/{lid}/ball{bi}: spawn sobre pared de laberinto")
            rad = ball_radius(b)
            for o in sp["obstacles"]:
                d = dist_to_rect(bx, by, o["x"], o["y"], o["w"], o["h"])
                if d < rad - 0.05:
                    issues.append(
                        f"{tag}/{lid}/ball{bi}: dentro/solapando obstaculo {o['type']}")
        for i in range(len(sp["balls"])):
            for j in range(i + 1, len(sp["balls"])):
                d = math.hypot(sp["balls"][i]["x"] - sp["balls"][j]["x"],
                               sp["balls"][i]["y"] - sp["balls"][j]["y"])
                if d < 5.0:
                    issues.append(f"{tag}/{lid}: bolas {i},{j} a {d:.2f} celdas (<5)")
        names_by_mundo.setdefault(sp["world"], 0)
        names_by_mundo[sp["world"]] += 1
    if issues:
        print(f"[VALIDACION {tag}] {len(issues)} problema(s):")
        for line in issues:
            print("  -", line)
        return False
    print(f"[VALIDACION {tag}] OK: {len(levels)} niveles, "
          f"{len(seen)} ids unicos consecutivos.")
    return True


# ---------------------------------------------------------------------------
# Escritura
# ---------------------------------------------------------------------------

def write_ron(path, levels, header):
    chunks = [header, "["]
    for sp in levels:
        chunks.append(render_spec(sp))
    chunks.append("]")
    body = "\n".join(chunks)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(body)
    return body


def main():
    original = build_original()
    enhanced = build_enhanced()

    ok = validate(original, "original.ron")
    ok = validate(enhanced, "enhanced.ron") and ok

    if not ok:
        print("ERROR: validaciones fallidas, no se escriben los ficheros.")
        sys.exit(1)

    h_orig = """// Niveles del modo Original (curva clasica del JezzBall).
// world 0, purist, sin power-ups, sin objetivos, sin obstaculos.
// Reglas: docs/LEVEL_SCHEMA.md
"""
    h_enh = """// Niveles del modo Enhanced: 6 mundos x 10 niveles.
// M1 Clasico (1-10) | M2 Velocidad (11-20) | M3 Obstaculos (21-30)
// M4 Bolas especiales (31-40) | M5 Power-ups (41-50) | M6 Retos (51-60).
// Nivel 5 de cada mundo = SpeedRun; nivel 10 = Boss/Chaos.
// Reglas: docs/LEVEL_SCHEMA.md
"""
    write_ron("assets/levels/original.ron", original, h_orig)
    write_ron("assets/levels/enhanced.ron", enhanced, h_enh)

    print()
    print("Ficheros escritos: assets/levels/original.ron "
          f"({len(original)} niveles) y "
          f"assets/levels/enhanced.ron ({len(enhanced)} niveles)")
    stats = {}
    for sp in enhanced:
        stats.setdefault(sp["world"], [0, set()])[0] += 1
        stats[sp["world"]][1].add(sp["kind"])
    for w in sorted(stats):
        print(f"  Mundo {w}: {stats[w][0]} niveles - kinds: {sorted(stats[w][1])}")


if __name__ == "__main__":
    main()