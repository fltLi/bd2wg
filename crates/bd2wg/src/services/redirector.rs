//! 资源重定向器

use std::{path::Path, rc::Rc, sync::Arc};

use regex::Regex;

use crate::{models::{redirect::*, webgal::Model}, traits::redirect::*};

//////////////// Redirect ////////////////

struct ModelRedirector {
    root: Rc<Path>,
    costume: Regex,
    motions: Arc<[Regex]>,
    expressions: Arc<[Regex]>,
}

impl ModelRedirector {
    // fn new(root: &'p Path, rule: ModelRedirectRule) -> Self {
    fn new(root: Rc<Path>, rule: &ModelRedirectRule) -> Self {
        Self { root }
    }

    fn redirect(&self, costume: &str) -> Option<MotionRedirector> {
        unimplemented!()
    }
}

/// 资源重定向器
pub struct Redirector {
    root: Rc<Path>,
    model: Vec<ModelRedirector>,
}

impl Redirector {
    /// 在指定目录下创建一个重定向器
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().into(),
            model: Vec::new(),
        }
    }
}

impl Redirect for Redirector {
    type MotionRedirector = MotionRedirector;

    fn add_rules(&mut self, rules: &RedirectRules) {
        // model
        self.model.extend(
            rules
                .model
                .iter()
                .map(|rule| ModelRedirector::new(self.root.clone(), rule)),
        );
    }

    fn redirect_model(&self, costume: &str) -> Option<Self::MotionRedirector> {
        self.model.iter().find_map(|rule| rule.redirect(costume))
    }
}

//////////////// ModelRedirect ////////////////

/// 单个模型重定向器
pub struct MotionRedirector {
    figure: String,
}

impl MotionRedirector {
    fn new(figure: String, config: &Model) -> Self {
        unimplemented!()
    }
}

impl MotionRedirect for MotionRedirector {
    fn redirect_motion(&self, motion: &str) -> RedirectResult<String> {
        unimplemented!("TODO: redirect_motion")
    }

    fn redirect_expression(&self, expression: &str) -> RedirectResult<String> {
        unimplemented!("TODO: redirect_expression")
    }
}
