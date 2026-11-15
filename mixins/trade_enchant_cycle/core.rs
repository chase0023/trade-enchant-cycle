// Trade & Enchant Cycle (Horizon) -- pure core.
//
// This file is compiled twice:
//   1. into the game, as the module `crate::game::tec_core` (horizon-mixins
//      appends it to `crates/horizon-client/src/game.rs`);
//   2. by `cargo test`, through `core/src/lib.rs` (`#[path]` include), so the
//      unit tests in `tests/` exercise the exact bytes the game runs.
//
// It therefore depends on nothing: no game types, no crates. Everything that
// needs the game (mobs, menus, seeds, networking) lives in the sibling
// modules `tec_config`, `tec_villager`, `tec_enchanting` and `tec_hooks`.

// ---------------------------------------------------------------- identity

/// The mod id. It is also the namespace of the mod-message channel below and
/// of the config file, so it stays `[a-z0-9_]`.
pub const ID: &str = "trade_enchant_cycle";
pub const NAME: &str = "Trade & Enchant Cycle";
pub const VERSION: &str = "1.1.0";
pub const DESCRIPTION: &str =
    "Reroll a villager's offers with a sneak-right-click, and reroll an enchanting table's three offers without spending lapis, levels or the item.";

/// `config/<this>` beside `options.txt`.
pub const CONFIG_FILE: &str = "trade-enchant-cycle.json";

/// The one custom payload channel the client half uses to ask its server half
/// for an enchanting reroll (API 40). `mod_id:name`.
pub const NET_CHANNEL: &str = "trade_enchant_cycle:reroll";

/// The highest villager level vanilla trades at.
pub const MAX_LEVEL: i32 = 5;

/// How long (in ticks) a pending confirmation waits for its second click.
pub const CONFIRM_TICKS: u64 = 100;

// ---------------------------------------------------------------- json

/// A minimal JSON value. The config is a small nested object, so a
/// dependency-free reader keeps the project buildable with plain `cargo test`
/// (no registry access) as well as inside the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

struct Reader<'a> {
    chars: Vec<char>,
    at: usize,
    _text: &'a str,
}

impl<'a> Reader<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().collect(),
            at: 0,
            _text: text,
        }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.at += 1;
        }
    }
    fn expect(&mut self, want: char) -> Result<(), String> {
        self.skip_ws();
        match self.bump() {
            Some(c) if c == want => Ok(()),
            Some(c) => Err(format!("expected {want:?} at {} but found {c:?}", self.at - 1)),
            None => Err(format!("expected {want:?} at the end of the text")),
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > 64 {
            return Err("the config nests too deeply".into());
        }
        self.skip_ws();
        match self.peek() {
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.literal("true", Json::Bool(true)),
            Some('f') => self.literal("false", Json::Bool(false)),
            Some('n') => self.literal("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected {c:?} at {}", self.at)),
            None => Err("the text ends where a value should be".into()),
        }
    }
    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        for want in word.chars() {
            if self.bump() != Some(want) {
                return Err(format!("expected {word:?} at {}", self.at));
            }
        }
        Ok(value)
    }
    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.expect('{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.bump();
            return Ok(Json::Obj(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.expect(':')?;
            let value = self.value(depth + 1)?;
            entries.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some('}') => return Ok(Json::Obj(entries)),
                Some(c) => return Err(format!("expected ',' or '}}' at {} but found {c:?}", self.at - 1)),
                None => return Err("the object is not closed".into()),
            }
        }
    }
    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.bump();
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some(']') => return Ok(Json::Arr(items)),
                Some(c) => return Err(format!("expected ',' or ']' at {} but found {c:?}", self.at - 1)),
                None => return Err("the array is not closed".into()),
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let mut code = 0u32;
                        for _ in 0..4 {
                            let c = self
                                .bump()
                                .ok_or_else(|| "a \\u escape is cut short".to_string())?;
                            let digit = c
                                .to_digit(16)
                                .ok_or_else(|| format!("{c:?} is not a hex digit"))?;
                            code = code * 16 + digit;
                        }
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    other => return Err(format!("bad escape {other:?}")),
                },
                Some(c) if (c as u32) < 0x20 => {
                    return Err(format!("a raw control character {c:?} in a string"))
                }
                Some(c) => out.push(c),
                None => return Err("the string is not closed".into()),
            }
        }
    }
    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.peek() == Some('-') {
            self.at += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.at += 1;
        }
        if self.peek() == Some('.') {
            self.at += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.at += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.at += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| format!("{text:?} is not a number"))
    }
}

/// Parses one JSON value (whitespace around it allowed).
pub fn parse(text: &str) -> Result<Json, String> {
    let mut reader = Reader::new(text);
    let value = reader.value(0)?;
    reader.skip_ws();
    if reader.peek().is_some() {
        return Err(format!("trailing text at {}", reader.at));
    }
    Ok(value)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

impl Json {
    /// Pretty JSON, two spaces a level, in insertion order.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }
    fn write(&self, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        let inner = "  ".repeat(depth + 1);
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    out.push_str(&format!("{}", *n as i64));
                } else {
                    out.push_str(&format!("{n}"));
                }
            }
            Json::Str(s) => {
                out.push('"');
                out.push_str(&escape(s));
                out.push('"');
            }
            Json::Arr(items) if items.is_empty() => out.push_str("[]"),
            Json::Arr(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&inner);
                    item.write(out, depth + 1);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push(']');
            }
            Json::Obj(entries) if entries.is_empty() => out.push_str("{}"),
            Json::Obj(entries) => {
                out.push_str("{\n");
                for (i, (key, value)) in entries.iter().enumerate() {
                    out.push_str(&inner);
                    out.push('"');
                    out.push_str(&escape(key));
                    out.push_str("\": ");
                    value.write(out, depth + 1);
                    if i + 1 < entries.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push('}');
            }
        }
    }
}

// ---------------------------------------------------------------- config

/// How a villager reroll starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// Sneak + right-click (the default): normal trading is untouched.
    SneakRightClick,
    /// A plain right-click rerolls instead of opening the trade screen. It
    /// takes trading away from the villager, so it is opt-in.
    RightClick,
}

impl Activation {
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "sneak_right_click" => Some(Self::SneakRightClick),
            "right_click" => Some(Self::RightClick),
            _ => None,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::SneakRightClick => "sneak_right_click",
            Self::RightClick => "right_click",
        }
    }
    /// Whether this activation can be told apart from normal trading.
    pub fn needs_sneak(self) -> bool {
        matches!(self, Self::SneakRightClick)
    }
}

/// How an enchanting reroll starts, as the config file spells it.
///
/// The keyboard shortcut answers in the mod's screens whatever this says — the
/// bar's label names the key (`Reroll [R]`), so a bar with a key that did
/// nothing would be a lie, and that was the bug the shortcut's first version
/// had (the default file said `interface_button`, so `R` was ignored while the
/// bar promised it). What is left for the setting to decide is the bar itself:
/// `interface_button` and `both` draw it, `key` leaves only the shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnchantActivation {
    /// The bar is drawn; clicking it and pressing the key both reroll.
    InterfaceButton,
    /// No bar: only the keyboard shortcut rerolls.
    Key,
    /// Both, which is the same as `interface_button` today and is kept so an
    /// old config file keeps its meaning.
    Both,
}

impl EnchantActivation {
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "interface_button" => Some(Self::InterfaceButton),
            "key" => Some(Self::Key),
            "both" => Some(Self::Both),
            _ => None,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::InterfaceButton => "interface_button",
            Self::Key => "key",
            Self::Both => "both",
        }
    }
    /// Whether the bar is drawn for this setting.
    pub fn uses_button(self) -> bool {
        matches!(self, Self::InterfaceButton | Self::Both)
    }
}

/// The JSON config, exactly the shape the mod documents:
///
/// ```json
/// {
///   "villager_trade_cycle": {"enabled": true, "activation": "sneak_right_click", ...},
///   "enchanting_cycle":     {"enabled": true, "activation": "interface_button", ...},
///   "notifications":        {"enabled": true, "show_success": true, "show_errors": true}
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub villager_enabled: bool,
    pub villager_activation: Activation,
    pub require_confirmation: bool,
    pub keep_used_trades: bool,
    pub include_wandering_traders: bool,
    /// Draw a Reroll bar in the villager's trading screen.
    pub villager_screen_button: bool,
    /// The key that rerolls from the trading screen.
    pub villager_key: String,
    pub enchanting_enabled: bool,
    pub enchanting_activation: EnchantActivation,
    pub free_rerolls: bool,
    pub reroll_cost: i32,
    pub show_button: bool,
    pub enchant_key: String,
    pub notify_enabled: bool,
    pub show_success: bool,
    pub show_errors: bool,
}

/// Problems found while reading a config: the value used and what was asked
/// for, so the game can log one line the player can act on.
pub fn warnings(cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    if !cfg.free_rerolls || cfg.reroll_cost != 0 {
        out.push(
            "enchanting_cycle.free_rerolls must be true and reroll_cost 0: this build never \
             spends lapis or levels on a reroll, so the enchanting reroll is off until the file says so"
                .into(),
        );
    }
    if cfg.enchant_key.trim().is_empty() {
        out.push("enchanting_cycle.key is empty: the keyboard shortcut is off".into());
    }
    out
}

impl Default for Config {
    fn default() -> Self {
        Self {
            villager_enabled: true,
            villager_activation: Activation::SneakRightClick,
            require_confirmation: false,
            keep_used_trades: true,
            include_wandering_traders: false,
            villager_screen_button: true,
            villager_key: "key.keyboard.r".into(),
            enchanting_enabled: true,
            enchanting_activation: EnchantActivation::InterfaceButton,
            free_rerolls: true,
            reroll_cost: 0,
            show_button: true,
            enchant_key: "key.keyboard.r".into(),
            notify_enabled: true,
            show_success: true,
            show_errors: true,
        }
    }
}

fn bool_at(root: &Json, group: &str, key: &str, default: bool) -> bool {
    root.get(group)
        .and_then(|g| g.get(key))
        .and_then(Json::as_bool)
        .unwrap_or(default)
}

fn number_at(root: &Json, group: &str, key: &str, default: i64) -> i64 {
    root.get(group)
        .and_then(|g| g.get(key))
        .and_then(Json::as_f64)
        .filter(|n| n.is_finite())
        .map(|n| n as i64)
        .unwrap_or(default)
}

fn string_at<'a>(root: &'a Json, group: &str, key: &str) -> Option<&'a str> {
    root.get(group).and_then(|g| g.get(key)).and_then(Json::as_str)
}

impl Config {
    /// Reads the config. A missing key keeps its default; a wrong type is
    /// ignored the same way, so a half-edited file never stops the mod.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let root = parse(text)?;
        if !matches!(root, Json::Obj(_)) {
            return Err("the config is not a JSON object".into());
        }
        let d = Self::default();
        let activation = string_at(&root, "villager_trade_cycle", "activation")
            .and_then(Activation::from_code)
            .unwrap_or(d.villager_activation);
        let enchanting_activation = string_at(&root, "enchanting_cycle", "activation")
            .and_then(EnchantActivation::from_code)
            .unwrap_or(d.enchanting_activation);
        Ok(Self {
            villager_enabled: bool_at(&root, "villager_trade_cycle", "enabled", d.villager_enabled),
            villager_activation: activation,
            require_confirmation: bool_at(
                &root,
                "villager_trade_cycle",
                "require_confirmation",
                d.require_confirmation,
            ),
            keep_used_trades: bool_at(
                &root,
                "villager_trade_cycle",
                "keep_used_trades",
                d.keep_used_trades,
            ),
            include_wandering_traders: bool_at(
                &root,
                "villager_trade_cycle",
                "include_wandering_traders",
                d.include_wandering_traders,
            ),
            villager_screen_button: bool_at(
                &root,
                "villager_trade_cycle",
                "screen_button",
                d.villager_screen_button,
            ),
            villager_key: string_at(&root, "villager_trade_cycle", "key")
                .map(str::to_owned)
                .unwrap_or(d.villager_key),
            enchanting_enabled: bool_at(&root, "enchanting_cycle", "enabled", d.enchanting_enabled),
            enchanting_activation,
            free_rerolls: bool_at(&root, "enchanting_cycle", "free_rerolls", d.free_rerolls),
            reroll_cost: number_at(&root, "enchanting_cycle", "reroll_cost", i64::from(d.reroll_cost))
                .clamp(0, 1_000) as i32,
            show_button: bool_at(&root, "enchanting_cycle", "show_button", d.show_button),
            enchant_key: string_at(&root, "enchanting_cycle", "key")
                .unwrap_or(&d.enchant_key)
                .to_owned(),
            notify_enabled: bool_at(&root, "notifications", "enabled", d.notify_enabled),
            show_success: bool_at(&root, "notifications", "show_success", d.show_success),
            show_errors: bool_at(&root, "notifications", "show_errors", d.show_errors),
        })
    }

    /// The file as the mod writes it: readable, in the documented order.
    pub fn to_json(&self) -> String {
        let json = Json::Obj(vec![
            (
                "villager_trade_cycle".into(),
                Json::Obj(vec![
                    ("enabled".into(), Json::Bool(self.villager_enabled)),
                    (
                        "activation".into(),
                        Json::Str(self.villager_activation.code().into()),
                    ),
                    (
                        "require_confirmation".into(),
                        Json::Bool(self.require_confirmation),
                    ),
                    ("keep_used_trades".into(), Json::Bool(self.keep_used_trades)),
                    (
                        "include_wandering_traders".into(),
                        Json::Bool(self.include_wandering_traders),
                    ),
                    (
                        "screen_button".into(),
                        Json::Bool(self.villager_screen_button),
                    ),
                    ("key".into(), Json::Str(self.villager_key.clone())),
                ]),
            ),
            (
                "enchanting_cycle".into(),
                Json::Obj(vec![
                    ("enabled".into(), Json::Bool(self.enchanting_enabled)),
                    (
                        "activation".into(),
                        Json::Str(self.enchanting_activation.code().into()),
                    ),
                    ("free_rerolls".into(), Json::Bool(self.free_rerolls)),
                    ("reroll_cost".into(), Json::Num(f64::from(self.reroll_cost))),
                    ("show_button".into(), Json::Bool(self.show_button)),
                    ("key".into(), Json::Str(self.enchant_key.clone())),
                ]),
            ),
            (
                "notifications".into(),
                Json::Obj(vec![
                    ("enabled".into(), Json::Bool(self.notify_enabled)),
                    ("show_success".into(), Json::Bool(self.show_success)),
                    ("show_errors".into(), Json::Bool(self.show_errors)),
                ]),
            ),
        ]);
        json.to_pretty()
    }
}

// ---------------------------------------------------------------- outcomes

/// What a reroll request did. The game turns these into the player's line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `added` offers are new; the rest were already used and kept.
    Rerolled { added: usize },
    /// The enchanting table's three offers were given a new seed.
    EnchantingRerolled,
    /// Every offer is used up, or the trade set rolled nothing new.
    NothingToReroll,
    /// Switched off in the config or in Options > Mods.
    FeatureDisabled,
    /// Not a villager (or a mob this module may not touch).
    NotAVillager,
    Baby,
    Trading,
    Sleeping,
    /// Unemployed or nitwit: no trade set to roll.
    NoProfession,
    Cooldown,
    /// The first click of a two-click confirmation.
    AwaitingConfirmation,
    /// A client of a server this mod cannot reroll on (the server is
    /// authoritative; the vanilla protocol has no reroll message).
    UnsupportedServer,
    /// `free_rerolls` is off or `reroll_cost` is not 0.
    CostUnsupported,
    /// No enchanting menu is open for that player.
    NoMenu,
    /// The input slot holds nothing enchantable (or it is already enchanted).
    NotEnchantable,
}

impl Outcome {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Outcome::Rerolled { .. }
                | Outcome::EnchantingRerolled
                | Outcome::AwaitingConfirmation
        )
    }
}

/// The player-facing line. Short: an action bar, not a book.
pub fn message(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Rerolled { added: 1 } => "Trades rerolled: 1 new offer.".into(),
        Outcome::Rerolled { added } => format!("Trades rerolled: {added} new offers."),
        Outcome::EnchantingRerolled => "Enchanting offers rerolled.".into(),
        Outcome::NothingToReroll => {
            "No unlocked trades to reroll: every offer here was already used.".into()
        }
        Outcome::FeatureDisabled => format!("{NAME} is off (config or Options > Mods)."),
        Outcome::NotAVillager => "Only a villager's offers can be rerolled.".into(),
        Outcome::Baby => "A baby villager has no trades.".into(),
        Outcome::Trading => "That villager is already trading.".into(),
        Outcome::Sleeping => "That villager is asleep.".into(),
        Outcome::NoProfession => "That villager has no profession, so it has no trade set.".into(),
        Outcome::Cooldown => "The reroll is still on cooldown.".into(),
        Outcome::AwaitingConfirmation => {
            "Sneak-right-click the villager again to confirm the reroll.".into()
        }
        Outcome::UnsupportedServer => {
            "This server does not run the mod: its enchanting table can only be rerolled where \
             the mod's server half runs (single player or a LAN host)."
                .into()
        }
        Outcome::CostUnsupported => {
            "The config asks for a reroll cost; this build only does free rerolls.".into()
        }
        Outcome::NoMenu => "Open an enchanting table first.".into(),
        Outcome::NotEnchantable => {
            "Put an enchantable, not yet enchanted item in the table's slot.".into()
        }
    }
}

// ---------------------------------------------------------------- trade sets

/// `VillagerProfession.getTrades(level)`: `minecraft:<profession>/level_<n>`.
/// None when the level is outside vanilla's 1..=5 or the profession is not a
/// plain resource name (so a data-pack id cannot inject a path).
pub fn trades_set_id(profession: &str, level: i32) -> Option<String> {
    if !(1..=MAX_LEVEL).contains(&level) {
        return None;
    }
    let name = profession
        .strip_prefix("minecraft:")
        .unwrap_or(profession);
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/')
    {
        return None;
    }
    Some(format!("minecraft:{name}/level_{level}"))
}

/// The wandering trader's three sets, in the order vanilla rolls them.
pub const WANDERING_SETS: [&str; 3] = [
    "minecraft:wandering_trader/buying",
    "minecraft:wandering_trader/uncommon",
    "minecraft:wandering_trader/common",
];

// ---------------------------------------------------------------- reroll plan

/// What one existing offer looks like to the planner. Deliberately data-only
/// so the merge rule is testable without a game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferKey {
    /// The offer has been taken at least once, or is out of stock. Vanilla
    /// counts a trade as spent from its first use (`uses > 0`) and refuses it
    /// at `uses >= maxUses`; a reroll must leave both alone.
    pub locked: bool,
    pub result: String,
    pub result_count: u32,
    /// Enchantments (and a book's stored ones) with their levels.
    pub result_enchantments: Vec<(String, u32)>,
    pub cost_a: String,
    pub cost_a_count: u32,
    pub cost_b: Option<(String, u32)>,
}

impl OfferKey {
    /// Two offers are the same entry when everything but the lock matches:
    /// same result stack (item, count, enchantments) for the same price.
    pub fn same_entry(&self, other: &Self) -> bool {
        self.result == other.result
            && self.result_count == other.result_count
            && self.result_enchantments == other.result_enchantments
            && self.cost_a == other.cost_a
            && self.cost_a_count == other.cost_a_count
            && self.cost_b == other.cost_b
    }
}

/// Which offers to keep and which fresh rolls to add.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RerollPlan {
    /// Indices into the existing offers, in their order.
    pub kept: Vec<usize>,
    /// Indices into the fresh offers, in their order.
    pub added: Vec<usize>,
}

/// Plans a reroll.
///
/// * `keep_locked` (the default) keeps every used or out-of-stock offer and
///   fills the rest of the merchant's list with fresh offers, so the number of
///   offers stays what vanilla gave the villager at its level.
/// * With `keep_locked` off the whole list is replaced, deduplicated.
///
/// Fresh offers that repeat a kept or already-added entry are skipped: no two
/// offers sell the same stack for the same price, so repeated rerolls cannot
/// pile up copies of one valuable trade.
pub fn plan_reroll(existing: &[OfferKey], fresh: &[OfferKey], keep_locked: bool) -> RerollPlan {
    let kept: Vec<usize> = if keep_locked {
        existing
            .iter()
            .enumerate()
            .filter(|(_, offer)| offer.locked)
            .map(|(index, _)| index)
            .collect()
    } else {
        Vec::new()
    };
    let target = if keep_locked {
        existing.len()
    } else {
        fresh.len()
    };
    let mut chosen: Vec<&OfferKey> = kept.iter().map(|&index| &existing[index]).collect();
    let mut added = Vec::new();
    for (index, offer) in fresh.iter().enumerate() {
        if chosen.len() >= target {
            break;
        }
        if chosen.iter().any(|seen| seen.same_entry(offer)) {
            continue;
        }
        chosen.push(offer);
        added.push(index);
    }
    RerollPlan { kept, added }
}

/// Whether a villager has anything left a reroll may replace.
pub fn has_unlocked(existing: &[OfferKey]) -> bool {
    existing.iter().any(|offer| !offer.locked)
}

/// The new offer list a plan describes: the kept offers in their order, then
/// the fresh ones. Only `offers` move; the game keeps everything else (a
/// villager's experience, level, profession and restock counters) untouched.
pub fn apply_plan<T: Clone>(existing: &[T], fresh: &[T], plan: &RerollPlan) -> Vec<T> {
    let mut out: Vec<T> = plan.kept.iter().map(|&i| existing[i].clone()).collect();
    out.extend(plan.added.iter().map(|&i| fresh[i].clone()));
    out
}

/// Whether the villager feature may run at all.
pub fn villager_policy(cfg: &Config) -> Result<(), Outcome> {
    if cfg.villager_enabled {
        Ok(())
    } else {
        Err(Outcome::FeatureDisabled)
    }
}

/// Whether the enchanting feature may run. A reroll never spends lapis or
/// levels, so a config that asks for a cost is refused rather than silently
/// charged: that keeps the feature's promise true whatever the file says.
pub fn enchanting_policy(cfg: &Config) -> Result<(), Outcome> {
    if !cfg.enchanting_enabled {
        return Err(Outcome::FeatureDisabled);
    }
    if !cfg.free_rerolls || cfg.reroll_cost != 0 {
        return Err(Outcome::CostUnsupported);
    }
    Ok(())
}

// ---------------------------------------------------------------- activation

/// The config file's key name against the name the game reports
/// (`config` may be `r`, `R` or `key.keyboard.r`).
pub fn key_matches(config: &str, pressed: &str) -> bool {
    let config = config.trim().to_ascii_lowercase();
    let pressed = pressed.trim().to_ascii_lowercase();
    if config.is_empty() || pressed.is_empty() {
        return false;
    }
    if config == pressed {
        return true;
    }
    if !config.contains('.') {
        return pressed == format!("key.keyboard.{config}");
    }
    false
}

/// The label drawn in the reroll bar.
pub const BUTTON_LABEL: &str = "Reroll";

/// The bar's height, in GUI pixels: vanilla's small widget scale, which holds
/// the 8 px font with three pixels above and three below it.
pub const BAR_HEIGHT: f32 = 15.0;

/// How far the bar is inset from the panel's left and right edges, so it reads
/// as a footer of the panel rather than as part of its border.
pub const BAR_INSET: f32 = 3.0;

/// The bar's rect in GUI pixels, for a panel at `panel` (the screen's own
/// `NativeScreenView.rect`, `[x, y, w, h]`).
///
/// It is a footer on the panel's bottom edge, as wide as the panel less
/// `BAR_INSET` on each side — placed from the panel's origin and size, never
/// from the window's, so it follows the panel at every GUI scale and window
/// size instead of needing per-resolution numbers.
///
/// Nothing of vanilla's is under it in either screen: the enchanting table's
/// panel (176x166) ends its offers at y=71 and its hotbar row at y=160, the
/// merchant's (276x166) ends its own hotbar row at y=160, and the panel's
/// border is the last thing drawn. The panel is centred in the window, which
/// at the smallest GUI the game allows (320x240 at scale 1) still leaves 37
/// pixels above and below a 166-high panel, so a 15-pixel footer is never
/// clipped at the window's edge either.
pub fn bar_rect(panel: [f32; 4]) -> [f32; 4] {
    [
        panel[0] + BAR_INSET,
        panel[1] + panel[3],
        panel[2] - 2.0 * BAR_INSET,
        BAR_HEIGHT,
    ]
}

/// The bar's text: the action, and the key that does the same thing.
pub fn bar_label(key: Option<&str>) -> String {
    match key.and_then(key_letter) {
        Some(letter) => format!("{BUTTON_LABEL} [{letter}]"),
        None => BUTTON_LABEL.into(),
    }
}

/// The letter to show for a configured key (`key.keyboard.r`, `r`, `R`).
fn key_letter(key: &str) -> Option<char> {
    let name = key.trim().trim_start_matches("key.keyboard.").trim();
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase()),
        _ => None,
    }
}

/// Whether a click at `local` — the pointer in the screen's own panel
/// coordinates — is on the bar `bar_rect` places for `panel`.
pub fn bar_hit(panel: [f32; 4], local: [f32; 2]) -> bool {
    inside(bar_rect(panel), [panel[0] + local[0], panel[1] + local[1]])
}

/// The one-byte payloads a client's request carries: which screen asked.
pub const NET_ENCHANTING: u8 = b'e';
pub const NET_MERCHANT: u8 = b'm';

/// Whether `point` is inside `rect` (`rect` is `[x, y, w, h]`).
pub fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    let [x, y, w, h] = rect;
    point[0] >= x && point[0] < x + w && point[1] >= y && point[1] < y + h
}

/// Whether `cooldown` ticks have passed since `last`; `0` always has, so a
/// caller with no cooldown passes everything. The mod's own guards use `1` —
/// "not in the same tick" — which is what keeps one click from rerolling twice,
/// never a wait the player can feel.
///
/// A clock
/// that went backwards (a new world, a reloaded one) counts as passed: the old
/// stamp cannot block the feature for a whole world's ticks.
pub fn cooldown_ready(now: u64, last: u64, cooldown: u64) -> bool {
    cooldown == 0 || now < last || now - last >= cooldown
}

/// Whether a pending confirmation made at `since` still covers `now`.
pub fn confirmation_alive(now: u64, since: u64) -> bool {
    now >= since && now - since <= CONFIRM_TICKS
}

// ---------------------------------------------------------------- settings page

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsAction {
    ToggleVillager,
    ToggleVillagerButton,
    ToggleEnchanting,
    ToggleEnchantButton,
    ToggleNotifications,
    Reload,
}

pub fn settings_action(button: usize) -> Option<SettingsAction> {
    Some(match button {
        0 => SettingsAction::ToggleVillager,
        1 => SettingsAction::ToggleVillagerButton,
        2 => SettingsAction::ToggleEnchanting,
        3 => SettingsAction::ToggleEnchantButton,
        4 => SettingsAction::ToggleNotifications,
        5 => SettingsAction::Reload,
        _ => return None,
    })
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

pub fn settings_labels(cfg: &Config) -> Vec<String> {
    vec![
        format!("Villager trade cycling: {}", on_off(cfg.villager_enabled)),
        format!("Villager Reroll bar: {}", on_off(cfg.villager_screen_button)),
        format!("Enchanting cycling: {}", on_off(cfg.enchanting_enabled)),
        format!("Enchanting Reroll bar: {}", on_off(cfg.show_button)),
        format!("Notifications: {}", on_off(cfg.notify_enabled)),
        "Reload config file".into(),
    ]
}

/// Applies a settings-page click, returning the new config.
pub fn apply_settings(cfg: &Config, action: SettingsAction) -> Config {
    let mut next = cfg.clone();
    match action {
        SettingsAction::ToggleVillager => next.villager_enabled = !next.villager_enabled,
        SettingsAction::ToggleVillagerButton => {
            next.villager_screen_button = !next.villager_screen_button
        }
        SettingsAction::ToggleEnchanting => next.enchanting_enabled = !next.enchanting_enabled,
        SettingsAction::ToggleEnchantButton => next.show_button = !next.show_button,
        SettingsAction::ToggleNotifications => next.notify_enabled = !next.notify_enabled,
        SettingsAction::Reload => {}
    }
    next
}
