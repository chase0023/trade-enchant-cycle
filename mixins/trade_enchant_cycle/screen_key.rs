// HEAD helper for `Game::native_screen_key_pressed_pre` (the open screen's own
// key hook, the analogue of Fabric's `ScreenKeyboardEvents.allowKeyPress`).
fn tec_horizon_screen_key(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    key: &str,
    repeat: bool,
    canceled: &mut bool,
) {
    crate::game::tec_hooks::screen_key(game, screen, key, repeat, canceled);
}
