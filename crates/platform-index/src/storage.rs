//! `PlatformIndex` — central storage с тремя коллекциями.
//!
//! Иерархия (правильная для 1С): системное перечисление это разновидность типа,
//! а не отдельная категория. Поэтому `types` — единый словарь, в котором
//! и обычные типы, и перечисления (последние с непустым `enum_values`).

use std::collections::HashMap;

use crate::entities::{Method, Property, Type};

/// Storage платформенного контекста (read-only после загрузки).
#[derive(Debug, Default, Clone)]
pub struct PlatformIndex {
    pub global_methods: Vec<Method>,
    pub global_properties: Vec<Property>,
    /// Ключ — `name_ru` в нижнем регистре. Тип-перечисление и обычный тип лежат вместе.
    pub types: HashMap<String, Type>,
}

impl PlatformIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Сколько типов, у которых заполнен `enum_values` (системные перечисления).
    pub fn enum_types_count(&self) -> usize {
        self.types.values().filter(|t| t.is_enum()).count()
    }

    /// Точный поиск типа по русскому имени (регистронезависимо).
    pub fn find_type(&self, name_ru: &str) -> Option<&Type> {
        self.types.get(&name_ru.to_lowercase())
    }

    /// Точный поиск глобального метода по русскому имени (регистронезависимо).
    pub fn find_global_method(&self, name_ru: &str) -> Option<&Method> {
        let key = name_ru.to_lowercase();
        self.global_methods
            .iter()
            .find(|m| m.name_ru.to_lowercase() == key)
    }

    /// Точный поиск глобального свойства по русскому имени (регистронезависимо).
    pub fn find_global_property(&self, name_ru: &str) -> Option<&Property> {
        let key = name_ru.to_lowercase();
        self.global_properties
            .iter()
            .find(|p| p.name_ru.to_lowercase() == key)
    }

    /// Вставка типа в storage. Перезаписывает по ключу `name_ru.lowercase()`.
    pub fn insert_type(&mut self, ty: Type) {
        let key = ty.name_ru.to_lowercase();
        self.types.insert(key, ty);
    }
}
