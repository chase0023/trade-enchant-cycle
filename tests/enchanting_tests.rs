// Enchanting table cycling: the reroll policy, tested without the game.
//
// The offers themselves are Horizon's own port of vanilla's
// `EnchantmentMenu.getEnchantmentList` (`horizon_client::stations::
// enchant_offers`), driven by the player's `xp_seed`, the bookshelves around
// the table and the item. A reroll only picks the next seed with the game's
// random and asks that code to fill the menu again -- so lapis, levels, the
// item and the enchantment cannot be spent by it.
//
// The game-side module `bundle/enchanting.rs` also carries `#[cfg(test)]`
// tests that run the real generator on a diamond sword, pickaxe, chestplate,
// bow and book (see README section 7).

use tec_core::{
    apply_settings, bar_hit, bar_label, bar_rect, cooldown_ready, enchanting_policy, key_matches, message,
    settings_action, settings_labels, Config, EnchantActivation, Outcome, SettingsAction,
    BAR_HEIGHT, BAR_INSET, BUTTON_LABEL,
};

// ------------------------------------------------------------ config

#[test]
fn the_default_config_matches_the_documented_example() {
    let cfg = Config::default();
    assert!(cfg.enchanting_enabled);
    assert_eq!(cfg.enchanting_activation, EnchantActivation::InterfaceButton);
    assert!(cfg.free_rerolls);
    assert_eq!(cfg.reroll_cost, 0, "a reroll never charges anything");
    assert!(cfg.show_button);
    assert_eq!(cfg.enchant_key, "key.keyboard.r");
    assert!(cfg.notify_enabled && cfg.show_success && cfg.show_errors);
}

#[test]
fn rerolls_have_no_cooldown_and_no_cooldown_setting() {
    // There is no wait between rerolls any more, and no key that could set one:
    // the file the previous version wrote keeps its `cooldown_ticks` and it is
    // simply ignored, which is what makes an upgrade drop the delay.
    let cfg = Config::from_json(
        r#"{"villager_trade_cycle":{"cooldown_ticks":10},"enchanting_cycle":{"cooldown_ticks":4}}"#,
    )
    .unwrap();
    assert_eq!(cfg, Config::default());
    assert!(!Config::default().to_json().contains("cooldown_ticks"));
    // The guards the game keeps are "not in the same tick", not a wait.
    assert!(!cooldown_ready(100, 100, 1), "one reroll per tick");
    assert!(cooldown_ready(101, 100, 1));
}

#[test]
fn the_written_config_has_the_documented_groups_and_keys() {
    let text = Config::default().to_json();
    let root = tec_core::parse(&text).unwrap();
    for (group, keys) in [
        (
            "villager_trade_cycle",
            &[
                "enabled",
                "activation",
                "require_confirmation",
                "keep_used_trades",
                "include_wandering_traders",
            ][..],
        ),
        (
            "enchanting_cycle",
            &[
                "enabled",
                "activation",
                "free_rerolls",
                "reroll_cost",
                "show_button",
                "key",
            ][..],
        ),
        (
            "notifications",
            &["enabled", "show_success", "show_errors"][..],
        ),
    ] {
        let group_value = root.get(group).unwrap_or_else(|| panic!("{group} is missing"));
        for key in keys {
            assert!(group_value.get(key).is_some(), "{group}.{key} is missing");
        }
    }
    // The exact example from the request still parses into the same values.
    let example = r#"{
      "villager_trade_cycle": {
        "enabled": true,
        "activation": "sneak_right_click",
        "require_confirmation": false
      },
      "enchanting_cycle": {
        "enabled": true,
        "activation": "interface_button",
        "free_rerolls": true,
        "reroll_cost": 0
      },
      "notifications": {
        "enabled": true,
        "show_success": true,
        "show_errors": true
      }
    }"#;
    let cfg = Config::from_json(example).unwrap();
    assert_eq!(cfg.villager_activation, tec_core::Activation::SneakRightClick);
    assert_eq!(cfg.enchanting_activation, EnchantActivation::InterfaceButton);
    assert_eq!(tec_core::warnings(&cfg), Vec::<String>::new());
}

#[test]
fn a_reroll_cost_is_refused_rather_than_charged() {
    let mut cfg = Config::default();
    cfg.reroll_cost = 3;
    assert_eq!(enchanting_policy(&cfg), Err(Outcome::CostUnsupported));
    assert_eq!(
        message(&Outcome::CostUnsupported),
        "The config asks for a reroll cost; this build only does free rerolls."
    );
    assert!(!tec_core::warnings(&cfg).is_empty());

    let mut cfg = Config::default();
    cfg.free_rerolls = false;
    assert_eq!(enchanting_policy(&cfg), Err(Outcome::CostUnsupported));

    let mut cfg = Config::default();
    cfg.enchanting_enabled = false;
    assert_eq!(enchanting_policy(&cfg), Err(Outcome::FeatureDisabled));
    assert_eq!(enchanting_policy(&Config::default()), Ok(()));
}

#[test]
fn activation_codes_select_the_button_the_key_or_both() {
    for activation in [
        EnchantActivation::InterfaceButton,
        EnchantActivation::Key,
        EnchantActivation::Both,
    ] {
        let cfg = Config::from_json(&format!(
            r#"{{"enchanting_cycle":{{"activation":"{}"}}}}"#,
            activation.code()
        ))
        .unwrap();
        assert_eq!(cfg.enchanting_activation, activation);
    }
    assert!(EnchantActivation::InterfaceButton.uses_button());
    assert!(!EnchantActivation::Key.uses_button(), "key only: no bar");
    assert!(EnchantActivation::Both.uses_button());
    assert_eq!(EnchantActivation::from_code("gesture"), None);
}

#[test]
fn key_only_activation_leaves_the_bar_off_but_not_the_shortcut() {
    // The shortcut is no longer a config toggle: the bar's label names it, so
    // it answers in the table's screen whatever `activation` says. `activation`
    // decides whether the bar is drawn (`bundle/hooks.rs`, `key_for`/`bar_for`).
    let mut cfg = Config::default();
    cfg.enchanting_activation = EnchantActivation::Key;
    assert!(!cfg.enchanting_activation.uses_button());
}

// ------------------------------------------------------------ the shortcut

#[test]
fn the_shortcut_accepts_the_vanilla_key_name_or_the_letter() {
    assert!(key_matches("key.keyboard.r", "key.keyboard.r"));
    assert!(key_matches("r", "key.keyboard.r"));
    assert!(key_matches("R", "key.keyboard.R"));
    assert!(key_matches(" key.keyboard.r ", "key.keyboard.r"));
    assert!(!key_matches("key.keyboard.r", "key.keyboard.q"));
    assert!(!key_matches("key.keyboard.r", "key.mouse.left"));
    assert!(!key_matches("", "key.keyboard.r"));
    assert!(!key_matches("key.keyboard.r", ""));
    // A digit or function key works the same way.
    assert!(key_matches("1", "key.keyboard.1"));
    assert!(key_matches("key.keyboard.f7", "key.keyboard.f7"));
}

// ------------------------------------------------------------ the button

#[test]
fn the_reroll_bar_is_a_footer_on_each_panel_and_clears_every_vanilla_element() {
    // Both panels are 166 high and are centred in the window, at every GUI
    // scale: the bar is placed from the panel, never from the window.
    for (width, height) in [(176.0, 166.0), (276.0, 166.0)] {
        let panel = [(320.0 - width) / 2.0, (240.0 - height) / 2.0, width, height];
        let bar = bar_rect(panel);
        assert_eq!(bar[1], panel[1] + panel[3], "on the panel's bottom edge");
        assert_eq!(bar[0], panel[0] + BAR_INSET, "inset from its left edge");
        assert_eq!(bar[2], panel[2] - 2.0 * BAR_INSET, "inset on both sides");
        assert_eq!(bar[3], BAR_HEIGHT);
        // The last thing either panel draws is its hotbar row (y=142..160) and
        // then its own border, so nothing of vanilla's is under the footer.
        assert!(bar[1] >= panel[1] + 160.0, "below the hotbar row");
        // 16 px of label is room for vanilla's 8 px font plus its border.
        assert!(bar[3] >= 13.0, "room for the label");
        // Centred in the window with room above and below: never clipped, even
        // at the smallest GUI the game allows.
        assert!(bar[1] + bar[3] <= 240.0, "inside the window at 320x240");
        assert!(bar[0] >= 0.0 && bar[0] + bar[2] <= 320.0, "inside 320 wide");
    }
    // A 176-wide panel is smaller than a 276-wide one, so the footer's width
    // follows the panel rather than being a screen constant.
    let small = bar_rect([72.0, 37.0, 176.0, 166.0]);
    let large = bar_rect([22.0, 37.0, 276.0, 166.0]);
    assert!(large[2] > small[2]);
    assert_eq!(small[3], large[3]);
}

#[test]
fn the_bar_follows_the_panel_wherever_the_game_puts_it() {
    // Two origins a DPI change or a different window size produces: the bar
    // moves with the panel and keeps its place under it.
    for panel in [
        [72.0, 37.0, 176.0, 166.0],
        [0.0, 0.0, 176.0, 166.0],
        [434.0, 300.0, 176.0, 166.0],
    ] {
        let bar = bar_rect(panel);
        assert_eq!(bar[0] - panel[0], BAR_INSET);
        assert_eq!(bar[0] + bar[2], panel[0] + panel[2] - BAR_INSET);
        assert_eq!(bar[1], panel[1] + panel[3]);
    }
}

#[test]
fn a_click_is_on_the_bar_only_within_its_own_bounds() {
    let panel = [72.0, 37.0, 176.0, 166.0];
    let bar = bar_rect(panel);
    let local = |x: f32, y: f32| [x - panel[0], y - panel[1]];
    assert!(bar_hit(panel, local(bar[0] + 1.0, bar[1] + 1.0)));
    assert!(bar_hit(
        panel,
        local(bar[0] + bar[2] - 0.1, bar[1] + bar[3] - 0.1)
    ));
    assert!(
        bar_hit(panel, local(bar[0] + 4.0, panel[1] + panel[3] + 1.0)),
        "a click just under the panel's edge is on the bar"
    );
    assert!(!bar_hit(panel, local(bar[0] - 2.0, bar[1] + 2.0)), "left of it");
    assert!(
        !bar_hit(panel, local(bar[0] + 2.0, bar[1] - 2.0)),
        "the panel's own hotbar row above it"
    );
    assert!(
        !bar_hit(panel, local(bar[0] + 2.0, bar[1] + bar[3] + 2.0)),
        "below it"
    );
    assert!(!bar_hit(panel, local(110.0, 100.0)), "the inventory grid");
}

#[test]
fn the_bar_names_the_key_that_does_the_same_thing() {
    assert_eq!(bar_label(Some("key.keyboard.r")), "Reroll [R]");
    assert_eq!(bar_label(Some("r")), "Reroll [R]");
    assert_eq!(bar_label(Some("R")), "Reroll [R]");
    assert_eq!(bar_label(None), "Reroll");
    assert_eq!(bar_label(Some("key.keyboard.left.shift")), "Reroll");
    assert_eq!(BUTTON_LABEL, "Reroll");
}

#[test]
fn a_cooldown_passes_exactly_when_it_is_over() {
    assert!(cooldown_ready(100, 0, 0), "no cooldown always passes");
    assert!(!cooldown_ready(10, 10, 4));
    assert!(!cooldown_ready(13, 10, 4));
    assert!(cooldown_ready(14, 10, 4));
    assert!(cooldown_ready(0, 0, 4) == false);
    // A clock that went backwards (a reloaded world) cannot block forever.
    assert!(cooldown_ready(5, 100, 4));
}

// ------------------------------------------------------------ settings page

#[test]
fn the_mods_page_toggles_the_features_and_reloads_the_file() {
    let cfg = Config::default();
    let labels = settings_labels(&cfg);
    assert_eq!(labels[0], "Villager trade cycling: on");
    assert_eq!(labels[1], "Villager Reroll bar: on");
    assert_eq!(labels[2], "Enchanting cycling: on");
    assert_eq!(labels[3], "Enchanting Reroll bar: on");
    assert_eq!(labels[4], "Notifications: on");
    assert_eq!(labels[5], "Reload config file");

    let next = apply_settings(&cfg, SettingsAction::ToggleVillager);
    assert!(!next.villager_enabled && next.enchanting_enabled);
    let next = apply_settings(&cfg, SettingsAction::ToggleVillagerButton);
    assert!(!next.villager_screen_button && next.show_button);
    let next = apply_settings(&cfg, SettingsAction::ToggleEnchanting);
    assert!(!next.enchanting_enabled && next.villager_enabled);
    let next = apply_settings(&cfg, SettingsAction::ToggleEnchantButton);
    assert!(!next.show_button && next.villager_screen_button);
    let next = apply_settings(&cfg, SettingsAction::ToggleNotifications);
    assert!(!next.notify_enabled);
    // Reload keeps what is in memory; the file is read by the caller.
    assert_eq!(apply_settings(&cfg, SettingsAction::Reload), cfg);

    assert_eq!(settings_action(0), Some(SettingsAction::ToggleVillager));
    assert_eq!(settings_action(1), Some(SettingsAction::ToggleVillagerButton));
    assert_eq!(settings_action(2), Some(SettingsAction::ToggleEnchanting));
    assert_eq!(settings_action(3), Some(SettingsAction::ToggleEnchantButton));
    assert_eq!(settings_action(4), Some(SettingsAction::ToggleNotifications));
    assert_eq!(settings_action(5), Some(SettingsAction::Reload));
    assert_eq!(settings_action(6), None);

    let off = apply_settings(&cfg, SettingsAction::ToggleVillager);
    assert_eq!(settings_labels(&off)[0], "Villager trade cycling: off");
}

// ------------------------------------------------------------ what a reroll is

#[test]
fn a_reroll_spends_nothing_and_applies_nothing() {
    // The policy only lets through a free reroll, and the mod's own plan type
    // (see `RerollPlan`) carries indices, never a cost, an item or an
    // enchantment. The game-side tests in bundle/enchanting.rs check the same
    // property against the real offer generator: the item is read, never
    // changed, and the next click is what applies an enchantment.
    let cfg = Config::default();
    assert_eq!(enchanting_policy(&cfg), Ok(()));
    assert_eq!(cfg.reroll_cost, 0);
    assert!(cfg.free_rerolls);
    assert_eq!(
        message(&Outcome::NoMenu),
        "Open an enchanting table first."
    );
    assert_eq!(
        message(&Outcome::NotEnchantable),
        "Put an enchantable, not yet enchanted item in the table's slot."
    );
    assert_eq!(message(&Outcome::EnchantingRerolled), "Enchanting offers rerolled.");
    assert!(Outcome::EnchantingRerolled.is_success());
    assert!(message(&Outcome::Rerolled { added: 3 }).contains("new offers"));
}

#[test]
fn every_refusal_explains_itself() {
    for outcome in [
        Outcome::FeatureDisabled,
        Outcome::UnsupportedServer,
        Outcome::CostUnsupported,
        Outcome::NoMenu,
        Outcome::NotEnchantable,
        Outcome::Cooldown,
    ] {
        let line = message(&outcome);
        assert!(!line.is_empty(), "{outcome:?}");
        assert!(line.len() < 200, "{outcome:?}: {line}");
    }
    assert!(message(&Outcome::UnsupportedServer).contains("server"));
}

#[test]
fn config_file_is_pretty_and_round_trips() {
    let text = Config::default().to_json();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.len() > 20, "the file is not indented: {text}");
    assert_eq!(lines[0], "{");
    assert_eq!(lines[1], "  \"villager_trade_cycle\": {");
    assert!(Config::from_json(&text).unwrap() == Config::default());
}
