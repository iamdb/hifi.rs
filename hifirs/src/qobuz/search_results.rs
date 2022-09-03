use qobuz_client::client::search_results::SearchResults;

use crate::ui::components::Table;

impl From<SearchResults> for Table {
    fn from(results: SearchResults) -> Self {
        let mut table = Table::new(None, None, None);

        table.set_header(results.headers());
        table.set_rows(results.rows());
        table.set_widths(results.widths());

        table
    }
}
