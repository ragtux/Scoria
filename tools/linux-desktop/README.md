# Desktop entry

A Linux desktop keeps an application's icon in neither the executable nor a bundle: it reads a
name out of a `.desktop` file and looks that name up in the icon theme. So the Linux half of the
icon work is two installed files rather than anything embedded, and `install.sh` is what puts
them in place:

```
just desktop-install                                      # a release build, then both files
tools/linux-desktop/install.sh --binary ~/bin/scoria      # an existing binary
tools/linux-desktop/install.sh --prefix /usr/local/share  # system-wide, needs root
tools/linux-desktop/install.sh --obsidian-scheme          # also handle obsidian:// links
tools/linux-desktop/install.sh --uninstall
```

Everything lands under `$XDG_DATA_HOME` (`~/.local/share` by default), so nothing needs root:
`applications/scoria.desktop`, and the hicolor theme from
`crates/scoria/resources/linux/icons`, cut from the logo master by `tools/app-icons/cut.py`.
It then refreshes `update-desktop-database` and `gtk-update-icon-cache` if either is installed,
and on Plasma drops the user's `icon-cache.kcache` and signals `KIconLoader`, because that cache
is keyed by name and an icon replaced under the same name otherwise stays the old picture in
the task bar. `--uninstall` removes exactly the files it wrote.

`scoria.desktop` ships `Exec=scoria %u`, which is right when the binary is on PATH and wrong for
an unpacked archive, so the script substitutes the absolute path of the binary it found. The
field code is `%u` rather than `%f` because the one argument Scoria takes may be a URL as well as
a path — a vault folder, a file inside one, or a `scoria://` URI. The script reads its sources
either from the repository or from beside itself, which is where the release archive puts it,
next to the binary.

Two keys in the entry are load-bearing and easy to break:

* **`Icon=scoria`** is a theme name, not a path. It has to be the stem the hicolor PNGs were
  written under.
* **`StartupWMClass=scoria`** is how the task bar ties a running window back to this entry, and
  so to the icon. It has to equal the `app_id` GPUI is given in `WindowOptions` (`src/main.rs`) —
  the Wayland app_id, the X11 `WM_CLASS`. Nothing but `crates/scoria/tests/icons.rs` connects the
  two strings, and the window falls back to the generic icon the moment they drift apart.

## What the entry claims

`MimeType=` is three things:

* **`inode/directory`** puts Scoria in a file manager's *Open With* for a folder, which is the
  shape a vault has.
* **`text/markdown`** does the same for a note. Opening one opens the vault that holds it — the
  nearest folder above it with a `.obsidian`, failing that one on the vault list, failing that
  the note's own folder — and then the note.
* **`x-scheme-handler/scoria`** is Scoria's own URI scheme: the `obsidian://` grammar under a
  name Obsidian does not own. `--obsidian-scheme` adds `x-scheme-handler/obsidian` as well, and
  the script warns first if Obsidian is installed, since on such a machine those links are its
  own.

With no argument at all Scoria opens the vault manager, which is where *Open folder as vault* is.

Being listed in `MimeType=` only *offers* the entry; what makes a link open in it is the
per-user default list, so the script also runs `xdg-mime default scoria.desktop
x-scheme-handler/scoria` for the schemes (not for the file types — a person whose notes open in
an editor meant that). That list is always the calling user's `mimeapps.list`, whatever
`--prefix` says, so a system-wide install writes none of it and prints the command to run
instead. `--uninstall` removes the files it wrote; a leftover line in `mimeapps.list` naming a
`scoria.desktop` that is gone is inert, and `xdg-mime default` is how to point it elsewhere.
