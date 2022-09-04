use smithay::{
    desktop::{Space, Window},
    utils::{Logical, Point},
};

pub fn position_window_center(space: &mut Space, window: Window, pointer_pos: Point<f64, Logical>) {
    window.refresh();

    let loc = space.output_under(pointer_pos).next().map(|output| {
        let output = space.output_geometry(output).unwrap();
        let window = window.geometry();

        let x = output.size.w / 2 - window.size.w / 2;
        let y = output.size.h / 2 - window.size.h / 2;

        (output.loc.x + x, output.loc.y + y)
    });

    if let Some(loc) = loc {
        space.map_window(&window, loc, None, false);
    } else {
        space.map_window(&window, (0, 0), None, false);
    }
}
