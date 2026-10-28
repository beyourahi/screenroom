//! Persistent user preferences.
//!
//! Preferences live in the `settings` key/value table that the schema already
//! carries, so nothing here changes the physical layout and an existing library
//! keeps its rows. Values are stored as text because that is what the table holds;
//! the typed [`Preferences`] shape is the only interface callers use, so no caller
//! parses a string.
//!
//! Reads are forgiving on purpose. A preference whose stored value is unreadable —
//! written by a newer build, or corrupted — falls back to its default rather than
//! failing the whole read, because a preference is an optimisation and must never
//! keep the library from opening.

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

use super::Library;

/// Keys used in the `settings` table.
///
/// The stored spelling is a compatibility surface: renaming a key silently
/// discards the user's choice, so these strings are fixed.
const AUTOPLAY_NEXT: &str = "autoplayNext";
const AUTO_SKIP_INTRO: &str = "autoSkipIntro";
const SUBTITLE_LANGUAGE: &str = "subtitleLanguage";
const AUDIO_LANGUAGE: &str = "audioLanguage";
const SUBTITLE_SCALE: &str = "subtitleScalePercent";
const SUBTITLE_POSITION: &str = "subtitlePositionPercent";
const DEFAULT_PROFILE: &str = "defaultProfileId";
const VOLUME_PERCENT: &str = "volumePercent";

/// Normal subtitle text size, as a percentage.
const SUBTITLE_SCALE_NORMAL: u16 = 100;

/// libmpv's own default for `sub-pos`: the bottom of the screen.
const SUBTITLE_POSITION_BOTTOM: u16 = 100;

/// The user's preferences for this device.
///
/// Every field has a defensible default, so a library that has never been
/// configured behaves predictably instead of needing a setup step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    /// Play the next verified episode when one finishes.
    ///
    /// Defaults to on: continuing a series is the common case, and the setting is
    /// one toggle away. It is still a stored preference rather than a constant so
    /// a viewer who dislikes it can switch it off and have that survive a restart.
    pub autoplay_next: bool,
    /// Jump past a stored intro segment without being asked.
    ///
    /// Off by default. Skipping a title sequence is a real convenience once the
    /// markers exist, but doing it uninvited is the app deciding on the viewer's
    /// behalf, so the manual button comes first and this is the opt-in. Only a
    /// marker that was actually stored can be skipped; guessing an intro is not
    /// what this setting does.
    pub auto_skip_intro: bool,
    /// Preferred subtitle language tag, for example `en`.
    ///
    /// `None` means "let the player choose", which is not the same as "no
    /// subtitles": the player keeps its own behaviour until the user expresses a
    /// preference.
    pub subtitle_language: Option<String>,
    /// Preferred audio language tag, or `None` for the player's own choice.
    pub audio_language: Option<String>,
    /// Subtitle text size, as a percentage of normal.
    ///
    /// A whole percentage rather than a fraction so the value round-trips through
    /// the text-valued `settings` table exactly, and so this type stays a plain
    /// comparable value with no floating-point equality to reason about.
    pub subtitle_scale_percent: u16,
    /// Where subtitles sit, in percent of the screen height measured from the top.
    ///
    /// This is libmpv's `sub-pos`, kept in its own units rather than translated:
    /// a preference that means something different from the property it sets is
    /// how a setting and its effect drift apart.
    pub subtitle_position_percent: u16,
    /// Output volume, in percent of libmpv's 0-100 scale.
    ///
    /// Stored because a viewer who turned the film down should not be shouted at
    /// by the next one. 100 is libmpv's own default and the honest starting point.
    pub volume_percent: u16,
    /// Profile the app opens with.
    ///
    /// `None` means "first profile", which is what a single-viewer device wants.
    pub default_profile: Option<String>,
}

impl Preferences {
    /// Smallest subtitle size the Settings screen offers, in percent.
    pub const SUBTITLE_SCALE_MIN_PERCENT: u16 = 50;
    /// Largest subtitle size the Settings screen offers, in percent.
    pub const SUBTITLE_SCALE_MAX_PERCENT: u16 = 300;
    /// Top edge a subtitle may be placed at, in percent of the screen height.
    pub const SUBTITLE_POSITION_MIN_PERCENT: u16 = 0;
    /// Bottom edge a subtitle may be placed at, in percent of the screen height.
    pub const SUBTITLE_POSITION_MAX_PERCENT: u16 = 100;
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            autoplay_next: true,
            auto_skip_intro: false,
            subtitle_language: None,
            audio_language: None,
            subtitle_scale_percent: SUBTITLE_SCALE_NORMAL,
            subtitle_position_percent: SUBTITLE_POSITION_BOTTOM,
            volume_percent: 100,
            default_profile: None,
        }
    }
}

impl Library {
    /// Reads every stored preference, filling unreadable entries with defaults.
    pub fn preferences(&self) -> Result<Preferences> {
        let connection = self.connection();
        let mut statement = connection.prepare("SELECT key, value FROM settings")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut preferences = Preferences::default();
        for row in rows {
            let (key, value) = row?;
            match key.as_str() {
                AUTOPLAY_NEXT => preferences.autoplay_next = value == "true",
                AUTO_SKIP_INTRO => preferences.auto_skip_intro = value == "true",
                SUBTITLE_LANGUAGE => preferences.subtitle_language = text(&value),
                AUDIO_LANGUAGE => preferences.audio_language = text(&value),
                SUBTITLE_SCALE => {
                    preferences.subtitle_scale_percent = percent(
                        &value,
                        Preferences::SUBTITLE_SCALE_MIN_PERCENT,
                        Preferences::SUBTITLE_SCALE_MAX_PERCENT,
                        SUBTITLE_SCALE_NORMAL,
                    )
                }
                SUBTITLE_POSITION => {
                    preferences.subtitle_position_percent = percent(
                        &value,
                        Preferences::SUBTITLE_POSITION_MIN_PERCENT,
                        Preferences::SUBTITLE_POSITION_MAX_PERCENT,
                        SUBTITLE_POSITION_BOTTOM,
                    )
                }
                VOLUME_PERCENT => {
                    preferences.volume_percent = percent(&value, 0, 100, 100);
                }
                DEFAULT_PROFILE => preferences.default_profile = text(&value),
                // A key written by a newer build is preserved, not deleted: this
                // build does not know what it means, but it belongs to the user.
                _ => {}
            }
        }
        Ok(preferences)
    }

    /// Writes every preference in one transaction.
    ///
    /// One transaction because the values are presented and accepted as a single
    /// settings screen: a half-applied save would show a state the user never
    /// chose. A language tag is validated before anything is written, so an
    /// invalid tag rejects the whole save instead of storing the other fields.
    pub fn set_preferences(&self, preferences: &Preferences) -> Result<()> {
        let subtitle = language(preferences.subtitle_language.as_deref())?;
        let audio = language(preferences.audio_language.as_deref())?;
        // Rejected rather than clamped on the way in: a caller that asks to store
        // an impossible size has a bug, and silently storing something else would
        // hide it behind a preference that looks saved.
        let scale = within(
            preferences.subtitle_scale_percent,
            Preferences::SUBTITLE_SCALE_MIN_PERCENT,
            Preferences::SUBTITLE_SCALE_MAX_PERCENT,
            "subtitle_scale_out_of_range",
            "Subtitle size must be between 50% and 300%.",
        )?;
        let position = within(
            preferences.subtitle_position_percent,
            Preferences::SUBTITLE_POSITION_MIN_PERCENT,
            Preferences::SUBTITLE_POSITION_MAX_PERCENT,
            "subtitle_position_out_of_range",
            "Subtitle position must be between 0% and 100% of the screen height.",
        )?;
        let volume = within(
            preferences.volume_percent,
            0,
            100,
            "volume_out_of_range",
            "Volume must be between 0% and 100%.",
        )?;
        let default_profile = preferences
            .default_profile
            .as_deref()
            .map(|value| super::validated(value, 120).map(str::to_string))
            .transpose()?;

        if let Some(profile_id) = &default_profile {
            let known: bool = self.connection().query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = ?1)",
                [profile_id],
                |row| row.get(0),
            )?;
            if !known {
                return Err(CoreError::library(
                    "profile_missing",
                    "That profile is no longer in the library.",
                ));
            }
        }

        let mut connection = self.connection();
        let tx = connection.transaction()?;
        write(
            &tx,
            AUTOPLAY_NEXT,
            if preferences.autoplay_next {
                "true"
            } else {
                "false"
            },
        )?;
        write(
            &tx,
            AUTO_SKIP_INTRO,
            if preferences.auto_skip_intro {
                "true"
            } else {
                "false"
            },
        )?;
        write_optional(&tx, SUBTITLE_LANGUAGE, subtitle.as_deref())?;
        write_optional(&tx, AUDIO_LANGUAGE, audio.as_deref())?;
        write(&tx, SUBTITLE_SCALE, &scale.to_string())?;
        write(&tx, SUBTITLE_POSITION, &position.to_string())?;
        write(&tx, VOLUME_PERCENT, &volume.to_string())?;
        write_optional(&tx, DEFAULT_PROFILE, default_profile.as_deref())?;
        tx.commit()?;
        Ok(())
    }

    /// Reads one raw setting, for keys this crate does not model.
    ///
    /// Kept because the schema predates this module and a client may still own a
    /// key, as the ingest path already does with `metadataEnabled`.
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Writes one raw setting.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let key = super::validated(key, 120)?;
        self.connection().execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

/// Reads a stored whole percentage, clamped into `min..=max`.
///
/// Clamped rather than rejected, and falling back when it is not a number at all:
/// this direction is a read, and a library written by a build with a wider range —
/// or corrupted — must still open. The write direction is where a bad value is
/// refused.
fn percent(value: &str, min: u16, max: u16, fallback: u16) -> u16 {
    match value.trim().parse::<u16>() {
        Ok(parsed) => parsed.clamp(min, max),
        Err(_) => fallback,
    }
}

/// Rejects a value outside `min..=max` with a stable code.
fn within(value: u16, min: u16, max: u16, code: &'static str, message: &str) -> Result<u16> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }
    Err(CoreError::library(code, message))
}

/// Treats an empty stored value as "no preference" rather than an empty language.
fn text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Validates an optional language tag.
///
/// A tag reaches libmpv as a track-selection argument, so it is restricted to the
/// characters a BCP-47 tag actually uses. That is what keeps a crafted preference
/// from being an injection point rather than a convenience.
fn language(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > 35
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(CoreError::library(
            "invalid_language",
            "Use a language tag such as \"en\" or \"pt-BR\".",
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// Writes one key/value pair inside a transaction.
fn write(tx: &Transaction<'_>, key: &str, value: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Writes a key, or clears it when the value is absent.
fn write_optional(tx: &Transaction<'_>, key: &str, value: Option<&str>) -> Result<()> {
    match value {
        Some(value) => write(tx, key, value),
        None => {
            tx.execute("DELETE FROM settings WHERE key = ?1", [key])?;
            Ok(())
        }
    }
}
