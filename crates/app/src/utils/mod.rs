use std::collections::VecDeque;

pub struct HistoryBuffer<T> {
    max_size: usize,
    items: VecDeque<T>,
}

impl<T> HistoryBuffer<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            items: VecDeque::with_capacity(max_size),
        }
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() == self.max_size {
            self.items.pop_back();
        }
        self.items.push_front(item);
    }

    pub fn peek(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_buffer_push_and_peek() {
        let mut buffer = HistoryBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.peek(0), Some(&3));
        assert_eq!(buffer.peek(1), Some(&2));
        assert_eq!(buffer.peek(2), Some(&1));
    }

    #[test]
    fn test_history_buffer_max_size() {
        let mut buffer = HistoryBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.peek(0), Some(&4));
        assert_eq!(buffer.peek(2), Some(&2));
        assert_eq!(buffer.peek(3), None);
    }

    #[test]
    fn test_history_buffer_empty() {
        let buffer: HistoryBuffer<i32> = HistoryBuffer::new(3);
        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(0), None);
    }
}
