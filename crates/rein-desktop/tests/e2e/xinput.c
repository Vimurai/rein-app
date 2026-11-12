// xinput — drive the window on Xvfb for the screens (E-50, D-052) through XTest:
//   xinput key ctrl+shift+p      press a chord (modifiers: ctrl, shift, alt; keys by keysym
//                                name or one character: p, comma, Return, Escape, Tab, F1)
//   xinput type 'some text'      type printable ASCII, one key at a time
//   xinput move X Y              move the pointer
//   xinput click X Y [BUTTON]    move, then click (button 1 by default)
// Exit 2: bad usage; 3: no display or no XTest; 4: a key that has no keycode.
#include <X11/Xlib.h>
#include <X11/extensions/XTest.h>
#include <X11/keysym.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static Display *d;

static KeyCode code_of(const char *name) {
  KeySym ks = NoSymbol;
  if (strlen(name) == 1) {
    char buf[2] = {name[0], 0};
    if (name[0] == ' ') return XKeysymToKeycode(d, XK_space);
    ks = XStringToKeysym(buf);
    if (ks == NoSymbol) ks = (KeySym)(unsigned char)name[0];
  } else {
    ks = XStringToKeysym(name);
  }
  return ks == NoSymbol ? 0 : XKeysymToKeycode(d, ks);
}

static void tap(KeyCode k, int shift) {
  KeyCode sh = XKeysymToKeycode(d, XK_Shift_L);
  if (shift) XTestFakeKeyEvent(d, sh, True, 0);
  XTestFakeKeyEvent(d, k, True, 0);
  XTestFakeKeyEvent(d, k, False, 0);
  if (shift) XTestFakeKeyEvent(d, sh, False, 0);
  XFlush(d);
  usleep(30000);
}

static int chord(char *spec) {
  KeyCode mods[3];
  int n = 0;
  char *last = spec, *tok = strtok(spec, "+");
  while (tok) {
    last = tok;
    tok = strtok(NULL, "+");
    if (!tok) break;
    KeySym m = !strcmp(last, "ctrl") ? XK_Control_L : !strcmp(last, "shift") ? XK_Shift_L : !strcmp(last, "alt") ? XK_Alt_L : NoSymbol;
    if (m == NoSymbol || n == 3) return 2;
    mods[n++] = XKeysymToKeycode(d, m);
  }
  KeyCode k = code_of(last);
  if (!k) return 4;
  for (int i = 0; i < n; i++) XTestFakeKeyEvent(d, mods[i], True, 0);
  XTestFakeKeyEvent(d, k, True, 0);
  XTestFakeKeyEvent(d, k, False, 0);
  for (int i = n - 1; i >= 0; i--) XTestFakeKeyEvent(d, mods[i], False, 0);
  XFlush(d);
  return 0;
}

static int type(const char *s) {
  static const char *shifted = "~!@#$%^&*()_+{}|:\"<>?";
  for (; *s; s++) {
    char c[2] = {*s, 0};
    int up = (*s >= 'A' && *s <= 'Z') || strchr(shifted, *s);
    KeyCode k = code_of(c);
    if (!k) return 4;
    tap(k, up);
  }
  return 0;
}

int main(int argc, char **argv) {
  if (argc < 3) return 2;
  d = XOpenDisplay(NULL);
  int ev, er, maj, min;
  if (!d || !XTestQueryExtension(d, &ev, &er, &maj, &min)) return 3;
  int rc = 0;
  if (!strcmp(argv[1], "key")) {
    rc = chord(argv[2]);
  } else if (!strcmp(argv[1], "type")) {
    rc = type(argv[2]);
  } else if (!strcmp(argv[1], "move") && argc >= 4) {
    XTestFakeMotionEvent(d, -1, atoi(argv[2]), atoi(argv[3]), 0);
  } else if (!strcmp(argv[1], "click") && argc >= 4) {
    unsigned b = argc >= 5 ? (unsigned)atoi(argv[4]) : 1;
    XTestFakeMotionEvent(d, -1, atoi(argv[2]), atoi(argv[3]), 0);
    XFlush(d);
    usleep(50000);
    XTestFakeButtonEvent(d, b, True, 0);
    XTestFakeButtonEvent(d, b, False, 0);
  } else {
    rc = 2;
  }
  XFlush(d);
  XCloseDisplay(d);
  return rc;
}
