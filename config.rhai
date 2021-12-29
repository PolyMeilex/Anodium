anodize.log.info("initalizing!!!");

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("key called P");
}, "Super_L", ["p"]);

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("key called H");
}, "Super_L", ["h"]);

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("starting weston terminal");
    anodize.system.exec("weston-terminal");
    anodize.system.exec("xterm");
}, "Control_L", ["t"]);


anodize.keyboard.callbacks.register(|| {
    if anodize.windows.focused != () {
        anodize.windows.focused.close()
    }
}, "Control_L", ["y"]);

anodize.keyboard.callbacks.register(|| {
    if anodize.windows.focused != () {
        anodize.windows.focused.maximize();
    }
}, "Control_L", ["w"]);

anodize.keyboard.callbacks.register(|| {
    if anodize.windows.focused != () {
        anodize.windows.focused.unmaximize()
    }
}, "Control_L", ["u"]);


anodize.keyboard.callbacks.register_capture(|x| {
    anodize.log.info(x);
    anodize.workspace.select(x);

}, "Control_L", ["s"], keyboard::numbers());




anodize.keyboard.callbacks.register(|| {
    let j = 0;
    anodize.system.add_timeout(|| {
        anodize.log.info("rhai timeout keyboard " + j.to_string());
        j = j + 1;
        if j > 20 {
            return false
        }
        return true;
    }, 20);

}, "Control_L", ["k"]);

let weather = text::widget("");
let date = text::widget("");
let logger_box = box::new_raw(700, 330, 5, 65, layout::vertical());
let logger = logger::widget();
//logger.c_debug = [1.0, 0.1, 1.0, 0.5];

logger_box.background = false;
logger_box.add_widget(logger.convert());

anodize.keyboard.callbacks.register(|| {
    if logger_box.visable {
        logger_box.visable = false;
    } else {
        logger_box.visable = true;
    }
}, "Control_L", ["l"]);

anodize.system.add_timeout(|| {
    let output = anodize.outputs[0];
    //output.set_wallpaper("./resources/wip-anodium-wallpaper.png");
    
    let box = box::new_raw(output.w, 50, 0, output.h - 50, layout::horizontal());
    box.alpha = 0.0;
    box.background = false;
    box.add_widget(weather.convert());
    box.add_widget(date.convert());

    output.shell.add_box(box);

    let box = box::new_raw(output.w - 110, 50, 110, 0, layout::vertical());
    output.shell.add_box(box);

    output.shell.add_box(logger_box);

    logger_box.w = output.w - 15;
    return false;
}, 500);

anodize.system.add_timeout(|| {
    anodize.system.exec_read("wget -o /dev/null -O - https://wttr.in/Warsaw?format=3", |o| {
        if o.status {
            weather.update(o.output);
        }
    });
    return true;
}, 10000);

anodize.system.add_timeout(|| {
    anodize.system.exec_read("date", |o| {
        if o.status {
             date.update(o.output);
        }
    });
    return true;
}, 1000);

let b = 0;
anodize.keyboard.callbacks.register(|| {
    b = b + 1;
    anodize.log.info("rhai b pressed " + b.to_string());
}, "Control_L", ["b"]);


anodize.keyboard.callbacks.register(|| {
    anodize.log.info("anodize");
}, "Control_L", ["g"]);

anodize.keyboard.callbacks.register(|| {
    for output in anodize.outputs {
        output.set_wallpaper("./resources/wip-anodium-wallpaper.png");
    }
}, "Control_L", ["r"]);


let mode_index = 0;
anodize.keyboard.callbacks.register(|| {
    let output = anodize.outputs[0];
    let mode = output.modes[mode_index];
    if mode == () {
        mode_index = 0;
    } else {
        output.update_mode(mode);
        output.set_wallpaper("./resources/wip-anodium-wallpaper.png");
    }
    mode_index += 1;
}, "Control_L", ["equal"]);


let rearrange_timeout;
anodize.outputs.on_rearrange(|| {
    anodize.system.clear_timeout(rearrange_timeout);
    rearrange_timeout = anodize.system.add_timeout(|| {
        anodize.log.info("anodize rearrange");
        let output_x = 0;
        for output in anodize.outputs {
            output.x = output_x;
            output_x += output.w;
            anodize.log.info("searching for mode");
        }
        return false;
    }, 500);
});

anodize.outputs.on_new(|output| {
    anodize.log.info("anodize on_new");
    if output.name == "Virtual-1" {
        let mode = output.modes.filter(1440, 900, 60);
        if mode != () {
            output.update_mode(mode);
        }
    }

    output.set_wallpaper("./resources/wip-anodium-wallpaper.png");

    let fps_box = box::new_raw(65, 15, 0, 0, layout::horizontal());
    let fps_widget = fps::widget(output);

    fps_box.add_widget(fps_widget.convert());

    output.shell.add_box(fps_box);

});




anodize.system.add_timeout(|| {
    anodize.system.exec_read("cat /sys/class/hwmon/hwmon1/temp1_input", |o| {
        if o.status {
             //anodize.log.info("anodize hwmon end, got: " + o.output);
        }
    });
    return true;
}, 1000);



anodize.log.info("anodize initalize end");
