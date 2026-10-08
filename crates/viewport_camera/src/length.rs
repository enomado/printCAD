use serde::{Deserialize, Serialize};

/// Длина в единицах пространства хозяина: расстояние вдоль луча, интервал отсечения,
/// глубина камеры. Конечна и не отрицательна.
///
/// Глубины камеры лежат в сохраняемых настройках (`NavigationLimits`), поэтому формат —
/// голое число (`serde(transparent)`). Десериализация
/// `new` не вызывает: значение из файла проверяет `NavigationLimits::validate`.
///
/// Арифметики пока нет намеренно: её добавляем, когда она понадобится коду библиотеки.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Length(pub f64);

impl Length {
    pub const ZERO: Self = Self::new(0.0);

    pub const fn new(value: f64) -> Self {
        assert!(
            value.is_finite() && value >= 0.0,
            "Length: must be finite and non-negative"
        );
        Self(value)
    }
}

/// Знаковая длина: значение сустава-сдвига, смещение вдоль оси.
/// Конечна, знак — направление вдоль оси. У [`Length`] инвариант `≥ 0`, для сдвига он
/// не годится.
///
/// Арифметики пока нет намеренно, как у [`Length`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Offset(pub f64);

impl Offset {
    pub const ZERO: Self = Self::new(0.0);

    pub const fn new(value: f64) -> Self {
        assert!(value.is_finite(), "Offset: must be finite");
        Self(value)
    }
}
