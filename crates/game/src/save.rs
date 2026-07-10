//! Local save file for player progression: a hand-rolled, line-based
//! `key value` format over std only (no serde on purpose — ARCHITECTURE.md
//! keeps simple save data as a plain file). Unknown keys are ignored and
//! missing keys default, so old saves survive new fields.

use std::io;
use std::path::{Path, PathBuf};

use crate::progress::{Daily, Day};
use crate::trophies::{TrophyCase, TrophyId};

const HEADER: &str = "zero-degrees-save v1";

/// Everything progression persists between sessions.
pub struct SaveData {
    pub daily: Daily,
    pub trophies: TrophyCase,
}

impl SaveData {
    pub fn new(today: Day) -> Self {
        Self {
            daily: Daily::new(today),
            trophies: TrophyCase::new(),
        }
    }
}

/// Where the save lives. `ZD_SAVE` overrides (tests, demo runs); otherwise a
/// platform-appropriate per-user data dir. Mobile builds will substitute the
/// OS-provided sandbox dir here once those pipelines land.
pub fn default_path() -> PathBuf {
    if let Some(p) = std::env::var_os("ZD_SAVE") {
        return PathBuf::from(p);
    }
    // Scripted demo runs stay out of the real save file.
    if std::env::var_os("ZD_DEMO").is_some() {
        return std::env::temp_dir().join("zero-degrees-demo.save");
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("zero-degrees")
        .join("progress.save")
}

pub fn serialize(data: &SaveData) -> String {
    let d = &data.daily;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    let mut line = |k: &str, v: String| {
        out.push_str(k);
        out.push(' ');
        out.push_str(&v);
        out.push('\n');
    };
    line("day", d.day.0.to_string());
    line("clears_today", d.clears_today.to_string());
    line("xp_today", d.xp_today.to_string());
    line("goal_met", u8::from(d.goal_met).to_string());
    line("streak", d.streak.to_string());
    line("best_streak", d.best_streak.to_string());
    line("shields", d.shields.to_string());
    for id in TrophyId::ALL {
        line(&format!("trophy.{}", id.key()), data.trophies.count(id).to_string());
    }
    out
}

/// Parse a save file body. Returns None only when the header is wrong —
/// individual bad lines are skipped rather than losing the whole save.
pub fn parse(text: &str) -> Option<SaveData> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(HEADER) {
        return None;
    }
    let mut data = SaveData::new(Day(0));
    let mut counts = [0u32; TrophyId::ALL.len()];
    for l in lines {
        let Some((key, value)) = l.trim().split_once(' ') else {
            continue;
        };
        let d = &mut data.daily;
        match key {
            "day" => d.day = Day(value.parse().unwrap_or(0)),
            "clears_today" => d.clears_today = value.parse().unwrap_or(0),
            "xp_today" => d.xp_today = value.parse().unwrap_or(0),
            "goal_met" => d.goal_met = value == "1",
            "streak" => d.streak = value.parse().unwrap_or(0),
            "best_streak" => d.best_streak = value.parse().unwrap_or(0),
            "shields" => d.shields = value.parse().unwrap_or(0),
            _ => {
                if let Some(name) = key.strip_prefix("trophy.") {
                    if let Some(i) = TrophyId::ALL.iter().position(|t| t.key() == name) {
                        counts[i] = value.parse().unwrap_or(0);
                    }
                }
            }
        }
    }
    data.trophies = TrophyCase::from_counts(counts);
    Some(data)
}

/// Load from `path`; None when absent or unreadable (fresh profile).
pub fn load(path: &Path) -> Option<SaveData> {
    parse(&std::fs::read_to_string(path).ok()?)
}

/// Write atomically-ish: temp file in the same dir, then rename.
pub fn store(path: &Path, data: &SaveData) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("save.tmp");
    std::fs::write(&tmp, serialize(data))?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{Sentiment, Urgency};
    use crate::progress::ClearEvent;

    fn sample() -> SaveData {
        let mut data = SaveData::new(Day(20_601));
        for _ in 0..5 {
            data.daily.on_clear(150, Day(20_601));
        }
        data.daily.best_streak = 9;
        data.daily.shields = 1;
        let critical = ClearEvent {
            message_id: 1,
            urgency: Urgency::Critical,
            sentiment: Sentiment::Angry,
            was_burning: true,
            response_seconds: 12.0,
            track_cleared: false,
            at: 0,
        };
        for _ in 0..6 {
            data.trophies.on_clear(&critical);
        }
        data
    }

    #[test]
    fn save_round_trips() {
        let data = sample();
        let restored = parse(&serialize(&data)).expect("parses");
        assert_eq!(restored.daily.day, data.daily.day);
        assert_eq!(restored.daily.clears_today, data.daily.clears_today);
        assert_eq!(restored.daily.xp_today, data.daily.xp_today);
        assert_eq!(restored.daily.goal_met, data.daily.goal_met);
        assert_eq!(restored.daily.streak, data.daily.streak);
        assert_eq!(restored.daily.best_streak, 9);
        assert_eq!(restored.daily.shields, 1);
        for id in TrophyId::ALL {
            assert_eq!(restored.trophies.count(id), data.trophies.count(id));
        }
    }

    #[test]
    fn wrong_header_is_rejected_bad_lines_are_skipped() {
        assert!(parse("not a save\nday 3").is_none());
        let text = format!("{HEADER}\nday 5\ngibberish\nstreak notanumber\nshields 2");
        let data = parse(&text).expect("parses");
        assert_eq!(data.daily.day, Day(5));
        assert_eq!(data.daily.streak, 0);
        assert_eq!(data.daily.shields, 2);
    }

    #[test]
    fn unknown_keys_are_ignored_for_forward_compat() {
        let text = format!("{HEADER}\nday 7\nfuture_feature 42\ntrophy.unknown 3");
        let data = parse(&text).expect("parses");
        assert_eq!(data.daily.day, Day(7));
    }

    #[test]
    fn store_and_load_via_disk() {
        let dir = std::env::temp_dir().join(format!("zd-save-test-{}", std::process::id()));
        let path = dir.join("progress.save");
        let data = sample();
        store(&path, &data).expect("writes");
        let restored = load(&path).expect("loads");
        assert_eq!(restored.daily.streak, data.daily.streak);
        assert_eq!(
            restored.trophies.count(TrophyId::Firefighter),
            data.trophies.count(TrophyId::Firefighter)
        );
        assert!(load(&dir.join("missing.save")).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
