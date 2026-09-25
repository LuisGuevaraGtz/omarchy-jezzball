//! El nivel bonus con la firma de Omarchy debe ser jugable de verdad, no sólo
//! parsear: sus obstáculos dibujan el logo, y un logo mal colocado podría
//! dejar bolas atrapadas dentro de un bloque o hacer el nivel imposible.

use jezzball_core::level::LevelSpec;
use jezzball_core::state::{step, GamePhase, GameState, PlayerInput};

const DT: f32 = 1.0 / 60.0;

fn cargar_nivel_omarchy() -> LevelSpec {
    let ruta = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/levels/enhanced.ron"
    );
    let texto = std::fs::read_to_string(ruta).expect("assets/levels/enhanced.ron legible");
    let niveles: Vec<LevelSpec> = ron::from_str(&texto).expect("enhanced.ron parsea");
    niveles
        .into_iter()
        .find(|l| l.id == 61)
        .expect("existe el nivel 61 (firma de Omarchy)")
}

#[test]
fn el_nivel_de_omarchy_arranca_con_todas_las_bolas_en_celda_abierta() {
    let spec = cargar_nivel_omarchy();
    let s = GameState::new(spec);

    assert!(!s.balls.is_empty(), "el nivel debe tener bolas");
    for (i, b) in s.balls.iter().enumerate() {
        let (cx, cy) = (b.pos.x as u16, b.pos.y as u16);
        assert!(
            s.arena.grid.is_open(cx, cy),
            "la bola {i} nace dentro de un obstaculo del logo, en la celda ({cx},{cy})"
        );
    }
}

#[test]
fn el_logo_deja_espacio_jugable_suficiente() {
    let spec = cargar_nivel_omarchy();
    let (w, h) = (spec.arena.w as u32, spec.arena.h as u32);
    let s = GameState::new(spec);

    let libre = s.arena.grid.open_count() as f32 / (w * h) as f32;

    // Si el logo ocupara casi toda la arena, el nivel seria injugable.
    assert!(
        libre > 0.45,
        "el logo deja solo {:.0}% de arena libre; el nivel seria injugable",
        libre * 100.0
    );
}

#[test]
fn el_nivel_de_omarchy_simula_sin_panic_y_las_bolas_no_se_salen() {
    let spec = cargar_nivel_omarchy();
    let (aw, ah) = (spec.arena.w as f32, spec.arena.h as f32);
    let mut s = GameState::new(spec);

    // 30 segundos simulados a 60 FPS: las bolas rebotan contra el logo miles
    // de veces. Si la geometria del logo tuviera un hueco mal cerrado, una
    // bola acabaria fuera de la arena o dentro de un bloque.
    for _ in 0..1800 {
        let (next, _) = step(&s, PlayerInput::None, DT);
        s = next;

        if s.phase != GamePhase::Running {
            break;
        }
        for (i, b) in s.balls.iter().enumerate() {
            assert!(
                b.pos.x >= 0.0 && b.pos.y >= 0.0 && b.pos.x <= aw && b.pos.y <= ah,
                "la bola {i} se salio de la arena: ({}, {})",
                b.pos.x,
                b.pos.y
            );
            let (cx, cy) = (b.pos.x as u16, b.pos.y as u16);
            assert!(
                s.arena.grid.is_open(cx, cy),
                "la bola {i} acabo dentro de un bloque del logo, en ({cx},{cy})"
            );
        }
    }
}
