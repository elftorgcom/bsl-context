//! Phase 8 MVP — локальный type inference в пределах одной процедуры.
//!
//! Собирает scope `Map<имя_переменной_lower, ТипX>` из трёх источников:
//!
//! 1. `Х = Новый ТипX` или `Х = Новый ТипX(args)` → переменная Х имеет тип `ТипX`.
//! 2. `Х = ТипX.ЗначениеY` (где `ТипX` есть в `PlatformIndex.types` как enum)
//!    → переменная Х имеет тип `ТипX` (не значение перечисления, а сам тип).
//! 3. `// @type ТипX` на строке непосредственно перед присваиванием
//!    `Х = <выражение>` (или в той же строке) → переменная Х получает тип `ТипX`.
//!    Аннотация переопределяет автоматический вывод.
//!
//! Не покрывает (это пост-MVP / Уровень 3):
//! - `Х = ИмяГлобальногоМетода(...)` → return-type метода (планируется в Phase 8).
//! - `Х = ИмяТипа.ЗначениеY` для не-enum типов.
//! - Inter-procedural type inference (вывод типа параметра процедуры по местам вызова).
//!
//! Сегментация на процедуры — простой regex по `Процедура`/`Функция` ...
//! `КонецПроцедуры`/`КонецФункции`. В BSL вложенных процедур нет — поэтому
//! линейного сканирования достаточно.

use std::collections::HashMap;

use regex::Regex;
use std::sync::OnceLock;

use platform_index::PlatformIndex;

/// Один scope — набор `имя_переменной_lower → ТипX` в пределах одной процедуры
/// (или модуля, если процедур нет).
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Включающий байтовый диапазон `[start..end)`.
    pub byte_start: usize,
    pub byte_end: usize,
    pub vars: HashMap<String, String>,
}

impl Scope {
    pub fn contains(&self, byte_idx: usize) -> bool {
        byte_idx >= self.byte_start && byte_idx < self.byte_end
    }
}

/// Контейнер: scope-ы по процедурам в порядке появления в исходнике.
#[derive(Debug, Clone, Default)]
pub struct ScopeMap {
    pub scopes: Vec<Scope>,
}

impl ScopeMap {
    /// Найти scope, охватывающий данный байтовый offset.
    pub fn lookup(&self, byte_idx: usize) -> Option<&Scope> {
        self.scopes.iter().find(|s| s.contains(byte_idx))
    }

    /// Получить тип переменной по имени, регистронезависимо.
    pub fn type_of_var(&self, byte_idx: usize, var_name: &str) -> Option<&String> {
        let scope = self.lookup(byte_idx)?;
        scope.vars.get(&var_name.to_lowercase())
    }
}

fn proc_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // (?is) — case-insensitive + dot matches newline.
        Regex::new(
            r"(?is)(?P<head>(?:Процедура|Функция)\s+\w+\s*\([^)]*\))(?P<body>.*?)(?P<tail>КонецПроцедуры|КонецФункции)",
        )
        .unwrap()
    })
}

fn assign_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Имя на левой стороне присваивания. Ловим "Идентификатор = ..." с возможным
        // префиксом из пробелов в начале строки. Lookbehind в regex crate нет,
        // поэтому используем (?m:^) и проверяем границы вручную.
        Regex::new(r"(?m:^)\s*(?P<lhs>[A-Za-zА-Яа-яЁё_][A-Za-zА-Яа-яЁё_0-9]*)\s*=\s*(?P<rhs>[^;\n]*)")
            .unwrap()
    })
}

fn new_rhs_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(?:Новый|New)\s+(?P<ty>[A-Za-zА-Яа-яЁё_][A-Za-zА-Яа-яЁё_0-9]*)").unwrap()
    })
}

fn enum_rhs_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*(?P<ty>[A-Za-zА-Яа-яЁё_][A-Za-zА-Яа-яЁё_0-9]*)\.(?P<member>[A-Za-zА-Яа-яЁё_][A-Za-zА-Яа-яЁё_0-9]*)\s*$",
        )
        .unwrap()
    })
}

fn type_annot_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `// @type ТипX` или `// @type: ТипX`
        Regex::new(r"//\s*@type:?\s+(?P<ty>[A-Za-zА-Яа-яЁё_][A-Za-zА-Яа-яЁё_0-9]*)").unwrap()
    })
}

/// Извлечь scope из исходника. На вход — уже очищенный от строк/комментариев
/// текст (но аннотации `// @type` извлекаются ДО очистки и подаются отдельно).
pub fn extract_scope_map(
    index: &PlatformIndex,
    cleaned: &str,
    annotations: &HashMap<usize, String>,
) -> ScopeMap {
    let mut scopes = Vec::new();
    let blocks: Vec<(usize, usize)> = proc_block_re()
        .find_iter(cleaned)
        .map(|m| (m.start(), m.end()))
        .collect();

    if blocks.is_empty() {
        // Глобальный scope на весь файл.
        let scope = build_scope(index, cleaned, 0, cleaned.len(), annotations);
        scopes.push(scope);
    } else {
        for (start, end) in blocks {
            let body = &cleaned[start..end];
            let scope = build_scope(index, body, start, end, annotations);
            scopes.push(scope);
        }
    }

    ScopeMap { scopes }
}

/// Извлечь все аннотации `// @type ТипX` из ИСХОДНОГО (не очищенного) текста.
/// Возвращает `byte_offset_следующей_строки → ТипX`. Аннотация применяется к
/// первому присваиванию на строке annotation_line+1 или дальше (до пустой строки).
pub fn extract_type_annotations(src: &str) -> HashMap<usize, String> {
    let mut out = HashMap::new();
    for cap in type_annot_re().captures_iter(src) {
        let ty = cap.name("ty").unwrap().as_str().to_string();
        let m = cap.get(0).unwrap();
        // Найти конец строки, где аннотация
        let end_of_line = src[m.end()..]
            .find('\n')
            .map(|i| m.end() + i + 1)
            .unwrap_or(src.len());
        out.insert(end_of_line, ty);
    }
    out
}

fn build_scope(
    index: &PlatformIndex,
    body: &str,
    byte_start: usize,
    byte_end: usize,
    annotations: &HashMap<usize, String>,
) -> Scope {
    let mut vars: HashMap<String, String> = HashMap::new();

    for cap in assign_re().captures_iter(body) {
        let lhs_match = cap.name("lhs").unwrap();
        let rhs_match = cap.name("rhs").unwrap();
        let lhs = lhs_match.as_str().to_string();
        let rhs = rhs_match.as_str().trim();

        let abs_start = byte_start + lhs_match.start();

        // 1. Проверить аннотацию: ищем последнюю аннотацию, чей end_of_line <= abs_start
        // и абсолютная разница не больше ~200 байт (примерно 4 строки).
        let mut typ: Option<String> = None;
        for (&annot_end, annot_ty) in annotations {
            if annot_end <= abs_start && abs_start.saturating_sub(annot_end) <= 200 {
                // Используем последнюю
                if typ.is_none() {
                    typ = Some(annot_ty.clone());
                }
            }
        }

        // 2. Если аннотации нет — пробуем извлечь из RHS.
        if typ.is_none() {
            if let Some(c) = new_rhs_re().captures(rhs) {
                let ty = c.name("ty").unwrap().as_str();
                if index.find_type(ty).is_some() {
                    typ = Some(ty.to_string());
                }
            }
        }
        if typ.is_none() {
            if let Some(c) = enum_rhs_re().captures(rhs) {
                let ty = c.name("ty").unwrap().as_str();
                if let Some(t) = index.find_type(ty) {
                    if t.is_enum() {
                        typ = Some(ty.to_string());
                    }
                }
            }
        }

        if let Some(t) = typ {
            vars.insert(lhs.to_lowercase(), t);
        }
    }

    Scope {
        byte_start,
        byte_end,
        vars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_var(map: &ScopeMap, byte_idx: usize, var: &str, expected_type: &str) {
        let t = map
            .type_of_var(byte_idx, var)
            .unwrap_or_else(|| panic!("var '{var}' not found in scope"));
        assert_eq!(t, expected_type);
    }

    #[test]
    fn extract_annotations_finds_type_directive() {
        let src = "// @type ТаблицаЗначений\nХ = СоздатьТЗ();\n";
        let annot = extract_type_annotations(src);
        assert_eq!(annot.values().next(), Some(&"ТаблицаЗначений".to_string()));
    }
}
