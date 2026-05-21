//! Phase 5: точечные валидаторы по точному имени.
//!
//! - [`validate_enum`] — проверка `<ТипX>.<ЗначениеY>` против `PlatformIndex.types`.
//! - [`validate_method_call`] — проверка вызова глобального метода
//!   (число аргументов + наличие именованных параметров).
//!
//! Возвращают структуры с булевым `valid`, списком похожих значений и
//! явным человеко-читаемым сообщением об ошибке (для модели). Без парсинга
//! BSL — это уровень MCP-tool, на вход приходят уже извлечённые имена.

use serde::Serialize;

use platform_index::{PlatformIndex, Signature};

/// Подсказка похожего значения. Сортируется по убыванию `score`
/// (расстояние Левенштейна, инвертированное в [0..1]).
#[derive(Debug, Clone, Serialize)]
pub struct SimilarValue {
    pub name: String,
    pub score: f32,
}

/// Краткое описание сигнатуры для возврата клиенту (без полных описаний).
#[derive(Debug, Clone, Serialize)]
pub struct SignatureBrief {
    pub name: String,
    pub min_args: usize,
    pub max_args: usize,
    pub formatted: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnumValidation {
    pub valid: bool,
    pub type_name: String,
    pub value_name: String,
    /// Все легальные значения у типа (для подсказки модели). Пуст, когда тип не enum или не найден.
    pub all_valid_values: Vec<String>,
    /// Топ-5 ближайших по Левенштейну значений (только при `valid=false`).
    pub similar: Vec<SimilarValue>,
    /// Удобочитаемый текст. Включается всегда (и при ok, и при ошибке).
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MethodCallValidation {
    pub valid: bool,
    pub method_name: String,
    pub arg_count: usize,
    /// Все известные сигнатуры метода. Когда метод не найден — пуст.
    pub signatures: Vec<SignatureBrief>,
    pub message: String,
}

/// Проверить значение системного перечисления.
pub fn validate_enum(index: &PlatformIndex, type_name: &str, value_name: &str) -> EnumValidation {
    let Some(ty) = index.find_type(type_name) else {
        return EnumValidation {
            valid: false,
            type_name: type_name.to_string(),
            value_name: value_name.to_string(),
            all_valid_values: Vec::new(),
            similar: Vec::new(),
            message: format!("❌ Тип '{type_name}' не найден в платформенном контексте."),
        };
    };
    if !ty.is_enum() {
        return EnumValidation {
            valid: false,
            type_name: ty.name_ru.clone(),
            value_name: value_name.to_string(),
            all_valid_values: Vec::new(),
            similar: Vec::new(),
            message: format!("❌ Тип '{}' не является системным перечислением.", ty.name_ru),
        };
    }

    let value_lower = value_name.to_lowercase();
    let valid = ty
        .enum_values
        .iter()
        .any(|v| v.name_ru.to_lowercase() == value_lower || v.name_en.to_lowercase() == value_lower);

    let all_valid_values: Vec<String> = ty.enum_values.iter().map(|v| v.name_ru.clone()).collect();

    if valid {
        EnumValidation {
            valid: true,
            type_name: ty.name_ru.clone(),
            value_name: value_name.to_string(),
            all_valid_values,
            similar: Vec::new(),
            message: format!(
                "✅ Значение '{}' допустимо для типа '{}'.",
                value_name, ty.name_ru
            ),
        }
    } else {
        let similar = top_similar(&value_lower, &ty.enum_values, 5);
        let suggestion = similar
            .first()
            .map(|s| format!(" Похожее: '{}'.", s.name))
            .unwrap_or_default();
        EnumValidation {
            valid: false,
            type_name: ty.name_ru.clone(),
            value_name: value_name.to_string(),
            all_valid_values,
            similar,
            message: format!(
                "❌ Значение '{}' не существует у типа '{}'.{}",
                value_name, ty.name_ru, suggestion
            ),
        }
    }
}

/// Проверить вызов глобального метода: число аргументов попадает в диапазон [min..=max]
/// хотя бы одной перегрузки. Если метод не найден — `valid=false`.
pub fn validate_method_call(
    index: &PlatformIndex,
    method_name: &str,
    arg_count: usize,
) -> MethodCallValidation {
    let Some(method) = index.find_global_method(method_name) else {
        return MethodCallValidation {
            valid: false,
            method_name: method_name.to_string(),
            arg_count,
            signatures: Vec::new(),
            message: format!(
                "❌ Глобальный метод '{method_name}' не найден в платформенном контексте."
            ),
        };
    };

    let signatures: Vec<SignatureBrief> = method.signatures.iter().map(brief_signature).collect();
    if signatures.is_empty() {
        // Метод без описанной сигнатуры — формально не можем проверить число аргументов.
        return MethodCallValidation {
            valid: true,
            method_name: method.name_ru.clone(),
            arg_count,
            signatures,
            message: format!(
                "⚠️ У метода '{}' нет описанных сигнатур — число аргументов не проверено.",
                method.name_ru
            ),
        };
    }

    let any_match = signatures
        .iter()
        .any(|s| arg_count >= s.min_args && arg_count <= s.max_args);

    if any_match {
        MethodCallValidation {
            valid: true,
            method_name: method.name_ru.clone(),
            arg_count,
            signatures,
            message: format!(
                "✅ Вызов '{}' с {} аргументами допустим.",
                method.name_ru, arg_count
            ),
        }
    } else {
        let allowed_ranges = signatures
            .iter()
            .map(|s| {
                if s.min_args == s.max_args {
                    format!("{}", s.min_args)
                } else {
                    format!("{}..{}", s.min_args, s.max_args)
                }
            })
            .collect::<Vec<_>>()
            .join(" / ");
        MethodCallValidation {
            valid: false,
            method_name: method.name_ru.clone(),
            arg_count,
            signatures,
            message: format!(
                "❌ Метод '{}' не принимает {} аргументов. Допустимо: {}.",
                method.name_ru, arg_count, allowed_ranges
            ),
        }
    }
}

fn brief_signature(s: &Signature) -> SignatureBrief {
    let max_args = s.parameters.len();
    let min_args = s.parameters.iter().filter(|p| p.required).count();
    let formatted = s
        .parameters
        .iter()
        .map(|p| {
            format!(
                "{}{}: {}",
                p.name,
                if p.required { "" } else { "?" },
                p.type_name
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    SignatureBrief {
        name: s.name.clone(),
        min_args,
        max_args,
        formatted,
    }
}

fn top_similar(query: &str, values: &[platform_index::EnumValue], top: usize) -> Vec<SimilarValue> {
    let mut scored: Vec<(f32, &str)> = values
        .iter()
        .flat_map(|v| {
            [v.name_ru.as_str(), v.name_en.as_str()]
                .into_iter()
                .filter(|n| !n.is_empty())
                .map(move |n| (similarity_score(query, &n.to_lowercase()), v.name_ru.as_str()))
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    scored
        .into_iter()
        .filter(|(_, n)| seen.insert(n.to_lowercase()))
        .take(top)
        .map(|(s, n)| SimilarValue {
            name: n.to_string(),
            score: s,
        })
        .collect()
}

/// Расстояние Левенштейна, нормированное в [0..=1] (1 = полное совпадение).
fn similarity_score(a: &str, b: &str) -> f32 {
    let max_len = a.chars().count().max(b.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a, b) as f32;
    1.0 - (dist / max_len as f32)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let (n, m) = (av.len(), bv.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if av[i - 1] == bv[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1) // вставка
                .min(prev[j] + 1) // удаление
                .min(prev[j - 1] + cost); // замена
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_index::EnumValue;

    fn enum_v(ru: &str) -> EnumValue {
        EnumValue {
            name_ru: ru.to_string(),
            name_en: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("Перенос", "Переносить"), 3);
        assert_eq!(levenshtein("Авто", "Авто"), 0);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn similarity_finds_closest_value() {
        let values = vec![
            enum_v("Авто"),
            enum_v("Забивать"),
            enum_v("Обрезать"),
            enum_v("Переносить"),
        ];
        let top = top_similar("перенос", &values, 3);
        assert_eq!(top[0].name, "Переносить");
    }
}
