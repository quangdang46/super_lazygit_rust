use crate::patch_exploring::state::SelectMode;

pub fn calculate_origin(
    current_origin: i32,
    buffer_height: i32,
    num_lines: i32,
    first_line_idx: i32,
    last_line_idx: i32,
    selected_line_idx: i32,
    mode: SelectMode,
) -> i32 {
    let (need_to_see_idx, want_to_see_idx) =
        get_need_and_want_line_idx(first_line_idx, last_line_idx, selected_line_idx, mode);

    calculate_new_origin_with_needed_and_wanted_idx(
        current_origin,
        buffer_height,
        num_lines,
        need_to_see_idx,
        want_to_see_idx,
    )
}

fn calculate_new_origin_with_needed_and_wanted_idx(
    current_origin: i32,
    buffer_height: i32,
    num_lines: i32,
    need_to_see_idx: i32,
    want_to_see_idx: i32,
) -> i32 {
    let mut origin = current_origin;

    if need_to_see_idx < current_origin || need_to_see_idx >= current_origin + buffer_height {
        origin = (need_to_see_idx - buffer_height / 2).max(num_lines - buffer_height).max(0);
    }

    let bottom = origin + buffer_height;

    if want_to_see_idx < origin {
        let required_change = origin - want_to_see_idx;
        let allowed_change = bottom - need_to_see_idx;
        origin - required_change.min(allowed_change)
    } else if want_to_see_idx >= bottom {
        let required_change = want_to_see_idx + 1 - bottom;
        let allowed_change = need_to_see_idx - origin;
        origin + required_change.min(allowed_change)
    } else {
        origin
    }
}

fn get_need_and_want_line_idx(
    first_line_idx: i32,
    last_line_idx: i32,
    selected_line_idx: i32,
    mode: SelectMode,
) -> (i32, i32) {
    match mode {
        SelectMode::Line => (selected_line_idx, selected_line_idx),
        SelectMode::Range => {
            if selected_line_idx == first_line_idx {
                (first_line_idx, last_line_idx)
            } else {
                (last_line_idx, first_line_idx)
            }
        }
        SelectMode::Hunk => (first_line_idx, last_line_idx),
    }
}
