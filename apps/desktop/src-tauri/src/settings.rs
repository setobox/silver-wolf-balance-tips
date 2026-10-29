use crate::content::{ContentConfig, ProviderKind};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme_id: String,
    pub character: Option<String>,
    pub sound: Option<String>,
    pub muted: bool,
    pub volume: f64,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme_id: "builtin".into(),
            character: None,
            sound: None,
            muted: false,
            volume: 0.5,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct HttpConfig {
    pub enabled: bool,
    pub port: u16,
}
impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 38427,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub scale: f64,
    pub follow: bool,
    pub codex_path: String,
    #[serde(rename = "dismissSeconds", alias = "quotaDismissSeconds")]
    pub quota_dismiss_seconds: Option<u32>,
    pub appearance: Appearance,
    pub content: ContentConfig,
    pub http: HttpConfig,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            scale: 1.0,
            follow: false,
            codex_path: String::new(),
            quota_dismiss_seconds: Some(8),
            appearance: Appearance::default(),
            content: ContentConfig::default(),
            http: HttpConfig::default(),
        }
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .quota_dismiss_seconds
            .is_some_and(|s| !(1..=3600).contains(&s))
        {
            return Err("提示时长必须为 1–3600 的整数秒数，或常驻".into());
        }
        if !self.scale.is_finite() || !(0.6..=1.6).contains(&self.scale) {
            return Err("缩放必须在 60%–160% 之间".into());
        }
        if self.codex_path.len() > 4096 || self.codex_path.contains('\0') {
            return Err("CLI 路径无效".into());
        }
        self.content.validate()?;
        if self.http.port == 0 {
            return Err("端口必须为 1–65535".into());
        }
        if !self.appearance.volume.is_finite()
            || !(0.0..=1.0).contains(&self.appearance.volume)
            || !crate::assets::valid_id(&self.appearance.theme_id)
            || [&self.appearance.character, &self.appearance.sound]
                .iter()
                .any(|id| id.as_ref().is_some_and(|s| !crate::assets::valid_id(s)))
        {
            return Err("外观设置无效".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub preferences: Preferences,
    pub right: f64,
    pub bottom: f64,
    pub desktop_position: Option<(i32, i32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            preferences: Preferences::default(),
            right: 16.0,
            bottom: 16.0,
            desktop_position: None,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        // Corrupt or huge files never prevent the tray from starting.
        let settings = fs::metadata(path)
            .ok()
            .filter(|m| m.len() < 4 * 1024 * 1024)
            .and_then(|_| fs::read(path).ok())
            .and_then(|bytes| {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
                let legacy = value.get("schemaVersion").is_none();
                if legacy {
                    let backup = path.with_extension("v1.backup.json");
                    if !backup.exists() && fs::write(&backup, &bytes).is_err() {
                        return None;
                    }
                    value
                        .as_object_mut()?
                        .entry("preferences")
                        .or_insert_with(|| serde_json::json!({}));
                }
                // Repair only this preference when loading hand-edited or older settings.
                // Command payloads still use strict Option<u32> deserialization and validation.
                if let Some(prefs) = value.get_mut("preferences").and_then(|v| v.as_object_mut()) {
                    if legacy {
                        prefs.entry("follow").or_insert(true.into());
                        prefs.entry("quotaDismissSeconds").or_insert(1.into());
                        prefs.insert(
                            "content".into(),
                            serde_json::to_value(ContentConfig {
                                provider: ProviderKind::Codex,
                                ..ContentConfig::default()
                            })
                            .ok()?,
                        );
                    }
                    let duration_key = if legacy {
                        "quotaDismissSeconds"
                    } else {
                        "dismissSeconds"
                    };
                    if let Some(seconds) = prefs.get(duration_key) {
                        if !seconds.is_null()
                            && !seconds.as_u64().is_some_and(|s| (1..=3600).contains(&s))
                        {
                            prefs.insert(duration_key.into(), if legacy { 1 } else { 8 }.into());
                        }
                    }
                }
                let settings = serde_json::from_value::<Self>(value).ok()?;
                if settings.schema_version != 2 || settings.preferences.validate().is_err() {
                    return None;
                }
                if legacy {
                    let _ = settings.save(path);
                }
                Some(settings)
            });
        match settings {
            Some(s)
                if s.preferences.scale.is_finite()
                    && (0.6..=1.6).contains(&s.preferences.scale)
                    && s.preferences.codex_path.len() <= 4096
                    && s.right.is_finite()
                    && (0.0..=100000.0).contains(&s.right)
                    && s.bottom.is_finite()
                    && (0.0..=100000.0).contains(&s.bottom) =>
            {
                s
            }
            _ => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().ok_or("配置目录无效")?;
        fs::create_dir_all(parent).map_err(|_| "无法创建配置目录")?;
        let temp = path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| "无法序列化配置")?;
        if bytes.len() >= 4 * 1024 * 1024 {
            return Err("配置超过 4 MB，请减少文本条目或文字长度".into());
        }
        fs::write(&temp, bytes).map_err(|_| "无法保存配置")?;
        fs::rename(&temp, path).map_err(|_| "无法更新配置文件".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_path() -> std::path::PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "silverwolf-settings-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn defaults_new_preferences_to_eight_seconds_and_accepts_persistent() {
        let prefs: Preferences = serde_json::from_str(r#"{"scale":0.8,"follow":false}"#).unwrap();
        assert_eq!(prefs.quota_dismiss_seconds, Some(8));
        assert_eq!(prefs.scale, 0.8);
        for seconds in [None, Some(1), Some(3), Some(3600)] {
            let prefs = Preferences {
                quota_dismiss_seconds: seconds,
                ..Preferences::default()
            };
            assert!(prefs.validate().is_ok());
            let decoded: Preferences =
                serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
            assert_eq!(decoded.quota_dismiss_seconds, seconds);
        }
        for seconds in [Some(0), Some(3601)] {
            assert!(Preferences {
                quota_dismiss_seconds: seconds,
                ..Preferences::default()
            }
            .validate()
            .is_err());
        }
        for value in ["-1", "1.5", "\"3\"", "true"] {
            assert!(serde_json::from_str::<Preferences>(&format!(
                r#"{{"quotaDismissSeconds":{value}}}"#
            ))
            .is_err());
        }
    }

    #[test]
    fn repairs_only_invalid_duration_when_loading_settings() {
        let path = temp_path();
        for value in ["0", "3601", "-1", "1.5", "\"bad\"", "true", "{}", "[]"] {
            fs::write(&path, format!(r#"{{"preferences":{{"scale":0.8,"follow":false,"codexPath":"test-path","quotaDismissSeconds":{value}}},"right":42,"bottom":24}}"#)).unwrap();
            let settings = Settings::load(&path);
            assert_eq!(settings.preferences.quota_dismiss_seconds, Some(1));
            assert_eq!(settings.preferences.scale, 0.8);
            assert!(!settings.preferences.follow);
            assert_eq!(settings.preferences.codex_path, "test-path");
            assert_eq!((settings.right, settings.bottom), (42.0, 24.0));
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn saves_and_reloads_custom_and_persistent_durations() {
        let path = temp_path();
        for seconds in [Some(17), None] {
            let settings = Settings {
                preferences: Preferences {
                    quota_dismiss_seconds: seconds,
                    ..Preferences::default()
                },
                ..Settings::default()
            };
            settings.save(&path).unwrap();
            assert_eq!(
                Settings::load(&path).preferences.quota_dismiss_seconds,
                seconds
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_nan_and_out_of_range_scale() {
        assert!(Preferences {
            scale: f64::NAN,
            ..Preferences::default()
        }
        .validate()
        .is_err());
        assert!(Preferences {
            scale: 5.0,
            ..Preferences::default()
        }
        .validate()
        .is_err());
        assert!(Preferences::default().validate().is_ok());
    }

    #[test]
    fn migrates_old_configuration_once_and_preserves_backup_and_position() {
        let path = temp_path();
        let original = r#"{"preferences":{"scale":0.8,"follow":true,"codexPath":"missing.exe","quotaDismissSeconds":null},"right":40,"bottom":24,"desktopPosition":[-100,25]}"#;
        fs::write(&path, original).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.schema_version, 2);
        assert_eq!(settings.preferences.content.provider, ProviderKind::Codex);
        assert_eq!(settings.preferences.quota_dismiss_seconds, None);
        assert_eq!(settings.desktop_position, Some((-100, 25)));
        assert!(settings.preferences.follow);
        assert_eq!(
            fs::read_to_string(path.with_extension("v1.backup.json")).unwrap(),
            original
        );
        assert_eq!(Settings::load(&path).preferences.codex_path, "missing.exe");
        assert_eq!(
            fs::read_to_string(path.with_extension("v1.backup.json")).unwrap(),
            original
        );
        fs::remove_file(&path).unwrap();
        fs::remove_file(path.with_extension("v1.backup.json")).unwrap();
    }

    #[test]
    fn fresh_install_is_local_and_does_not_require_codex() {
        let settings = Settings::load(&temp_path());
        assert_eq!(settings.preferences.content.provider, ProviderKind::Local);
        assert!(!settings.preferences.follow);
        assert!(!settings.preferences.http.enabled);
        assert_eq!(settings.preferences.quota_dismiss_seconds, Some(8));
        assert!(Preferences {
            codex_path: "missing.exe".into(),
            ..Preferences::default()
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn partial_legacy_configuration_keeps_legacy_defaults() {
        let path = temp_path();
        fs::write(&path, r#"{"right":42}"#).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.preferences.content.provider, ProviderKind::Codex);
        assert!(settings.preferences.follow);
        assert_eq!(settings.preferences.quota_dismiss_seconds, Some(1));
        assert_eq!(settings.right, 42.0);
        fs::remove_file(&path).unwrap();
        fs::remove_file(path.with_extension("v1.backup.json")).unwrap();
    }
}
