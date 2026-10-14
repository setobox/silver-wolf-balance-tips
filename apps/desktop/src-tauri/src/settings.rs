use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub scale: f64,
    pub follow: bool,
    pub codex_path: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            scale: 1.0,
            follow: true,
            codex_path: String::new(),
        }
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
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
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok());
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
