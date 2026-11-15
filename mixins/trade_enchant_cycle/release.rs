// HEAD helper for `Game::native_screen_mouse_button_released_pre`.
fn tec_horizon_screen_release(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::tec_hooks::screen_release(game, screen, button, canceled);
}
