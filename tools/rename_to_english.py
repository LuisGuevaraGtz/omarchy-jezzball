#!/usr/bin/env python3
"""Renombra a inglés los identificadores en español del código Rust.

Se aplica sobre palabras completas (\\b) en todos los .rs versionados. El mapa
es explícito a propósito: un renombrado por regex "creativo" es justo lo que
rompe un refactor de este tamaño sin que los tests lo noten.

Uso:  python3 tools/rename_to_english.py [--dry-run]
"""
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

# --- Identificadores de código (funciones, variables, campos, tipos) ---
CODIGO = {
    # i18n
    "Catalogo": "Catalog",
    "CATALOGOS": "CATALOGS",
    "IDIOMA_POR_DEFECTO": "DEFAULT_LANGUAGE",
    "catalogo_de": "catalog_for",
    "codigo_del_entorno": "code_from_env",
    "resolver_codigo": "resolve_code",
    "activo": "active",
    "parsear": "parse",
    "claves": "keys",
    "codigo": "code",
    "textos": "texts",
    "clave": "key",
    # menús / listas
    "ajustar_ventana": "adjust_scroll_window",
    "filas_visibles_para": "visible_rows_for",
    "filas_visibles": "visible_rows",
    "fila_alto": "row_height",
    "lista_y": "list_y",
    "primero": "first",
    "visibles": "visible",
    "disponible": "available",
    # test helpers
    "literales": "string_literals",
    "permitido": "is_allowed",
    "parece_texto_visible": "looks_like_ui_text",
    "hallazgos": "findings",
    "FUENTES": "SOURCES",
    "NO_INTERFAZ": "NOT_UI",
    "EXCEPCIONES": "EXCEPTIONS",
    "raiz": "root",
    "ruta": "path",
    "contenido": "contents",
    "codigo_fuente": "source",
    "linea": "line",
    "idioma": "language",
    "referencia": "reference",
    "faltan": "missing",
    "sobran": "extra",
    "otro": "other",
    "limpio": "trimmed",
    "palabras": "words",
    "actual": "current",
    "dentro": "inside",
    "previo": "prev",
    "titulo": "title",
    "lineas": "lines",
    "cuerpo": "body",
    "selladas": "sealed_cells",
    "vulnerables": "vulnerable",
    "digitos": "digits",
    "escala": "scale",
    "alto": "height",
    "ancho": "width",
    "bloques": "blocks",
    "libres": "free_cells",
}

# --- Nombres de test (frases completas) ---
TESTS = {
    "arena_sin_celdas_abiertas_descarta_la_bola":
        "arena_with_no_open_cells_discards_the_ball",
    "celda_rellenada_detras_no_empuja_a_la_bola":
        "filled_cell_behind_does_not_push_the_ball",
    "cierra_solo_la_region_sin_bolas":
        "closes_only_the_region_without_balls",
    "con_ambas_mitades_ancladas_el_muro_parte_el_area":
        "with_both_halves_anchored_the_wall_splits_the_area",
    "ninguna_bola_queda_dentro_de_celda_cerrada":
        "no_ball_ends_up_inside_a_closed_cell",
    "el_nivel_de_omarchy_arranca_con_todas_las_bolas_en_celda_abierta":
        "the_omarchy_level_starts_with_every_ball_on_an_open_cell",
    "el_nivel_de_omarchy_simula_sin_panic_y_las_bolas_no_se_salen":
        "the_omarchy_level_simulates_without_panic_and_balls_stay_inside",
    "el_logo_deja_espacio_jugable_suficiente":
        "the_logo_leaves_enough_playable_space",
    "cargar_nivel_omarchy": "load_omarchy_level",
    "la_ayuda_tiene_todas_las_paginas_con_contenido":
        "every_help_page_has_content",
    "la_capa_de_render_no_tiene_textos_incrustados":
        "the_render_layer_has_no_hardcoded_text",
    "los_catalogos_de_idioma_existen_en_el_repositorio":
        "the_language_catalogs_exist_in_the_repository",
    "la_ventana_de_lista_sigue_a_la_seleccion":
        "the_list_window_follows_the_selection",
    "la_regla_vale_para_muros_horizontales":
        "the_rule_holds_for_horizontal_walls",
    "las_celdas_cubiertas_son_siempre_validas":
        "covered_cells_are_always_valid",
    "muro_en_arena_abierta_cubre_toda_la_linea":
        "wall_in_open_arena_covers_the_whole_line",
    "muro_interior_vertical_invierte_solo_vx":
        "inner_vertical_wall_flips_only_vx",
    "fuzz_niveles_sinteticos_no_panica":
        "fuzz_synthetic_levels_does_not_panic",
    "fuzz_progresion_completando_niveles_no_panica":
        "fuzz_progression_completing_levels_does_not_panic",
    "clave_desconocida_devuelve_la_propia_clave":
        "unknown_key_returns_the_key_itself",
    "todos_los_catalogos_parsean": "every_catalog_parses",
    "todos_los_idiomas_tienen_las_mismas_claves":
        "every_language_has_the_same_keys",
    "resuelve_el_idioma_del_entorno": "resolves_the_language_from_the_env",
    "la_escotilla_del_juego_gana_al_locale_del_sistema":
        "the_game_override_wins_over_the_system_locale",
    "enhanced_reconoce_puerta_original": "enhanced_honours_the_original_gate",
    "enhanced_bloqueado_en_el_menu_hasta_terminar_original":
        "enhanced_is_locked_in_the_menu_until_original_is_finished",
    "desbloqueo_original_estricto": "original_unlocking_is_strict",
    "corrupto_hace_backup_y_empieza_limpio":
        "corrupt_save_is_backed_up_and_starts_clean",
    "alacritty_como_ultimo_recurso": "alacritty_as_last_resort",
    "bloque_exacto_de_theming_md_parsea": "exact_block_from_theming_md_parses",
    "current_colors_toml_del_estado": "current_colors_toml_from_state",
    "mapeo_de_teclas_claves": "key_mapping_is_correct",
    "assets_reales_via_variable_de_entorno_simulada":
        "real_assets_via_simulated_env_var",
    "assets_reales_via_exe_dir_simulado_apuntando_al_repo":
        "real_assets_via_simulated_exe_dir_pointing_at_the_repo",
    "los_assets_reales_se_parsean": "the_real_assets_parse",
    "ningun_spawn_de_los_assets_reales_cae_en_celda_no_abierta":
        "no_spawn_in_the_real_assets_lands_on_a_non_open_cell",
    "ningun_candidato_contiene_el_segmento_assets_equivocado":
        "no_candidate_contains_the_wrong_assets_segment",
    "orden_de_candidatos_seis_rutas_documentadas":
        "candidate_order_matches_the_six_documented_paths",
}

MAPA = {**CODIGO, **TESTS}


def ficheros_rust():
    salida = subprocess.run(
        ["git", "ls-files", "*.rs"],
        cwd=RAIZ, capture_output=True, text=True, check=True,
    ).stdout.split()
    return [RAIZ / f for f in salida]


def main():
    dry = "--dry-run" in sys.argv
    # Los nombres largos primero: evita que "clave" destroce
    # "clave_desconocida_devuelve_la_propia_clave".
    claves = sorted(MAPA, key=len, reverse=True)
    patron = re.compile(r"\b(" + "|".join(re.escape(k) for k in claves) + r")\b")

    total = 0
    for ruta in ficheros_rust():
        original = ruta.read_text(encoding="utf-8")
        nuevo, n = patron.subn(lambda m: MAPA[m.group(1)], original)
        if n:
            total += n
            print(f"{n:5d}  {ruta.relative_to(RAIZ)}")
            if not dry:
                ruta.write_text(nuevo, encoding="utf-8")
    print(f"\n{'(simulacion) ' if dry else ''}{total} sustituciones")


if __name__ == "__main__":
    main()
