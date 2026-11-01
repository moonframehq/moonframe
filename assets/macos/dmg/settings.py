# dmgbuild settings for the release disk image (scripts/release/make-dmg.sh
# passes app, icon and background with -D). A 640x400 window with no
# toolbar or sidebar: the app on the left, the Applications shortcut on the
# right, over background.tiff (scripts/draw-dmg-background.swift), whose
# arrow and caption are drawn for exactly these positions.
# ruff: noqa: F821 (dmgbuild provides `defines`)
import os.path

app = defines["app"]
format = "UDZO"
compression_level = 9
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}
icon = defines["icon"]
background = defines["background"]

window_rect = ((200, 120), (640, 400))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 0
show_icon_preview = False
arrange_by = None
icon_size = 128
text_size = 13
label_pos = "bottom"
icon_locations = {
    os.path.basename(app): (160, 150),
    "Applications": (480, 150),
}
