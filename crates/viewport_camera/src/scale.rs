use serde::{Deserialize, Serialize};

/// Масштаб вида: единиц пространства хозяина на один логический пиксель экрана.
/// Строго положителен и конечен.
///
/// Лежит в сохраняемых настройках камеры (`crate::navigation::NavigationLimits`),
/// поэтому формат — голое число (`serde(transparent)`). Десериализация `new` не вызывает; значение из
/// файла проверяет `NavigationLimits::validate` на границе файла.
///
/// Арифметики пока нет намеренно: её добавляем, когда она понадобится коду библиотеки.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PerPx(pub f64);

impl PerPx {
    pub const fn new(value: f64) -> Self {
        assert!(
            value.is_finite() && value > 0.0,
            "PerPx: scale must be finite and positive"
        );
        Self(value)
    }
}
