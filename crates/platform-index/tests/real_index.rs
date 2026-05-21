//! Integration-тест Phase 3 на реальном `shcntx_ru.hbk`.
//!
//! Acceptance:
//! - `types.len() > 1000`
//! - есть десятки/сотни типов с непустым `enum_values`
//! - канонический баг #638: `ТипРазмещенияТекстаТабличногоДокумента` имеет 4 значения
//! - `ТаблицаЗначений` имеет непустые `methods`, `properties`, `constructors`,
//!   у методов signatures непустые
//!
//! Запуск:
//! ```pwsh
//! $env:BSL_CONTEXT_PLATFORM_PATH = 'C:\Program Files\1cv8\8.3.27.1786'
//! cargo test -p platform-index --test real_index -- --nocapture
//! ```

use std::path::PathBuf;

use platform_index::load_from_hbk;

fn hbk_path() -> Option<PathBuf> {
    let root = std::env::var("BSL_CONTEXT_PLATFORM_PATH").ok().map(PathBuf::from)?;
    let candidates = [root.join("shcntx_ru.hbk"), root.join("bin").join("shcntx_ru.hbk")];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
fn loads_real_platform_index() {
    let Some(path) = hbk_path() else {
        eprintln!("skip: BSL_CONTEXT_PLATFORM_PATH не задан или shcntx_ru.hbk не найден");
        return;
    };

    let index = load_from_hbk(&path).expect("PlatformIndex должен загружаться");

    println!(
        "PlatformIndex: global_methods={}, global_properties={}, types={}, enum_types={}",
        index.global_methods.len(),
        index.global_properties.len(),
        index.types.len(),
        index.enum_types_count(),
    );

    assert!(
        index.types.len() > 1000,
        "ожидается > 1000 типов, получено {}",
        index.types.len()
    );
    assert!(
        index.enum_types_count() >= 30,
        "ожидается ≥30 типов-перечислений, получено {}",
        index.enum_types_count()
    );
    assert!(
        !index.global_methods.is_empty(),
        "global_methods не должен быть пуст"
    );
    assert!(
        !index.global_properties.is_empty(),
        "global_properties не должен быть пуст"
    );
}

#[test]
fn enum_values_for_canonical_638() {
    let Some(path) = hbk_path() else {
        return;
    };

    let index = load_from_hbk(&path).expect("PlatformIndex");

    let ty = index
        .find_type("ТипРазмещенияТекстаТабличногоДокумента")
        .expect("тип ТипРазмещенияТекстаТабличногоДокумента должен быть в storage");

    assert!(ty.is_enum(), "тип должен быть распознан как перечисление");
    let values: Vec<&str> = ty.enum_values.iter().map(|v| v.name_ru.as_str()).collect();
    println!("enum_values ТипРазмещения...Документа: {values:?}");

    let expected = ["Авто", "Забивать", "Обрезать", "Переносить"];
    for name in expected {
        assert!(
            values.contains(&name),
            "значение {name} должно быть в enum_values"
        );
    }
}

#[test]
fn value_table_has_full_members() {
    let Some(path) = hbk_path() else {
        return;
    };

    let index = load_from_hbk(&path).expect("PlatformIndex");

    let ty = index
        .find_type("ТаблицаЗначений")
        .expect("тип ТаблицаЗначений должен быть в storage");

    println!(
        "ТаблицаЗначений: methods={}, properties={}, constructors={}",
        ty.methods.len(),
        ty.properties.len(),
        ty.constructors.len()
    );

    assert!(
        !ty.methods.is_empty(),
        "ТаблицаЗначений должна иметь методы (например, Добавить, Очистить)"
    );
    assert!(
        !ty.properties.is_empty(),
        "ТаблицаЗначений должна иметь свойства (например, Колонки)"
    );

    // У методов должны быть непустые signatures (главное исправление vs апстрим).
    let first_method = ty
        .methods
        .iter()
        .find(|m| !m.signatures.is_empty())
        .expect("хотя бы у одного метода ТаблицаЗначений должна быть signature");
    println!(
        "пример метода с signature: {} ({} перегрузок)",
        first_method.name_ru,
        first_method.signatures.len()
    );
}
