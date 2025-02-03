#[allow(unused_imports)]
use tmaze::{
    app::{
        app::{App, AppData},
        Activity, ActivityHandler,
    },
    ui::Popup,
};

#[cfg(feature = "updates")]
fn main() {
    let mut app = App::new(
        Activity::new_base_boxed(
            "activity",
            MyActivity(Popup::new(
                "Checking for updates in the background".to_string(),
                vec![
                    "Please wait...".to_string(),
                    "Result will be shown in the notification area".to_string(),
                ],
            )),
        ),
        true,
    );

    app.run();
}

#[cfg(not(feature = "updates"))]
fn main() {
    panic!("Cannot run `updates` example without the `updates` feature");
}

#[cfg(feature = "updates")]
struct MyActivity(Popup);

#[cfg(feature = "updates")]
impl ActivityHandler for MyActivity {
    fn update(
        &mut self,
        events: Vec<tmaze::app::Event>,
        data: &mut AppData,
    ) -> Option<tmaze::app::Change> {
        self.0.update(events, data)
    }

    fn screen(&self) -> &dyn tmaze::ui::Screen {
        &self.0
    }
}
