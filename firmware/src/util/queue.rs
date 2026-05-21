pub struct DropOldestQueue<T, const N: usize> {
    buf: [Option<T>; N],
    head: usize,
    len: usize,
}

impl<T, const N: usize> DropOldestQueue<T, N> {
    pub const fn new() -> Self {
        Self {
            buf: [const { None }; N],
            head: 0,
            len: 0,
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    pub fn push(&mut self, item: T) -> Option<T> {
        if N == 0 {
            return Some(item);
        }

        if self.len == N {
            let dropped = self.buf[self.head].replace(item);
            self.head = (self.head + 1) % N;
            dropped
        } else {
            let index = (self.head + self.len) % N;
            self.buf[index] = Some(item);
            self.len += 1;
            None
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 || N == 0 {
            return None;
        }

        let item = self.buf[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        item
    }

    pub fn front(&self) -> Option<&T> {
        if self.len == 0 || N == 0 {
            return None;
        }

        self.buf[self.head].as_ref()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len || N == 0 {
            return None;
        }

        self.buf[(self.head + index) % N].as_ref()
    }
}

impl<T, const N: usize> Default for DropOldestQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_queue_pop_returns_none() {
        let mut queue = DropOldestQueue::<u8, 2>::new();

        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn normal_push_pop_order() {
        let mut queue = DropOldestQueue::<u8, 3>::new();

        assert_eq!(queue.push(1), None);
        assert_eq!(queue.push(2), None);
        assert_eq!(queue.push(3), None);

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn front_returns_oldest_without_removing_it() {
        let mut queue = DropOldestQueue::<u8, 2>::new();

        assert_eq!(queue.front(), None);

        queue.push(1);
        queue.push(2);

        assert_eq!(queue.front(), Some(&1));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.front(), Some(&2));
    }

    #[test]
    fn get_returns_items_in_fifo_order() {
        let mut queue = DropOldestQueue::<u8, 3>::new();

        queue.push(1);
        queue.push(2);
        queue.push(3);
        queue.pop();
        queue.push(4);

        assert_eq!(queue.get(0), Some(&2));
        assert_eq!(queue.get(1), Some(&3));
        assert_eq!(queue.get(2), Some(&4));
        assert_eq!(queue.get(3), None);
    }

    #[test]
    fn full_queue_drops_oldest() {
        let mut queue = DropOldestQueue::<u8, 2>::new();

        assert_eq!(queue.push(1), None);
        assert_eq!(queue.push(2), None);
        assert_eq!(queue.push(3), Some(1));

        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn repeated_overflow_does_not_panic() {
        let mut queue = DropOldestQueue::<u8, 2>::new();

        for value in 0..10 {
            queue.push(value);
        }

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop(), Some(8));
        assert_eq!(queue.pop(), Some(9));
    }

    #[test]
    fn zero_capacity_queue_drops_everything() {
        let mut queue = DropOldestQueue::<u8, 0>::new();

        assert_eq!(queue.push(1), Some(1));
        assert_eq!(queue.pop(), None);
        assert_eq!(queue.len(), 0);
    }
}
