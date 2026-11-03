#!/usr/bin/env bash
# Put Scoria in the desktop's menu and give its window an icon.
#
#   tools/linux-desktop/install.sh                  # from the repository, or the archive
#   tools/linux-desktop/install.sh --binary ~/bin/scoria
#   tools/linux-desktop/install.sh --prefix /usr/local/share   # system-wide, needs root
#   tools/linux-desktop/install.sh --obsidian-scheme           # also handle obsidian:// links
#   tools/linux-desktop/install.sh --uninstall
#
# A Linux desktop keeps an application's icon in neither the executable nor a bundle but in the
# icon theme, looked up by the `Icon=` name in a .desktop file. The task bar finds the entry for
# a running window by matching its Wayland app_id (or X11 WM_CLASS) against `StartupWMClass`,
# which is why the window is drawn with the generic icon until both files are installed.
#
# The entry also claims the file types and the URI scheme Scoria opens: a folder, a Markdown
# note, and `scoria://`. `--obsidian-scheme` adds `obsidian://` to that list and makes Scoria its
# default handler, which is for a machine without Obsidian: where Obsidian is installed, it owns
# that scheme and taking it away would break its own links.
#
# Everything lands under the XDG data directory, which is per-user by default and needs no root.
# On Plasma the user's icon cache is dropped afterwards, or the task bar keeps the old picture.
set -euo pipefail

user_prefix=${XDG_DATA_HOME:-$HOME/.local/share}
prefix=$user_prefix
binary=
uninstall=false
obsidian_scheme=false

while [ $# -gt 0 ]; do
  case $1 in
    --uninstall) uninstall=true ;;
    --obsidian-scheme) obsidian_scheme=true ;;
    --binary) binary=${2:?--binary needs a path}; shift ;;
    --prefix) prefix=${2:?--prefix needs a directory}; shift ;;
    -h|--help) sed -n '2,19p' "${BASH_SOURCE[0]}" | cut -c3-; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
desktop="$prefix/applications/scoria.desktop"
icons="$prefix/icons/hicolor"

# Refresh whatever caches this desktop keeps. Both are best-effort: a missing tool means the
# desktop reads the directories directly, and GNOME and KDE both notice the new files anyway.
refresh() {
  command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "$prefix/applications" 2>/dev/null || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache -qtf "$icons" 2>/dev/null || true
  refresh_plasma
}

# Plasma keeps every icon it has drawn in a cache keyed by the icon's name, and an icon file
# replaced under the same name is not noticed: the task bar and the menu go on drawing the old
# picture until the cache is dropped and KIconLoader is told, which is what this does. The cache
# and the session are the calling user's, so a system-wide install leaves them alone: that
# user's desktop is somebody else's.
refresh_plasma() {
  [ "$prefix" = "$user_prefix" ] || return 0
  case ${XDG_CURRENT_DESKTOP:-} in *KDE*) ;; *) [ -n "${KDE_FULL_SESSION:-}" ] || return 0 ;; esac
  rm -f "${XDG_CACHE_HOME:-$HOME/.cache}/icon-cache.kcache"
  command -v kbuildsycoca6 >/dev/null 2>&1 && kbuildsycoca6 >/dev/null 2>&1 || true
  command -v dbus-send >/dev/null 2>&1 || return 0
  # One signal per KIconLoader group: Desktop, Toolbar, MainToolbar, Small, Panel, Dialog.
  for group in 0 1 2 3 4 5; do
    dbus-send --session --type=signal /KIconLoader org.kde.KIconLoader.iconChanged \
      "int32:$group" 2>/dev/null || true
  done
}

# `MimeType=` only offers the entry; what makes a link open in it is the per-user default list,
# which is what xdg-mime writes. Best-effort, and only for the schemes: the file types are left
# as a choice in "Open With", because a person whose notes open in an editor meant that.
#
# That list is always the *calling user's* ($XDG_CONFIG_HOME/mimeapps.list), whatever --prefix
# says, so a system-wide install writes nothing: root's defaults are not the ones that matter,
# and nothing run for someone else should rewrite their choices.
set_default() {
  if [ "$prefix" != "$user_prefix" ]; then
    echo "  not touching $(basename "${XDG_CONFIG_HOME:-$HOME/.config}")/mimeapps.list for $1:"
    echo "    --prefix is not this user's data directory; run \`xdg-mime default scoria.desktop $1\`"
    return 0
  fi
  command -v xdg-mime >/dev/null 2>&1 || return 0
  xdg-mime default scoria.desktop "$1" 2>/dev/null || true
}

if $uninstall; then
  rm -f "$desktop"
  # Only the files this script wrote, so an icon theme of the user's own is left alone.
  find "$icons" -name scoria.png -path '*/apps/*' -delete 2>/dev/null || true
  rm -f "$icons/scalable/apps/scoria.svg"
  refresh
  echo "removed $desktop and the scoria icons under $icons"
  exit 0
fi

# Two layouts: the repository, and the release archive, where install.sh sits beside the binary.
if [ -d "$here/../../crates/scoria/resources/linux" ]; then
  source_dir="$here/../../crates/scoria/resources/linux"
elif [ -d "$here/icons/hicolor" ]; then
  source_dir="$here"
else
  echo "cannot find scoria.desktop and icons/ next to $here" >&2
  exit 1
fi

# The entry ships `Exec=scoria`, which is right only when the binary is on PATH. It is not when
# the archive has been unpacked into a folder, so the absolute path goes in instead.
if [ -z "$binary" ]; then
  if [ -x "$here/scoria" ]; then
    binary="$here/scoria"
  elif command -v scoria >/dev/null 2>&1; then
    binary=scoria
  else
    echo "no scoria binary found; pass --binary <path>" >&2
    exit 1
  fi
fi
if [ "$binary" != "scoria" ]; then
  binary=$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")
  [ -x "$binary" ] || { echo "$binary is not executable" >&2; exit 1; }
fi

mkdir -p "$prefix/applications"
# A path with a space in it has to be quoted for the Exec key, and `exec` is the only key that
# changes, so the rest of the entry is copied through untouched.
exec_line="Exec=$binary %u"
case $binary in *[[:space:]]*) exec_line="Exec=\"$binary\" %u" ;; esac
sed "s|^Exec=.*|$exec_line|" "$source_dir/scoria.desktop" > "$desktop"

# obsidian:// is not in the shipped entry: it is this flag's doing, and only where Obsidian is
# not installed to have it taken from.
if $obsidian_scheme; then
  if command -v obsidian >/dev/null 2>&1; then
    echo "warning: Obsidian is installed on this machine and obsidian:// links are its own;" >&2
    echo "         claiming the scheme for Scoria will send them here instead." >&2
  fi
  sed -i "s|^MimeType=.*|&x-scheme-handler/obsidian;|" "$desktop"
fi

mkdir -p "$icons"
cp -r "$source_dir/icons/hicolor/." "$icons/"

set_default x-scheme-handler/scoria
if $obsidian_scheme; then set_default x-scheme-handler/obsidian; fi

refresh
echo "installed $desktop"
echo "  Exec=$binary"
echo "  icons under $icons"
echo "  handles scoria:// links, folders and Markdown files"
if $obsidian_scheme; then echo "  and obsidian:// links"; fi
