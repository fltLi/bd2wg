//! Bestdori 脚本指令

use serde::{Deserialize, Serialize};

use super::*;

/// Bestdori 脚本指令
#[derive(Debug, Clone, Deserialize, Serialize)]
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

impl Action {
    pub fn is_wait(&self) -> bool {
        match self {
            Self::Talk(a) => a.wait,
            Self::Sound(a) => a.wait,
            Self::Effect(a) => a.wait,
            Self::Layout(a) => a.wait,
            Self::Motion(a) => a.wait,
            Self::Unknown => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TalkAction {
    pub wait: bool,
    pub delay: f32,
    pub name: String,
    #[serde(rename = "body")]
    pub text: String,
    pub motions: Vec<Motion>,
    pub characters: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SoundAction {
    pub wait: bool,
    pub delay: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bgm: Option<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub se: Option<Resource>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "effectType", rename_all = "camelCase")]
pub enum Effect {
    ChangeBackground {
        #[serde(rename = "background")]
        image: Resource,
    },
    ChangeCardStill {
        #[serde(flatten)]
        image: Resource,
    },
    Telop {
        text: String,
    },
    BlackIn,
    BlackOut,
    WhiteIn,
    WhiteOut,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EffectAction {
    pub wait: bool,
    pub delay: f32,
    #[serde(flatten)]
    pub effect: Effect,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutType {
    Appear,
    Hide,
    Move,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutSideType {
    LeftInside,
    LeftOver,
    Center,
    RightInside,
    RightOver,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MotionAction {
    pub wait: bool,
    #[serde(rename = "costume")]
    pub model: String,
    #[serde(flatten)]
    pub motion: Motion,
}
