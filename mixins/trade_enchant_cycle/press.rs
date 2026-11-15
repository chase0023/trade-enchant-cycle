// HEAD helper for `Game::native_screen_mouse_button_pressed_pre` (API 80).
fn tec_horizon_screen_press(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    let _ = modifiers;
    crate::game::tec_hooks::screen_press(game, screen, button, canceled);
}
