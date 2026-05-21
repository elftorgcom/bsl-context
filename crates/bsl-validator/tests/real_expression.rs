//! Integration-тесты Phase 6 (`validate_expression`) на реальном `shcntx_ru.hbk`.
//!
//! Acceptance из плана:
//! 1. `Если Х = ТипРазмещенияТекстаТабличногоДокумента.Перенос Тогда` →
//!    ошибка с подсказкой 'Переносить'.
//! 2. Вызов глобальной функции с лишним числом аргументов → ошибка.
//! 3. Корректный код → нет ошибок.

use std::path::PathBuf;

use bsl_validator::{validate_expression, ExprErrorKind};
use platform_index::load_from_hbk;

fn hbk_path() -> Option<PathBuf> {
    let root = std::env::var("BSL_CONTEXT_PLATFORM_PATH").ok().map(PathBuf::from)?;
    let candidates = [root.join("shcntx_ru.hbk"), root.join("bin").join("shcntx_ru.hbk")];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
fn canonical_638_enum_typo() {
    let Some(path) = hbk_path() else {
        eprintln!("skip: hbk не найден");
        return;
    };
    let index = load_from_hbk(&path).expect("PlatformIndex");

    let src =
        "Если Х = ТипРазмещенияТекстаТабличногоДокумента.Перенос Тогда\n  // что-то\nКонецЕсли;";
    let result = validate_expression(&index, src);
    println!("{result:#?}");

    assert!(!result.valid, "ожидается valid=false");
    let err = result
        .errors
        .iter()
        .find(|e| e.kind == ExprErrorKind::UnknownEnumValue)
        .expect("должна быть ошибка UnknownEnumValue");
    assert_eq!(err.suggestion.as_deref(), Some("Переносить"));
}

#[test]
fn extra_argument_to_global_method() {
    let Some(path) = hbk_path() else { return };
    let index = load_from_hbk(&path).expect("PlatformIndex");

    // У 'СтрНайти' максимум 5 параметров; 6 — точно invalid.
    let src = "Поз = СтрНайти(Текст, Подстрока, 1, 1, 1, ЛишнийАргумент);";
    let result = validate_expression(&index, src);
    println!("{result:#?}");
    assert!(!result.valid, "ожидается valid=false");
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.kind == ExprErrorKind::WrongArgumentCount),
        "должна быть ошибка WrongArgumentCount"
    );
}

#[test]
fn unknown_new_type() {
    let Some(path) = hbk_path() else { return };
    let index = load_from_hbk(&path).expect("PlatformIndex");

    // Заведомо несуществующий тип. Похожих хватает (НесуществующийТип / Запрос),
    // suggestion может прийти любая — главное, что зафиксирован UnknownNewType.
    let src = "Х = Новый ЗапрозБезОшибок;";
    let result = validate_expression(&index, src);
    println!("{result:#?}");
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.kind == ExprErrorKind::UnknownNewType));
}

#[test]
fn correct_code_yields_no_errors() {
    let Some(path) = hbk_path() else { return };
    let index = load_from_hbk(&path).expect("PlatformIndex");

    let src = "Если Х = ТипРазмещенияТекстаТабличногоДокумента.Переносить Тогда\n  Поз = СтрНайти(\"a.b\", \".\");\nКонецЕсли;";
    let result = validate_expression(&index, src);
    println!("{result:#?}");
    assert!(
        result.valid,
        "корректный код не должен порождать ошибок, получили: {:#?}",
        result.errors
    );
}

#[test]
fn ignores_comments_and_strings() {
    let Some(path) = hbk_path() else { return };
    let index = load_from_hbk(&path).expect("PlatformIndex");

    // Внутри строки — несуществующее значение перечисления; внутри комментария —
    // вызов 'СтрНайти' с лишним числом аргументов. Оба должны игнорироваться.
    let src = "А = \"ТипРазмещенияТекстаТабличногоДокумента.Перенос\"; // СтрНайти(а,б,в,г,д,е)\n";
    let result = validate_expression(&index, src);
    println!("{result:#?}");
    assert!(
        result.valid,
        "не должно быть ошибок при работе со строками/комментариями"
    );
}
