# Run by smoke.sh as an ordinary user in a clean container where poe-overlay
# was just installed. Checks the ticket 02 contract that can run without a
# game: the program and its data, the libraries, `launch` passing the game's
# exit code through, and, under a headless sway, the overlay starting (GTK and
# layer-shell up), one overlay for two launches, and none after the games end.
set -eu
fail() { echo "FAIL: $*"; exit 1; }

version=$(poe-overlay --version)
[ "$version" = "poe-overlay $VERSION" ] || fail "version: $version"
poe-overlay credits | grep -q HeartofPhos || fail credits
if [ -z "${APPIMAGE_TEST:-}" ] && ldd "$(command -v poe-overlay)" | grep 'not found'; then fail libraries; fi
code=0
env -u WAYLAND_DISPLAY poe-overlay launch -- sh -c 'exit 7' 2>/dev/null || code=$?
[ "$code" = 7 ] || fail "launch exit code: $code"

if [ -n "${APPIMAGE_TEST:-}" ]; then
	export STEAM_MARKER='kept as is'
	env -0 | tr '\0' '\n' | grep -v '^_=' | sort >/tmp/session
	env -u WAYLAND_DISPLAY poe-overlay launch -- env -0 2>/dev/null | tr '\0' '\n' | grep -v '^_=' | sort >/tmp/game
	diff /tmp/session /tmp/game || fail 'the game got another environment'
fi

# Processes started as `… overlay --game-pid N` (the AppImage adds its
# runtime's); the brackets keep grep from counting itself.
overlays() {
	for cmdline in /proc/[0-9]*/cmdline; do
		tr '\0' ' ' <"$cmdline" 2>/dev/null
		echo
	done | grep -c ' [o]verlay --game-pid ' || :
}
export XDG_RUNTIME_DIR=/tmp/runtime WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
mkdir -m 700 -p "$XDG_RUNTIME_DIR"
# Ubuntu 24.04's sway refuses to start where the host has NVIDIA's driver
# loaded; it renders nothing on the GPU here anyway.
sway --unsupported-gpu -c /dev/null >/tmp/sway.log 2>&1 &
for _ in $(seq 100); do
	[ -S "$XDG_RUNTIME_DIR/wayland-1" ] && break
	sleep 0.1
done
export WAYLAND_DISPLAY=wayland-1
poe-overlay launch -- sleep 6 2>/tmp/first.log &
first=$!
poe-overlay launch -- sleep 6 2>/tmp/second.log &
second=$!
sleep 4
running=$(overlays)
cat /tmp/first.log /tmp/second.log >/tmp/launches.log
wait "$first" "$second"
sleep 2
[ "$running" -ge 1 ] || { cat /tmp/launches.log; fail 'no overlay running beside the game'; }
grep -q 'an overlay is already running' /tmp/launches.log || { cat /tmp/launches.log; fail 'the second launch started another overlay'; }
! grep -E 'cannot start|lacks wlr-layer-shell' /tmp/launches.log || fail 'the overlay failed to start'
[ "$(overlays)" = 0 ] || fail 'the overlay outlived the game'
echo "ok: $(sed -n 's/^PRETTY_NAME=//p' /etc/os-release)"
