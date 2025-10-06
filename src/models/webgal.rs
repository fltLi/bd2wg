//! webgal 脚本数据模型

use std::fmt::{self, Display};

use action::Actionable;
use serde::Serialize;

use crate::models::bestdori::LayoutSideType;

/// webgal 命令标记特型
pub trait Actionable: Display {}

/// webgal 命令
pub struct Action(pub Box<dyn Actionable + Send + Sync>);

impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 自定义序列化行为
trait ActionCustom {
    fn get_head(&self) -> String {
        String::default()
    }

    fn get_main(&self) -> String {
        String::default()
    }

    fn get_other_args(&self) -> Option<Vec<(String, Option<String>)>> {
        None
    }
}

/// 为支持 Serialize 的对象实现 Display
macro_rules! impl_serde_display {
    ($name:ident) => {
        paste::paste! {
            impl Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", serde_json::to_string(self).map_err(|_| fmt::Error)?)
                }
            }
        }
    };
}

/// 调用场景
#[derive(Actionable)]
#[action(head = "callScene", main = "single")]
pub struct CallSceneAction {
    #[action(main)]
    pub file: String,
}

/// 分支选择
/// - /effect/telop
#[derive(Actionable)]
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
/// - /talk
#[derive(Actionable)]
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
/// - effect/cardstill
#[derive(Actionable)]
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

#[derive(Clone)]
pub enum FigureSide {
    Left,
    Center,
    Right,
}

impl From<LayoutSideType> for FigureSide {
    fn from(value: LayoutSideType) -> Self {
        match value {
            LayoutSideType::Center => Self::Center,
            LayoutSideType::LeftOver | LayoutSideType::LeftInside => Self::Left,
            LayoutSideType::RightOver | LayoutSideType::RightInside => Self::Right,
        }
    }
}

#[derive(Serialize, Default, Clone)]
pub struct Position {
    pub x: i16,
}

#[derive(Serialize, Default, Clone)]
pub struct Transform {
    pub position: Position,
}

impl Transform {
    pub fn new_x(x: i16) -> Self {
        Self {
            position: Position { x },
        }
    }
}

impl_serde_display! {Transform}

impl Default for FigureSide {
    fn default() -> Self {
        Self::Center
    }
}

/// 切换立绘
/// - /motion
/// - /talk/motion
/// - /layout/motion
#[derive(Actionable)]
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
            model: None,
            id,
            next,
            side: FigureSide::default(),
            transform: None,
            motion: None,
            expression: None,
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
/// - /layout/motion/move
#[derive(Actionable)]
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
/// - /effect/background
/// - /effect/cardstill
#[derive(Actionable)]
#[action(head = "changeBg", main = "single")]
pub struct ChangeBgAction {
    #[action(main, nullable, none)]
    pub image: Option<String>,
    #[action(arg = "tag")]
    pub next: bool,
}

/// 背景音乐
/// - /sound/bgm
#[derive(Actionable)]
#[action(head = "bgm", main = "single")]
pub struct BgmAction {
    #[action(main, nullable, none)]
    pub sound: Option<String>,
}

/// 效果声音
/// - /sound/se
#[derive(Actionable)]
#[action(head = "playEffect", main = "single")]
pub struct PlayEffectAction {
    #[action(main, nullable, none)]
    pub sound: Option<String>,
}

/// 设置动画
/// - /effect/...
#[derive(Actionable)]
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
fn test_webgal_serialize() {
    let choose = ChooseAction {
        file: String::from("start.txt"),
        text: String::from("???"),
    };

    let say = SayAction {
        name: String::from("Soyo"),
        text: String::from("ごきげんよう~"),
        next: true,
        character: Some(39),
    };

    let change_figure = ChangeFigureAction {
        model: Some(String::from("036_casual-2023")),
        id: 36,
        next: false,
        side: FigureSide::Left,
        transform: Some(Transform {
            position: Position { x: 0 },
        }),
        motion: Some(String::from("angry01")),
        expression: Some(String::from("angry01")),
    };

    let change_bg = ChangeBgAction {
        image: None,
        next: false,
    };

    let bgm = BgmAction {
        sound: Some(String::from("01. ショパン「雨だれ」.flac")),
    };

    let set_animation = SetAnimation {
        animation: String::from("rgbFilm"),
        target: String::from("bg-main"),
        next: true,
    };

    assert_eq!(choose.to_string(), r#"choose:???:start.txt"#);

    assert_eq!(
        say.to_string(),
        r#"Soyo:ごきげんよう~ -notend -id -figureId=39"#
    );

    assert_eq!(
        change_figure.to_string(),
        r#"changeFigure:036_casual-2023 -id=36 -transform={"position":{"x":0}} -motion=angry01 -expression=angry01 -left"#
    );

    assert_eq!(change_bg.to_string(), r#"changeBg:none"#);

    assert_eq!(bgm.to_string(), r#"bgm:01. ショパン「雨だれ」.flac"#);

    assert_eq!(
        set_animation.to_string(),
        r#"setAnimation:rgbFilm -target=bg-main -next"#
    );
}

