// HEAD helper for `Game::native_screen_render_post` (API 78).
fn tec_horizon_render_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    crate::game::tec_hooks::render_post(game, batch, screen);
}
