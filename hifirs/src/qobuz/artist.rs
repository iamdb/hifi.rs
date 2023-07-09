use crate::cursive::CursiveFormat;
use cursive::utils::markup::StyledString;
use hifirs_qobuz_api::client::artist::Artist;

impl CursiveFormat for Artist {
    fn list_item(&self) -> StyledString {
        StyledString::plain(self.name.clone())
    }
}
