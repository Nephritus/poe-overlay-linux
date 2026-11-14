//! Hotkeys chosen in config.toml's `[hotkeys]`.

use poe_overlay::config::Config;
use poe_overlay::running::Action;

fn config(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|err| err.to_string())
}

#[test]
fn currency_rates_is_on_alt_r_unless_configured() {
    let hotkeys = config("").unwrap().hotkeys;

    assert_eq!(hotkeys.get(Action::CurrencyRates).map(ToString::to_string).as_deref(), Some("Alt+R"));
}

#[test]
fn empty_key_leaves_the_action_without_a_hotkey() {
    let hotkeys = config("[hotkeys]\ncurrency-rates = \"\"").unwrap().hotkeys;

    assert!(hotkeys.get(Action::CurrencyRates).is_none());
    assert!(hotkeys.iter().all(|(action, _)| action != Action::CurrencyRates));
}

#[test]
fn configured_key_replaces_the_default() {
    let hotkeys = config("[hotkeys]\ncurrency-rates = \"ctrl+shift+F5\"").unwrap().hotkeys;

    assert_eq!(hotkeys.get(Action::CurrencyRates).unwrap().to_string(), "Ctrl+Shift+F5");
}

#[test]
fn mistakes_in_hotkeys_are_reported_with_what_is_wrong() {
    for (value, expected) in [
        ("[hotkeys]\ncurrency-rates = \"Alt+Rr\"", "unknown key `Rr`"),
        ("[hotkeys]\ncurrency-rates = \"Hyper+R\"", "unknown modifier `Hyper`"),
        ("[hotkeys]\ncurrency-rates = \"Alt+\"", "no key"),
        // `Mod` means different keys in different compositors.
        ("[hotkeys]\ncurrency-rates = \"Mod+R\"", "unknown modifier `Mod`"),
        ("[hotkeys]\ncurrency = \"Alt+R\"", "currency-rates"),
        ("[hotkeys]\ncurrency-rates = 5", "must be a string"),
    ] {
        let err = config(value).unwrap_err();
        assert!(err.contains(expected), "{value:?}: {err}");
    }
}
