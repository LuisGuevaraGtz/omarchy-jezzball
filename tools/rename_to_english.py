#!/usr/bin/env python3
"""Rename the Spanish identifiers in the Rust code to English.

It matches whole words (\\b) across every tracked .rs file. The map is
explicit on purpose: a "creative" regex rename is exactly what breaks a
refactor of this size without the tests noticing.

Usage:  python3 tools/rename_to_english.py [--dry-run]
"""
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

# --- Code identifiers (functions, variables, fields, types) ---
CODE = {
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
    # menus / lists
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

# --- Test names (full phrases) ---
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

# --- Second pass: test names the first mapping missed ---
TESTS_ROUND_2 = {
    "al_golpear_la_mitad_viva_sobrevive_la_anclada":
        "hitting_the_live_half_leaves_the_anchored_one_standing",
    "la_mitad_anclada_en_la_pared_es_inmune":
        "the_half_anchored_to_the_wall_is_immune",
    "arena_minima_no_panica": "minimal_arena_does_not_panic",
    "construir_arena_de_cualquier_tamano_no_panica":
        "building_an_arena_of_any_size_does_not_panic",
    "el_area_conquistada_es_monotona_y_valida":
        "the_conquered_area_is_monotonic_and_valid",
    "f32_en_rango_unitario": "f32_stays_in_the_unit_range",
    "fichero_inexistente_devuelve_limpio": "missing_file_returns_a_clean_save",
    "frente_hi_no_atraviesa_obstaculo_con_paso_grande":
        "hi_front_does_not_cross_an_obstacle_with_a_large_step",
    "frente_lo_no_atraviesa_obstaculo_con_paso_grande":
        "lo_front_does_not_cross_an_obstacle_with_a_large_step",
    "fuzz_indices_hostiles_no_panica": "fuzz_hostile_indices_does_not_panic",
    "fuzz_launch_cli_no_panica": "fuzz_cli_launch_does_not_panic",
    "guardar_y_cargar_atomico": "saving_and_loading_is_atomic",
    "hasta_consolidar": "until_consolidated",
    "instalacion_local_de_install_sh_converge_y_colapsa":
        "local_install_sh_layout_converges_and_collapses",
    "instalar_hook_de_panico": "install_panic_hook",
    "misma_semilla_misma_secuencia": "same_seed_same_sequence",
    "nav_repeat_cambio_de_direccion": "nav_repeat_on_direction_change",
    "nombre_de_tema_busca_en_sistema_y_usuario":
        "theme_name_is_looked_up_in_system_and_user_dirs",
    "nombre_malicioso_no_viaja": "malicious_theme_name_cannot_traverse",
    "nunca_invierte_ambos_ejes_fuera_de_una_esquina":
        "never_flips_both_axes_outside_a_corner",
    "override_con_hex_invalido_cae_al_paso_2":
        "override_with_invalid_hex_falls_through_to_step_2",
    "override_con_rol_faltante_cae_al_paso_2":
        "override_with_a_missing_role_falls_through_to_step_2",
    "override_de_usuario_gana": "user_override_wins",
    "override_hex_sin_almohadilla_y_mayusculas":
        "override_accepts_hex_without_hash_and_uppercase",
    "parse_hex_funciona": "parse_hex_works",
    "pasillo_estrecho_rebota_limpio": "narrow_corridor_bounces_cleanly",
    "rangos_extremos_no_panican": "extreme_ranges_do_not_panic",
    "rango_vacio_no_panica": "empty_range_does_not_panic",
    "rebote_conserva_la_rapidez_y_avanza_de_verdad":
        "bouncing_preserves_speed_and_actually_advances",
    "rebote_en_esquina_mantiene_la_bola_dentro":
        "corner_bounce_keeps_the_ball_inside",
    "rebote_no_teletransporta_ni_atraviesa_el_muro":
        "bouncing_neither_teleports_nor_crosses_the_wall",
    "retro82_plano": "retro82_flat_palette",
    "tokyo_night_plano": "tokyo_night_flat_palette",
    "semillas_distintas_difieren": "different_seeds_differ",
    "siempre_dentro_del_rango": "always_within_range",
    "sin_ficheros_devuelve_fallback_sin_panic":
        "with_no_files_it_returns_the_fallback_without_panicking",
    "sin_ninguna_ruta_la_carga_devuelve_vacio_sin_panic":
        "with_no_path_at_all_loading_returns_empty_without_panicking",
    "spawn_sobre_solid_se_reubica_a_la_abierta_mas_cercana":
        "spawn_on_solid_relocates_to_the_nearest_open_cell",
    "trayectoria_no_se_cicla_sobre_si_misma":
        "the_trajectory_does_not_loop_back_on_itself",
    "desbloqueo_original_estricto": "original_unlocking_is_strict",
    "bloque_exacto_de_theming_md_parsea": "exact_block_from_theming_md_parses",
    # Spanish identifiers left inside the code
    "ocupadas": "occupied",
    "celdas": "cells",
    "vulnerables": "vulnerable",
}

MAP = {**CODE, **TESTS, **TESTS_ROUND_2}


def ficheros_rust():
    salida = subprocess.run(
        ["git", "ls-files", "*.rs"],
        cwd=RAIZ, capture_output=True, text=True, check=True,
    ).stdout.split()
    return [RAIZ / f for f in salida]


def main():
    dry = "--dry-run" in sys.argv
    # Longest names first: this stops "clave" from mangling
    # "clave_desconocida_devuelve_la_propia_clave".
    claves = sorted(MAP, key=len, reverse=True)
    patron = re.compile(r"\b(" + "|".join(re.escape(k) for k in claves) + r")\b")

    total = 0
    for ruta in ficheros_rust():
        original = ruta.read_text(encoding="utf-8")
        nuevo, n = patron.subn(lambda m: MAP[m.group(1)], original)
        if n:
            total += n
            print(f"{n:5d}  {ruta.relative_to(RAIZ)}")
            if not dry:
                ruta.write_text(nuevo, encoding="utf-8")
    print(f"\n{'(dry run) ' if dry else ''}{total} replacements")


if __name__ == "__main__":
    main()
