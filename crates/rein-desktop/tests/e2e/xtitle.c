// xtitle <title> [-g]: print how many mapped top-level windows have exactly this title (WM_NAME
// or _NET_WM_NAME); with -g, print the first one's "x y w h" instead. Exit 0 when at least one
// does, 1 when none. A window run uses it as proof that
// a native dialog is on screen before it checks what the dialog's answer did (E-67).
#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <string.h>

static int titled(Display *d, Window w, const char *want) {
  int hit = 0;
  Atom net = XInternAtom(d, "_NET_WM_NAME", False), utf8 = XInternAtom(d, "UTF8_STRING", False);
  Atom type;
  int fmt;
  unsigned long n, after;
  unsigned char *v = NULL;
  if (XGetWindowProperty(d, w, net, 0, 1024, False, utf8, &type, &fmt, &n, &after, &v) == Success && v) {
    hit = strcmp((char *)v, want) == 0;
    XFree(v);
  }
  char *name = NULL;
  if (!hit && XFetchName(d, w, &name) && name) {
    hit = strcmp(name, want) == 0;
    XFree(name);
  }
  return hit;
}

int main(int argc, char **argv) {
  if (argc < 2) return 2;
  Display *d = XOpenDisplay(NULL);
  if (!d) return 3;
  Window root_ret, parent, *kids = NULL;
  unsigned n = 0;
  int found = 0, geo = argc > 2 && strcmp(argv[2], "-g") == 0;
  XWindowAttributes first;
  XQueryTree(d, DefaultRootWindow(d), &root_ret, &parent, &kids, &n);
  for (unsigned i = 0; i < n; i++) {
    XWindowAttributes a;
    XGetWindowAttributes(d, kids[i], &a);
    if (a.map_state == IsViewable && titled(d, kids[i], argv[1]) && found++ == 0) first = a;
  }
  if (kids) XFree(kids);
  if (geo && found)
    printf("%d %d %d %d\n", first.x, first.y, first.width, first.height);
  else if (!geo)
    printf("%d\n", found);
  return found ? 0 : 1;
}
