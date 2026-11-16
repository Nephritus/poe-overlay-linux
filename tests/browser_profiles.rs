//! Finding browser profiles under a home folder built here, laid out the way
//! the browsers and Flatpak lay them out.

use std::path::Path;

use poe_overlay::browsers::profiles;

fn profile(home: &Path, dir: &str) {
    let dir = home.join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("cookies.sqlite"), "").unwrap();
}

fn found(home: &Path, default: Option<&str>) -> Vec<(&'static str, String)> {
    profiles(home, default).into_iter().map(|profile| (profile.browser.name, profile.dir.strip_prefix(home).unwrap().to_string_lossy().into_owned())).collect()
}

#[test]
fn firefox_family_profiles_are_found_in_native_xdg_and_flatpak_folders() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    profile(home, ".mozilla/firefox/a1.default-release");
    profile(home, ".config/mozilla/firefox/b2.default-release");
    profile(home, ".var/app/org.mozilla.firefox/.mozilla/firefox/c3.default");
    profile(home, ".librewolf/d4.default-default");
    profile(home, ".var/app/io.gitlab.librewolf-community/.librewolf/e5.default");
    profile(home, ".zen/f6.Default (release)");
    profile(home, ".var/app/one.ablaze.floorp/.floorp/g7.default");
    profile(home, ".waterfox/h8.68-edition-default");
    // A profile never started has no cookie file; files aren't profiles.
    std::fs::create_dir_all(home.join(".mozilla/firefox/i9.unused")).unwrap();
    std::fs::write(home.join(".mozilla/firefox/profiles.ini"), "[General]\n").unwrap();

    let mut found = found(home, None);
    found.sort();
    assert_eq!(
        found,
        [
            ("Firefox", ".config/mozilla/firefox/b2.default-release".to_owned()),
            ("Firefox", ".mozilla/firefox/a1.default-release".to_owned()),
            ("Firefox", ".var/app/org.mozilla.firefox/.mozilla/firefox/c3.default".to_owned()),
            ("Floorp", ".var/app/one.ablaze.floorp/.floorp/g7.default".to_owned()),
            ("LibreWolf", ".librewolf/d4.default-default".to_owned()),
            ("LibreWolf", ".var/app/io.gitlab.librewolf-community/.librewolf/e5.default".to_owned()),
            ("Waterfox", ".waterfox/h8.68-edition-default".to_owned()),
            ("Zen", ".zen/f6.Default (release)".to_owned()),
        ]
    );
}

#[test]
fn the_default_browser_s_profiles_come_first() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    profile(home, ".mozilla/firefox/a1.default-release");
    profile(home, ".zen/f6.Default (release)");
    profile(home, ".var/app/io.gitlab.librewolf-community/.librewolf/e5.default");

    let browsers = |default| found(home, Some(default)).into_iter().map(|(browser, _)| browser).collect::<Vec<_>>();
    assert_eq!(browsers("io.gitlab.librewolf-community.desktop")[0], "LibreWolf");
    assert_eq!(browsers("zen.desktop")[0], "Zen");
    assert_eq!(browsers("firefox.desktop")[0], "Firefox");
}

#[test]
fn a_firefox_profile_s_user_agent_comes_from_the_version_that_last_ran_it() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    profile(home, ".mozilla/firefox/a1.default-release");
    std::fs::write(
        home.join(".mozilla/firefox/a1.default-release/compatibility.ini"),
        "[Compatibility]\nLastVersion=159.0_20261027211755/20261027211755\nLastOSABI=Linux_x86_64-gcc3\nLastPlatformDir=/nowhere/firefox\n",
    )
    .unwrap();

    let found = profiles(home, None);
    assert_eq!(found[0].user_agent.as_deref(), Some("Mozilla/5.0 (X11; Linux x86_64; rv:159.0) Gecko/20100101 Firefox/159.0"));
}

#[test]
fn a_fork_s_user_agent_comes_from_its_gecko_version_not_its_own() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    profile(home, ".zen/f6.Default (release)");
    let install = home.join("zen-install");
    std::fs::create_dir_all(&install).unwrap();
    std::fs::write(install.join("platform.ini"), "[Build]\nBuildID=20261101000000\nMilestone=145.0.1\n").unwrap();
    let compatibility = format!("[Compatibility]\nLastVersion=1.17.5b_20261101000000/20261101000000\nLastPlatformDir={}\n", install.display());
    std::fs::write(home.join(".zen/f6.Default (release)/compatibility.ini"), compatibility).unwrap();
    // A fork whose Gecko version can't be told gets no User-Agent of its own.
    profile(home, ".floorp/g7.default");
    std::fs::write(home.join(".floorp/g7.default/compatibility.ini"), "[Compatibility]\nLastVersion=12.3.0_20261101000000/20261101000000\nLastPlatformDir=/app/lib/floorp\n").unwrap();

    let found = profiles(home, None);
    let agent = |name| found.iter().find(|profile| profile.browser.name == name).unwrap().user_agent.as_deref();
    assert_eq!(agent("Zen"), Some("Mozilla/5.0 (X11; Linux x86_64; rv:145.0) Gecko/20100101 Firefox/145.0"));
    assert_eq!(agent("Floorp"), None);
}

#[test]
fn a_user_agent_set_in_the_profile_s_preferences_is_used_as_is() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    profile(home, ".librewolf/d4.default-default");
    let dir = home.join(".librewolf/d4.default-default");
    std::fs::write(dir.join("compatibility.ini"), "[Compatibility]\nLastVersion=146.0_20261101000000/20261101000000\n").unwrap();
    std::fs::write(dir.join("prefs.js"), "user_pref(\"browser.startup.page\", 3);\nuser_pref(\"general.useragent.override\", \"Mozilla/5.0 (Windows NT 10.0; rv:140.0) Gecko/20100101 Firefox/140.0\");\n").unwrap();

    assert_eq!(profiles(home, None)[0].user_agent.as_deref(), Some("Mozilla/5.0 (Windows NT 10.0; rv:140.0) Gecko/20100101 Firefox/140.0"));
}
