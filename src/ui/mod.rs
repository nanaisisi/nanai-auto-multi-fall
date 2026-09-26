pub mod exit_dialog;
pub mod hud;

use bevy::prelude::*;

pub use exit_dialog::*;
pub use hud::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExitDialogState>()
            .add_systems(Startup, (setup_hud, setup_exit_dialog))
            .add_systems(
                PostUpdate,
                (update_hud_system, exit_dialog_interaction_system),
            );
    }
}
