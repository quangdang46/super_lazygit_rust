pub fn next_index(numbers: &[i32], current_number: i32) -> usize {
    for (index, &number) in numbers.iter().enumerate() {
        if number > current_number {
            return index;
        }
    }
    numbers.len().saturating_sub(1)
}

pub fn prev_index(numbers: &[i32], current_number: i32) -> usize {
    let end = numbers.len().saturating_sub(1);
    for i in (0..=end).rev() {
        if numbers[i] < current_number {
            return i;
        }
    }
    0
}

pub fn next_int_in_cycle(sl: &[i32], current: i32) -> i32 {
    for (i, &val) in sl.iter().enumerate() {
        if val == current {
            if i == sl.len() - 1 {
                return sl[0];
            }
            return sl[i + 1];
        }
    }
    sl[0]
}

pub fn prev_int_in_cycle(sl: &[i32], current: i32) -> i32 {
    for (i, &val) in sl.iter().enumerate() {
        if val == current {
            if i > 0 {
                return sl[i - 1];
            }
            return sl[sl.len() - 1];
        }
    }
    *sl.last().unwrap_or(&0)
}

pub fn string_arrays_overlap(str_arr_a: &[String], str_arr_b: &[String]) -> bool {
    for first in str_arr_a {
        if str_arr_b.contains(first) {
            return true;
        }
    }
    false
}

pub fn limit(values: &[String], limit: usize) -> Vec<String> {
    if values.len() > limit {
        values[..limit].to_vec()
    } else {
        values.to_vec()
    }
}

pub fn limit_str(value: &str, limit: usize) -> String {
    let mut n = 0;
    for (i, _) in value.char_indices() {
        if n >= limit {
            return value[..i].to_string();
        }
        n += 1;
    }
    value.to_string()
}

pub fn multi_group_by<T: Clone, K: std::hash::Hash + Eq, F>(
    slice: &[T],
    f: F,
) -> std::collections::HashMap<K, Vec<T>>
where
    F: Fn(&T) -> Vec<K>,
{
    let mut result = std::collections::HashMap::new();
    for item in slice {
        for key in f(item) {
            result
                .entry(key)
                .or_insert_with(Vec::new)
                .push(item.clone());
        }
    }
    result
}

pub fn move_element<T: Clone>(slice: &[T], from: usize, to: usize) -> Vec<T> {
    let mut new_slice = slice.to_vec();

    if from == to || from >= slice.len() || to >= slice.len() {
        return new_slice;
    }

    if from < to {
        new_slice[from..=to].rotate_left(1);
    } else {
        new_slice[to..=from].rotate_right(1);
    }

    new_slice
}

pub fn values_at_indices<T: Clone + Default>(slice: &[T], indices: &[usize]) -> Vec<T> {
    let mut result = vec![T::default(); indices.len()];
    for (i, &index) in indices.iter().enumerate() {
        if index < slice.len() {
            result[i] = slice[index].clone();
        }
    }
    result
}

pub fn partition<T: Clone>(slice: &[T], test: impl Fn(&T) -> bool) -> (Vec<T>, Vec<T>) {
    let mut left = Vec::with_capacity(slice.len());
    let mut right = Vec::with_capacity(slice.len());

    for value in slice {
        if test(value) {
            left.push(value.clone());
        } else {
            right.push(value.clone());
        }
    }

    (left, right)
}

pub fn prepend<T: Clone>(slice: &[T], values: &[T]) -> Vec<T> {
    let mut result = values.to_vec();
    result.extend_from_slice(slice);
    result
}

pub fn remove<T>(slice: &[T], index: usize) -> Vec<T> {
    if index >= slice.len() {
        return slice.to_vec();
    }
    let mut result = slice.to_vec();
    result.remove(index);
    result
}

pub fn move_item<T: Clone>(slice: &[T], from_index: usize, to_index: usize) -> Vec<T> {
    if from_index >= slice.len() || to_index >= slice.len() {
        return slice.to_vec();
    }
    let mut result = slice.to_vec();
    let item = result.remove(from_index);
    result.insert(to_index, item);
    result
}

pub fn pop<T: Clone>(slice: &[T]) -> Option<(T, Vec<T>)> {
    if slice.is_empty() {
        return None;
    }
    let index = slice.len() - 1;
    let value = slice[index].clone();
    let mut new_slice = slice.to_vec();
    new_slice.pop();
    Some((value, new_slice))
}

pub fn shift<T: Clone>(slice: &[T]) -> Option<(T, Vec<T>)> {
    if slice.is_empty() {
        return None;
    }
    let value = slice[0].clone();
    Some((value, slice[1..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_index() {
        let numbers = vec![1, 3, 5, 7];
        assert_eq!(next_index(&numbers, 2), 1);
        assert_eq!(next_index(&numbers, 3), 2);
        assert_eq!(next_index(&numbers, 8), 3);
    }

    #[test]
    fn test_prev_index() {
        let numbers = vec![1, 3, 5, 7];
        assert_eq!(prev_index(&numbers, 5), 1);
        assert_eq!(prev_index(&numbers, 8), 2);
        assert_eq!(prev_index(&numbers, 0), 0);
    }

    #[test]
    fn test_next_int_in_cycle() {
        let sl = vec![1, 2, 3];
        assert_eq!(next_int_in_cycle(&sl, 1), 2);
        assert_eq!(next_int_in_cycle(&sl, 3), 1);
        assert_eq!(next_int_in_cycle(&sl, 5), 1);
    }

    #[test]
    fn test_prev_int_in_cycle() {
        let sl = vec![1, 2, 3];
        assert_eq!(prev_int_in_cycle(&sl, 2), 1);
        assert_eq!(prev_int_in_cycle(&sl, 1), 3);
        assert_eq!(prev_int_in_cycle(&sl, 5), 3);
    }

    #[test]
    fn test_string_arrays_overlap() {
        let a = vec!["apple".to_string(), "banana".to_string()];
        let b = vec!["banana".to_string(), "cherry".to_string()];
        let c = vec!["date".to_string(), "elderberry".to_string()];

        assert!(string_arrays_overlap(&a, &b));
        assert!(!string_arrays_overlap(&a, &c));
    }

    #[test]
    fn test_limit() {
        let values = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(limit(&values, 2).len(), 2);
        assert_eq!(limit(&values, 5).len(), 3);
    }

    #[test]
    fn test_limit_str() {
        assert_eq!(limit_str("hello", 3), "hel");
        assert_eq!(limit_str("hi", 10), "hi");
        assert_eq!(limit_str("日本語", 2), "日本");
    }

    #[test]
    fn test_multi_group_by() {
        let items = vec![1, 2, 3, 4, 5, 6];
        let result = multi_group_by(&items, &|x| {
            if x % 2 == 0 {
                vec!["even"]
            } else {
                vec!["odd"]
            }
        });
        assert_eq!(result.get("even").unwrap().len(), 3);
        assert_eq!(result.get("odd").unwrap().len(), 3);
    }

    #[test]
    fn test_move_element_forward() {
        let slice = vec![1, 2, 3, 4, 5];
        let result = move_element(&slice, 1, 3);
        assert_eq!(result, vec![1, 3, 4, 2, 5]);
    }

    #[test]
    fn test_move_element_backward() {
        let slice = vec![1, 2, 3, 4, 5];
        let result = move_element(&slice, 3, 1);
        assert_eq!(result, vec![1, 4, 2, 3, 5]);
    }

    #[test]
    fn test_values_at_indices() {
        let slice = vec!['a', 'b', 'c', 'd'];
        let result = values_at_indices(&slice, &[0, 2, 4]);
        assert_eq!(result, vec!['a', 'c', '\0']);
    }

    #[test]
    fn test_partition() {
        let slice = vec![1, 2, 3, 4, 5, 6];
        let (evens, odds) = partition(&slice, &|x| x % 2 == 0);
        assert_eq!(evens, vec![2, 4, 6]);
        assert_eq!(odds, vec![1, 3, 5]);
    }

    #[test]
    fn test_prepend() {
        let slice = vec![3, 4];
        let result = prepend(&slice, &[1, 2]);
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_remove() {
        let slice = vec![1, 2, 3, 4];
        let result = remove(&slice, 2);
        assert_eq!(result, vec![1, 2, 4]);
    }

    #[test]
    fn test_move_item() {
        let slice = vec![1, 2, 3, 4];
        let result = move_item(&slice, 0, 3);
        assert_eq!(result, vec![2, 3, 4, 1]);
    }

    #[test]
    fn test_pop() {
        let slice = vec![1, 2, 3];
        let (value, rest) = pop(&slice).unwrap();
        assert_eq!(value, 3);
        assert_eq!(rest, vec![1, 2]);
    }

    #[test]
    fn test_pop_empty() {
        let slice: Vec<i32> = vec![];
        assert!(pop(&slice).is_none());
    }

    #[test]
    fn test_shift() {
        let slice = vec![1, 2, 3];
        let (value, rest) = shift(&slice).unwrap();
        assert_eq!(value, 1);
        assert_eq!(rest, vec![2, 3]);
    }

    #[test]
    fn test_shift_empty() {
        let slice: Vec<i32> = vec![];
        assert!(shift(&slice).is_none());
    }
}
