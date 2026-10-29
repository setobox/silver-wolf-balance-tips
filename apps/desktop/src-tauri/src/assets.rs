use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};

pub const MAX_IMAGE: usize = 8 * 1024 * 1024;
const MAX_THEME: usize = 32 * 1024 * 1024;
const MAX_RUNTIME: u64 = 256 * 1024 * 1024;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.')
        && !id.contains("..")
}
pub fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && path.split('/').all(|part| {
            let stem = part
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            part.trim_end_matches([' ', '.']) == part
                && !part.contains(['?', '#', '%', '<', '>', '|', '*', '"'])
                && ![
                    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
                    "LPT8", "LPT9",
                ]
                .contains(&stem.as_str())
        })
}
pub fn mime(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}
pub fn image_extension(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.len() > MAX_IMAGE {
        return Err("图片不能超过 8 MB".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "图片格式无法识别")?;
    let extension = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::WebP => "webp",
        image::ImageFormat::Gif => "gif",
        _ => return Err("支持 PNG、JPEG、WebP、GIF 图片".into()),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| "图片已损坏")?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_777_216 {
        return Err("图片尺寸超过 1600 万像素".into());
    }
    Ok(extension)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Actions {
    pub x: f64,
    pub left_x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub width: f64,
    pub height: f64,
    pub character: Rect,
    pub bubble: Rect,
    pub actions: Actions,
}
impl Default for Layout {
    fn default() -> Self {
        serde_json::from_str(include_str!(
            "../../../../packages/core/src/default-layout.json"
        ))
        .expect("bundled layout")
    }
}
impl Layout {
    fn validate(&self) -> Result<(), String> {
        if !(376.0..=800.0).contains(&self.width) || !(430.0..=1000.0).contains(&self.height) {
            return Err("主题画布应在 376×430 至 800×1000 之间".into());
        }
        for rect in [&self.character, &self.bubble] {
            if ![rect.x, rect.y, rect.width, rect.height]
                .iter()
                .all(|v| v.is_finite())
                || rect.x < 0.0
                || rect.y < 0.0
                || rect.width < 64.0
                || rect.height < 64.0
                || rect.x + rect.width > self.width
                || rect.y + rect.height > self.height
            {
                return Err("主题布局超出画布".into());
            }
        }
        for x in [self.actions.x, self.actions.left_x] {
            if !x.is_finite() || x < 0.0 || x + 32.0 > self.width {
                return Err("工具栏位置无效".into());
            }
        }
        if !self.actions.y.is_finite()
            || self.actions.y < 0.0
            || self.actions.y + 100.0 > self.height
        {
            return Err("工具栏位置无效".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeManifest {
    pub schema_version: u32,
    pub name: String,
    pub character: Option<String>,
    pub dialog: Option<String>,
    pub sound: Option<String>,
    #[serde(default)]
    pub layout: Layout,
}
#[derive(Clone, Serialize)]
pub struct RuntimeTheme {
    pub id: String,
    pub name: String,
    pub character: String,
    pub dialog: Option<String>,
    pub sound: Option<String>,
    pub layout: Layout,
}
#[derive(Deserialize)]
pub struct ImportFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub struct Assets {
    pub root: PathBuf,
    pub theme_base: String,
    writes: std::sync::Mutex<()>,
    downloads: tokio::sync::Semaphore,
}
impl Assets {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        for dir in ["assets", "themes", "runtime"] {
            fs::create_dir_all(root.join(dir)).map_err(|_| "无法创建素材目录")?;
        }
        // This directory contains only generated, disposable media from previous runs.
        for entry in fs::read_dir(root.join("runtime"))
            .map_err(|_| "无法读取临时素材")?
            .flatten()
        {
            if entry.file_type().is_ok_and(|t| t.is_file()) {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(Self {
            root,
            theme_base: String::new(),
            writes: std::sync::Mutex::new(()),
            downloads: tokio::sync::Semaphore::new(4),
        })
    }
    pub fn url(id: &str) -> String {
        format!("http://media.localhost/assets/{id}")
    }
    pub fn asset_path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("素材标识无效".into());
        }
        let folder = if id.starts_with("tmp-") {
            "runtime"
        } else {
            "assets"
        };
        let path = self.root.join(folder).join(id);
        if !path.is_file() {
            return Err("素材不存在".into());
        }
        Ok(path)
    }
    pub fn store(&self, bytes: &[u8], extension: &str, temporary: bool) -> Result<String, String> {
        let _write = self.writes.lock().map_err(|_| "素材存储不可用")?;
        if bytes.len() > MAX_IMAGE {
            return Err("素材不能超过 8 MB".into());
        }
        let folder = self.root.join(if temporary { "runtime" } else { "assets" });
        let total: u64 = fs::read_dir(&folder)
            .map_err(|_| "素材目录不可用")?
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        if total + bytes.len() as u64 > MAX_RUNTIME {
            return Err("素材空间已满（256 MB），请移除不用的素材或重启清理临时图片".into());
        }
        let id = format!(
            "{}{}.{}",
            if temporary { "tmp-" } else { "" },
            uuid::Uuid::new_v4(),
            extension
        );
        fs::write(folder.join(&id), bytes).map_err(|_| "无法保存素材")?;
        Ok(id)
    }
    pub fn import(&self, name: &str, bytes: &[u8], kind: &str) -> Result<String, String> {
        let ext = if kind == "image" {
            image_extension(bytes)?.to_string()
        } else if kind == "sound" {
            let ext = name
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !["mp3", "wav", "ogg"].contains(&ext.as_str()) {
                return Err("音效支持 MP3、WAV、OGG".into());
            }
            ext
        } else {
            return Err("素材类型无效".into());
        };
        self.store(bytes, &ext, false)
    }
    pub fn import_zip(&self, bytes: Vec<u8>) -> Result<String, String> {
        if bytes.len() > MAX_THEME {
            return Err("主题压缩包超过 32 MB".into());
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "无效 ZIP 文件")?;
        if archive.len() > 128 {
            return Err("主题最多包含 128 个文件".into());
        }
        let mut files = Vec::new();
        let mut total = 0;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|_| "无法读取 ZIP 条目")?;
            if file.is_symlink() || !safe_relative(file.name().trim_end_matches('/')) {
                return Err("主题包含越界路径或符号链接".into());
            }
            if file.is_dir() {
                continue;
            }
            let mut bytes = Vec::new();
            (&mut file)
                .take((MAX_THEME + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| "主题解压失败")?;
            total += bytes.len();
            if total > MAX_THEME {
                return Err("主题解压后超过 32 MB".into());
            }
            files.push(ImportFile {
                path: file.name().to_owned(),
                bytes,
            });
        }
        self.import_theme(files)
    }
    pub fn import_theme(&self, mut files: Vec<ImportFile>) -> Result<String, String> {
        if files.is_empty()
            || files.len() > 128
            || files.iter().map(|f| f.bytes.len()).sum::<usize>() > MAX_THEME
        {
            return Err("主题最多 128 个文件、合计 32 MB".into());
        }
        if !files.iter().any(|f| f.path == "theme.json") {
            let manifest = files
                .iter()
                .find(|f| f.path.ends_with("/theme.json"))
                .ok_or("缺少 theme.json")?;
            let prefix = manifest.path.strip_suffix("theme.json").unwrap().to_owned();
            for file in &mut files {
                file.path = file
                    .path
                    .strip_prefix(&prefix)
                    .ok_or("主题必须位于同一个目录")?
                    .to_owned();
            }
        }
        let mut names = std::collections::HashSet::new();
        for file in &files {
            if !safe_relative(&file.path) || !names.insert(file.path.to_lowercase()) {
                return Err("主题包含无效或重复路径".into());
            }
        }
        let manifest: ThemeManifest = serde_json::from_slice(
            &files
                .iter()
                .find(|f| f.path == "theme.json")
                .ok_or("缺少 theme.json")?
                .bytes,
        )
        .map_err(|_| "theme.json 格式无效")?;
        manifest.layout.validate()?;
        if manifest.schema_version != 1 || manifest.name.is_empty() || manifest.name.len() > 100 {
            return Err("不支持的主题版本或名称".into());
        }
        for path in [&manifest.character, &manifest.dialog, &manifest.sound]
            .into_iter()
            .flatten()
        {
            if !safe_relative(path) || !files.iter().any(|f| &f.path == path) {
                return Err("主题引用了不存在的资源".into());
            }
        }
        if let Some(path) = &manifest.character {
            image_extension(&files.iter().find(|f| &f.path == path).unwrap().bytes)?;
        }
        if manifest
            .dialog
            .as_ref()
            .is_some_and(|s| !s.ends_with(".html"))
        {
            return Err("对话框入口必须是 HTML 文件".into());
        }
        if manifest
            .sound
            .as_ref()
            .is_some_and(|path| !["audio/mpeg", "audio/wav", "audio/ogg"].contains(&mime(path)))
        {
            return Err("主题音效支持 MP3、WAV、OGG".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let root = self.root.join("themes").join(&id);
        for file in files {
            let path = root.join(&file.path);
            fs::create_dir_all(path.parent().unwrap()).map_err(|_| "无法创建主题目录")?;
            fs::write(path, file.bytes).map_err(|_| "无法保存主题文件")?;
        }
        Ok(id)
    }
    pub fn theme(&self, appearance: &crate::settings::Appearance) -> RuntimeTheme {
        let mut theme = RuntimeTheme {
            id: "builtin".into(),
            name: "银狼 · 云朵".into(),
            character: "/silverwolf.png".into(),
            dialog: None,
            sound: None,
            layout: Layout::default(),
        };
        if appearance.theme_id != "builtin" && valid_id(&appearance.theme_id) {
            let root = self.root.join("themes").join(&appearance.theme_id);
            if let Ok(manifest) = fs::read(root.join("theme.json")).and_then(|bytes| {
                serde_json::from_slice::<ThemeManifest>(&bytes).map_err(std::io::Error::other)
            }) {
                if manifest.schema_version == 1
                    && manifest.layout.validate().is_ok()
                    && [&manifest.character, &manifest.dialog, &manifest.sound]
                        .into_iter()
                        .flatten()
                        .all(|p| safe_relative(p))
                {
                    let url = |path: String| {
                        format!("{}/themes/{}/{path}", self.theme_base, appearance.theme_id)
                    };
                    theme.id = appearance.theme_id.clone();
                    theme.name = manifest.name;
                    theme.layout = manifest.layout;
                    if let Some(path) = manifest.character {
                        theme.character = url(path);
                    }
                    theme.dialog = manifest.dialog.map(url);
                    theme.sound = manifest.sound.map(url);
                }
            }
        }
        if let Some(id) = &appearance.character {
            if self.asset_path(id).is_ok() {
                theme.character = Self::url(id);
            }
        }
        if let Some(id) = &appearance.sound {
            if self.asset_path(id).is_ok() {
                theme.sound = Some(Self::url(id));
            }
        }
        theme
    }
    pub fn read_resource(&self, path: &str) -> Result<Vec<u8>, String> {
        if let Some(id) = path.strip_prefix("/assets/") {
            return fs::read(self.asset_path(id)?).map_err(|_| "读取素材失败".into());
        }
        let relative = path.strip_prefix("/themes/").ok_or("未知资源")?;
        if !safe_relative(relative) {
            return Err("无效资源路径".into());
        }
        let root = self
            .root
            .join("themes")
            .canonicalize()
            .map_err(|_| "主题不存在")?;
        let target = root
            .join(relative)
            .canonicalize()
            .map_err(|_| "资源不存在")?;
        if !target.starts_with(&root) {
            return Err("资源超出主题目录".into());
        }
        fs::read(target).map_err(|_| "读取主题失败".into())
    }
    pub async fn download(&self, client: &reqwest::Client, url: &str) -> Result<String, String> {
        let _permit = self
            .downloads
            .acquire()
            .await
            .map_err(|_| "图片下载服务不可用")?;
        let mut response = client
            .get(crate::content::validate_url(url)?)
            .send()
            .await
            .map_err(|_| "图片下载失败")?
            .error_for_status()
            .map_err(|_| "图片服务返回错误")?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "读取图片失败")? {
            if bytes.len() + chunk.len() > MAX_IMAGE {
                return Err("图片超过 8 MB".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        self.store(&bytes, image_extension(&bytes)?, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_layout_cannot_escape_their_boundaries() {
        for path in [
            "../secret",
            "C:/secret",
            "foo/../../secret",
            "/absolute",
            "a\\b",
            "a:stream",
            "CON.txt",
            "foo/NUL",
            "trailing.",
            "space ",
            "encoded%2fpath",
        ] {
            assert!(!safe_relative(path));
        }
        assert!(safe_relative("images/角色.png"));
        assert!(Layout::default().validate().is_ok());
        let mut layout = Layout::default();
        layout.bubble.width = 900.0;
        assert!(layout.validate().is_err());
        assert!(image_extension(b"<svg onload='evil()'>").is_err());
    }

    #[test]
    fn imports_copy_resources_and_zip_paths_cannot_escape() {
        use std::io::Write;
        let root = std::env::temp_dir().join(format!("silverwolf-assets-{}", uuid::Uuid::new_v4()));
        let assets = Assets::new(root.clone()).unwrap();
        let manifest = br#"{"schemaVersion":1,"name":"test","dialog":"dialog.html"}"#;
        let id = assets
            .import_theme(vec![
                ImportFile {
                    path: "theme/theme.json".into(),
                    bytes: manifest.to_vec(),
                },
                ImportFile {
                    path: "theme/dialog.html".into(),
                    bytes: b"<h1>theme</h1>".to_vec(),
                },
            ])
            .unwrap();
        let theme = assets.theme(&crate::settings::Appearance {
            theme_id: id.clone(),
            ..Default::default()
        });
        assert_eq!(theme.id, id);
        assert_eq!(
            assets
                .read_resource(&format!("/themes/{id}/dialog.html"))
                .unwrap(),
            b"<h1>theme</h1>"
        );
        assert!(assets
            .read_resource(&format!("/themes/{id}/../../http-token"))
            .is_err());
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("../escaped.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"escape").unwrap();
        assert!(assets
            .import_zip(zip.finish().unwrap().into_inner())
            .is_err());
        assert!(!root.join("escaped.txt").exists());
        assert!(assets
            .import_theme(vec![ImportFile {
                path: "theme.json".into(),
                bytes: manifest.to_vec()
            }])
            .is_err());
        assert!(assets.import("script.svg", b"<svg/>", "image").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
