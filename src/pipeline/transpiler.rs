//! bd2wg 语法转译

use std::collections::{HashMap, VecDeque, hash_map::Entry};
use std::iter::Peekable;

use super::definition::*;
use crate::error::*;
use crate::models::{
    bestdoli::{LayoutSide, LayoutSideType, LayoutType, Motion},
    internal::{self, *},
    webgal::{self, *},
};

pub enum TranspileResult {
    Action(webgal::Action),
    Scene(String), // 切换场景
}

// impl From<webgal::Action> for TranspileResult {
//     fn from(value: webgal::Action) -> Self {
//         TranspileResult::Action(value)
//     }
// }

// impl From<String> for TranspileResult {
//     fn from(value: String) -> Self {
//         TranspileResult::Scene(value)
//     }
// }

/// webgal 脚本转译器
///
/// - 将内部脚本转换为 webgal 脚本
/// - 为 Extractor 提供场景切换辅助信息
pub trait Transpiler: Iterator<Item = Result<TranspileResult>> {}

/// 脚本上下文信息
#[derive(Default)]
struct Context {
    scene: u16,                 // 当前场景
    background: Option<String>, // 当前背景
    models: HashMap<u8, Model>, // 当前角色状态
}

/// 模型上下文信息
#[derive(Default)]
struct Model {
    model: String,
    side: FigureSide,
    transform: Transform,
    motion: Option<String>,
    expression: Option<String>,
}

/// 默认 bestdoli -> webgal 转译器
pub struct DefaultTranspiler<I>
where
    I: Iterator<Item = internal::Action>,
{
    in_iter: Peekable<I>,
    context: Context,
    pending: VecDeque<Result<TranspileResult>>,
}

impl<I> DefaultTranspiler<I>
where
    I: Iterator<Item = internal::Action>,
{
    /// 创建一个新的转译器
    pub fn new(in_iter: I) -> Self {
        let mut transpiler = Self {
            in_iter: in_iter.peekable(),
            context: Context::default(),
            pending: VecDeque::with_capacity(2),
        };
        let scene = transpiler.next_scene();

        // start.txt 只是入口, 需要切入对应场景. 场景依据 Telop 划分
        // transpiler
        //     .pending
        //     .push_back(Ok(TranspileResult::Scene(String::from("start.txt"))));  // start.txt 是 Extractor 的默认入口
        transpiler.pending.push_back(Ok(TranspileResult::Action(
            CallSceneAction {
                file: scene.clone(),
            }
            .into(),
        )));
        transpiler
            .pending
            .push_back(Ok(TranspileResult::Scene(scene)));

        transpiler
    }

    /// 生成下一个场景文件名
    fn next_scene(&mut self) -> String {
        self.context.scene += 1;
        format!("scene-{}.txt", self.context.scene)
    }

    /// 查看下一条输入命令的 wait
    fn peek_wait(&mut self) -> bool {
        match self.in_iter.peek() {
            Some(action) => action.wait,
            None => false,
        }
    }

    /// 处理单个命令
    fn transpile(&mut self, action: internal::Action) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();

        let internal::Action {
            delay: _delay,
            detail,
            ..
        } = action;

        match detail {
            ActionDetail::Say {
                name,
                text,
                characters,
                motions,
            } => {
                items.extend(self.transpile_say(name, text, characters));
            }

            ActionDetail::Bgm(sound) => items.extend(self.transpile_bgm(sound)),

            ActionDetail::Sound(sound) => items.extend(self.transpile_sound(sound)),

            ActionDetail::Background(image) => items.extend(self.transpile_background(image)),

            ActionDetail::CardStill(image) => items.extend(self.transpile_cardstill(image)),

            ActionDetail::Transition(transition) => {
                items.extend(self.transpile_transition(transition))
            }

            ActionDetail::Telop(text) => items.extend(self.transpile_telop(text)),

            ActionDetail::Layout {
                model,
                motion,
                side,
                kind,
            } => items.extend(self.transpile_layout(model, motion, side, kind)),

            ActionDetail::Motion { model, motion } => {
                items.extend(self.transpile_motion(model, motion))
            }

            ActionDetail::Unknown => items.push(Err(ScriptError::Unknown.into())),
        }

        items
    }

    // helper: 封装 push webgal::Action
    fn push_action(items: &mut Vec<Result<TranspileResult>>, action: webgal::Action) {
        items.push(Ok(TranspileResult::Action(action)));
    }

    // helper: 封装 push 场景切换
    fn push_scene(items: &mut Vec<Result<TranspileResult>>, scene: String) {
        items.push(Ok(TranspileResult::Scene(scene)));
    }

    // SAY
    fn transpile_say(
        &mut self,
        name: String,
        text: String,
        characters: Vec<u8>,
    ) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        Self::push_action(
            &mut items,
            webgal::SayAction {
                name: name.trim().to_string(),
                text: text.trim().to_string(),
                next: !self.peek_wait(),
                character: characters.first().copied(),
            }
            .into(),
        );
        items
    }

    // BGM
    fn transpile_bgm(&mut self, sound: String) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        Self::push_action(&mut items, webgal::BgmAction { sound: Some(sound) }.into());
        items
    }

    // Sound effect
    fn transpile_sound(&mut self, sound: String) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        Self::push_action(
            &mut items,
            webgal::PlayEffectAction { sound: Some(sound) }.into(),
        );
        items
    }

    // Background
    fn transpile_background(&mut self, image: String) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        self.context.background = Some(image.clone());
        Self::push_action(
            &mut items,
            webgal::ChangeBgAction {
                image: Some(image),
                next: !self.peek_wait(),
            }
            .into(),
        );
        items
    }

    // CardStill
    fn transpile_cardstill(&mut self, image: String) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        Self::push_action(&mut items, SetTextboxAction { visible: false }.into());
        self.context.models.iter().for_each(|(id, _)| {
            Self::push_action(&mut items, ChangeFigureAction::new_hide(*id, true).into());
        });

        Self::push_action(
            &mut items,
            ChangeBgAction {
                image: Some(image),
                next: false,
            }
            .into(),
        );
        Self::push_action(
            &mut items,
            ChangeBgAction {
                image: self.context.background.clone(),
                next: true,
            }
            .into(),
        );

        self.context.models.iter().for_each(|(id, model)| {
            Self::push_action(
                &mut items,
                ChangeFigureAction {
                    model: Some(model.model.clone()),
                    id: *id,
                    next: true,
                    side: model.side.clone(),
                    transform: Some(model.transform.clone()),
                    motion: model.motion.clone(),
                    expression: model.expression.clone(),
                }
                .into(),
            );
        });

        Self::push_action(&mut items, SetTextboxAction { visible: true }.into());
        items
    }

    // Transition
    fn transpile_transition(&mut self, transition: TransitionType) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        let effect = match transition {
            TransitionType::BlackIn | TransitionType::WhiteIn => "enter",
            TransitionType::BlackOut | TransitionType::WhiteOut => "exit",
        };
        Self::push_action(
            &mut items,
            webgal::SetAnimation {
                animation: effect.to_string(),
                target: "bg-main".to_string(),
                next: self.peek_wait(),
            }
            .into(),
        );
        items
    }

    // Telop
    fn transpile_telop(&mut self, text: String) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        let scene = self.next_scene();
        Self::push_action(
            &mut items,
            webgal::ChooseAction {
                file: scene.clone(),
                text,
            }
            .into(),
        );
        Self::push_scene(&mut items, scene);
        items
    }

    // Layout (Appear / Hide / Move)
    fn transpile_layout(
        &mut self,
        model: String,
        motion: Motion,
        side: LayoutSide,
        kind: LayoutType,
    ) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();

        match kind {
            LayoutType::Appear => {
                let new_model = Model {
                    model: model.clone(),
                    side: side.to.into(),
                    transform: Transform::new_x(side.to_x),
                    motion: Some(motion.motion.clone()),
                    expression: Some(motion.expression.clone()),
                };

                let next = self.peek_wait();

                let entry = match self.context.models.entry(motion.character) {
                    Entry::Vacant(v) => v.insert(new_model),
                    Entry::Occupied(mut o) => {
                        *o.get_mut() = new_model;
                        o.into_mut()
                    }
                };

                Self::push_action(
                    &mut items,
                    ChangeFigureAction {
                        model: Some(model),
                        id: motion.character,
                        next,
                        side: entry.side.clone(),
                        transform: Some(entry.transform.clone()),
                        motion: Some(motion.motion),
                        expression: Some(motion.expression),
                    }
                    .into(),
                );
            }

            LayoutType::Hide => {
                if let Some(_model) = self.context.models.remove(&motion.character) {
                    Self::push_action(
                        &mut items,
                        ChangeFigureAction::new_hide(motion.character, self.peek_wait()).into(),
                    );
                } else {
                    items.push(Err(ScriptError::IdNotFound(motion.character).into()));
                }
            }

            LayoutType::Move => {
                let next = self.peek_wait();

                if let Entry::Occupied(mut o) = self.context.models.entry(motion.character) {
                    let mut entry = o.get_mut();
                    *entry = Model {
                        model: model.clone(),
                        side: side.to.into(),
                        transform: Transform::new_x(side.to_x),
                        motion: Some(motion.motion.clone()),
                        expression: Some(motion.expression.clone()),
                    };

                    Self::push_action(
                        &mut items,
                        ChangeFigureAction {
                            model: Some(model),
                            id: motion.character,
                            next,
                            side: entry.side.clone(),
                            transform: Some(entry.transform.clone()),
                            motion: Some(motion.motion),
                            expression: Some(motion.expression),
                        }
                        .into(),
                    );
                } else {
                    items.push(Err(ScriptError::IdNotFound(motion.character).into()));
                }
            }
        }

        items
    }

    // Motion
    fn transpile_motion(&mut self, model: String, motion: Motion) -> Vec<Result<TranspileResult>> {
        let mut items = Vec::new();
        let next = self.peek_wait();

        match self.context.models.entry(motion.character) {
            Entry::Occupied(mut o) => {
                let entry = o.get_mut();
                entry.motion = Some(motion.motion.clone());
                entry.expression = Some(motion.expression.clone());

                Self::push_action(
                    &mut items,
                    ChangeFigureAction {
                        model: Some(entry.model.clone()),
                        id: motion.character,
                        next,
                        side: entry.side.clone(),
                        transform: Some(entry.transform.clone()),
                        motion: Some(motion.motion),
                        expression: Some(motion.expression),
                    }
                    .into(),
                );
            }

            Entry::Vacant(v) => {
                let new_model = Model {
                    model: model.clone(),
                    side: FigureSide::default(),
                    transform: Transform::default(),
                    motion: Some(motion.motion.clone()),
                    expression: Some(motion.expression.clone()),
                };
                let entry = v.insert(new_model);

                items.push(Err(ScriptError::IdNotFound(motion.character).into()));
                Self::push_action(
                    &mut items,
                    ChangeFigureAction {
                        model: Some(model),
                        id: motion.character,
                        next,
                        side: entry.side.clone(),
                        transform: Some(entry.transform.clone()),
                        motion: Some(motion.motion),
                        expression: Some(motion.expression),
                    }
                    .into(),
                );
            }
        }

        items
    }
}

impl<I> Transpiler for DefaultTranspiler<I> where I: Iterator<Item = internal::Action> {}

impl<I> Iterator for DefaultTranspiler<I>
where
    I: Iterator<Item = internal::Action>,
{
    type Item = Result<TranspileResult>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.pending.pop_front() {
            return Some(item);
        }

        match self.in_iter.next() {
            Some(action) => {
                let items = self.transpile(action);
                for it in items {
                    self.pending.push_back(it);
                }
                self.pending.pop_front()
            }
            None => None,
        }
    }
}
