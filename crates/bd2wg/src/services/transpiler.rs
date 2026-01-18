//! 脚本转译器

// TODO: 处理 delay 字段.

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use crate::{
    error::*,
    models::{
        bestdori::{self, Motion},
        webgal::{self, ChangeFigureAction, FigureSide, Resource, SayAction, Scene, Transform},
    },
    return_ok,
    traits::{asset::Asset, resolve::*, transpile::*},
};

type PreResult<T> = std::result::Result<T, TranspileErrorKind>;

/// 模型上下文信息
#[derive(Debug)]
struct Model<R: ModelDisplayResolve> {
    path: String,
    side: FigureSide,
    transform: Transform,
    motion: Option<String>,
    expression: Option<String>,
    resolver: Option<Arc<R>>,
}

impl<R: ModelDisplayResolve> Model<R> {
    fn resolve_motion(&self, motion: &str) -> PreResult<String> {
        match &self.resolver {
            Some(rsv) => rsv.resolve_motion(motion).map_err(TranspileErrorKind::from),
            None => Ok(motion.to_string()),
        }
    }

    fn resolve_expression(&self, expression: &str) -> PreResult<String> {
        match &self.resolver {
            Some(rsv) => rsv
                .resolve_expression(expression)
                .map_err(TranspileErrorKind::from),
            None => Ok(expression.to_string()),
        }
    }

    fn get_motion(&self) -> PreResult<Option<String>> {
        self.motion
            .as_ref()
            .map(|motion| self.resolve_motion(motion))
            .transpose()
    }

    fn get_expression(&self) -> PreResult<Option<String>> {
        self.expression
            .as_ref()
            .map(|expression| self.resolve_expression(expression))
            .transpose()
    }
}

impl<R: ModelDisplayResolve> Clone for Model<R> {
    fn clone(&self) -> Self {
        Model {
            path: self.path.clone(),
            side: self.side,
            transform: self.transform.clone(),
            motion: self.motion.clone(),
            expression: self.expression.clone(),
            resolver: self.resolver.clone(),
        }
    }
}

impl<R: ModelDisplayResolve> Default for Model<R> {
    fn default() -> Self {
        Self {
            path: String::default(),
            side: FigureSide::default(),
            transform: Transform::default(),
            motion: None,
            expression: None,
            resolver: None,
        }
    }
}

/// 上下文信息
#[derive(Debug)]
struct Context<R: ModelDisplayResolve> {
    background: Option<String>,
    models: HashMap<u8, Model<R>>,
}

impl<R: ModelDisplayResolve> Default for Context<R> {
    fn default() -> Self {
        Self {
            background: None,
            models: HashMap::default(),
        }
    }
}

/// 脚本转译器
///
/// 若希望复用 Resolver, 考虑使用 Arc 包装一个实现.
pub struct Transpiler<R: Resolve> {
    resolver: R,
    context: Context<R::ModelDisplayResolver>,
    scenes: Vec<Scene>,
    resources: Vec<Arc<Resource>>,
}

impl<R: Resolve> Transpiler<R> {
    pub fn new(resolver: R) -> Self {
        let mut transpiler = Self {
            resolver,
            context: Context::default(),
            scenes: vec![Scene::new_start_scene()],
            resources: Vec::new(),
        };

        transpiler.push_action_and_change_scene(
            webgal::CallSceneAction {
                file: transpiler.next_scene_name(),
            }
            .into(),
        );

        transpiler
    }

    fn into_result(self, errors: Vec<Error>) -> TranspileResult {
        TranspileResult {
            story: webgal::Story(self.scenes),
            resources: self.resources,
            errors,
        }
    }

    /// 清空场景
    fn clear(&mut self) -> Context<R::ModelDisplayResolver> {
        // 移除人物
        let actions: Vec<webgal::Action> = self
            .context
            .models
            .keys()
            .map(|&id| webgal::ChangeFigureAction::new_hide(id, true).into())
            // 移除背景
            .chain(std::iter::once(webgal::ChangeBgAction::default().into()))
            .collect();

        for act in actions {
            self.push_action(act);
        }

        std::mem::take(&mut self.context)
    }

    /// 设置上下文
    fn set_context(&mut self, context: Context<R::ModelDisplayResolver>) {
        // 清空场景 (场景大概为空)
        self.clear();

        // 设置人物
        for (&id, model) in &context.models {
            // // 模型动作 / 表情错误在其被首次调用时就已经抛出过了, 故此处忽略以避免重复错误过多.
            let _ = self.display_model(id, model.clone(), true);
        }

        // 设置背景
        self.push_action(
            webgal::ChangeBgAction {
                image: context.background.clone(),
                next: false,
            }
            .into(),
        );

        // 设置场景
        self.context = context;
    }

    /// 下一个场景的名称
    fn next_scene_name(&self) -> String {
        format!("scene-{}.txt", self.scenes.len())
    }

    fn push_action(&mut self, action: webgal::Action) {
        self.scenes.last_mut().unwrap().actions.push(action);
    }

    fn push_action_and_change_scene(&mut self, action: webgal::Action) {
        self.push_action(action);
        self.scenes.push(Scene::new(&self.next_scene_name()));
    }

    /// 识别并记录新资源
    ///
    /// 始终在上下文使用完资源后调用以记录
    fn maybe_push_resource(&mut self, res: ResourceEntry) {
        if let ResourceEntry::Vacant(v) = res {
            self.resources.push(v);
        }
    }

    // ---------------- transpile ----------------

    /// 转译单个场景
    fn transpile(&mut self, action: &bestdori::Action, wait: bool) -> Result<()> {
        use bestdori::Action;

        match action {
            Action::Talk(a) => self.transpile_talk(a, wait),
            Action::Sound(a) => self.transpile_sound(a),
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
            res = res.and(self.try_display_motion(motion, true));
        }

        // 执行对话
        self.push_action(
            SayAction {
                name: name.clone(),
                text: text.trim().to_string(),
                next: !wait,
                character: characters.first().cloned(),
            }
            .into(),
        );

        res
    }

    fn transpile_sound(&mut self, action: &bestdori::SoundAction) -> PreResult<()> {
        let bestdori::SoundAction { bgm, se, .. } = action;

        Ok(())
            // 执行 bgm
            .and(bgm.as_ref().map_or(Ok(()), |bgm| self.transpile_bgm(bgm)))
            // 执行 se
            .and(se.as_ref().map_or(Ok(()), |se| self.transpile_se(se)))
    }

    fn transpile_effect(&mut self, action: &bestdori::EffectAction, wait: bool) -> PreResult<()> {
        use bestdori::Effect;

        match &action.effect {
            // 入场
            Effect::BlackIn | Effect::WhiteIn => self.display_transition("enter", !wait),

            // 退场
            Effect::BlackOut | Effect::WhiteOut => self.display_transition("exit", !wait),

            // 呈现字幕
            Effect::Telop { text } => self.display_telop(text),

            // 修改背景
            Effect::ChangeBackground { image } => self.display_background(image, !wait)?,

            // 呈现卡面
            Effect::ChangeCardStill { image } => self.display_cardstill(image, !wait)?,
        }

        Ok(())
    }

    fn transpile_layout(&mut self, action: &bestdori::LayoutAction, wait: bool) -> PreResult<()> {
        let bestdori::LayoutAction {
            kind,
            model,
            motion,
            side: bestdori::LayoutSide { to, to_x, .. },
            ..
        } = action;

        match kind {
            // 执行退场
            bestdori::LayoutType::Hide => self.remove_model(motion.character, !wait),

            // 执行移动
            bestdori::LayoutType::Move => {
                let model = self
                    .context
                    .models
                    .get_mut(&motion.character)
                    .ok_or(TranspileErrorKind::UninitFigure(motion.character))?;

                model.side = (*to).into();
                model.transform = Transform::new_with_x(*to_x);

                self.display_motion_unwrap(motion, !wait)
            }

            // 执行登场
            bestdori::LayoutType::Appear => {
                let (res, resolver) = self.resolver.resolve_model(model);

                self.display_motion(&res.relative_path(), (*to).into(), motion, !wait, resolver)?;

                self.maybe_push_resource(res);

                Ok(())
            }
        }
    }

    fn transpile_motion(&mut self, action: &bestdori::MotionAction, wait: bool) -> PreResult<()> {
        let bestdori::MotionAction { model, motion, .. } = action;

        let (res, resolver) = self.resolver.resolve_model(model);

        // 执行模型动作
        self.display_motion(
            &res.relative_path(),
            FigureSide::default(),
            motion,
            !wait,
            resolver,
        )?;

        self.maybe_push_resource(res);

        Ok(())
    }

    // ---------------- transpile ----------------

    /// 转译 sound/bgm
    fn transpile_bgm(&mut self, res: &bestdori::Resource) -> PreResult<()> {
        let res = self.resolver.resolve_normal(res, ResourceType::Bgm)?;

        self.push_action(
            webgal::BgmAction {
                sound: Some(res.relative_path()),
            }
            .into(),
        );

        self.maybe_push_resource(res);

        Ok(())
    }

    /// 转译 sound/se
    fn transpile_se(&mut self, res: &bestdori::Resource) -> PreResult<()> {
        let res = self.resolver.resolve_normal(res, ResourceType::Bgm)?;

        self.push_action(
            webgal::PlayEffectAction {
                sound: Some(res.relative_path()),
            }
            .into(),
        );

        self.maybe_push_resource(res);

        Ok(())
    }

    /// 执行转场
    ///
    /// 是否需要清空背景?
    fn display_transition(&mut self, animation: &str, next: bool) {
        self.push_action(
            webgal::SetAnimation {
                animation: animation.to_string(),
                target: "bg-main".to_string(),
                next,
            }
            .into(),
        );
    }

    /// 呈现字幕 (通过切换场景实现)
    fn display_telop(&mut self, text: &str) {
        self.push_action_and_change_scene(
            webgal::ChooseAction {
                file: self.next_scene_name(),
                text: text.to_string(),
            }
            .into(),
        );
    }

    /// 修改背景
    fn display_background(&mut self, res: &bestdori::Resource, next: bool) -> PreResult<()> {
        let res = self.resolver.resolve_normal(res, ResourceType::Image)?;
        let path = res.relative_path();

        // 修改上下文
        self.context.background = Some(path.clone());

        // 显示背景
        self.push_action(
            webgal::ChangeBgAction {
                image: Some(path),
                next,
            }
            .into(),
        );

        self.maybe_push_resource(res);

        Ok(())
    }

    /// 呈现卡面
    fn display_cardstill(&mut self, res: &bestdori::Resource, next: bool) -> PreResult<()> {
        let res = self.resolver.resolve_normal(res, ResourceType::Image)?;

        // 记录并清空场景
        let ctx = self.clear();

        // 显示背景
        self.push_action(
            webgal::ChangeBgAction {
                image: Some(res.relative_path()),
                next,
            }
            .into(),
        );

        // 恢复场景
        self.set_context(ctx);

        self.maybe_push_resource(res);

        Ok(())
    }

    /// 执行模型动作
    ///
    /// 若采用 model: &Model, 仍需要对每个字段 clone, 故直接移动 (调用者 clone).
    fn display_model(
        &mut self,
        id: u8,
        model: Model<R::ModelDisplayResolver>,
        next: bool,
    ) -> PreResult<()> {
        let motion = model.get_motion()?;
        let expression = model.get_expression()?;

        self.push_action(
            ChangeFigureAction {
                model: Some(model.path),
                id,
                next,
                side: model.side,
                transform: Some(model.transform),
                motion,
                expression,
            }
            .into(),
        );

        Ok(())
    }

    /// 修改模型动作 (当模型存在时)
    fn try_display_motion(&mut self, motion: &Motion, next: bool) -> PreResult<()> {
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
            .and_then(|model| self.display_model(*character, model, next)) // 应用修改
    }

    /// 修改模型动作 (模型一定存在)
    fn display_motion_unwrap(&mut self, motion: &Motion, next: bool) -> PreResult<()> {
        match self.try_display_motion(motion, next) {
            Err(TranspileErrorKind::UninitFigure(id)) => {
                unreachable!("model {id} should exist but not.")
            }
            v => v,
        }
    }

    /// 修改模型动作 (不存在时插入模型)
    fn display_motion(
        &mut self,
        model: &str,
        side: FigureSide,
        motion: &Motion,
        next: bool,
        resolver: Option<R::ModelDisplayResolver>,
    ) -> PreResult<()> {
        if let Entry::Vacant(v) = self.context.models.entry(motion.character) {
            v.insert(Model {
                path: model.to_string(),
                side,
                resolver: resolver.map(Arc::new),
                ..Default::default()
            });
        }

        self.display_motion_unwrap(motion, next)
    }

    /// 移除模型
    fn remove_model(&mut self, id: u8, next: bool) -> PreResult<()> {
        match self.context.models.remove(&id) {
            Some(_) => {
                return_ok! {self.push_action(webgal::ChangeFigureAction::new_hide(id, next).into())}
            }
            None => Err(TranspileErrorKind::UninitFigure(id)),
        }
    }
}

impl<R: Resolve + Default> Default for Transpiler<R> {
    fn default() -> Self {
        Self::new(R::default())
    }
}

impl<R: Resolve> Transpile for Transpiler<R> {
    fn transpile(mut self, story: &bestdori::Story) -> TranspileResult {
        let errors = story
            .iter_with_wait()
            .filter_map(|(a, wait)| <Self>::transpile(&mut self, a, wait).err())
            .collect();

        self.into_result(errors)
    }
}
