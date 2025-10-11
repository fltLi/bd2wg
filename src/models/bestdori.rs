//! bestdori 脚本数据模型

use std::collections::LinkedList;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;

use crate::error::*;

/// bestdori 脚本
pub struct Story(pub LinkedList<Action>);

#[derive(Deserialize)]
struct ScriptHelper {
    bgm: Option<Address>,
    background: Option<Address>,
    #[serde(rename = "actions")]
    script: LinkedList<Action>,
}

impl From<ScriptHelper> for Story {
    fn from(val: ScriptHelper) -> Self {
        let ScriptHelper {
            bgm,
            background,
            mut script,
        } = val;
        if let Some(bgm) = bgm {
            script.push_front(Action::Sound(SoundAction {
                wait: false,
                delay: 0.,
                bgm: Some(bgm),
                se: None,
            }));
        }
        if let Some(background) = background {
            script.push_front(Action::Effect(EffectAction {
                wait: false,
                delay: 0.,
                effect: EffectDetail::ChangeBackground { image: background },
            }));
        }
        Story(script)
    }
}

impl Story {
    pub fn from_byte(byte: &[u8]) -> Result<Self> {
        let script: ScriptHelper = serde_json::from_slice(byte)?;
        Ok(script.into())
    }

    pub fn from_file(fp: &Path) -> Result<Self> {
        let reader = BufReader::new(File::open(fp)?);
        let script: ScriptHelper = serde_json::from_reader(reader)?;
        Ok(script.into())
    }
}

impl FromStr for Story {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {  // why?
        let script: ScriptHelper = serde_json::from_str(s)?;
        Ok(script.into())
    }
}

impl From<Story> for LinkedList<Action> {
    fn from(val: Story) -> Self {
        val.0
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Talk(TalkAction),
    Sound(SoundAction),
    Effect(EffectAction),
    Layout(LayoutAction),
    Motion(MotionAction),
    #[serde(other)]
    Unknown,
}

/// Live2D 动作
#[derive(Debug, Clone, Deserialize)]
pub struct Motion {
    pub delay: f32,
    pub character: u8,  // *Bushiroad 的生产力没有超过 u8
    pub motion: String,
    pub expression: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddressType {
    #[default]
    Bandori,
    Custom,
    Common,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(untagged)]
pub enum AddressPath {
    Url {
        url: String,
    },
    File {
        #[serde(alias = "se")]
        file: String,
        bundle: Option<String>,
    },
}

/// 资源路径
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct Address {
    #[serde(rename = "type", default)]
    pub kind: AddressType,
    #[serde(flatten)]
    pub address: AddressPath,
}

impl Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.address {
            AddressPath::Url { url } => write!(f, "{:?}: {}", self.kind, url),
            AddressPath::File { file, bundle } => match bundle {
                Some(b) => write!(f, "{:?}: {} -> {}", self.kind, b, file),
                None => write!(f, "{:?}: {}", self.kind, file),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TalkAction {
    pub wait: bool,
    pub delay: f32,
    pub name: String,
    #[serde(rename = "body")]
    pub text: String,
    pub motions: Vec<Motion>,
    pub characters: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundAction {
    pub wait: bool,
    pub delay: f32,
    pub bgm: Option<Address>,
    pub se: Option<Address>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "effectType", rename_all = "camelCase")]
pub enum EffectDetail {
    ChangeBackground {
        #[serde(rename = "background")]
        image: Address,
    },
    ChangeCardStill {
        #[serde(flatten)]
        image: Address,
    },
    Telop {
        text: String,
    },
    BlackIn,
    BlackOut,
    WhiteIn,
    WhiteOut,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EffectAction {
    pub wait: bool,
    pub delay: f32,
    #[serde(flatten)]
    pub effect: EffectDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutType {
    Appear,
    Hide,
    Move,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutSideType {
    LeftInside,
    LeftOver,
    Center,
    RightInside,
    RightOver,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutSide {
    #[serde(rename = "sideFrom")]
    pub from: LayoutSideType,
    #[serde(rename = "sideTo")]
    pub to: LayoutSideType,
    #[serde(rename = "sideFromOffsetX")]
    pub from_x: i16,
    #[serde(rename = "sideToOffsetX")]
    pub to_x: i16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutAction {
    pub wait: bool,
    #[serde(rename = "layoutType")]
    pub kind: LayoutType,
    #[serde(rename = "costume")]
    pub model: String,
    #[serde(flatten)]
    pub motion: Motion,
    #[serde(flatten)]
    pub side: LayoutSide,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotionAction {
    pub wait: bool,
    #[serde(rename = "costume")]
    pub model: String,
    #[serde(flatten)]
    pub motion: Motion,
}

// // 这个测试很弱, 以后需要改成一个精简且覆盖完全的, 从字符串反序列化并比较的测试.
// #[test]
// fn test_bestdori_deserialize() -> Result<()> {
//     let result = Story::from_file(Path::new("assets/test.json"))?;
//     Ok(())
// }
