fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt::init();
    cosmic_applet_arch_updates::run()
}
