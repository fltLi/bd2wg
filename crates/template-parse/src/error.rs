use thiserror::Error;

// pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    #[error("Variable parse failed: template={template}, variable={variable}")]
    VariableParse { template: String, variable: String },
}
