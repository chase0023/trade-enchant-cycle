// The hook entry points: what the two features do when Horizon calls them.
//
// Injected into `crates/horizon-client/src/game.rs` as `pub(crate) mod
// tec_hooks`. Every entry point is called from a thin helper file named in
// `horizon-mixin.toml`; this module holds the decisions, the drawing, the
// player feedback and the small amount of per-process state (confirmation,
// debounce, a bar clicked this frame, where the bar was drawn).

use crate::game::native_hooks::NativeScreenView;
use crate::game::tec_config as config;
use crate::game::tec_core as core;
use crate::game::Game;
use horizon_client::hud::HudBatch;
use horizon_client::text::Text;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use winit::keyboard::KeyCode;

/// The two screens the mod draws a Reroll bar in, as the server names their
/// menu types.
const ENCHANTING_MENU: &str = "minecraft:enchantment";
const MERCHANT_MENU: &str = "minecraft:merchant";

/// A bar was clicked this frame; the next client tick sends the request. The
/// byte is the payload the server reads back (`core::NET_ENCHANTING`,
/// `core::NET_MERCHANT`); 0 is nothing.
static PENDING: AtomicU8 = AtomicU8::new(0);

/// Where the bar was drawn last frame, as the panel's origin `[x, y]`.
///
/// The press hook and the render hook can see the same panel at two origins:
/// both centre it, but the window the game draws into and the window the input
/// arrives on need not be the same size (a title bar or a DPI change resizes
/// one before the other). Recording the drawn origin lets a press be mapped
/// onto what the player actually clicked. `NO_PANEL` until the bar is drawn.
static DRAWN_AT: AtomicU64 = AtomicU64::new(u64::MAX);

/// A bar press is in flight: the button went down on the mod's bar, so the
/// release that ends it is the bar's too. A container screen treats a release
/// outside its panel as `THROW` and drops the carried stack (`doClick` with
/// `slotId = -999`), which is exactly where the bar is — so the release is
/// cancelled as well, and clicking the bar never throws what the player holds.
static BAR_PRESSED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------- feedback

/// Sends one line about a reroll.
///
/// `player` is the guest whose request it is where the hook knows it; otherwise
/// the acting player is used. Both go out as the action bar (a line over the
/// hotbar) and fall back to chat. Outside a player's action the line goes to
/// the window's own chat, which is what a client-half hook has.
pub(in crate::game) fn notify(
    game: &mut Game<'_>,
    cfg: &core::Config,
    player: Option<u128>,
    text: &str,
    success: bool,
) {
    if !cfg.notify_enabled || (success && !cfg.show_success) || (!success && !cfg.show_errors) {
        return;
    }
    let uuid = player.or_else(|| game.native_acting_player().map(|p| p.uuid));
    if let Some(uuid) = uuid {
        let line = Text::literal(text);
        if game.native_action_bar(uuid, line.clone()) {
            return;
        }
        if game.native_tell(uuid, line) {
            return;
        }
        return;
    }
    game.native_chat(text);
}

// ---------------------------------------------------------------- state

/// The acting player's sneak state.
///
/// A guest's body is swapped into the game during its action, but its client's
/// key state lives on the guest (`ServerboundPlayerInputPacket`'s shift flag);
/// the host's own body keeps the window's keys.
fn sneaking(game: &Game<'_>) -> bool {
    if let Some(conn) = game.lan_acting
        && let Some(host) = game.lan.as_ref()
        && let Some(guest) = host.hub.guest(conn)
    {
        return guest.sneaking;
    }
    game.player.as_ref().is_some_and(|p| p.shift_key_down)
        || game.keys.contains(&KeyCode::ShiftLeft)
        || game.keys.contains(&KeyCode::ShiftRight)
}

/// Whether this villager may be rerolled now: one reroll per villager per tick,
/// and no other limit. The stamp is written on every attempt, not only on a
/// successful one, so the main-hand and off-hand call of the same click cannot
/// both act; two clicks, however quick, are 50 ms apart at the least and both
/// reroll.
fn attempt_ready(villager: u32, now: u64) -> bool {
    static LAST: OnceLock<Mutex<HashMap<u32, u64>>> = OnceLock::new();
    let map = LAST.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut map) = map.lock() else {
        return false;
    };
    let previous = map.get(&villager).copied().unwrap_or(0);
    if !core::cooldown_ready(now, previous, 1) {
        return false;
    }
    map.insert(villager, now);
    true
}

/// The two-click confirmation: the first click records the villager and says
/// what to do, a second one within `core::CONFIRM_TICKS` returns true.
fn confirmed(villager: u32, player: Option<u128>, now: u64) -> bool {
    static PENDING: OnceLock<Mutex<HashMap<u32, (Option<u128>, u64)>>> = OnceLock::new();
    let map = PENDING.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut map) = map.lock() else {
        return false;
    };
    match map.get(&villager).copied() {
        Some((who, since)) if who == player && core::confirmation_alive(now, since) => {
            map.remove(&villager);
            true
        }
        _ => {
            map.insert(villager, (player, now));
            false
        }
    }
}

// ---------------------------------------------------------------- villager

/// `native_interact_entity` (API 29): a player right-clicked an entity, on the
/// authoritative server, once per hand.
pub(in crate::game) fn interact_entity(
    game: &mut Game<'_>,
    entity: i32,
    kind: &str,
    claimed: &mut bool,
) {
    if *claimed || game.remote.is_some() {
        return;
    }
    // Nothing here for any other entity: a cow's click is not the mod's.
    if !matches!(kind, "minecraft:villager" | "minecraft:wandering_trader") {
        return;
    }
    if !game.native_on(core::ID) {
        return;
    }
    let cfg = config::get();
    // Off in the config: leave the click to vanilla, silently (a notification
    // on every villager would be noise).
    if core::villager_policy(&cfg).is_err() {
        return;
    }
    if cfg.villager_activation.needs_sneak() && !sneaking(game) {
        return;
    }
    let player = game.native_acting_player().map(|p| p.uuid);
    let now = game.ticks;
    if !attempt_ready(entity as u32, now) {
        return;
    }
    if cfg.require_confirmation && !confirmed(entity as u32, player, now) {
        // Claim the click so the trade screen does not open under the prompt;
        // the villager's offers are untouched.
        *claimed = true;
        let line = core::message(&core::Outcome::AwaitingConfirmation);
        notify(game, &cfg, player, &line, true);
        return;
    }
    let outcome = crate::game::trading::tec_villager::reroll(
        game,
        entity,
        kind,
        cfg.include_wandering_traders,
        cfg.keep_used_trades,
    );
    match outcome {
        // Only a real reroll takes the click. Everything else falls through to
        // vanilla, so a failed reroll still opens the trade screen.
        core::Outcome::Rerolled { .. } => *claimed = true,
        core::Outcome::NotAVillager => return,
        _ => {}
    }
    log::info!("{}: villager {entity}: {outcome:?}", core::NAME);
    let line = core::message(&outcome);
    notify(game, &cfg, player, &line, outcome.is_success());
}

// ---------------------------------------------------------------- screens

/// Whether the open menu is the enchanting table's.
///
/// The server's menu type is what the window shows; the deprecated in-frame
/// path has no server menu, so its own station is the fallback.
fn is_enchanting(menu: Option<&str>, station: Option<crate::game::stations::Kind>) -> bool {
    menu == Some(ENCHANTING_MENU) || station == Some(crate::game::stations::Kind::Enchanting)
}

/// Whether the open menu is a villager's.
fn is_merchant(menu: Option<&str>, station: Option<crate::game::stations::Kind>) -> bool {
    menu == Some(MERCHANT_MENU) || station == Some(crate::game::stations::Kind::Merchant)
}

/// The local station kind, for the in-frame path that has no server menu.
fn station_kind(game: &Game<'_>) -> Option<crate::game::stations::Kind> {
    game.station.as_ref().map(|station| station.kind)
}

/// The bar the open screen shows and the request its click sends, if the
/// screen has one and the config allows it.
fn bar_for(screen: &NativeScreenView<'_>, cfg: &core::Config) -> Option<u8> {
    if is_enchanting(screen.menu, None)
        && core::enchanting_policy(cfg).is_ok()
        && cfg.show_button
        && cfg.enchanting_activation.uses_button()
    {
        return Some(core::NET_ENCHANTING);
    }
    if is_merchant(screen.menu, None) && core::villager_policy(cfg).is_ok() && cfg.villager_screen_button
    {
        return Some(core::NET_MERCHANT);
    }
    None
}

/// The key that rerolls from the open menu, if one is bound for it.
///
/// The key is bound whenever the feature is on and the menu is its own: the
/// bar's label names the key (`Reroll [R]`), so the key has to work, and
/// `enchanting_activation` only decides whether the bar is drawn beside it.
/// Nothing outside those two menus answers — not the inventory, not the world,
/// not a menu with a text box — because the key is read from the open screen.
fn key_for(
    menu: Option<&str>,
    station: Option<crate::game::stations::Kind>,
    cfg: &core::Config,
    key: &str,
) -> Option<u8> {
    if is_enchanting(menu, station)
        && core::enchanting_policy(cfg).is_ok()
        && core::key_matches(&cfg.enchant_key, key)
    {
        return Some(core::NET_ENCHANTING);
    }
    if is_merchant(menu, station)
        && core::villager_policy(cfg).is_ok()
        && core::key_matches(&cfg.villager_key, key)
    {
        return Some(core::NET_MERCHANT);
    }
    None
}

/// The key each screen's bar names.
fn key_label<'a>(screen: &NativeScreenView<'_>, cfg: &'a core::Config) -> &'a str {
    if is_merchant(screen.menu, None) {
        &cfg.villager_key
    } else {
        &cfg.enchant_key
    }
}

/// Asks the server half for a reroll. The client never changes offers itself:
/// the server owns the seed, the menu data slots and the merchant's list.
fn send_request(game: &mut Game<'_>, cfg: &core::Config, request: u8) {
    match game.native_net_send_to_server(core::NET_CHANNEL, &[request]) {
        Ok(()) => {}
        Err(e) => {
            let line = format!("{}: {e}", core::NAME);
            notify(game, cfg, None, &line, false);
        }
    }
}

/// Sends `request` — there is no cooldown: every press and every click is sent,
/// and the bar and the key share this one path. What keeps a held key to one
/// reroll is the screen key hook's `repeat` flag, not a timer.
fn ask(game: &mut Game<'_>, cfg: &core::Config, request: u8) {
    send_request(game, cfg, request);
}

/// `native_screen_key_pressed_pre`: the shortcut, read from the screen it
/// belongs to.
///
/// This is the screen's own key hook (the analogue of Fabric's
/// `ScreenKeyboardEvents.allowKeyPress`): it only fires while a screen is open,
/// it says which screen, and it says whether the key is the press itself or the
/// key repeating while held — so one press is one reroll however long it is
/// held. The press is cancelled when it is the mod's, so nothing else (a
/// vanilla screen handler, another mod) acts on the same key.
///
/// The hook is `&self`, so like a bar click it only records the request; the
/// client tick sends it. That also means the key and the bar go through the
/// same request and the same server-side checks.
pub(in crate::game) fn screen_key(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    key: &str,
    repeat: bool,
    canceled: &mut bool,
) {
    if repeat || !game.native_on(core::ID) {
        return;
    }
    let cfg = config::get();
    let Some(request) = key_for(screen.menu, None, &cfg, key) else {
        return;
    };
    PENDING.store(request, Ordering::SeqCst);
    *canceled = true;
}

/// `native_key` (base API): the same shortcut, for the in-frame station path
/// that opens no server menu and so fires no screen key hook.
///
/// A press is one reroll: repeats and releases are ignored.
pub(in crate::game) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || game.remote.is_none() || !game.native_on(core::ID) {
        return;
    }
    let cfg = config::get();
    let menu = game.remote_menu_type();
    let station = station_kind(game);
    let Some(request) = key_for(menu, station, &cfg, key) else {
        return;
    };
    ask(game, &cfg, request);
}

/// What slot `slot` shows, or `None` when it is empty.
///
/// The hook is `&Game` and the game's own lookup (`slot_view`) needs `&mut Game`,
/// so the same places are read directly: a station's inputs and result, and the
/// player's own inventory, armour and off hand — the slots of the two menus the
/// bar belongs to.
fn slot_stack<'a>(game: &'a Game<'_>, slot: usize) -> Option<&'a horizon_client::items::ItemStack> {
    if slot >= crate::game::stations::INPUT {
        let station = game.station.as_ref()?;
        if slot == crate::game::stations::RESULT {
            return station.result.as_ref();
        }
        return station.inputs.get(slot - crate::game::stations::INPUT)?.as_ref();
    }
    if (100..109).contains(&slot) {
        return game.craft_grid.get(slot - 100)?.as_ref();
    }
    if slot == 504 {
        return game.offhand.as_ref();
    }
    if (500..504).contains(&slot) {
        return game.armor[slot - 500].as_ref();
    }
    game.inventory.get(slot)?.as_ref()
}

/// Puts the hovered item's tooltip back on top of the bar.
///
/// The game draws its tooltips *before* `native_screen_render_post`, so the bar
/// — always drawn, always in the same place — would be painted over one that
/// reaches down to it (an item in the panel's lowest row, or an item with a
/// tall tooltip). Rather than hiding the bar for those frames, the tooltip is
/// drawn again over it, which is the order vanilla itself uses: a tooltip is the
/// topmost thing on a screen and a widget underneath it is only partly visible.
///
/// The second draw is the game's own, line for line and pixel for pixel: the
/// same lines (`stack_tooltip`, the source `draw_slot_tooltip` uses), the same
/// geometry primitives and the same box and text drawing (`draw_tooltip_lines`),
/// at the same position. Only the tooltips that actually reach the bar are drawn
/// again, so nothing is duplicated where it does not matter.
fn restore_tooltip(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
    bar: [f32; 4],
) {
    let [x, y] = screen.mouse;
    let Some(slot) = game.slot_at(batch, x, y) else {
        return;
    };
    let Some(stack) = slot_stack(game, slot) else {
        return;
    };
    // `draw_slot_tooltip` keeps the tooltip off a carried stack that cannot take
    // this one; the same rule, so the redraw adds nothing the game left out.
    if game.carried.is_some() && !game.tooltip_with_carried(stack) {
        return;
    }
    let lines = game.stack_tooltip(stack);
    if lines.is_empty() {
        return;
    }
    let (_, top, _, height) = Game::tooltip_geometry(
        &game.gui.atlas,
        &lines,
        (x, y),
        (batch.width, batch.height),
        [0.0; 2],
    );
    // Reaching the bar is what matters; clear of it, the tooltip is untouched.
    if top + height <= bar[1] || top >= bar[1] + bar[3] {
        return;
    }
    game.draw_tooltip_lines(batch, &lines, x, y);
}

/// `native_screen_mouse_button_pressed_pre` (API 80): a bar drawn by the mod
/// was pressed. The hook is `&self`, so it only records the press; the next
/// tick sends the request.
pub(in crate::game) fn screen_press(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if button != 0 || !game.native_on(core::ID) {
        return;
    }
    // A press that is not the bar's leaves no release to cancel.
    BAR_PRESSED.store(false, Ordering::SeqCst);
    let cfg = config::get();
    let Some(request) = bar_for(screen, &cfg) else {
        return;
    };
    if !hit_bar(screen, screen.rect) {
        return;
    }
    PENDING.store(request, Ordering::SeqCst);
    BAR_PRESSED.store(true, Ordering::SeqCst);
    // The press is the bar's: no slot, no drag, no hotbar swap.
    *canceled = true;
}

/// `native_screen_mouse_button_released_pre`: the release that ends a press
/// the bar took is the bar's too.
///
/// Without this the screen would see a release it never saw pressed, outside
/// its panel, and take it as a click into the void — `AbstractContainerMenu`
/// `THROW`, which drops the whole carried stack. The press hook is `&self`, so
/// it records what it took in `BAR_PRESSED` and this reads it back.
pub(in crate::game) fn screen_release(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    let _ = (game, screen);
    if button != 0 {
        return;
    }
    if BAR_PRESSED.swap(false, Ordering::SeqCst) {
        *canceled = true;
    }
}

/// `native_tick`: the client half sends what a press or a key recorded.
pub(in crate::game) fn tick(game: &mut Game<'_>) {
    let request = PENDING.swap(0, Ordering::SeqCst);
    if request == 0 || game.remote.is_none() || !game.native_on(core::ID) {
        return;
    }
    let cfg = config::get();
    ask(game, &cfg, request);
}

/// `native_screen_render_post` (API 78): draws the open screen's Reroll bar,
/// after the panel the game has already drawn.
pub(in crate::game) fn render_post(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &NativeScreenView<'_>,
) {
    if !game.native_on(core::ID) {
        return;
    }
    let cfg = config::get();
    let Some(_) = bar_for(screen, &cfg) else {
        return;
    };
    DRAWN_AT.store(pack(screen.rect[0], screen.rect[1]), Ordering::Relaxed);
    let bar = core::bar_rect(screen.rect);
    let hovered = hit_bar(screen, screen.rect);
    let label = core::bar_label(Some(key_label(screen, &cfg)));
    draw_bar(game, batch, bar, &label, hovered);
    // The bar is always there; a tooltip that reaches it goes on top of it.
    restore_tooltip(game, batch, screen, bar);
}

/// One of the mod's bars: a black outline, a lit top-left edge and a shaded
/// bottom-right one over a grey face, and the label centred in it — vanilla's
/// own widget shading, from fills the game already knows how to draw.
fn draw_bar(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], label: &str, hovered: bool) {
    let [x, y, w, h] = rect;
    let face = if hovered { 0xFF_A2_A2_A2 } else { 0xFF_8B_8B_8B };
    game.native_fill(batch, [x, y, w, h], 0xFF_00_00_00);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], face);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, 1.0], 0xFF_E6_E6_E6);
    game.native_fill(batch, [x + 1.0, y + 1.0, 1.0, h - 2.0], 0xFF_E6_E6_E6);
    game.native_fill(batch, [x + 1.0, y + h - 2.0, w - 2.0, 1.0], 0xFF_4A_4A_4A);
    game.native_fill(batch, [x + w - 2.0, y + 1.0, 1.0, h - 2.0], 0xFF_4A_4A_4A);
    let text = game.native_text_width(label);
    let at = [
        (x + (w - text) / 2.0).floor(),
        (y + (h - 8.0) / 2.0).floor(),
    ];
    game.native_text(batch, label, at, 0xFF_FF_FF, true);
}

/// Whether the pointer is on the bar `bar_rect` places for `panel`.
///
/// Two readings of the pointer are tried, because the game's drawing and its
/// input can disagree about where the panel is: the panel as drawn last frame
/// (its recorded origin) and the panel as this event's own screen places it. A
/// window resized, a DPI change or a title bar makes one of them lie; they
/// differ by a few pixels at most, and the bar is under the panel's bottom
/// edge, where no slot or button of either reading can be.
fn hit_bar(screen: &NativeScreenView<'_>, panel: [f32; 4]) -> bool {
    let pointer = screen.mouse;
    let drawn = unpack(DRAWN_AT.load(Ordering::Relaxed)).unwrap_or([panel[0], panel[1]]);
    let candidates = [
        [pointer[0] - drawn[0], pointer[1] - drawn[1]],
        [pointer[0] - panel[0], pointer[1] - panel[1]],
    ];
    candidates
        .iter()
        .any(|&local| core::bar_hit(panel, local))
}

/// Two `f32` GUI coordinates in one atomic word.
fn pack(x: f32, y: f32) -> u64 {
    (u64::from(x.to_bits()) << 32) | u64::from(y.to_bits())
}

fn unpack(packed: u64) -> Option<[f32; 2]> {
    if packed == u64::MAX {
        return None;
    }
    let x = f32::from_bits((packed >> 32) as u32);
    let y = f32::from_bits(packed as u32);
    (x.is_finite() && y.is_finite()).then_some([x, y])
}

/// `native_net_from_client` (API 40): the client half's reroll request, heard
/// on the server. Everything it asks for is checked here again: the channel,
/// the one-byte screen it names, the config, and the player's own open menu.
pub(in crate::game) fn net_from_client(
    game: &mut Game<'_>,
    player: u128,
    channel: &str,
    data: &[u8],
) {
    if channel != core::NET_CHANNEL || game.remote.is_some() || !game.native_on(core::ID) {
        return;
    }
    let cfg = config::get();
    let outcome = match data {
        [core::NET_ENCHANTING] => {
            if let Err(refusal) = core::enchanting_policy(&cfg) {
                let line = core::message(&refusal);
                notify(game, &cfg, Some(player), &line, false);
                return;
            }
            crate::game::lan_stations::tec_enchanting::reroll(game, player)
        }
        [core::NET_MERCHANT] => {
            if let Err(refusal) = core::villager_policy(&cfg) {
                let line = core::message(&refusal);
                notify(game, &cfg, Some(player), &line, false);
                return;
            }
            crate::game::lan_trading::tec_merchant::reroll(game, player, cfg.keep_used_trades)
        }
        _ => return,
    };
    log::info!(
        "{}: request from {player} for {:?}: {outcome:?}",
        core::NAME,
        data
    );
    let line = core::message(&outcome);
    notify(game, &cfg, Some(player), &line, outcome.is_success());
}

// ---------------------------------------------------------------- mods page

/// The Options > Mods row, with the player's on/off switch.
pub(in crate::game) fn listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: core::ID.into(),
        title: format!("{} {}", core::NAME, core::VERSION),
        description: core::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}

/// The mod's own settings rows.
pub(in crate::game) fn settings_buttons() -> Vec<String> {
    config::settings_buttons()
}

/// A click on one of them.
pub(in crate::game) fn settings_click(button: usize) {
    config::settings_click(button);
}
