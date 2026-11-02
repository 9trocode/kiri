# dmgbuild settings for Kiri's drag-to-install disk image (xtask passes the -D values):
#   app=<path to Kiri.app>  background=<1x png; a <name>@2x.png next to it is joined into one HiDPI TIFF>
# A fixed 640x400 (content) icon-view window with no toolbar, sidebar or status bar: the app on the left, an
# Applications alias on the right, and the background's arrow between them. dmgbuild writes the .DS_Store itself, so
# this needs no Finder or AppleScript. The positions match scripts/dmg-background.swift.
import os.path

app = defines["app"]  # noqa: F821 (dmgbuild provides `defines`)
app_name = os.path.basename(app)

format = "ULMO"  # LZMA: the smallest of hdiutil's formats
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}
icon = os.path.join(app, "Contents/Resources/Kiri.icns")  # the volume's icon

background = defines["background"]  # noqa: F821
window_rect = ((200, 120), (640, 432))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
show_icon_preview = False
include_icon_view_settings = True
include_list_view_settings = False

arrange_by = None
icon_size = 128
text_size = 13
label_pos = "bottom"
icon_locations = {
    app_name: (160, 190),
    "Applications": (480, 190),
}
