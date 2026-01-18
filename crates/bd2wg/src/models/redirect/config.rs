//! 重定向规则结构化绑定

use serde::{Deserialize, Serialize};

/// 重定向配置文件
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename = "redirect")]
pub struct Config {
    pub model: Vec<Model>,
}

/// 模型重定向
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Model {
    #[serde(rename = "@match")]
    pub pattern: String,
    pub costumes: Patterns,
    pub motions: Patterns,
    pub expressions: Patterns,
}

/// 匹配规则集
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Patterns {
    #[serde(rename = "pattern")]
    pub patterns: Vec<String>,
}

#[cfg(test)]
fn get_test_model_redirect() -> Model {
    Model {
        pattern: "^037_(.*)".to_string(),
        costumes: Patterns {
            patterns: vec!["anon/${figure:^([^-]*)(?:-.*)?$}/model.json".into()],
        },
        motions: Patterns {
            patterns: vec!["anon/${motion}".into(), "anon_${motion}".into()],
        },
        expressions: Patterns {
            patterns: vec!["anon/${expression}".into(), "anon_${expression}".into()],
        },
    }
}

#[test]
#[cfg(test)]
fn test_redirect_deserialize() {
    let xml = r#"
        <redirect>
            <model match="^037_(.*)">
                <costumes>
                    <pattern>anon/${figure:^([^-]*)(?:-.*)?$}/model.json</pattern>
                </costumes>

                <motions>
                    <pattern>anon/${motion}</pattern>
                    <pattern>anon_${motion}</pattern>
                </motions>

                <expressions>
                    <pattern>anon/${expression}</pattern>
                    <pattern>anon_${expression}</pattern>
                </expressions>
            </model>

            <!-- ... -->
        </redirect>"#;
    let redirect = Config {
        model: vec![get_test_model_redirect()],
    };

    let result: Config = serde_xml_rs::from_str(xml).unwrap();
    assert_eq!(result, redirect);
}
