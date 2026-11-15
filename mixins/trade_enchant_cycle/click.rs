// HEAD helper for `Game::native_settings_click`.
fn tec_horizon_settings_click(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    button: usize,
) {
    let _ = game;
    if mod_id != crate::game::tec_core::ID {
        return;
    }
    crate::game::tec_hooks::settings_click(button);
}
