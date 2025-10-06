//! bd2wg 内部数据结构

use super::bestdoli::{EffectDetail, LayoutSide, LayoutSideType, LayoutType, Motion};

/// 内部脚本
#[derive(Debug, Clone)]
pub struct Action {
    pub wait: bool,
    pub delay: f32,
    pub detail: ActionDetail,
}

#[derive(Debug, Clone)]
pub enum TransitionType {
    BlackIn,
    BlackOut,
    WhiteIn,
    WhiteOut,
}

impl TransitionType {
    pub fn unwrap_from(effect: EffectDetail) -> Self {
        match effect {
            EffectDetail::BlackIn => Self::BlackIn,
            EffectDetail::BlackOut => Self::BlackOut,
            EffectDetail::WhiteIn => Self::WhiteIn,
            EffectDetail::WhiteOut => Self::WhiteOut,
            _ => panic!("Impossible convert from EffectDetail `{effect:?}` to TransitionType."),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionDetail {
    Say {
        name: String,
        text: String,
        characters: Vec<u8>,
        motions: Vec<Motion>,
    },

    Bgm(String),

    Sound(String),

    Background(String),

    CardStill(String),

    Transition(TransitionType),

    Telop(String),

    Layout {
        model: String,
        motion: Motion,
        side: LayoutSide,
        kind: LayoutType,
    },

    Motion {
        model: String,
        motion: Motion,
    },

    Unknown,
}
