# Event-Timers

## Test build 1.1.6

- Saves configuration every two seconds with temporary-file and backup recovery.
- Preserves in-session changes when the online timetable updates.
- Dynamically applies the quick-access visibility setting.
- Adds Dragon Bash Hologram Stampede and Festival of the Four Winds timers;
  seasonal tracks start hidden and can be enabled in settings.
- Fixes wrapped repeating schedules and keeps only the nearest occurrence of
  each event in the Upcoming Events panel.
- Adds optional GW2 API daily-completion tracking. An API key with `account`
  and `progression` permissions can strike completed mapped events in the
  upcoming and tracked-event tables.
