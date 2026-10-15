use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub scale: f64,
    pub follow: bool,
    pub codex_path: String,
    pub quota_dismiss_seconds: Option<u32>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            scale: 1.0,
            follow: true,
            codex_path: String::new(),
            quota_dismiss_seconds: Some(1),
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
        if !self.codex_path.is_empty() {
            codex_usage::find_cli(&self.codex_path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub preferences: Preferences,
    pub right: f64,
    pub bottom: f64,
    pub desktop_position: Option<(i32, i32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
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
            .filter(|m| m.len() < 65536)
            .and_then(|_| fs::read(path).ok())
            .and_then(|bytes| {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
                // Repair only this preference when loading hand-edited or older settings.
                // Command payloads still use strict Option<u32> deserialization and validation.
                if let Some(prefs) = value.get_mut("preferences").and_then(|v| v.as_object_mut()) {
                    if let Some(seconds) = prefs.get("quotaDismissSeconds") {
                        if !seconds.is_null()
                            && !seconds.as_u64().is_some_and(|s| (1..=3600).contains(&s))
                        {
                            prefs.insert("quotaDismissSeconds".into(), 1.into());
                        }
                    }
                }
                serde_json::from_value::<Self>(value).ok()
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
    fn defaults_old_preferences_to_one_second_and_accepts_persistent() {
        let prefs: Preferences = serde_json::from_str(r#"{"scale":0.8,"follow":false}"#).unwrap();
        assert_eq!(prefs.quota_dismiss_seconds, Some(1));
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
}
