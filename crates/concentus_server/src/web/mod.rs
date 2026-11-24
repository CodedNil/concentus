mod artwork;
mod assets;
mod browse;
mod editor;
mod icons;
mod layout;
mod listing;
mod records;
mod tags;

use topcoat::router::{RouterBuilder, path_param};

path_param!(collection);
path_param!(record_id);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .route(assets::stylesheet)
        .route(assets::favicon)
        .route(assets::editor_script)
        .route(assets::save_edits_script)
        .route(assets::tag_script)
        .route(assets::tag_details_script)
        .route(assets::browse_script)
        .route(tags::save)
        .page(browse::home)
        .page(browse::more)
        .page(tags::tag_details)
        .page(records::record_page)
        .route(editor::edit_action)
}
