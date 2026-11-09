"""Synthetic GTK/WebKit input fixture, only on an explicitly isolated X server."""
import argparse
import ctypes
import json
import os
from pathlib import Path

import gi

gi.require_version('Gtk', '3.0')
gi.require_version('GdkX11', '3.0')
from gi.repository import Gdk, GdkX11, GLib, Gtk

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('kind', choices=['entry', 'web'])
parser.add_argument('state', type=Path)
args = parser.parse_args()
if os.environ.get('LVF_ISOLATED_X11') != '1' or os.environ.get('DISPLAY') != ':198':
    parser.error('Run only on the dedicated Xvfb :198 server')

window = Gtk.Window(title='Linux Voice Flow — isolated synthetic input')
window.set_default_size(600, 240)
window.connect('destroy', Gtk.main_quit)
ready = False
if args.kind == 'entry':
    field = Gtk.Entry()
else:
    gi.require_version('WebKit2', '4.1')
    from gi.repository import WebKit2
    field = WebKit2.WebView()
    field.load_html('<!doctype html><html lang="fr"><title>Isolated input</title><textarea id="input" autofocus rows="6" cols="60"></textarea></html>', None)

window.add(field)
window.show_all()
field.grab_focus()

xlib = ctypes.CDLL('libX11.so.6')
xlib.XOpenDisplay.restype = ctypes.c_void_p
xlib.XOpenDisplay.argtypes = [ctypes.c_char_p]
xlib.XDefaultRootWindow.restype = ctypes.c_ulong
xlib.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
xlib.XInternAtom.restype = ctypes.c_ulong
xlib.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
xlib.XChangeProperty.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
xlib.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
xlib.XFlush.argtypes = [ctypes.c_void_p]
display = xlib.XOpenDisplay(None)
if not display:
    raise RuntimeError('Isolated display unavailable')
xid = GdkX11.X11Window.get_xid(window.get_window())


def property32(target, name, value, kind):
    data = ctypes.c_ulong(value)
    xlib.XChangeProperty(display, target, xlib.XInternAtom(display, name, 0),
                         xlib.XInternAtom(display, kind, 0), 32, 0,
                         ctypes.byref(data), 1)


def activate():
    Gdk.Display.get_default().sync()
    root = xlib.XDefaultRootWindow(display)
    property32(root, b'_NET_SUPPORTED', xlib.XInternAtom(display, b'_NET_ACTIVE_WINDOW', 0), b'ATOM')
    property32(root, b'_NET_ACTIVE_WINDOW', xid, b'WINDOW')
    property32(xid, b'_NET_WM_PID', os.getpid(), b'CARDINAL')
    xlib.XSetInputFocus(display, xid, 1, 0)
    xlib.XFlush(display)


def publish(text):
    global ready
    state = {'window': xid, 'text': text, 'ready': ready}
    temporary = args.state.with_suffix('.pending')
    temporary.write_text(json.dumps(state), encoding='utf-8')
    temporary.replace(args.state)


def poll():
    if args.kind == 'entry':
        publish(field.get_text())
    elif ready:
        field.run_javascript('document.getElementById("input").value', None, web_result, None)
    return True


def web_result(view, result, _):
    value = view.run_javascript_finish(result).get_js_value().to_string()
    publish(value)


def web_loaded(view, event):
    global ready
    if event == WebKit2.LoadEvent.FINISHED:
        view.run_javascript('document.getElementById("input").focus()', None, None, None)
        activate()
        ready = True


if args.kind == 'web':
    field.connect('load-changed', web_loaded)
else:
    activate()
    ready = True
GLib.timeout_add(40, poll)
Gtk.main()
