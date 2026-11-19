#!/bin/sh
# After installing: applies the uinput rule now rather than at the next boot.
if command -v udevadm >/dev/null 2>&1; then
	udevadm control --reload >/dev/null 2>&1 || :
	udevadm trigger --subsystem-match=misc --sysname-match=uinput --action=change >/dev/null 2>&1 || :
fi
exit 0
