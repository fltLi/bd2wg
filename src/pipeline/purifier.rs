//! bestdori 脚本预处理

use std::collections::VecDeque;
use std::rc::Rc;

use super::definition::*;
use super::resolver::{ResolveCommonResult, ResolveModelResult, Resolver};
use crate::error::*;
use crate::models::{
    bestdori::{self, *},
    internal::{self, *},
};

/// 预处理结果
pub enum PurifyResult {
    Action(internal::Action),
    ResourceTask(ResourceTask),
}

// impl From<internal::Action> for PurifyResult {
//     fn from(value: internal::Action) -> Self {
//         PurifyResult::Action(value)
//     }
// }

// impl From<Rc<Resource>> for PurifyResult {
//     fn from(value: Rc<Resource>) -> Self {
//         PurifyResult::Resource(value)
//     }
// }

/// bestdori 脚本预处理器
///
/// - 将 bestdori 脚本中的资源转换为内部表示
/// - 收集并转换资源, 创建下载任务, 收集 Resolver 需要的数据
pub trait Purifier: Iterator<Item = Result<PurifyResult>> {}

/// 默认 bestdori 脚本预处理器
pub struct DefaultPurifier<'a, I, R>
where
    I: Iterator<Item = bestdori::Action>,
    R: Resolver,
{
    in_iter: I,
    resolver: &'a mut R,
    pending: VecDeque<Result<PurifyResult>>,
}

impl<'a, I, R> DefaultPurifier<'a, I, R>
where
    I: Iterator<Item = bestdori::Action>,
    R: Resolver,
{
    /// 创建一个新的预处理器
    pub fn new(in_iter: I, resolver: &'a mut R) -> Self {
        Self {
            in_iter,
            resolver,
            pending: VecDeque::new(),
        }
    }

    /// 处理一条指令
    fn purify(&mut self, action: bestdori::Action) -> Vec<Result<PurifyResult>> {
        let mut items: Vec<Result<PurifyResult>> = Vec::new();

        match action {
            bestdori::Action::Talk(talk) => items.extend(self.purify_talk(talk)),
            bestdori::Action::Sound(sound) => items.extend(self.purify_sound(sound)),
            bestdori::Action::Motion(motion) => items.extend(self.purify_motion(motion)),
            bestdori::Action::Layout(layout) => items.extend(self.purify_layout(layout)),
            bestdori::Action::Effect(effect) => items.extend(self.purify_effect(effect)),
            bestdori::Action::Unknown => items.extend(self.purify_unknown()),
        }

        items
    }

    // Helper to push resources vector into items as Ok(Resource)
    fn push_resources_to_items(
        &self,
        items: &mut Vec<Result<PurifyResult>>,
        resources: Vec<Rc<Resource>>,
    ) {
        items.extend(
            resources
                .into_iter()
                .map(|r| Ok(PurifyResult::ResourceTask(ResourceTask::Task(r)))),
        );
    }

    fn purify_talk(
        &mut self,
        TalkAction {
            wait,
            delay,
            name,
            text,
            motions,
            characters,
        }: TalkAction,
    ) -> Vec<Result<PurifyResult>> {
        let mut items: Vec<Result<PurifyResult>> = Vec::new();

        for m in &motions {
            match self.resolver.resolve_motion(m.character, &m.motion) {
                Ok(ResolveModelResult::Normal(res)) => {
                    items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(res))));
                }
                Ok(ResolveModelResult::Bind { url, task }) => {
                    items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Bind {
                        url,
                        task,
                    })));
                }
                Ok(ResolveModelResult::Existing) => {}
                Err(e) => {
                    items.push(Err(e));
                    return items;
                }
            }

            if !m.expression.is_empty() {
                match self.resolver.resolve_expression(m.character, &m.expression) {
                    Ok(ResolveModelResult::Normal(res)) => {
                        items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(res))));
                    }
                    Ok(ResolveModelResult::Bind { url, task }) => {
                        items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Bind {
                            url,
                            task,
                        })));
                    }
                    Ok(ResolveModelResult::Existing) => {}
                    Err(e) => {
                        items.push(Err(e));
                        return items;
                    }
                }
            }
        }

        items.push(Ok(PurifyResult::Action(internal::Action {
            wait,
            delay,
            detail: ActionDetail::Say {
                name,
                text,
                characters,
                motions,
            },
        })));

        items
    }

    fn purify_sound(
        &mut self,
        SoundAction {
            wait,
            delay,
            bgm,
            se,
        }: SoundAction,
    ) -> Vec<Result<PurifyResult>> {
        let mut items: Vec<Result<PurifyResult>> = Vec::new();

        if let Some(addr) = bgm {
            match self.resolver.resolve_bgm(&addr) {
                Ok(result) => {
                    items.push(Ok(PurifyResult::Action(internal::Action {
                        wait,
                        delay,
                        detail: ActionDetail::Bgm(result.as_ref().path.clone()),
                    })));
                    if let ResolveCommonResult::New(resource) = result {
                        items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(resource))));
                    }
                }
                Err(e) => {
                    items.push(Err(e));
                    return items;
                }
            }
        }
        if let Some(addr) = se {
            match self.resolver.resolve_se(&addr) {
                Ok(result) => {
                    items.push(Ok(PurifyResult::Action(internal::Action {
                        wait,
                        delay,
                        detail: ActionDetail::Sound(result.as_ref().path.clone()),
                    })));
                    if let ResolveCommonResult::New(resource) = result {
                        items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(resource))));
                    }
                }
                Err(e) => {
                    items.push(Err(e));
                    return items;
                }
            }
        }

        items
    }

    fn purify_motion(
        &mut self,
        bestdori::MotionAction {
            wait,
            model,
            motion,
        }: bestdori::MotionAction,
    ) -> Vec<Result<PurifyResult>> {
        let mut items: Vec<Result<PurifyResult>> = Vec::new();
        let mut model = model;

        match self.resolver.resolve_model(motion.character, &mut model) {
            Ok(ResolveModelResult::Normal(res)) => {
                // resolver is expected to have updated `model` if needed
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(res))));
            }
            Ok(ResolveModelResult::Bind { url, task }) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Bind {
                    url,
                    task,
                })));
            }
            Ok(ResolveModelResult::Existing) => {}
            Err(e) => {
                items.push(Err(e));
                return items;
            }
        }

        match self
            .resolver
            .resolve_motion(motion.character, &motion.motion)
        {
            Ok(ResolveModelResult::Normal(res)) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(res))));
            }
            Ok(ResolveModelResult::Bind { url, task }) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Bind {
                    url,
                    task,
                })));
            }
            Ok(ResolveModelResult::Existing) => {}
            Err(e) => {
                items.push(Err(e));
                return items;
            }
        }

        items.push(Ok(PurifyResult::Action(internal::Action {
            wait,
            delay: motion.delay,
            detail: ActionDetail::Motion { model, motion },
        })));

        items
    }

    fn purify_layout(
        &mut self,
        LayoutAction {
            wait,
            kind,
            model,
            motion,
            side,
        }: LayoutAction,
    ) -> Vec<Result<PurifyResult>> {
        let mut items: Vec<Result<PurifyResult>> = Vec::new();
        let mut model = model;

        match self.resolver.resolve_model(motion.character, &mut model) {
            Ok(ResolveModelResult::Normal(res)) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(res))));
            }
            Ok(ResolveModelResult::Bind { url, task }) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Bind {
                    url,
                    task,
                })));
            }
            Ok(ResolveModelResult::Existing) => {}
            Err(e) => {
                items.push(Err(e));
                return items;
            }
        }

        match self
            .resolver
            .resolve_motion(motion.character, &motion.motion)
        {
            Ok(ResolveModelResult::Normal(res)) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(res))));
            }
            Ok(ResolveModelResult::Bind { url, task }) => {
                items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Bind {
                    url,
                    task,
                })));
            }
            Ok(ResolveModelResult::Existing) => {}
            Err(e) => {
                items.push(Err(e));
                return items;
            }
        }

        items.push(Ok(PurifyResult::Action(internal::Action {
            wait,
            delay: motion.delay,
            detail: ActionDetail::Layout {
                model,
                motion,
                side,
                kind,
            },
        })));

        items
    }

    fn purify_effect(
        &mut self,
        EffectAction {
            wait,
            delay,
            effect,
        }: EffectAction,
    ) -> Vec<Result<PurifyResult>> {
        let mut items: Vec<Result<PurifyResult>> = Vec::new();
        let mut resource = None;

        items.push(Ok(PurifyResult::Action(internal::Action {
            wait,
            delay,
            detail: match effect {
                EffectDetail::ChangeBackground { image } => {
                    match self.resolver.resolve_background(&image) {
                        Ok(result) => {
                            let path = result.as_ref().path.clone();
                            if let ResolveCommonResult::New(resource_) = result {
                                resource = Some(resource_);
                            }
                            ActionDetail::Background(path)
                        }
                        Err(e) => {
                            items.push(Err(e));
                            return items;
                        }
                    }
                }
                EffectDetail::ChangeCardStill { image } => {
                    match self.resolver.resolve_cardstill(&image) {
                        Ok(result) => {
                            let path = result.as_ref().path.clone();
                            if let ResolveCommonResult::New(resource_) = result {
                                resource = Some(resource_);
                            }
                            ActionDetail::CardStill(path)
                        }
                        Err(e) => {
                            items.push(Err(e));
                            return items;
                        }
                    }
                }
                EffectDetail::Telop { text } => ActionDetail::Telop(text),
                other => ActionDetail::Transition(TransitionType::unwrap_from(other)),
            },
        })));

        if let Some(resource) = resource {
            items.push(Ok(PurifyResult::ResourceTask(ResourceTask::Task(resource))))
        }

        items
    }

    fn purify_unknown(&self) -> Vec<Result<PurifyResult>> {
        vec![Err(Error::Script(ScriptError::Unknown))]
    }
}

impl<'a, I, R> Purifier for DefaultPurifier<'a, I, R>
where
    I: Iterator<Item = bestdori::Action>,
    R: Resolver,
{
}

impl<'a, I, R> Iterator for DefaultPurifier<'a, I, R>
where
    I: Iterator<Item = bestdori::Action>,
    R: Resolver,
{
    type Item = Result<PurifyResult>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.pending.pop_front() {
            return Some(item);
        }

        match self.in_iter.next() {
            Some(action) => {
                let items = self.purify(action);
                for it in items {
                    self.pending.push_back(it);
                }
                self.pending.pop_front()
            }
            None => None,
        }
    }
}
