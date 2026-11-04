# Event-Timers

## 1.2.0

- Reorganized Settings into a 5-tab interface (**Tracking & Upcoming**, **Notifications**, **Appearance & Window**, **Tracks & Schedule**, and **Daily Completion**) with quick event search.
- Added **GW2 API & Manual Daily Completion Tracking**:
  - Right-click any event in the Timeline, Upcoming Panel, or Settings to **Mark as Done Today** (automatically resets at `00:00 UTC`).
  - Optional GW2 API key support (`account` and `progression` permissions) to automatically strike out completed daily World Bosses and Hero's Choice Chests (including optional shared regional chests).
  - Added options to mute notifications for completed events and hide completed events in the Upcoming Events panel.
- Added **Upcoming Events Panel Filter Modes**: choose between **Tracked Only**, **Tracked + Favorites**, or **All Visible Events**.
- Added **Sticky Time Ruler** option to keep the time header pinned at the top while scrolling through tracks.
- Added **Event Color Modes**: **Default (Per-Event)**, **By Expansion / Category**, and **Monochrome (Highlight Tracked & Favorites)**.
- Added **Persistent Default Event Customizations**: rename events, edit waypoints (`copy_text`), colors, durations, start offsets, and reorder tracks within categories—saved in `user_config.json` and preserved across schedule updates.
- Added new schedules: **Leyspring Hollows** (`Depths of Cruelty`), **Nexus of Eternity** Convergence, and **Seasonal Events** (`Halloween`, `Dragon Bash`, `Festival of the Four Winds`, `Wintersday`, `Lunar New Year`, and `Super Adventure Festival`).
- Hardened `event_tracks.json` schedule updates with a jsDelivr CDN fallback, validation against `429 Too Many Requests` error responses, and automatic backup/built-in recovery.
- Fixed `Close with ESC` key state tracking and midnight-spanning event active highlights.
