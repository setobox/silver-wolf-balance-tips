use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub text: String,
    pub image: Option<String>,
    pub author: String,
    pub source: String,
    pub observed_at: u64,
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[default]
    Local,
    Codex,
    Hitokoto,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEntry {
    pub id: String,
    pub text: String,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ApiConfig {
    pub url: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub format: String,
    pub text_pointer: String,
    pub author_pointer: String,
    pub source_pointer: String,
    pub image_pointer: String,
}
impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            format: "json".into(),
            text_pointer: "/text".into(),
            author_pointer: String::new(),
            source_pointer: String::new(),
            image_pointer: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContentConfig {
    pub provider: ProviderKind,
    pub entries: Vec<TextEntry>,
    pub random: bool,
    pub interval_seconds: Option<u32>,
    pub categories: Vec<String>,
    pub api: ApiConfig,
}
impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Local,
            entries: vec![TextEntry {
                id: "welcome".into(),
                text: "你好！点击我换一句话，在设置中选择喜欢的外观和内容。".into(),
                image: None,
            }],
            random: false,
            interval_seconds: None,
            categories: vec!["a".into()],
            api: ApiConfig::default(),
        }
    }
}
impl ContentConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.entries.len() > 500
            || self.entries.iter().any(|e| {
                e.text.len() > 32000
                    || e.id.len() > 100
                    || e.image
                        .as_ref()
                        .is_some_and(|id| !crate::assets::valid_id(id))
            })
        {
            return Err("文本列表或图片标识无效（最多 500 条，每条 8000 字左右）".into());
        }
        if self
            .interval_seconds
            .is_some_and(|s| !(10..=86400).contains(&s))
        {
            return Err("定时换句间隔为 10–86400 秒".into());
        }
        if self.categories.len() > 12
            || self
                .categories
                .iter()
                .any(|s| s.len() != 1 || !("a"..="l").contains(&s.as_str()))
        {
            return Err("一言分类无效".into());
        }
        if !["text", "json"].contains(&self.api.format.as_str()) {
            return Err("API 响应格式无效".into());
        }
        for pointer in [
            &self.api.text_pointer,
            &self.api.author_pointer,
            &self.api.source_pointer,
            &self.api.image_pointer,
        ] {
            if pointer.len() > 512 || !pointer.is_empty() && !pointer.starts_with('/') {
                return Err("JSON Pointer 应以 / 开头，留空表示不读取该字段".into());
            }
        }
        if self.provider == ProviderKind::Http {
            validate_url(&self.api.url)?;
        }
        if self.api.headers.len() > 32
            || self.api.query.len() > 32
            || self
                .api
                .headers
                .iter()
                .chain(self.api.query.iter())
                .any(|(k, v)| {
                    k.len() > 256
                        || v.len() > 8192
                        || k.contains(['\r', '\n'])
                        || v.contains(['\r', '\n'])
                })
        {
            return Err("请求参数或请求头无效".into());
        }
        Ok(())
    }
}

pub fn validate_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "请输入完整的 HTTP(S) 地址")?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("只支持不含内嵌凭据的 HTTP(S) 地址".into());
    }
    Ok(url)
}

pub fn pointer_text(value: &serde_json::Value, pointer: &str) -> String {
    if pointer.is_empty() {
        return String::new();
    }
    value
        .pointer(pointer)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}

pub async fn fetch(config: &ContentConfig, client: &reqwest::Client) -> Result<Document, String> {
    let mut result = Document {
        id: uuid::Uuid::new_v4().to_string(),
        observed_at: now(),
        ..Document::default()
    };
    let request = match config.provider {
        ProviderKind::Hitokoto => client.get("https://v1.hitokoto.cn/").query(
            &config
                .categories
                .iter()
                .map(|c| ("c", c))
                .collect::<Vec<_>>(),
        ),
        ProviderKind::Http => {
            let mut request = client
                .get(validate_url(&config.api.url)?)
                .query(&config.api.query);
            for (key, value) in &config.api.headers {
                request = request.header(key, value);
            }
            request
        }
        _ => return Err("该内容源不使用语录网络接口".into()),
    };
    let mut response = request
        .send()
        .await
        .map_err(|_| "连接语录服务失败，请检查地址和网络")?
        .error_for_status()
        .map_err(|_| "语录服务返回错误状态")?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "读取语录响应失败")? {
        if bytes.len() + chunk.len() > 262144 {
            return Err("语录响应超过 256 KB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if config.provider == ProviderKind::Http && config.api.format == "text" {
        result.text = String::from_utf8(bytes).map_err(|_| "响应不是 UTF-8 文本")?;
    } else {
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "响应不是有效 JSON")?;
        if config.provider == ProviderKind::Hitokoto {
            result.text = pointer_text(&value, "/hitokoto");
            result.author = pointer_text(&value, "/from_who");
            result.source = pointer_text(&value, "/from");
        } else {
            result.text = pointer_text(&value, &config.api.text_pointer);
            result.author = pointer_text(&value, &config.api.author_pointer);
            result.source = pointer_text(&value, &config.api.source_pointer);
            let image = pointer_text(&value, &config.api.image_pointer);
            if !image.is_empty() {
                validate_url(&image)?;
                result.image = Some(image);
            }
        }
    }
    if result.text.trim().is_empty() && result.image.is_none() {
        return Err("响应中没有文字或图片，请检查字段映射".into());
    }
    if result.text.len() > 32000 || result.author.len() > 1024 || result.source.len() > 1024 {
        return Err("语录内容过长".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_mapping_is_data_only_and_supports_arrays() {
        let value = serde_json::json!({"data":[{"a/b":"<script>alert(1)</script>"}]});
        assert_eq!(
            pointer_text(&value, "/data/0/a~1b"),
            "<script>alert(1)</script>"
        );
        assert_eq!(pointer_text(&value, "/missing"), "");
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("https://user:secret@example.org").is_err());
    }
}
