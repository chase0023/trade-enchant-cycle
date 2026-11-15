// HEAD helper for `Game::native_settings`: the mod's own rows.
fn tec_horizon_settings(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    let _ = game;
    if mod_id != crate::game::tec_core::ID {
        return;
    }
    buttons.extend(crate::game::tec_hooks::settings_buttons());
}
