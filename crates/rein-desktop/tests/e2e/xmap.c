// xmap <class> map|unmap: map or unmap the top-level windows whose WM_CLASS contains <class>
// — what minimising does to the page's visibility (`document.visibilityState`), for a window
// run that checks a hidden window reads nothing (E-60). Exit 1: no such window.
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
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
  if (argc < 3 || (strcmp(argv[2], "map") != 0 && strcmp(argv[2], "unmap") != 0)) return 2;
  Display *d = XOpenDisplay(NULL);
  if (!d) return 3;
  int map = strcmp(argv[2], "map") == 0;
  Window root_ret, parent, *kids = NULL;
  unsigned n = 0;
  int found = 0;
  XQueryTree(d, DefaultRootWindow(d), &root_ret, &parent, &kids, &n);
  for (unsigned i = 0; i < n; i++) {
    if (!matches(d, kids[i], argv[1])) continue;
    if (map)
      XMapWindow(d, kids[i]);
    else
      XUnmapWindow(d, kids[i]);
    found++;
  }
  if (kids) XFree(kids);
  XSync(d, False);
  printf("%d\n", found);
  return found ? 0 : 1;
}
