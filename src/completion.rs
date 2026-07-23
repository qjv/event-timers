use nexus::paths::get_addon_dir;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL_SECONDS: i64 = 60;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CompletionSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
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
        status: "Completion tracking is disabled".into(),
        ..CompletionState::default()
    })
});
static LAST_REFRESH_ATTEMPT: AtomicI64 = AtomicI64::new(0);

fn settings_path() -> Option<std::path::PathBuf> {
    get_addon_dir("event_timers").map(|path| path.join("completion_settings.json"))
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

pub fn load() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Ok(json) = std::fs::read(&path) {
        if let Ok(settings) = serde_json::from_slice::<CompletionSettings>(&json) {
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
    if let Ok(json) = serde_json::to_vec_pretty(&*SETTINGS.lock()) {
        let _ = std::fs::write(path, json);
    }
}

pub fn settings() -> CompletionSettings {
    SETTINGS.lock().clone()
}

pub fn update_settings(enabled: bool, api_key: String) {
    let mut settings = SETTINGS.lock();
    if settings.enabled == enabled && settings.api_key == api_key {
        return;
    }
    settings.enabled = enabled;
    settings.api_key = api_key;
    drop(settings);
    save();
    request_refresh();
}

pub fn status() -> String {
    STATE.lock().status.clone()
}

pub fn request_refresh() {
    LAST_REFRESH_ATTEMPT.store(0, Ordering::Release);
}

fn completion_token(track_name: &str, event_name: &str) -> Option<&'static str> {
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
        ("Verdant Brink", "Night Bosses") => Some("mapchest:verdant_brink_heros_choice_chest"),
        ("Auric Basin", "Battle in Tarir") => Some("mapchest:auric_basin_heros_choice_chest"),
        ("Tangled Depths", "King of the Jungle") => {
            Some("mapchest:tangled_depths_heros_choice_chest")
        }
        ("Dragon's Stand", "Advancing on the Blighting Towers") => {
            Some("mapchest:dragons_stand_heros_choice_chest")
        }
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
        _ => None,
    }
}

pub fn is_completed(track_name: &str, event_name: &str) -> bool {
    let settings = SETTINGS.lock();
    if !settings.enabled {
        return false;
    }
    drop(settings);
    completion_token(track_name, event_name)
        .is_some_and(|token| STATE.lock().completed.contains(token))
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
    let settings = SETTINGS.lock().clone();
    if !settings.enabled {
        let mut state = STATE.lock();
        state.completed.clear();
        state.status = "Completion tracking is disabled".into();
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

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            STATE.lock().status = format!("Could not initialize API client: {error}");
            return;
        }
    };
    let worldbosses = fetch_ids(
        &client,
        "https://api.guildwars2.com/v2/account/worldbosses",
        settings.api_key.trim(),
    );
    let mapchests = fetch_ids(
        &client,
        "https://api.guildwars2.com/v2/account/mapchests",
        settings.api_key.trim(),
    );
    match (worldbosses, mapchests) {
        (Ok(worldbosses), Ok(mapchests)) => {
            let mut completed = HashSet::new();
            completed.extend(worldbosses.into_iter().map(|id| format!("worldboss:{id}")));
            completed.extend(mapchests.into_iter().map(|id| format!("mapchest:{id}")));
            let count = completed.len();
            let mut state = STATE.lock();
            state.completed = completed;
            state.status = format!("Updated · {count} daily completions found");
        }
        (Err(error), _) | (_, Err(error)) => {
            STATE.lock().status = format!("GW2 API error: {error}");
        }
    }
}
