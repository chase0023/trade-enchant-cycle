// Villager trade cycling: the reroll rules, tested without the game.
//
// The offer generator itself is Horizon's own port of vanilla's
// `AbstractVillager.addOffersFromTradeSet` (with the data-driven
// `villager_trade`/`trade_set` registries and the world's `RandomSequences`);
// the native mod reuses it and only decides which offers to keep and which
// fresh rolls to add. That decision is what these tests cover.

use tec_core::{
    apply_plan, has_unlocked, message, plan_reroll, trades_set_id, Activation, Config, Json,
    OfferKey, Outcome, RerollPlan, CONFIRM_TICKS, MAX_LEVEL, WANDERING_SETS,
};

fn offer(result: &str, cost_a: u32, locked: bool) -> OfferKey {
    OfferKey {
        locked,
        result: result.to_string(),
        result_count: 1,
        result_enchantments: Vec::new(),
        cost_a: "minecraft:emerald".into(),
        cost_a_count: cost_a,
        cost_b: None,
    }
}

fn enchanted(result: &str, enchantment: &str, level: u32) -> OfferKey {
    OfferKey {
        result_enchantments: vec![(enchantment.to_string(), level)],
        ..offer(result, 1, false)
    }
}

// ------------------------------------------------------------ trade stages

#[test]
fn a_villager_with_no_unlocked_trades_cannot_be_rerolled() {
    let existing = [offer("minecraft:bread", 1, true), offer("minecraft:wheat", 1, true)];
    assert!(!has_unlocked(&existing));
    let fresh = [offer("minecraft:apple", 1, false)];
    let plan = plan_reroll(&existing, &fresh, true);
    assert!(plan.added.is_empty(), "nothing may replace a used offer");
    let merged = apply_plan(&existing, &fresh, &plan);
    assert_eq!(merged, existing.to_vec());
    assert_eq!(
        message(&Outcome::NothingToReroll),
        "No unlocked trades to reroll: every offer here was already used."
    );
}

#[test]
fn untouched_offers_are_replaced_and_used_ones_are_kept() {
    let existing = [offer("minecraft:bread", 1, false), offer("minecraft:wheat", 2, true)];
    let fresh = [offer("minecraft:apple", 1, false), offer("minecraft:potato", 2, false)];
    let plan = plan_reroll(&existing, &fresh, true);
    assert_eq!(plan.kept, vec![1], "the used wheat stays");
    // Two offers existed and one stays, so one fresh roll fills the list: the
    // merchant's offer count does not grow with rerolls.
    assert_eq!(plan.added, vec![0]);
    let merged = apply_plan(&existing, &fresh, &plan);
    assert_eq!(merged.len(), existing.len(), "the merchant's list keeps its length");
    assert_eq!(merged[0].result, "minecraft:wheat");
    assert_eq!(merged[1].result, "minecraft:apple");
}

#[test]
fn a_completed_trade_is_locked_and_never_reset() {
    // `out_of_stock` is `uses >= maxUses`; the mod maps both a completed and a
    // partly used offer to `locked`, so neither is given back.
    let mut completed = offer("minecraft:mending_book", 1, false);
    completed.result_enchantments = vec![("minecraft:mending".into(), 1)];
    completed.locked = true;
    let existing = [completed.clone(), offer("minecraft:glass", 1, false)];
    let fresh = [offer("minecraft:glass", 1, false)];
    let plan = plan_reroll(&existing, &fresh, true);
    assert_eq!(plan.kept, vec![0], "the sold-out trade is kept");
    let merged = apply_plan(&existing, &fresh, &plan);
    assert_eq!(merged[0], completed, "the sold-out book trade is back once more");
    assert_eq!(merged.len(), 2, "and the free slot is filled again");
}

#[test]
fn the_offer_count_follows_the_villagers_level() {
    // A level-1 villager has one set (2 offers), a level-5 one has five.
    for level in 1..=MAX_LEVEL {
        let existing: Vec<OfferKey> = (0..level)
            .map(|i| offer(&format!("minecraft:item_{i}"), i as u32 + 1, false))
            .collect();
        let fresh: Vec<OfferKey> = (100..100 + level)
            .map(|i| offer(&format!("minecraft:item_{i}"), 1, false))
            .collect();
        let plan = plan_reroll(&existing, &fresh, true);
        let merged = apply_plan(&existing, &fresh, &plan);
        assert_eq!(merged.len() as i32, level, "level {level}");
        assert_eq!(plan.added.len() as i32, level);
    }
}

#[test]
fn every_profession_and_level_maps_to_its_own_trade_set() {
    let professions = [
        "armorer",
        "butcher",
        "cartographer",
        "cleric",
        "farmer",
        "fisherman",
        "fletcher",
        "leatherworker",
        "librarian",
        "mason",
        "shepherd",
        "toolsmith",
        "weaponsmith",
    ];
    let mut seen = Vec::new();
    for profession in professions {
        assert_eq!(
            trades_set_id(profession, 1).as_deref(),
            Some(format!("minecraft:{profession}/level_1").as_str())
        );
        assert_eq!(
            trades_set_id(&format!("minecraft:{profession}"), MAX_LEVEL).as_deref(),
            Some(format!("minecraft:{profession}/level_{MAX_LEVEL}").as_str()),
            "the minecraft: prefix is accepted"
        );
        for level in 1..=MAX_LEVEL {
            seen.push(trades_set_id(profession, level).unwrap());
        }
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), professions.len() * MAX_LEVEL as usize);
    // Out of range, empty and malformed names give nothing (no set exists).
    for level in [0, -1, 6, 100] {
        assert_eq!(trades_set_id("farmer", level), None);
    }
    for name in ["", "minecraft:", "farmer level", "farmer/../../x"] {
        assert_eq!(trades_set_id(name, 1), None, "{name:?}");
    }
    // Wandering traders use their three named sets, not levels.
    assert_eq!(
        WANDERING_SETS,
        [
            "minecraft:wandering_trader/buying",
            "minecraft:wandering_trader/uncommon",
            "minecraft:wandering_trader/common"
        ]
    );
}

#[test]
fn repeated_rerolls_never_pile_up_duplicate_entries() {
    // Two fresh rolls of the same stack and price collapse to one.
    let existing = [offer("minecraft:bread", 1, false), offer("minecraft:wheat", 1, false)];
    let fresh = [
        offer("minecraft:apple", 1, false),
        offer("minecraft:apple", 1, false),
        offer("minecraft:apple", 2, false),
    ];
    let plan = plan_reroll(&existing, &fresh, true);
    assert_eq!(plan.added, vec![0, 2], "same stack, different price is a different entry");
    let merged = apply_plan(&existing, &fresh, &plan);
    let unique: Vec<&str> = merged.iter().map(|o| o.result.as_str()).collect();
    assert_eq!(unique, ["minecraft:apple", "minecraft:apple"]);

    // A second reroll over the merged list keeps the entries it just added
    // only if they were used; untouched ones are replaced again.
    let mut used = merged.clone();
    used[0].locked = true;
    let again = plan_reroll(&used, &fresh, true);
    assert_eq!(again.kept, vec![0]);
    assert_eq!(again.added, vec![2], "the second apple at another price");
}

#[test]
fn an_enchanted_book_is_only_a_duplicate_with_the_same_enchantment() {
    let existing = [
        enchanted("minecraft:enchanted_book", "minecraft:mending", 1),
        offer("minecraft:glass", 1, false),
    ];
    // The kept book is a used trade; a fresh Mending book at the same price
    // would be the same entry, so the one free slot goes to Sharpness.
    let mut kept_book = existing[0].clone();
    kept_book.locked = true;
    let existing = [kept_book, offer("minecraft:glass", 1, false)];
    let fresh = [
        enchanted("minecraft:enchanted_book", "minecraft:mending", 1),
        enchanted("minecraft:enchanted_book", "minecraft:sharpness", 3),
    ];
    let plan = plan_reroll(&existing, &fresh, true);
    assert_eq!(plan.kept, vec![0]);
    assert_eq!(plan.added, vec![1], "different books are different trades");
    assert_eq!(
        plan_reroll(&existing, &fresh, true),
        plan,
        "the same inputs plan the same reroll"
    );
}

#[test]
fn turning_keep_used_trades_off_replaces_the_whole_list() {
    let existing = [offer("minecraft:bread", 1, true), offer("minecraft:wheat", 1, false)];
    let fresh = [offer("minecraft:apple", 1, false), offer("minecraft:apple", 1, false)];
    let plan = plan_reroll(&existing, &fresh, false);
    assert!(plan.kept.is_empty());
    assert_eq!(plan.added, vec![0], "still deduplicated, and no longer length-bound");
    assert_eq!(apply_plan(&existing, &fresh, &plan).len(), 1);
}

#[test]
fn a_villagers_own_state_survives_a_reroll() {
    // What the game keeps outside the offer list. A reroll only returns
    // indices, so nothing here can change.
    #[derive(Clone, Debug, PartialEq)]
    struct Villager {
        profession: String,
        level: i32,
        xp: i32,
        restocks_today: i32,
        offers: Vec<OfferKey>,
    }
    let villager = Villager {
        profession: "minecraft:librarian".into(),
        level: 3,
        xp: 120,
        restocks_today: 1,
        offers: vec![offer("minecraft:bread", 1, true), offer("minecraft:paper", 1, false)],
    };
    let fresh = [offer("minecraft:book", 3, false)];
    let plan = plan_reroll(&villager.offers, &fresh, true);
    let mut after = villager.clone();
    after.offers = apply_plan(&villager.offers, &fresh, &plan);
    assert_eq!(after.profession, "minecraft:librarian");
    assert_eq!((after.level, after.xp, after.restocks_today), (3, 120, 1));
    assert_eq!(after.offers[0], villager.offers[0], "the used trade is byte-identical");
}

#[test]
fn an_unknown_villager_reference_is_refused() {
    assert_eq!(
        message(&Outcome::NotAVillager),
        "Only a villager's offers can be rerolled."
    );
    // The planner never invents an entry for a list it was not given.
    let plan = plan_reroll(&[], &[offer("minecraft:bread", 1, false)], true);
    assert_eq!(plan, RerollPlan::default());
    assert!(apply_plan::<OfferKey>(&[], &[], &plan).is_empty());
}

// ------------------------------------------------------------ activation

#[test]
fn the_default_activation_is_sneak_right_click() {
    let cfg = Config::default();
    assert!(cfg.villager_enabled);
    assert_eq!(cfg.villager_activation, Activation::SneakRightClick);
    assert!(cfg.villager_activation.needs_sneak());
    assert!(!cfg.require_confirmation);
    assert!(cfg.keep_used_trades);
    assert!(!cfg.include_wandering_traders);
}

#[test]
fn activation_codes_round_trip_and_unknown_ones_fall_back() {
    for activation in [Activation::SneakRightClick, Activation::RightClick] {
        assert_eq!(Activation::from_code(activation.code()), Some(activation));
    }
    assert_eq!(Activation::from_code("left_click"), None);
    let cfg = Config::from_json(
        r#"{"villager_trade_cycle":{"activation":"right_click","require_confirmation":true}}"#,
    )
    .unwrap();
    assert_eq!(cfg.villager_activation, Activation::RightClick);
    assert!(!cfg.villager_activation.needs_sneak());
    assert!(cfg.require_confirmation);
    // An unknown code keeps the safe default (sneak, so trading still works).
    let cfg =
        Config::from_json(r#"{"villager_trade_cycle":{"activation":"punch"}}"#).unwrap();
    assert_eq!(cfg.villager_activation, Activation::SneakRightClick);
}

#[test]
fn a_confirmation_window_is_short_and_bounded() {
    assert!(tec_core::confirmation_alive(100, 100));
    assert!(tec_core::confirmation_alive(100 + CONFIRM_TICKS, 100));
    assert!(!tec_core::confirmation_alive(101 + CONFIRM_TICKS, 100));
    assert!(tec_core::confirmation_alive(0, 0));
}

// ------------------------------------------------------------ config

#[test]
fn the_villager_config_survives_a_round_trip() {
    let mut cfg = Config::default();
    cfg.villager_enabled = false;
    cfg.include_wandering_traders = true;
    let text = cfg.to_json();
    assert_eq!(Config::from_json(&text).unwrap(), cfg);
}

#[test]
fn a_partial_or_broken_config_keeps_the_defaults() {
    let cfg = Config::from_json(r#"{"villager_trade_cycle":{"enabled":false}}"#).unwrap();
    assert!(!cfg.villager_enabled);
    assert!(cfg.keep_used_trades);
    assert_eq!(cfg.enchanting_activation, tec_core::EnchantActivation::InterfaceButton);

    // Wrong types are ignored field by field, not fatal.
    let cfg = Config::from_json(r#"{"villager_trade_cycle":{"enabled":"yes"}}"#).unwrap();
    assert!(cfg.villager_enabled);

    // Not JSON at all is an error the caller falls back on.
    assert!(Config::from_json("").is_err());
    assert!(Config::from_json("[]").is_err());
    assert!(Config::from_json(r#"{"villager_trade_cycle":{"enabled":true}"#).is_err());
}

#[test]
fn numbers_are_kept_in_range() {
    // A reroll cost is the one number left; it never becomes negative and never
    // grows past the clamp.
    let cfg = Config::from_json(r#"{"enchanting_cycle":{"reroll_cost":-5}}"#).unwrap();
    assert_eq!(cfg.reroll_cost, 0);
    let cfg = Config::from_json(r#"{"enchanting_cycle":{"reroll_cost":999999999}}"#).unwrap();
    assert_eq!(cfg.reroll_cost, 1_000);
    // A cooldown key from an older file is ignored, not honoured.
    let cfg = Config::from_json(r#"{"villager_trade_cycle":{"cooldown_ticks":600}}"#).unwrap();
    assert_eq!(cfg, Config::default());
}

#[test]
fn the_json_reader_handles_the_shapes_a_config_file_has() {
    let value = tec_core::parse(
        r#"{"a": [1, -2.5, 1e3], "b": "a\"b\\c\u0041", "c": true, "d": null}"#,
    )
    .unwrap();
    assert_eq!(value.get("b").and_then(Json::as_str), Some("a\"b\\cA"));
    assert_eq!(value.get("c").and_then(Json::as_bool), Some(true));
    assert!(matches!(value.get("d"), Some(Json::Null)));
    let array = value.get("a").unwrap();
    let Json::Arr(items) = array else { panic!("an array") };
    assert_eq!(items[0].as_f64(), Some(1.0));
    assert_eq!(items[1].as_f64(), Some(-2.5));
    assert_eq!(items[2].as_f64(), Some(1000.0));
    assert!(tec_core::parse("{\"a\": 1,}").is_err());
    assert!(tec_core::parse("nope").is_err());
}

#[test]
fn the_outcome_messages_are_short_and_distinct() {
    let lines = [
        message(&Outcome::Rerolled { added: 1 }),
        message(&Outcome::Rerolled { added: 3 }),
        message(&Outcome::NothingToReroll),
        message(&Outcome::Baby),
        message(&Outcome::Trading),
        message(&Outcome::Sleeping),
        message(&Outcome::NoProfession),
        message(&Outcome::Cooldown),
    ];
    assert!(lines.iter().all(|line| !line.is_empty() && line.len() < 120));
    assert_eq!(message(&Outcome::Rerolled { added: 1 }), "Trades rerolled: 1 new offer.");
    assert_eq!(message(&Outcome::Rerolled { added: 3 }), "Trades rerolled: 3 new offers.");
}
