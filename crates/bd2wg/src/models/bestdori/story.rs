//! Bestdori 故事脚本

use serde::Deserialize;

use crate::impl_iter_for_tuple;

use super::*;

/// Bestdori 故事脚本
///
/// 请使用 Self::from_slice 方法经由中间结构体反序列化.
pub struct Story(pub Vec<Action>);

impl_iter_for_tuple! {Story, Action}

impl Story {
    pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        let helper: StoryHelper = serde_json::from_slice(bytes)?;
        Ok(helper.into())
    }

    /// 迭代, 每次提供下一项的 wait
    pub fn iter_with_wait(&self) -> impl Iterator<Item = (&Action, bool)> {
        self.iter().zip(
            self.iter()
                .map(|a| a.is_wait())
                .skip(1)
                .chain(std::iter::once(false)),
        )
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
        if let Some(res) = bgm {
            story.push(Action::Sound(SoundAction {
                wait: false,
                delay: 0.,
                bgm: Some(res),
                se: None,
            }));
        }

        if let Some(res) = background {
            story.push(Action::Effect(EffectAction {
                wait: false,
                delay: 0.,
                effect: Effect::ChangeBackground { image: res },
            }));
        }

        Self(story)
    }
}
