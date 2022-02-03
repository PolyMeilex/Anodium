// anodize.log.info("initalizing!!!");

// anodize.keyboard.callbacks.register(|| {
//     anodize.log.info("key called P");
// }, "Super_L", ["p"]);

// anodize.keyboard.callbacks.register(|| {
//     anodize.log.info("key called H");
// }, "Super_L", ["h"]);

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("starting weston terminal");
    anodize.system.exec("weston-terminal");
    //anodize.system.exec("xterm");
}, "Control_L", ["t"]);


// anodize.keyboard.callbacks.register(|| {
//     if anodize.windows.focused != () {
//         anodize.windows.focused.close()
//     }
// }, "Control_L", ["y"]);

// anodize.keyboard.callbacks.register(|| {
//     if anodize.windows.focused != () {
//         anodize.windows.focused.maximize();
//     }
// }, "Control_L", ["w"]);

// anodize.keyboard.callbacks.register(|| {
//     if anodize.windows.focused != () {
//         anodize.windows.focused.unmaximize()
//     }
// }, "Control_L", ["u"]);


// anodize.keyboard.callbacks.register_capture(|x| {
//     anodize.log.info(x);
//     anodize.workspace.select(x);

// }, "Control_L", ["s"], keyboard::numbers());




// anodize.keyboard.callbacks.register(|| {
//     let j = 0;
//     anodize.system.add_timeout(|| {
//         anodize.log.info("rhai timeout keyboard " + j.to_string());
//         j = j + 1;
//         if j > 20 {
//             return false
//         }
//         return true;
//     }, 20);

// }, "Control_L", ["k"]);

// let weather = widget::text("");
// let date = widget::text("");
// let logger_box = container::box(700, 330, 35, 35, layout::vertical());
// let logger = widget::logger();
// logger.c_debug = [255, 26, 255, 128];

// logger_box.background = false;
// logger_box.add_widget(logger.convert());

// anodize.keyboard.callbacks.register(|| {
//     if logger_box.visable {
//         logger_box.visable = false;
//     } else {
//         logger_box.visable = true;
//     }
// }, "Control_L", ["l"]);

// anodize.system.add_timeout(|| {
//     let output = anodize.outputs[0];
//     //output.set_wallpaper("./resources/wip-anodium-wallpaper.png");
    
//     let box = container::box(output.w, 30, 0, output.h - 30, layout::horizontal());
//     box.alpha = 0.0;
//     box.background = false;
//     box.scroll = false;
//     let button = widget::button("Test");
//     button.click(|| {
//         anodize.log.info("button clicked!");
//     });

//     box.add_widget(button.convert());
//     box.add_widget(weather.convert());
//     box.add_widget(date.convert());
    


//     output.shell.add_box(box);

//     output.shell.add_box(logger_box);

//     logger_box.w = output.w - 15;

//     let output = anodize.outputs[1];
//     if output != () {
//         let box = container::box(output.w, 30, 0, output.h - 30, layout::horizontal());
//         box.alpha = 0.0;
//         box.background = false;
//         box.scroll = false;
//         let button = widget::button("Test2");
//         button.click(|| {
//             anodize.log.info("button2 clicked!");
//         });

//         box.add_widget(button.convert());

//         output.shell.add_box(box);
//     }

//     return false;
// }, 500);

// anodize.system.add_timeout(|| {
//     anodize.system.exec_read("wget -o /dev/null -O - https://wttr.in/Warsaw?format=3", |o| {
//         if o.status {
//             weather.update(o.output);
//         }
//     });
//     return true;
// }, 10000);

// anodize.system.add_timeout(|| {
//     anodize.system.exec_read("date", |o| {
//         if o.status {
//              date.update(o.output);
//         }
//     });
//     return true;
// }, 1000);

// let b = 0;
// anodize.keyboard.callbacks.register(|| {
//     b = b + 1;
//     anodize.log.info("rhai b pressed " + b.to_string());
// }, "Control_L", ["b"]);


// anodize.keyboard.callbacks.register(|| {
//     anodize.log.info("anodize");
// }, "Control_L", ["g"]);

// anodize.keyboard.callbacks.register(|| {
//     for output in anodize.outputs {
//         output.set_wallpaper("./resources/anocube.png");
//     }
// }, "Control_L", ["r"]);


// let mode_index = 0;
// anodize.keyboard.callbacks.register(|| {
//     let output = anodize.outputs[0];
//     let mode = output.modes[mode_index];
//     if mode == () {
//         mode_index = 0;
//     } else {
//         output.update_mode(mode);
//         output.set_wallpaper("./resources/anocube.png");
//     }
//     mode_index += 1;
// }, "Control_L", ["equal"]);


anodize.outputs.on_rearrange(|outputs| {
    anodize.log.info("anodize rearrange");
    let output_x = 0;

    outputs.map(|output| {
        let x = output_x;
        output_x += output.w;
        
        return [x, 0];
    })
});

anodize.outputs.on_mode_select(|desc, modes| {
    if desc.name == "Virtual-1" {
        let mode = modes.find(1440, 900, 60);
        if mode != () {
            return mode;
        }
    }

    modes[0]
});

let logger_added = false;

anodize.outputs.on_new(|output| {
    anodize.log.info("anodize on_new");

    let fps_widget = widget::fps(output);
    let fps_box = container::box(560, 0, 50, 30, layout::horizontal());
    fps_box.add_widget(fps_widget.convert());
    output.shell.add_box(fps_box);

    if !logger_added {
        logger_added = true;

        let top_box = container::box(50, 0, 500, 100, layout::vertical());
        top_box.scroll = false;
        top_box.alpha = 0.5;
        top_box.background = false;

        let logger = widget::logger();
        top_box.add_widget(logger.convert());

        output.shell.add_box(top_box);
    }

    let panel = container::panel(40, layout::horizontal(), position::left());
    
    let menu = widget::menu("Menu");
    menu.add_item(menu::label("weston-terminal", || { anodize.system.exec("weston-terminal") }).convert());

    panel.add_widget(menu.convert());
    output.shell.add_panel(panel);
});

// anodize.system.add_timeout(|| {
//     anodize.system.exec_read("cat /sys/class/hwmon/hwmon1/temp1_input", |o| {
//         if o.status {
//              //anodize.log.info("anodize hwmon end, got: " + o.output);
//         }
//     });
//     return true;
// }, 1000);



// anodize.log.info("anodize initalize end");
