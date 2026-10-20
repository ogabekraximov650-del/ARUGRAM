//! Navigatsiya steki: ekranlar (marshrutlar) ketma-ketligi. Holat o'zgarsa UI qayta quriladi.

use uzcoders_core::State;

/// `R` — ilovaning marshrut turi (odatda `enum`).
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

    /// Hozirgi (eng yuqoridagi) ekran.
    pub fn current(&self) -> R {
        self.stack
            .with(|s| s.last().cloned().expect("stek bo'sh bo'lmaydi"))
    }

    pub fn push(&self, r: R) {
        self.stack.update(|s| s.push(r));
    }

    /// Joriy ekranni almashtiradi.
    pub fn replace(&self, r: R) {
        self.stack.update(|s| {
            s.pop();
            s.push(r);
        });
    }

    /// Stekni tozalab, `r` ni yagona ekran qiladi.
    pub fn reset(&self, r: R) {
        self.stack.set(vec![r]);
    }

    /// Orqaga: bitta ekran qolgan bo'lsa `false` (tizim ilovani yopsin).
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
