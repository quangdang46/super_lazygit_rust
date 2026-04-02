#![allow(dead_code)]

use ratatui::layout::Alignment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NonModelItem {
    pub(crate) index: usize,
    pub(crate) content: String,
    pub(crate) column: usize,
}

pub(crate) struct ListRenderer {
    model_len: usize,
    get_display_strings: Box<dyn Fn(usize, usize) -> Vec<Vec<String>>>,
    get_column_alignments: Option<Box<dyn Fn() -> Vec<Alignment>>>,
    get_non_model_items: Option<Box<dyn Fn() -> Vec<NonModelItem>>>,
    num_non_model_items: usize,
    view_indices_by_model_index: Vec<usize>,
    model_indices_by_view_index: Vec<usize>,
    column_positions: Vec<usize>,
}

impl ListRenderer {
    pub(crate) fn new(
        model_len: usize,
        get_display_strings: impl Fn(usize, usize) -> Vec<Vec<String>> + 'static,
    ) -> Self {
        Self {
            model_len,
            get_display_strings: Box::new(get_display_strings),
            get_column_alignments: None,
            get_non_model_items: None,
            num_non_model_items: 0,
            view_indices_by_model_index: Vec::new(),
            model_indices_by_view_index: Vec::new(),
            column_positions: Vec::new(),
        }
    }

    pub(crate) fn with_column_alignments(
        mut self,
        get_column_alignments: impl Fn() -> Vec<Alignment> + 'static,
    ) -> Self {
        self.get_column_alignments = Some(Box::new(get_column_alignments));
        self
    }

    pub(crate) fn with_non_model_items(
        mut self,
        get_non_model_items: impl Fn() -> Vec<NonModelItem> + 'static,
    ) -> Self {
        self.get_non_model_items = Some(Box::new(get_non_model_items));
        self
    }

    pub(crate) fn model_index_to_view_index(&self, model_index: isize) -> usize {
        let clamped = model_index.clamp(0, self.model_len as isize) as usize;
        if self.view_indices_by_model_index.is_empty() {
            clamped
        } else {
            self.view_indices_by_model_index[clamped]
        }
    }

    pub(crate) fn view_index_to_model_index(&self, view_index: isize) -> usize {
        let max = (self.model_len + self.num_non_model_items) as isize;
        let clamped = view_index.clamp(0, max) as usize;
        if self.model_indices_by_view_index.is_empty() {
            clamped
        } else {
            self.model_indices_by_view_index[clamped]
        }
    }

    pub(crate) fn column_positions(&self) -> &[usize] {
        &self.column_positions
    }

    pub(crate) fn render_lines(&mut self, start_idx: isize, end_idx: isize) -> String {
        let column_alignments = self
            .get_column_alignments
            .as_ref()
            .map(|callback| callback())
            .unwrap_or_default();
        let non_model_items = self
            .get_non_model_items
            .as_ref()
            .map(|callback| callback())
            .unwrap_or_default();
        self.num_non_model_items = non_model_items.len();
        if !non_model_items.is_empty() {
            self.prepare_conversion_arrays(&non_model_items);
        }

        let start_idx = if start_idx < 0 { 0 } else { start_idx as usize };
        let mut start_model_idx = 0;
        if start_idx > 0 {
            start_model_idx = self.view_index_to_model_index(start_idx as isize);
        }

        let end_idx = if end_idx < 0 {
            self.model_len + non_model_items.len()
        } else {
            end_idx as usize
        };
        let mut end_model_idx = self.model_len;
        if end_idx < self.model_len + non_model_items.len() {
            end_model_idx = self.view_index_to_model_index(end_idx as isize);
        }

        let display_strings = (self.get_display_strings)(start_model_idx, end_model_idx);
        let (mut lines, positions) = render_display_strings(&display_strings, &column_alignments);
        self.column_positions = positions.clone();
        lines = insert_non_model_items(non_model_items, end_idx, start_idx, lines, &positions);
        lines.join("\n")
    }

    fn prepare_conversion_arrays(&mut self, non_model_items: &[NonModelItem]) {
        let mut view_indices_by_model_index = (0..=self.model_len).collect::<Vec<_>>();
        let mut model_indices_by_view_index = (0..=self.model_len).collect::<Vec<_>>();
        for (offset, item) in non_model_items.iter().enumerate() {
            for value in view_indices_by_model_index
                .iter_mut()
                .take(self.model_len + 1)
                .skip(item.index)
            {
                *value += 1;
            }
            model_indices_by_view_index.insert(
                item.index + offset,
                model_indices_by_view_index[item.index + offset],
            );
        }
        self.view_indices_by_model_index = view_indices_by_model_index;
        self.model_indices_by_view_index = model_indices_by_view_index;
    }
}

fn render_display_strings(
    rows: &[Vec<String>],
    alignments: &[Alignment],
) -> (Vec<String>, Vec<usize>) {
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; column_count];
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.chars().count());
        }
    }

    let mut positions = vec![0usize; column_count];
    let mut current = 0usize;
    for (index, width) in widths.iter().enumerate() {
        positions[index] = current;
        current += width + 1;
    }

    let lines = rows
        .iter()
        .map(|row| {
            let mut line = String::new();
            for (index, width) in widths.iter().copied().enumerate().take(column_count) {
                if index > 0 {
                    line.push(' ');
                }
                let value = row.get(index).cloned().unwrap_or_default();
                let alignment = alignments.get(index).copied().unwrap_or(Alignment::Left);
                match alignment {
                    Alignment::Right => {
                        line.push_str(&format!("{value:>width$}"));
                    }
                    _ => {
                        line.push_str(&format!("{value:<width$}"));
                    }
                }
            }
            line.trim_end().to_string()
        })
        .collect();

    (lines, positions)
}

fn insert_non_model_items(
    non_model_items: Vec<NonModelItem>,
    end_idx: usize,
    start_idx: usize,
    mut lines: Vec<String>,
    column_positions: &[usize],
) -> Vec<String> {
    for (offset, item) in non_model_items.into_iter().enumerate() {
        if item.index + offset >= end_idx {
            break;
        }
        if item.index + offset >= start_idx {
            let padding = column_positions
                .get(item.column)
                .map(|value| " ".repeat(*value))
                .unwrap_or_default();
            lines.insert(
                item.index + offset - start_idx,
                format!("{padding}{}", item.content),
            );
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lines_matches_upstream_sections_and_ranges() {
        let scenarios = [
            (
                vec!["a", "b", "c"],
                Vec::<usize>::new(),
                0,
                3,
                "a\nb\nc",
            ),
            (
                vec!["a", "b", "c"],
                vec![1, 3],
                0,
                5,
                "a\n--- 1 (0) ---\nb\nc\n--- 3 (1) ---",
            ),
            (
                vec!["a", "b", "c"],
                vec![0, 0, 2, 2, 2],
                0,
                8,
                "--- 0 (0) ---\n--- 0 (1) ---\na\nb\n--- 2 (2) ---\n--- 2 (3) ---\n--- 2 (4) ---\nc",
            ),
        ];

        for (model_strings, header_indices, start_idx, end_idx, expected) in scenarios {
            let model = model_strings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let mut renderer = ListRenderer::new(model.len(), move |start, end| {
                model[start..end]
                    .iter()
                    .map(|value| vec![value.clone()])
                    .collect()
            })
            .with_non_model_items(move || {
                header_indices
                    .iter()
                    .enumerate()
                    .map(|(header_index, model_index)| NonModelItem {
                        index: *model_index,
                        content: format!("--- {model_index} ({header_index}) ---"),
                        column: 0,
                    })
                    .collect()
            });

            assert_eq!(renderer.render_lines(start_idx, end_idx), expected);
        }
    }

    #[test]
    fn model_and_view_indices_match_upstream_conversion_rules() {
        let mut renderer = ListRenderer::new(3, |start, end| {
            (start..end)
                .map(|index| vec![index.to_string()])
                .collect::<Vec<_>>()
        })
        .with_non_model_items(|| {
            vec![
                NonModelItem {
                    index: 1,
                    content: String::new(),
                    column: 0,
                },
                NonModelItem {
                    index: 2,
                    content: String::new(),
                    column: 0,
                },
            ]
        });
        let _ = renderer.render_lines(-1, -1);

        let model_indices = [-1, 0, 1, 2, 3, 4];
        let expected_view_indices = [0, 0, 2, 4, 5, 5];
        for (model_index, expected_view_index) in
            model_indices.into_iter().zip(expected_view_indices)
        {
            assert_eq!(
                renderer.model_index_to_view_index(model_index),
                expected_view_index
            );
        }

        let view_indices = [-1, 0, 1, 2, 3, 4, 5, 6];
        let expected_model_indices = [0, 0, 1, 1, 2, 2, 3, 3];
        for (view_index, expected_model_index) in
            view_indices.into_iter().zip(expected_model_indices)
        {
            assert_eq!(
                renderer.view_index_to_model_index(view_index),
                expected_model_index
            );
        }
    }
}
