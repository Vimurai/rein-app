// LD_PRELOAD for terminal-size.sh: realpath() of a path containing $SLOW_OPEN_MARK sleeps
// $SLOW_OPEN_MS first. Project validation canonicalizes the project, so this holds tab_open
// open for as long as wsl.exe does on Windows (E-52), without a seam in the app.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

char *realpath(const char *path, char *resolved) {
  static char *(*real)(const char *, char *);
  if (!real) real = (char *(*)(const char *, char *))dlsym(RTLD_NEXT, "realpath");
  const char *mark = getenv("SLOW_OPEN_MARK");
  const char *ms = getenv("SLOW_OPEN_MS");
  if (path && mark && *mark && strstr(path, mark)) usleep(1000 * (ms ? atoi(ms) : 3000));
  return real(path, resolved);
}
