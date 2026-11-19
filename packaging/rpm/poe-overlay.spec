# Built by packaging/rpm/build.sh, with the Rust toolchain from rustup.
%global debug_package %{nil}

Name:           poe-overlay
Version:        %{version}
Release:        1%{?dist}
Summary:        Path of Exile 1 overlay for Wayland
License:        MIT OR Apache-2.0
URL:            https://github.com/nephritus/poe-overlay-linux
Source0:        poe-overlay-%{version}.tar.gz

BuildRequires:  gcc
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(gtk4-layer-shell-0)
BuildRequires:  pkgconfig(xkbcommon)
BuildRequires:  pkgconfig(sqlite3)
BuildRequires:  systemd-rpm-macros
# GTK's renderer loads it at run time; without it the overlay aborts on a
# minimal install.
Requires:       libGLESv2.so.2()(64bit)
Recommends:     xorg-x11-server-Xwayland

%description
Leveling Route, Price Check, Currency Rates and Trade Requests over the game,
shown only while it's focused. Start it with the Steam launch option
"poe-overlay launch -- %%command%%".

%prep
%autosetup
cp data/exile-leveling/LICENSE LICENSE-exile-leveling

%build
cargo build --locked --release

%install
sh packaging/install-files.sh %{buildroot} target/release/poe-overlay

%post
%udev_rules_update
# Gives the logged-in user /dev/uinput now rather than at the next boot.
udevadm trigger --subsystem-match=misc --sysname-match=uinput --action=change >/dev/null 2>&1 || :

%files
%license LICENSE-MIT LICENSE-APACHE LICENSE-exile-leveling
%doc README.md
%{_bindir}/poe-overlay
%{_datadir}/applications/io.github.nephritus.PoeOverlay.Settings.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.nephritus.PoeOverlay.svg
%{_udevrulesdir}/60-poe-overlay-uinput.rules
