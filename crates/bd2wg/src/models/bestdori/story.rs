//! Bestdori 故事脚本

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::*;

use super::*;

/// Bestdori 故事脚本
///
/// 请使用 Self::from_str 方法经由中间结构体反序列化.
pub struct Story(Vec<Action>);

impl FromStr for Story {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let helper: StoryHelper = serde_json::from_str(s)?;
        Ok(helper.into())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct StoryHelper {
    bgm: Option<Resource>,
    background: Option<Resource>,
    actions: Vec<Action>,
}

impl From<StoryHelper> for Story {
    fn from(val: StoryHelper) -> Self {
        let StoryHelper {
            bgm,
            background,
            actions,
        } = val;

        let mut story = Vec::with_capacity(actions.len() + 2);

        // 推入初始 bgm, background
        if let Some(bgm) = bgm {
            story.push(Action::Sound(SoundAction {
                wait: false,
                delay: 0.,
                bgm: Some(bgm),
                se: None,
            }));
        }

        if let Some(background) = background {
            story.push(Action::Effect(EffectAction {
                wait: false,
                delay: 0.,
                effect: Effect::ChangeBackground { image: background },
            }));
        }

        Self(story)
    }
}
