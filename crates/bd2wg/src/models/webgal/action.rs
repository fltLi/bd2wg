//! WebGAL 脚本指令

use std::{
    fmt::{self, Display},
    ops::Deref,
};

use derive_builder::Builder;
use serde::Serialize;
use webgal_derive::{ActionCustom, Actionable};

use crate::impl_display_for_serde;

/// WebGAL 命令
pub struct Action(pub Box<dyn Actionable + Send + Sync + 'static>);

impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 渲染指令迭代器
pub fn display_action_iter<I, A>(iter: I, f: &mut fmt::Formatter<'_>) -> fmt::Result
where
    I: Iterator<Item = A>,
    A: Deref<Target = Action>,
{
    for action in iter {
        writeln!(f, "{}", action.deref())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub enum FigureSide {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Position {
    pub x: i16,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Transform {
    pub position: Position,
}

impl Transform {
    pub fn new_with_x(x: i16) -> Self {
        Self {
            position: Position { x },
        }
    }
}

impl_display_for_serde! {Transform}

// ---------------- model ----------------

/// 调用场景
#[derive(Debug, Clone, Actionable)]
#[action(head = "callScene", main = "single")]
pub struct CallSceneAction {
    #[action(main)]
    pub file: String,
}

/// 分支选择
#[derive(Debug, Clone, Actionable)]
#[action(head = "choose", custom)]
pub struct ChooseAction {
    pub file: String,
    pub text: String,
}

impl ActionCustom for ChooseAction {
    fn get_main(&self) -> String {
        format!("{}:{}", self.text, self.file)
    }
}

/// 普通对话
#[derive(Debug, Clone, Actionable)]
#[action(main = "single", custom)]
pub struct SayAction {
    pub name: String,
    #[action(main)]
    pub text: String,
    #[action(arg = "tag", rename = "notend")]
    pub next: bool,
    #[action(arg = "pair", nullable, rename = "figureId", tie = "id")]
    pub character: Option<u8>,
}

impl ActionCustom for SayAction {
    fn get_head(&self) -> String {
        self.name.clone() + ":"
    }
}

/// 文本显示
#[derive(Debug, Clone, Actionable)]
#[action(head = "setTextbox", custom)]
pub struct SetTextboxAction {
    pub visible: bool,
}

impl ActionCustom for SetTextboxAction {
    fn get_main(&self) -> String {
        if self.visible {
            String::from("on")
        } else {
            String::from("hide")
        }
    }
}

/// 切换立绘
#[derive(Debug, Clone, Default, Builder, Actionable)]
#[action(head = "changeFigure", main = "single", custom)]
pub struct ChangeFigureAction {
    #[action(main, nullable, none)]
    pub model: Option<String>,
    #[action(arg = "pair")]
    pub id: u8,
    #[action(arg = "tag")]
    pub next: bool,
    pub side: FigureSide,
    #[action(arg = "pair", nullable)]
    pub transform: Option<Transform>,
    #[action(arg = "pair", nullable)]
    pub motion: Option<String>,
    #[action(arg = "pair", nullable)]
    pub expression: Option<String>,
}

impl ChangeFigureAction {
    pub fn new_hide(id: u8, next: bool) -> Self {
        Self {
            id,
            next,
            ..Default::default()
        }
    }
}

impl ActionCustom for ChangeFigureAction {
    fn get_other_args(&self) -> Option<Vec<(String, Option<String>)>> {
        match self.side {
            FigureSide::Center => None,
            FigureSide::Left => Some(vec![(String::from("left"), None)]),
            FigureSide::Right => Some(vec![(String::from("right"), None)]),
        }
    }
}

/// 设置效果
#[derive(Debug, Clone, Actionable)]
#[action(head = "setEffect", main = "single")]
pub struct SetEffectAction {
    #[action(main)]
    pub transform: Transform,
    #[action(arg = "pair")]
    pub target: u8,
    #[action(arg = "tag")]
    pub next: bool,
}

/// 切换背景
#[derive(Debug, Clone, Default, Actionable)]
#[action(head = "changeBg", main = "single")]
pub struct ChangeBgAction {
    #[action(main, nullable, none)]
    pub image: Option<String>,
    #[action(arg = "tag")]
    pub next: bool,
}

/// 背景音乐
#[derive(Debug, Clone, Actionable)]
#[action(head = "bgm", main = "single")]
pub struct BgmAction {
    #[action(main, nullable, none)]
    pub sound: Option<String>,
}

/// 效果声音
#[derive(Debug, Clone, Actionable)]
#[action(head = "playEffect", main = "single")]
pub struct PlayEffectAction {
    #[action(main, nullable, none)]
    pub sound: Option<String>,
}

/// 设置动画
#[derive(Debug, Clone, Actionable)]
#[action(head = "setAnimation", main = "single")]
pub struct SetAnimation {
    #[action(main)]
    pub animation: String,
    #[action(arg = "pair")]
    pub target: String,
    #[action(arg = "tag")]
    pub next: bool,
}

#[test]
#[cfg(test)]
fn test_action_serialize() {
    assert_eq!(
        ChooseAction {
            file: String::from("start.txt"),
            text: String::from("???"),
        }
        .to_string(),
        r#"choose:???:start.txt;"#
    );

    assert_eq!(
        SayAction {
            name: String::from("Soyo"),
            text: String::from("ごきげんよう~"),
            next: true,
            character: Some(39),
        }
        .to_string(),
        r#"Soyo:ごきげんよう~ -notend -id -figureId=39;"#
    );

    assert_eq!(
        ChangeFigureAction {
            model: Some(String::from("036_casual-2023")),
            id: 36,
            next: false,
            side: FigureSide::Left,
            transform: Some(Transform {
                position: Position { x: 0 },
            }),
            motion: Some(String::from("angry01")),
            expression: Some(String::from("angry01")),
        }
        .to_string(),
        r#"changeFigure:036_casual-2023 -id=36 -transform={"position":{"x":0}} -motion=angry01 -expression=angry01 -left;"#
    );

    assert_eq!(
        ChangeBgAction {
            image: None,
            next: false,
        }
        .to_string(),
        r#"changeBg:none;"#
    );

    assert_eq!(
        BgmAction {
            sound: Some(String::from("01. ショパン「雨だれ」.flac")),
        }
        .to_string(),
        r#"bgm:01. ショパン「雨だれ」.flac;"#
    );

    assert_eq!(
        SetAnimation {
            animation: String::from("rgbFilm"),
            target: String::from("bg-main"),
            next: true,
        }
        .to_string(),
        r#"setAnimation:rgbFilm -target=bg-main -next;"#
    );
}
