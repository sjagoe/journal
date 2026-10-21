pub fn pagination(
    item_count: usize,
    visible_item_count: usize,
    selected_index: usize,
) -> (usize, usize, usize) {
    let page_start_index = selected_index - (selected_index % visible_item_count);
    let pages = if !item_count.is_multiple_of(visible_item_count) {
        item_count / visible_item_count + 1
    } else {
        item_count / visible_item_count
    };
    let page = if !page_start_index.is_multiple_of(visible_item_count) {
        page_start_index / visible_item_count + 1
    } else {
        page_start_index / visible_item_count
    };
    (page, pages, page_start_index)
}
