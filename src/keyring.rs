//! The system keyring over D-Bus, where Chromium browsers keep their
//! cookies' key: the Secret Service (GNOME Keyring, KDE's, KeePassXC), else
//! an open KWallet. Never asks the player to unlock anything.

use std::collections::HashMap;

use gtk::gio;
use gtk::glib::prelude::ToVariant;
use gtk::glib::{Variant, variant::ObjectPath};

use crate::browsers::{Keyring, KeyringError};

const TIMEOUT_MS: i32 = 3000;

pub struct SystemKeyring;

impl Keyring for SystemKeyring {
    fn password(&self, name: &str) -> Result<Vec<u8>, KeyringError> {
        let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).map_err(|err| KeyringError::Unavailable(err.to_string()))?;
        match secret_service(&bus, &format!("{name} Safe Storage")) {
            Err(KeyringError::Missing | KeyringError::Unavailable(_)) if let Some(kwallet) = KWallet::running(&bus) => kwallet.password(name),
            found => found,
        }
    }
}

fn call(bus: &gio::DBusConnection, service: &str, path: &str, interface: &str, method: &str, args: Option<&Variant>) -> Result<Variant, KeyringError> {
    bus.call_sync(Some(service), path, interface, method, args, None, gio::DBusCallFlags::NO_AUTO_START, TIMEOUT_MS, gio::Cancellable::NONE)
        .map_err(|err| KeyringError::Unavailable(err.to_string()))
}

const SECRETS: &str = "org.freedesktop.secrets";

/// The secret of the item labelled `label`: the label is what Chromium
/// browsers and yt-dlp look for.
fn secret_service(bus: &gio::DBusConnection, label: &str) -> Result<Vec<u8>, KeyringError> {
    // Started on demand, as for the browser itself.
    let search = bus
        .call_sync(
            Some(SECRETS),
            "/org/freedesktop/secrets",
            "org.freedesktop.Secret.Service",
            "SearchItems",
            Some(&(HashMap::<String, String>::new(),).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .map_err(|err| KeyringError::Unavailable(err.to_string()))?;
    let (unlocked, locked) = search.get::<(Vec<ObjectPath>, Vec<ObjectPath>)>().ok_or_else(|| unexpected("SearchItems"))?;
    let labelled = |item: &&ObjectPath| {
        let args = ("org.freedesktop.Secret.Item", "Label").to_variant();
        call(bus, SECRETS, item.as_str(), "org.freedesktop.DBus.Properties", "Get", Some(&args)).ok().and_then(|reply| reply.child_value(0).as_variant()?.str().map(|found| found == label)).unwrap_or(false)
    };
    let Some(item) = unlocked.iter().find(labelled) else {
        return Err(if locked.iter().any(|item| labelled(&item)) { KeyringError::Locked } else { KeyringError::Missing });
    };
    let opened = call(bus, SECRETS, "/org/freedesktop/secrets", "org.freedesktop.Secret.Service", "OpenSession", Some(&("plain", "".to_variant()).to_variant()))?;
    let session = opened.child_value(1);
    let session = session.str().ok_or_else(|| unexpected("OpenSession"))?;
    let secret = call(bus, SECRETS, item.as_str(), "org.freedesktop.Secret.Item", "GetSecret", Some(&(ObjectPath::try_from(session.to_owned()).map_err(|_| unexpected("OpenSession"))?,).to_variant()));
    let _ = call(bus, SECRETS, session, "org.freedesktop.Secret.Session", "Close", None);
    // ((session, parameters, value, content type))
    secret?.child_value(0).child_value(2).get::<Vec<u8>>().ok_or_else(|| unexpected("GetSecret"))
}

fn unexpected(method: &str) -> KeyringError {
    KeyringError::Unavailable(format!("unexpected answer to {method}"))
}

/// KDE's wallet, used only when it's running and open, so nothing asks
/// for its password.
struct KWallet<'b> {
    bus: &'b gio::DBusConnection,
    service: &'static str,
    path: &'static str,
}

impl<'b> KWallet<'b> {
    const APP: &'static str = "poe-overlay";

    fn running(bus: &'b gio::DBusConnection) -> Option<KWallet<'b>> {
        [("org.kde.kwalletd6", "/modules/kwalletd6"), ("org.kde.kwalletd5", "/modules/kwalletd5")].into_iter().find_map(|(service, path)| {
            let owned = call(bus, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner", Some(&(service,).to_variant())).ok()?;
            owned.child_value(0).get::<bool>()?.then_some(KWallet { bus, service, path })
        })
    }

    fn call(&self, method: &str, args: Variant) -> Result<Variant, KeyringError> {
        call(self.bus, self.service, self.path, "org.kde.KWallet", method, Some(&args)).map(|reply| reply.child_value(0))
    }

    /// Chromium keeps it as "`name` Safe Storage" in the folder "`name` Keys".
    fn password(&self, name: &str) -> Result<Vec<u8>, KeyringError> {
        let wallet = self.call("networkWallet", ().to_variant())?;
        let wallet = wallet.str().ok_or_else(|| unexpected("networkWallet"))?;
        if !self.call("isOpen", (wallet,).to_variant())?.get::<bool>().unwrap_or(false) {
            return Err(KeyringError::Locked);
        }
        let handle = self.call("open", (wallet, 0i64, Self::APP).to_variant())?.get::<i32>().filter(|handle| *handle >= 0).ok_or(KeyringError::Locked)?;
        let read = self.call("readPassword", (handle, format!("{name} Keys"), format!("{name} Safe Storage"), Self::APP).to_variant());
        let _ = self.call("close", (handle, false, Self::APP).to_variant());
        match read?.str() {
            Some("") | None => Err(KeyringError::Missing),
            Some(password) => Ok(password.as_bytes().to_vec()),
        }
    }
}
