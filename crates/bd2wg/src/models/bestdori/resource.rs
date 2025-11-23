//! Bestdori 资源

use serde::{Deserialize, Serialize};

/// Bestdori 资源类型
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceType {
    #[default]
    Bandori,
    Custom,
    Common,
}

/// Bestdori 资源路径
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ResourcePath {
    Url {
        url: String,
    },
    File {
        #[serde(alias = "se")]
        file: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bundle: Option<String>,
    },
}

/// Bestdori 资源类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Resource {
    #[serde(rename = "type", default)]
    pub kind: ResourceType,
    #[serde(flatten)]
    pub path: ResourcePath,
}

#[test]
fn test_resource_serialize() {
    let data = Resource {
        kind: ResourceType::Bandori,
        path: ResourcePath::File {
            file: "04_Nobiri".to_string(),
            bundle: None,
        },
    };
    let json = serde_json::json!({
        "type": "bandori",
        "file": "04_Nobiri"
    });

    assert_eq!(
        data,
        serde_json::from_value::<Resource>(json.clone()).unwrap()
    );
    assert_eq!(json, serde_json::to_value(&data).unwrap());
}
