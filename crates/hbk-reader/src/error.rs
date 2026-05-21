//! Ошибки при чтении hbk-контейнера.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum HbkError {
    #[error("hbk-файл не существует: {0}")]
    NotFound(PathBuf),

    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("неожиданный формат hbk-контейнера: {0}")]
    BadFormat(String),

    #[error("ошибка распаковки PackBlock (zlib/zip): {0}")]
    Inflate(String),

    #[error("ошибка zip-архива FileStorage: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("entity '{0}' не найдена в hbk-контейнере")]
    EntityNotFound(String),

    #[error("файл '{0}' не найден в архиве FileStorage")]
    HtmlEntryNotFound(String),

    #[error("ошибка парсинга TOC: {0}")]
    TocParse(String),
}

pub type Result<T> = std::result::Result<T, HbkError>;
