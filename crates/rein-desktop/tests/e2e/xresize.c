// xresize <class> <w> <h>: move the mapped top-level windows whose WM_CLASS contains <class>
// to 0,0 and resize them — what a tiling window manager or a maximise does. Exit 1: none.
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int matches(Display *d, Window w, const char *name) {
  XClassHint h;
  int hit = 0;
  if (XGetClassHint(d, w, &h)) {
    hit = (h.res_name && strstr(h.res_name, name)) || (h.res_class && strstr(h.res_class, name));
    XFree(h.res_name);
    XFree(h.res_class);
  }
  return hit;
}

int main(int argc, char **argv) {
  if (argc < 4) return 2;
  Display *d = XOpenDisplay(NULL);
  if (!d) return 3;
  Window root_ret, parent, *kids = NULL;
  unsigned n = 0;
  int found = 0;
  XQueryTree(d, DefaultRootWindow(d), &root_ret, &parent, &kids, &n);
  for (unsigned i = 0; i < n; i++) {
    XWindowAttributes a;
    XGetWindowAttributes(d, kids[i], &a);
    if (a.map_state == IsViewable && matches(d, kids[i], argv[1])) {
      XMoveResizeWindow(d, kids[i], 0, 0, atoi(argv[2]), atoi(argv[3]));
      found++;
    }
  }
  if (kids) XFree(kids);
  XSync(d, False);
  printf("%d\n", found);
  return found ? 0 : 1;
}
