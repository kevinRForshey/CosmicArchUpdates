pub mod app;
pub mod config;
pub mod updates;

pub const APP_ID: &str = "com.example.CosmicAppletArchUpdates";

pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<app::ArchUpdatesApplet>(())
}
