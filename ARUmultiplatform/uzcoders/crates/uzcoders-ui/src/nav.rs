//! UZ: Navigatsiya steki: ekranlar (marshrutlar) ketma-ketligi. Holat o'zgarsa UI qayta quriladi.
//! EN: Navigation stack: a sequence of screens (routes). When the state changes, the UI is rebuilt.
//! RU: Стек навигации: последовательность экранов (маршрутов). При изменении состояния UI перестраивается.

use uzcoders_core::State;

/// UZ: `R` — ilovaning marshrut turi (odatda `enum`).
/// EN: `R` is the app's route type (usually an `enum`).
/// RU: `R` — тип маршрута приложения (обычно `enum`).
pub struct Navigator<R: Clone> {
    stack: State<Vec<R>>,
}

impl<R: Clone> Clone for Navigator<R> {
    fn clone(&self) -> Self {
        Navigator {
            stack: self.stack.clone(),
        }
    }
}

impl<R: Clone> Navigator<R> {
    pub fn new(root: R) -> Self {
        Navigator {
            stack: State::new(vec![root]),
        }
    }

    /// UZ: Hozirgi (eng yuqoridagi) ekran.
    /// EN: The current (topmost) screen.
    /// RU: Текущий (верхний) экран.
    pub fn current(&self) -> R {
        self.stack
            .with(|s| s.last().cloned().expect("stek bo'sh bo'lmaydi"))
    }

    pub fn push(&self, r: R) {
        self.stack.update(|s| s.push(r));
    }

    /// UZ: Joriy ekranni boshqasi bilan almashtiradi.
    /// EN: Replaces the current screen with another one.
    /// RU: Заменяет текущий экран другим.
    pub fn replace(&self, r: R) {
        self.stack.update(|s| {
            s.pop();
            s.push(r);
        });
    }

    /// UZ: Stekni tozalab, `r` ni yagona ekran qiladi.
    /// EN: Clears the stack and makes `r` the only screen.
    /// RU: Очищает стек и делает `r` единственным экраном.
    pub fn reset(&self, r: R) {
        self.stack.set(vec![r]);
    }

    /// UZ: Orqaga qaytadi: bitta ekran qolgan bo'lsa `false` qaytadi (tizim ilovani yopsin).
    /// EN: Goes back: returns `false` if only one screen is left (so the system can close the app).
    /// RU: Возвращается назад: если остался один экран — возвращает `false` (система закроет приложение).
    pub fn pop(&self) -> bool {
        if self.depth() <= 1 {
            return false;
        }
        self.stack.update(|s| {
            s.pop();
        });
        true
    }

    pub fn depth(&self) -> usize {
        self.stack.with(|s| s.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stek() {
        let n = Navigator::new("a");
        assert!(!n.pop());
        n.push("b");
        n.push("c");
        assert_eq!(n.current(), "c");
        assert!(n.pop());
        assert_eq!(n.current(), "b");
        n.replace("x");
        assert_eq!((n.current(), n.depth()), ("x", 2));
        n.reset("z");
        assert_eq!((n.current(), n.depth()), ("z", 1));
    }
}
