use crate::config::TrackedEventId;
use crate::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL_SECONDS: i64 = 60;
const SECONDS_PER_UTC_DAY: i64 = 86_400;

fn default_true() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CompletionSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub include_shared_chests: bool,
    #[serde(default)]
    pub manual_completed: HashSet<TrackedEventId>,
    #[serde(default)]
    pub last_reset_utc_day: i64,
}

impl Default for CompletionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            include_shared_chests: true,
            manual_completed: HashSet::new(),
            last_reset_utc_day: current_utc_day(),
        }
    }
}

#[derive(Default)]
struct CompletionState {
    completed: HashSet<String>,
    status: String,
}

static SETTINGS: Lazy<Mutex<CompletionSettings>> =
    Lazy::new(|| Mutex::new(CompletionSettings::default()));
static STATE: Lazy<Mutex<CompletionState>> = Lazy::new(|| {
    Mutex::new(CompletionState {
        status: "API completion tracking is disabled".into(),
        ..CompletionState::default()
    })
});
static LAST_REFRESH_ATTEMPT: AtomicI64 = AtomicI64::new(0);
static HTTP_CLIENT: Lazy<Option<reqwest::blocking::Client>> = Lazy::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()
});

fn settings_path() -> Option<std::path::PathBuf> {
    get_addon_dir("event_timers").map(|path| path.join("completion_settings.json"))
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn current_utc_day() -> i64 {
    unix_seconds().div_euclid(SECONDS_PER_UTC_DAY)
}

fn ensure_daily_reset_current() -> bool {
    let today = current_utc_day();
    let mut should_save = false;
    {
        let mut settings = SETTINGS.lock();
        if settings.last_reset_utc_day != today {
            if !settings.manual_completed.is_empty() {
                settings.manual_completed.clear();
            }
            settings.last_reset_utc_day = today;
            should_save = true;
        }
    }
    if should_save {
        STATE.lock().completed.clear();
        save();
        request_refresh();
    }
    should_save
}

pub fn load() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Ok(json) = std::fs::read(&path) {
        if let Ok(mut settings) = serde_json::from_slice::<CompletionSettings>(&json) {
            let today = current_utc_day();
            if settings.last_reset_utc_day != today {
                settings.manual_completed.clear();
                settings.last_reset_utc_day = today;
            }
            *SETTINGS.lock() = settings;
        }
    }
    request_refresh();
}

pub fn save() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_vec_pretty(&*SETTINGS.lock()).ok();
    if let Some(json) = json {
        let _ = std::fs::write(path, json);
    }
}

pub fn settings() -> CompletionSettings {
    ensure_daily_reset_current();
    SETTINGS.lock().clone()
}

pub fn update_settings(enabled: bool, api_key: String, include_shared_chests: bool) {
    let mut settings = SETTINGS.lock();
    if settings.enabled == enabled
        && settings.api_key == api_key
        && settings.include_shared_chests == include_shared_chests
    {
        return;
    }
    settings.enabled = enabled;
    settings.api_key = api_key;
    settings.include_shared_chests = include_shared_chests;
    drop(settings);
    STATE.lock().completed.clear();
    save();
    request_refresh();
}

pub fn is_manually_completed(track_name: &str, event_name: &str) -> bool {
    ensure_daily_reset_current();
    let id = TrackedEventId::new(track_name, event_name);
    SETTINGS.lock().manual_completed.contains(&id)
}

pub fn toggle_manual_completion(track_name: &str, event_name: &str) {
    ensure_daily_reset_current();
    let id = TrackedEventId::new(track_name, event_name);
    {
        let mut settings = SETTINGS.lock();
        if settings.manual_completed.contains(&id) {
            settings.manual_completed.remove(&id);
        } else {
            settings.manual_completed.insert(id);
        }
    }
    save();
}

pub fn clear_manual_completions() {
    {
        let mut settings = SETTINGS.lock();
        if settings.manual_completed.is_empty() {
            return;
        }
        settings.manual_completed.clear();
    }
    save();
}

pub fn status() -> String {
    STATE.lock().status.clone()
}

pub fn request_refresh() {
    LAST_REFRESH_ATTEMPT.store(0, Ordering::Release);
}

fn completion_token(
    track_name: &str,
    event_name: &str,
    include_shared_chests: bool,
) -> Option<&'static str> {
    match (track_name, event_name) {
        ("World Bosses", "Admiral Taidha Covington") => Some("worldboss:admiral_taidha_covington"),
        ("World Bosses", "Svanir Shaman Chief") => Some("worldboss:svanir_shaman_chief"),
        ("World Bosses", "Megadestroyer") => Some("worldboss:megadestroyer"),
        ("World Bosses", "Shadow Behemoth") => Some("worldboss:shadow_behemoth"),
        ("World Bosses", "The Shatterer") => Some("worldboss:the_shatterer"),
        ("World Bosses", "Great Jungle Wurm") => Some("worldboss:great_jungle_wurm"),
        ("World Bosses", "Modniir Ulgoth") => Some("worldboss:modniir_ulgoth"),
        ("World Bosses", "Fire Elemental") => Some("worldboss:fire_elemental"),
        ("World Bosses", "Golem Mark II") => Some("worldboss:inquest_golem_mark_ii"),
        ("World Bosses", "Claw of Jormag") => Some("worldboss:claw_of_jormag"),
        ("Hard World Bosses", "Karka Queen") => Some("worldboss:karka_queen"),
        ("Hard World Bosses", "Tequatl") => Some("worldboss:tequatl_the_sunless"),
        ("Hard World Bosses", "Triple Trouble") => Some("worldboss:triple_trouble_wurm"),
        ("Bjora Marches", "Champion of the Ice Dragon") => Some("worldboss:drakkar"),
        ("Janthir Syntri", "Of Mists and Monsters") => Some("worldboss:mists_and_monsters_titans"),
        ("Bava Nisos", "A Titanic Voyage") => Some("mapchest:bava_nisos_heros_choice_chest"),
        ("Verdant Brink", "Night Bosses") => Some("mapchest:verdant_brink_heros_choice_chest"),
        ("Auric Basin", "Battle in Tarir") => Some("mapchest:auric_basin_heros_choice_chest"),
        ("Tangled Depths", "King of the Jungle") => {
            Some("mapchest:tangled_depths_heros_choice_chest")
        }
        ("Dragon's Stand", "Advancing on the Blighting Towers") => {
            Some("mapchest:dragons_stand_heros_choice_chest")
        }
        ("Crystal Oasis", "Choya Piñata") => Some("mapchest:crystal_oasis_heros_choice_chest"),
        ("Elon Riverlands", "Doppelganger") => Some("mapchest:elon_riverlands_heros_choice_chest"),
        ("Seitung Province", "Aetherblade Assault") => {
            Some("mapchest:seitung_province_heros_choice_chest")
        }
        ("New Kaineng City", "Kaineng Blackout") => {
            Some("mapchest:new_kaineng_city_heros_choice_chest")
        }
        ("The Echovald Wilds", "The Gang War of Echovald") => {
            Some("mapchest:echovald_wilds_heros_choice_chest")
        }
        ("Dragon's End", "The Battle for the Jade Sea") => {
            Some("mapchest:dragons_end_heros_choice_chest")
        }
        ("Skywatch Archipelago", "Unlocking the Wizard's Tower") => {
            Some("mapchest:skywatch_archipelago_heros_choice_chest")
        }
        ("Amnytas", "The Defense of Amnytas") => Some("mapchest:amnytas_heros_choice_chest"),
        // Shared regional Hero's Choice chests (enabled when include_shared_chests is true):
        ("Convergences", "Outer Nayos" | "Mount Balrior" | "Nexus of Eternity")
            if include_shared_chests =>
        {
            Some("mapchest:convergence_heros_choice_chest")
        }
        ("The Desolation", "Maws of Torment" | "Junundu Rising") if include_shared_chests => {
            Some("mapchest:the_desolation_heros_choice_chest")
        }
        ("Domain of Vabbi", "Forged with Fire" | "Serpent's Ire") if include_shared_chests => {
            Some("mapchest:domain_of_vabbi_heros_choice_chest")
        }
        (
            "Shipwreck Strand" | "Starlit Weald" | "Garden of the Ancients" | "Leyspring Hollows",
            "Hammerhart Rumble!"
            | "Secrets of the Weald"
            | "Shackles of the Ancients"
            | "Depths of Cruelty",
        ) if include_shared_chests => Some("mapchest:wild_island_heros_choice_chest"),
        _ => None,
    }
}

pub fn is_completed(track_name: &str, event_name: &str) -> bool {
    ensure_daily_reset_current();
    let (api_enabled, include_shared, manual_done) = {
        let settings = SETTINGS.lock();
        let id = TrackedEventId::new(track_name, event_name);
        (
            settings.enabled,
            settings.include_shared_chests,
            settings.manual_completed.contains(&id),
        )
    };
    if manual_done {
        return true;
    }
    if !api_enabled {
        return false;
    }
    let Some(token) = completion_token(track_name, event_name, include_shared) else {
        return false;
    };
    STATE.lock().completed.contains(token)
}

fn fetch_ids(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
    client
        .get(endpoint)
        .bearer_auth(api_key)
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json::<Vec<String>>()
        .map_err(|error| error.to_string())
}

pub fn refresh_if_needed() {
    ensure_daily_reset_current();
    let settings = SETTINGS.lock().clone();
    if !settings.enabled {
        let mut state = STATE.lock();
        state.completed.clear();
        state.status = "API completion tracking is disabled".into();
        return;
    }
    if settings.api_key.trim().is_empty() {
        STATE.lock().status = "Enter an API key with account and progression permissions".into();
        return;
    }

    let now = unix_seconds();
    let last = LAST_REFRESH_ATTEMPT.load(Ordering::Acquire);
    if now.saturating_sub(last) < REFRESH_INTERVAL_SECONDS {
        return;
    }
    LAST_REFRESH_ATTEMPT.store(now, Ordering::Release);

    let Some(client) = HTTP_CLIENT.as_ref() else {
        STATE.lock().status = "Could not initialize HTTP client".into();
        return;
    };
    let worldbosses = fetch_ids(
        client,
        "https://api.guildwars2.com/v2/account/worldbosses",
        settings.api_key.trim(),
    );
    let mapchests = fetch_ids(
        client,
        "https://api.guildwars2.com/v2/account/mapchests",
        settings.api_key.trim(),
    );
    // Ignore responses for an API key or UTC day that changed during the request.
    let current = SETTINGS.lock();
    if current.api_key != settings.api_key
        || current.enabled != settings.enabled
        || current.last_reset_utc_day != current_utc_day()
    {
        return;
    }
    match (worldbosses, mapchests) {
        (Ok(worldbosses), Ok(mapchests)) => {
            let mut completed = HashSet::new();
            completed.extend(worldbosses.into_iter().map(|id| format!("worldboss:{id}")));
            completed.extend(mapchests.into_iter().map(|id| format!("mapchest:{id}")));
            let count = completed.len();
            let mut state = STATE.lock();
            state.completed = completed;
            state.status = format!("Updated · {count} API daily completions found");
        }
        (Err(error), _) | (_, Err(error)) => {
            STATE.lock().status = format!("GW2 API error: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::completion_token;

    #[test]
    fn completion_tokens_match_expanded_schedule_names() {
        assert_eq!(
            completion_token("Bjora Marches", "Champion of the Ice Dragon", false),
            Some("worldboss:drakkar")
        );
        assert_eq!(
            completion_token("Auric Basin", "Battle in Tarir", false),
            Some("mapchest:auric_basin_heros_choice_chest")
        );
        assert_eq!(
            completion_token("Crystal Oasis", "Choya Piñata", false),
            Some("mapchest:crystal_oasis_heros_choice_chest")
        );
        assert_eq!(
            completion_token("Elon Riverlands", "Doppelganger", false),
            Some("mapchest:elon_riverlands_heros_choice_chest")
        );
        assert_eq!(
            completion_token("Convergences", "Nexus of Eternity", true),
            Some("mapchest:convergence_heros_choice_chest")
        );
        assert_eq!(
            completion_token("Convergences", "Nexus of Eternity", false),
            None
        );
    }
}
