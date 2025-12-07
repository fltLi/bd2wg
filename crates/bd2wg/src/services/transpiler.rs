//! 脚本转译器

// TODO: 处理 delay 字段.

use std::{collections::HashMap, sync::Arc};

use crate::error::*;
use crate::models::webgal::{ChangeFigureAction, SayAction};
use crate::models::{
    bestdori::{self, Motion},
    webgal::{self, FigureSide, Resource, Scene, Transform},
};
use crate::traits::resolver::Resolver;
use crate::traits::transpiler::{TranspileResult, Transpiler as TranspilerTrait};

type PreResult<T> = std::result::Result<T, TranspileErrorKind>;

/// 模型上下文信息
#[derive(Debug, Clone)]
struct Model {
    path: String,
    side: FigureSide,
    transform: Transform,
    motion: Option<String>,
    expression: Option<String>,
}

/// 上下文信息
#[derive(Debug, Default)]
struct Context {
    background: Option<String>,
    models: HashMap<u8, Model>,
}

/// 脚本转译器
///
/// 若希望复用 Resolver, 考虑使用 Arc 包装一个实现.
pub struct Transpiler<R: Resolver> {
    resolver: R,
    context: Context,
    scenes: Vec<Scene>,
    resources: Vec<Arc<Resource>>,
}

impl<R: Resolver> Transpiler<R> {
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            context: Context::default(),
            scenes: vec![Scene::new_start_scene()],
            resources: Vec::new(),
        }
    }

    fn into_result(self, errors: Vec<Error>) -> TranspileResult {
        TranspileResult {
            story: webgal::Story(self.scenes),
            resources: self.resources,
            errors,
        }
    }

    /// 清空场景
    fn clear(&mut self) {
        unimplemented!()
    }

    /// 清理并切换场景
    fn next_scene(&mut self) {
        self.clear();
        self.scenes
            .push(Scene::new(&format!("scene-{}.txt", self.scenes.len())));
    }

    fn push_action(&mut self, action: webgal::Action) {
        self.scenes.last_mut().unwrap().actions.push(action);
    }

    // ---------------- transpile ----------------

    /// 转译单个场景
    fn transpile(&mut self, action: &bestdori::Action, wait: bool) -> Result<()> {
        use bestdori::Action;

        match action {
            Action::Talk(a) => self.transpile_talk(a, wait),
            Action::Sound(a) => self.transpile_sound(a, wait),
            Action::Effect(a) => self.transpile_effect(a, wait),
            Action::Layout(a) => self.transpile_layout(a, wait),
            Action::Motion(a) => self.transpile_motion(a, wait),
            Action::Unknown => Err(TranspileErrorKind::Unknown),
        }
        .map_err(|e| {
            TranspileError {
                action: Box::new(action.clone()),
                error: e,
            }
            .into()
        })
    }

    fn transpile_talk(&mut self, action: &bestdori::TalkAction, wait: bool) -> PreResult<()> {
        let bestdori::TalkAction {
            name,
            text,
            motions,
            characters,
            ..
        } = action;

        let mut res = Ok(()); // 至多收集 1 个错误

        // 执行动作
        for motion in motions {
            if let Err(e) = self.display_motion(motion, true) {
                res = Err(e);
            }
        }

        // 执行对话
        self.push_action(
            SayAction {
                name: name.clone(),
                text: text.clone(),
                next: !wait,
                character: characters.first().cloned(),
            }
            .into(),
        );

        res
    }

    fn transpile_sound(&mut self, action: &bestdori::SoundAction, wait: bool) -> PreResult<()> {
        unimplemented!()
    }

    fn transpile_effect(&mut self, action: &bestdori::EffectAction, wait: bool) -> PreResult<()> {
        unimplemented!()
    }

    fn transpile_layout(&mut self, action: &bestdori::LayoutAction, wait: bool) -> PreResult<()> {
        unimplemented!()
    }

    fn transpile_motion(&mut self, action: &bestdori::MotionAction, wait: bool) -> PreResult<()> {
        unimplemented!()
    }

    // ---------------- transpile ----------------

    /// 执行模型动作
    ///
    /// 若采用 model: &Model, 仍需要对每个字段 clone, 故直接移动 (调用者 clone).
    fn display_model(&mut self, id: u8, model: Model, next: bool) {
        self.push_action(
            ChangeFigureAction {
                model: Some(model.path),
                id,
                next,
                side: model.side,
                transform: Some(model.transform),
                motion: model.motion,
                expression: model.expression,
            }
            .into(),
        );
    }

    /// 修改模型动作
    fn display_motion(&mut self, motion: &Motion, next: bool) -> PreResult<()> {
        let Motion {
            character,
            motion,
            expression,
            ..
        } = motion;

        self.context
            .models
            .get_mut(character)
            .ok_or(TranspileErrorKind::UninitFigure(*character))
            .map(|model| {
                // 修改上下文
                model.motion = Some(motion.clone());
                model.expression = Some(expression.clone());
                model.clone()
            })
            .map(|model| self.display_model(*character, model, next)) // 应用修改
    }
}

impl<R: Resolver + Default> Default for Transpiler<R> {
    fn default() -> Self {
        Self::new(R::default())
    }
}

impl<R: Resolver> TranspilerTrait for Transpiler<R> {
    fn transpile(mut self, story: &bestdori::Story) -> TranspileResult {
        let errors = story
            .iter_with_wait()
            .filter_map(|(a, wait)| <Self>::transpile(&mut self, a, wait).err())
            .collect();

        self.into_result(errors)
    }
}
