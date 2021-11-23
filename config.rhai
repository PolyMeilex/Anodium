print("config init!!!");

log::info("initalizing!!!");

keyboard::register(|| {
    log::info("key called P");
}, "Super_L", ["p"]);

keyboard::register(|| {
    log::info("key called H");
}, "Super_L", ["h"]);

keyboard::register(|| {
    log::info("starting weston terminal");
    system::exec("weston-terminal");
}, "Control_L", ["t"]);


keyboard::register(|| {
    this.test();
}, "Control_L", ["y"]);

fn arrange_outputs(outputs) {
    // let output_x = 0;

    // outputs.reverse();

    // Update of rhai broke array maps, so we have to wait for a fix XD
    // outputs.map(|output| {
        // let x = output.x;

        // output.x = output_x;
        // output.y = 0;

        // output_x += output.w;

    //     output
    // });
    
    outputs
}


fn configure_output(name, modes){
    let find_mode = |w, h, r|{
        for mode in modes {
            if mode.w == w && mode.h == h && mode.refresh == r{
                return mode;
            }
        }
    };

    switch name {
        "HDMI-A-1" => {
            find_mode.call(1920, 1080, 60)
        },
        "DP-1" => {
            find_mode.call(1920, 1080, 60)
        }
        _ => {}
    }
}
