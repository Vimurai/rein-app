// notifyd <click-file> — a stand-in freedesktop notification server for a window run (E-79):
// it owns org.freedesktop.Notifications on the session bus named by DBUS_SESSION_BUS_ADDRESS
// (the run's own private bus, never the user's), answers Notify with a new id and prints
//   notify id=<n> app=<app_name> summary=<summary> body=<body>
// for each one; GetCapabilities ("body", "actions"), GetServerInformation and
// CloseNotification are answered too. When <click-file> appears it is removed and the last
// notification is "clicked": the ActionInvoked signal with the "default" action, as a real
// server sends for a click on the body; it prints `clicked id=<n>`.
// Exit 2: bad usage or no bus; 3: the name is taken.
#include <dbus/dbus.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#define PATH "/org/freedesktop/Notifications"
#define IFACE "org.freedesktop.Notifications"

static void reply_empty(DBusConnection *c, DBusMessage *m) {
  DBusMessage *r = dbus_message_new_method_return(m);
  dbus_connection_send(c, r, NULL);
  dbus_message_unref(r);
}

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  DBusError e;
  dbus_error_init(&e);
  DBusConnection *c = dbus_bus_get(DBUS_BUS_SESSION, &e);
  if (!c) {
    fprintf(stderr, "notifyd: %s\n", e.message);
    return 2;
  }
  if (dbus_bus_request_name(c, IFACE, DBUS_NAME_FLAG_DO_NOT_QUEUE, &e) != DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER) return 3;
  printf("ready\n");
  fflush(stdout);
  dbus_uint32_t next = 1, last = 0;
  for (;;) {
    dbus_connection_read_write(c, 100);
    if (last && access(argv[1], F_OK) == 0) {
      unlink(argv[1]);
      DBusMessage *s = dbus_message_new_signal(PATH, IFACE, "ActionInvoked");
      const char *action = "default";
      dbus_message_append_args(s, DBUS_TYPE_UINT32, &last, DBUS_TYPE_STRING, &action, DBUS_TYPE_INVALID);
      dbus_connection_send(c, s, NULL);
      dbus_connection_flush(c);
      dbus_message_unref(s);
      printf("clicked id=%u\n", last);
      fflush(stdout);
    }
    DBusMessage *m;
    while ((m = dbus_connection_pop_message(c))) {
      if (dbus_message_is_method_call(m, IFACE, "Notify")) {
        DBusMessageIter it;
        const char *app = "", *icon = "", *summary = "", *body = "";
        dbus_uint32_t replaces = 0;
        dbus_message_iter_init(m, &it);
        if (dbus_message_iter_get_arg_type(&it) == DBUS_TYPE_STRING) dbus_message_iter_get_basic(&it, &app);
        dbus_message_iter_next(&it);
        if (dbus_message_iter_get_arg_type(&it) == DBUS_TYPE_UINT32) dbus_message_iter_get_basic(&it, &replaces);
        dbus_message_iter_next(&it);
        if (dbus_message_iter_get_arg_type(&it) == DBUS_TYPE_STRING) dbus_message_iter_get_basic(&it, &icon);
        dbus_message_iter_next(&it);
        if (dbus_message_iter_get_arg_type(&it) == DBUS_TYPE_STRING) dbus_message_iter_get_basic(&it, &summary);
        dbus_message_iter_next(&it);
        if (dbus_message_iter_get_arg_type(&it) == DBUS_TYPE_STRING) dbus_message_iter_get_basic(&it, &body);
        last = next++;
        printf("notify id=%u app=%s summary=%s body=%s\n", last, app, summary, body);
        fflush(stdout);
        DBusMessage *r = dbus_message_new_method_return(m);
        dbus_message_append_args(r, DBUS_TYPE_UINT32, &last, DBUS_TYPE_INVALID);
        dbus_connection_send(c, r, NULL);
        dbus_message_unref(r);
      } else if (dbus_message_is_method_call(m, IFACE, "GetCapabilities")) {
        DBusMessage *r = dbus_message_new_method_return(m);
        DBusMessageIter it, arr;
        const char *caps[] = {"body", "actions"};
        dbus_message_iter_init_append(r, &it);
        dbus_message_iter_open_container(&it, DBUS_TYPE_ARRAY, "s", &arr);
        for (int i = 0; i < 2; i++) dbus_message_iter_append_basic(&arr, DBUS_TYPE_STRING, &caps[i]);
        dbus_message_iter_close_container(&it, &arr);
        dbus_connection_send(c, r, NULL);
        dbus_message_unref(r);
      } else if (dbus_message_is_method_call(m, IFACE, "GetServerInformation")) {
        DBusMessage *r = dbus_message_new_method_return(m);
        const char *name = "notifyd", *vendor = "rein-app tests", *ver = "1", *spec = "1.2";
        dbus_message_append_args(r, DBUS_TYPE_STRING, &name, DBUS_TYPE_STRING, &vendor, DBUS_TYPE_STRING, &ver, DBUS_TYPE_STRING, &spec, DBUS_TYPE_INVALID);
        dbus_connection_send(c, r, NULL);
        dbus_message_unref(r);
      } else if (dbus_message_is_method_call(m, IFACE, "CloseNotification")) {
        reply_empty(c, m);
      }
      dbus_message_unref(m);
    }
  }
}
